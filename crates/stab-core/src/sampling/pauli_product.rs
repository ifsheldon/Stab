use crate::{CircuitError, CircuitResult, PauliBasis};

pub(crate) fn normalize_terms(
    raw_terms: Vec<(usize, PauliBasis, bool)>,
    base_inverted: bool,
) -> CircuitResult<(Vec<(usize, PauliBasis)>, bool)> {
    let mut terms = Vec::new();
    let mut inverted = base_inverted;
    let mut phase = 0u8;
    for (qubit, basis, term_inverted) in raw_terms {
        multiply_term(&mut terms, qubit, basis, &mut phase);
        inverted ^= term_inverted;
    }
    match phase {
        0 => Ok((terms, inverted)),
        2 => Ok((terms, !inverted)),
        _ => Err(CircuitError::invalid_sampler_compilation(
            "MPP Pauli product is anti-Hermitian",
        )),
    }
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
