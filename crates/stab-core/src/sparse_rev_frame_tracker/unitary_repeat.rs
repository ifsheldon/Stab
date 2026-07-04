use std::collections::BTreeSet;

use crate::{Circuit, CircuitError, CircuitItem, CircuitResult, DemTarget, QubitId, RepeatBlock};

use super::{SparseReverseFrameTracker, toggle_targets};

pub(super) fn try_undo_supported_unitary_repeat(
    tracker: &mut SparseReverseFrameTracker,
    repeat: &RepeatBlock,
) -> CircuitResult<bool> {
    if !is_supported_unitary_circuit(repeat.body()) {
        return Ok(false);
    }

    let transform = SlotTransform::for_body(repeat.body(), tracker.xs.len())?;
    transform.pow(repeat.repeat_count().get()).apply_to(tracker);
    Ok(true)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SlotTransform {
    destinations: Vec<BTreeSet<usize>>,
}

impl SlotTransform {
    fn identity(slot_count: usize) -> Self {
        Self {
            destinations: (0..slot_count).map(|slot| BTreeSet::from([slot])).collect(),
        }
    }

    fn for_body(body: &Circuit, qubit_count: usize) -> CircuitResult<Self> {
        let slot_count = qubit_count.checked_mul(2).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse unitary repeat slot count overflowed",
            )
        })?;
        let mut destinations = Vec::with_capacity(slot_count);
        for slot in 0..slot_count {
            let target = slot_target(slot)?;
            let mut basis_tracker = SparseReverseFrameTracker::new(qubit_count, 0, 0, true);
            seed_slot(&mut basis_tracker, qubit_count, slot, target)?;
            basis_tracker.undo_circuit(body)?;
            destinations.push(collect_target_slots(&basis_tracker, target));
        }
        Ok(Self { destinations })
    }

    fn pow(&self, mut exponent: u64) -> Self {
        let mut result = Self::identity(self.destinations.len());
        let mut base = self.clone();
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result.then(&base);
            }
            exponent >>= 1;
            if exponent > 0 {
                base = base.then(&base);
            }
        }
        result
    }

    fn then(&self, next: &Self) -> Self {
        let destinations = self
            .destinations
            .iter()
            .map(|middle_slots| {
                let mut result = BTreeSet::new();
                for middle in middle_slots {
                    if let Some(next_slots) = next.destinations.get(*middle) {
                        toggle_slots(&mut result, next_slots.iter().copied());
                    }
                }
                result
            })
            .collect();
        Self { destinations }
    }

    fn apply_to(&self, tracker: &mut SparseReverseFrameTracker) {
        let qubit_count = tracker.xs.len();
        let slot_count = self.destinations.len();
        let mut old_slots = Vec::with_capacity(slot_count);
        old_slots.extend(tracker.xs.iter().cloned());
        old_slots.extend(tracker.zs.iter().cloned());

        let mut new_slots = vec![BTreeSet::new(); slot_count];
        for (source_slot, source_targets) in old_slots.iter().enumerate() {
            if source_targets.is_empty() {
                continue;
            }
            if let Some(destination_slots) = self.destinations.get(source_slot) {
                for destination_slot in destination_slots {
                    if let Some(destination_targets) = new_slots.get_mut(*destination_slot) {
                        toggle_targets(destination_targets, source_targets.iter().copied());
                    }
                }
            }
        }

        tracker.xs = new_slots.iter().take(qubit_count).cloned().collect();
        tracker.zs = new_slots.iter().skip(qubit_count).cloned().collect();
    }
}

fn is_supported_unitary_circuit(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => matches!(
            instruction.gate().canonical_name(),
            "H" | "H_XY" | "S" | "S_DAG" | "C_XYZ" | "CX" | "CZ"
        ),
        CircuitItem::RepeatBlock(repeat) => is_supported_unitary_circuit(repeat.body()),
    })
}

fn seed_slot(
    tracker: &mut SparseReverseFrameTracker,
    qubit_count: usize,
    slot: usize,
    target: DemTarget,
) -> CircuitResult<()> {
    if slot < qubit_count {
        let qubit = QubitId::new(u32::try_from(slot).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse unitary repeat slot does not fit u32",
            )
        })?)?;
        tracker.toggle_xs(qubit, &BTreeSet::from([target]))
    } else {
        let z_slot = slot.checked_sub(qubit_count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse unitary repeat z slot underflowed",
            )
        })?;
        let qubit = QubitId::new(u32::try_from(z_slot).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse unitary repeat slot does not fit u32",
            )
        })?)?;
        tracker.toggle_zs(qubit, &BTreeSet::from([target]))
    }
}

fn collect_target_slots(tracker: &SparseReverseFrameTracker, target: DemTarget) -> BTreeSet<usize> {
    let mut slots = BTreeSet::new();
    let qubit_count = tracker.xs.len();
    for (index, targets) in tracker.xs.iter().enumerate() {
        if targets.contains(&target) {
            slots.insert(index);
        }
    }
    for (index, targets) in tracker.zs.iter().enumerate() {
        if targets.contains(&target) {
            slots.insert(index + qubit_count);
        }
    }
    slots
}

fn slot_target(slot: usize) -> CircuitResult<DemTarget> {
    let observable = u64::try_from(slot).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "sparse reverse unitary repeat slot does not fit u64",
        )
    })?;
    DemTarget::logical_observable(observable)
}

fn toggle_slots(target: &mut BTreeSet<usize>, values: impl Iterator<Item = usize>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used,
        reason = "unit tests use compact fixed-slot tracker assertions"
    )]

    use super::*;

    fn circuit(text: &str) -> Circuit {
        Circuit::from_stim_str(text).unwrap()
    }

    fn repeat(text: &str) -> RepeatBlock {
        let parsed = circuit(text);
        let Some(CircuitItem::RepeatBlock(repeat)) = parsed.items().first() else {
            panic!("expected one repeat block in {text}");
        };
        repeat.clone()
    }

    fn tracker_from_pauli_text(text: &str) -> SparseReverseFrameTracker {
        let mut tracker = SparseReverseFrameTracker::new(text.len(), 0, 0, true);
        let sensitivity = BTreeSet::from([DemTarget::logical_observable(0).unwrap()]);
        for (index, character) in text.chars().enumerate() {
            let qubit = QubitId::new(u32::try_from(index).unwrap()).unwrap();
            match character {
                'I' => {}
                'X' => tracker.toggle_xs(qubit, &sensitivity).unwrap(),
                'Y' => {
                    tracker.toggle_xs(qubit, &sensitivity).unwrap();
                    tracker.toggle_zs(qubit, &sensitivity).unwrap();
                }
                'Z' => tracker.toggle_zs(qubit, &sensitivity).unwrap(),
                _ => panic!("unexpected Pauli text character {character}"),
            }
        }
        tracker
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_mixed_clifford_loop() {
        let repeat = repeat(
            "
            REPEAT 37 {
                H 0
                S 1
                CX 0 1
                CZ 1 2
                C_XYZ 2
            }
            ",
        );
        let mut folded = tracker_from_pauli_text("XYZ");
        assert!(try_undo_supported_unitary_repeat(&mut folded, &repeat).unwrap());

        let mut naive = tracker_from_pauli_text("XYZ");
        for _ in 0..repeat.repeat_count().get() {
            naive.undo_circuit(repeat.body()).unwrap();
        }
        assert_eq!(folded, naive);
    }

    #[test]
    fn unitary_repeat_folding_handles_huge_periodic_loop() {
        let repeat = repeat(
            "
            REPEAT 1000001 {
                H 0
            }
            ",
        );
        let mut actual = tracker_from_pauli_text("X");
        assert!(try_undo_supported_unitary_repeat(&mut actual, &repeat).unwrap());
        assert_eq!(actual, tracker_from_pauli_text("Z"));
    }

    #[test]
    fn unitary_repeat_folding_declines_non_unitary_and_unsupported_gates() {
        let mut tracker = SparseReverseFrameTracker::new(2, 0, 0, true);
        assert!(
            !try_undo_supported_unitary_repeat(
                &mut tracker,
                &repeat(
                    "
                    REPEAT 2 {
                        M 0
                    }
                    "
                ),
            )
            .unwrap()
        );
        assert!(
            !try_undo_supported_unitary_repeat(
                &mut tracker,
                &repeat(
                    "
                    REPEAT 2 {
                        CY 0 1
                    }
                    "
                ),
            )
            .unwrap()
        );
    }
}
