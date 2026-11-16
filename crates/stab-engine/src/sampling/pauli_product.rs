use std::collections::HashMap;

use stab_algebra::PauliBasis;

use super::SamplingCompileError;

/// Products with at most this many raw terms find a repeated qubit by scanning the terms so far;
/// longer ones index them, so normalizing a product stays linear in its length.
const SCANNED_PRODUCT_TERMS: usize = 32;

/// Multiplies the raw terms of one product left to right into one term per qubit, in order of
/// each qubit's first surviving appearance, and folds the phase into the inversion flag.
pub(crate) fn normalize_terms(
    raw_terms: Vec<(usize, PauliBasis, bool)>,
    base_inverted: bool,
) -> Result<(Vec<(usize, PauliBasis)>, bool), SamplingCompileError> {
    let mut inverted = base_inverted;
    let mut phase = 0u8;
    let terms = if raw_terms.len() <= SCANNED_PRODUCT_TERMS {
        let mut terms = Vec::new();
        for (qubit, basis, term_inverted) in raw_terms {
            multiply_term(&mut terms, qubit, basis, &mut phase);
            inverted ^= term_inverted;
        }
        terms
    } else {
        multiply_indexed_terms(raw_terms, &mut inverted, &mut phase)
    };
    match phase {
        0 => Ok((terms, inverted)),
        2 => Ok((terms, !inverted)),
        _ => Err(SamplingCompileError::invalid_circuit(
            "MPP Pauli product is anti-Hermitian",
        )),
    }
}

/// The indexed form of repeated [`multiply_term`] calls: a cancelled term leaves an empty slot
/// instead of shifting the later terms, which keeps the surviving terms in the same order.
fn multiply_indexed_terms(
    raw_terms: Vec<(usize, PauliBasis, bool)>,
    inverted: &mut bool,
    phase: &mut u8,
) -> Vec<(usize, PauliBasis)> {
    let mut slots: Vec<Option<(usize, PauliBasis)>> = Vec::with_capacity(raw_terms.len());
    let mut slot_of_qubit: HashMap<usize, usize> = HashMap::with_capacity(raw_terms.len());
    for (qubit, incoming, term_inverted) in raw_terms {
        *inverted ^= term_inverted;
        let existing = slot_of_qubit
            .get(&qubit)
            .and_then(|&slot| Some((slot, slots.get_mut(slot)?.take()?)));
        let Some((slot, (_, existing))) = existing else {
            slot_of_qubit.insert(qubit, slots.len());
            slots.push(Some((qubit, incoming)));
            continue;
        };
        let (product, phase_delta) = multiply_bases(existing, incoming);
        *phase = (*phase + phase_delta) % 4;
        match (product, slots.get_mut(slot)) {
            (Some(product), Some(entry)) => *entry = Some((qubit, product)),
            _ => {
                slot_of_qubit.remove(&qubit);
            }
        }
    }
    slots.into_iter().flatten().collect()
}

fn multiply_term(
    terms: &mut Vec<(usize, PauliBasis)>,
    qubit: usize,
    incoming: PauliBasis,
    phase: &mut u8,
) {
    let Some(index) = terms
        .iter()
        .position(|(existing_qubit, _)| *existing_qubit == qubit)
    else {
        terms.push((qubit, incoming));
        return;
    };
    let (_, existing) = terms.remove(index);
    let (product, phase_delta) = multiply_bases(existing, incoming);
    *phase = (*phase + phase_delta) % 4;
    if let Some(product) = product {
        terms.insert(index, (qubit, product));
    }
}

fn multiply_bases(left: PauliBasis, right: PauliBasis) -> (Option<PauliBasis>, u8) {
    match (left, right) {
        (PauliBasis::I, PauliBasis::I) => (None, 0),
        (PauliBasis::I, basis) | (basis, PauliBasis::I) => (Some(basis), 0),
        (PauliBasis::X, PauliBasis::X)
        | (PauliBasis::Y, PauliBasis::Y)
        | (PauliBasis::Z, PauliBasis::Z) => (None, 0),
        (PauliBasis::X, PauliBasis::Y) => (Some(PauliBasis::Z), 1),
        (PauliBasis::Y, PauliBasis::Z) => (Some(PauliBasis::X), 1),
        (PauliBasis::Z, PauliBasis::X) => (Some(PauliBasis::Y), 1),
        (PauliBasis::Y, PauliBasis::X) => (Some(PauliBasis::Z), 3),
        (PauliBasis::Z, PauliBasis::Y) => (Some(PauliBasis::X), 3),
        (PauliBasis::X, PauliBasis::Z) => (Some(PauliBasis::Y), 3),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "the differential test draws in-range indices"
)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::{RngExt as _, SeedableRng as _};
    use stab_algebra::PauliBasis;

    use super::{multiply_indexed_terms, multiply_term};

    /// The indexed path must give the scanned path's terms in the same order, phase, and sign,
    /// including qubits that cancel and later reappear.
    #[test]
    fn indexed_normalization_matches_the_scanned_one() {
        let mut rng = SmallRng::seed_from_u64(11);
        let bases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
        for _ in 0..500 {
            let length = rng.random_range(33..200);
            let qubits = rng.random_range(1..40);
            let raw: Vec<_> = (0..length)
                .map(|_| {
                    let basis = *bases
                        .get(rng.random_range(0..bases.len()))
                        .expect("basis index is in range");
                    (rng.random_range(0..qubits), basis, rng.random::<bool>())
                })
                .collect();
            let (mut scanned, mut scanned_phase, mut scanned_inverted) = (Vec::new(), 0, false);
            for &(qubit, basis, inverted) in &raw {
                multiply_term(&mut scanned, qubit, basis, &mut scanned_phase);
                scanned_inverted ^= inverted;
            }
            let (mut indexed_phase, mut indexed_inverted) = (0, false);
            let indexed = multiply_indexed_terms(raw, &mut indexed_inverted, &mut indexed_phase);
            assert_eq!(indexed, scanned);
            assert_eq!(
                (indexed_phase, indexed_inverted),
                (scanned_phase, scanned_inverted)
            );
        }
    }
}
