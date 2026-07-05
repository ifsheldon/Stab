use std::collections::BTreeSet;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget, QubitId,
    RepeatBlock, SingleQubitClifford, Target,
};

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
        CircuitItem::Instruction(instruction) => is_supported_unitary_instruction(instruction),
        CircuitItem::RepeatBlock(repeat) => is_supported_unitary_circuit(repeat.body()),
    })
}

fn is_supported_unitary_instruction(instruction: &CircuitInstruction) -> bool {
    if SingleQubitClifford::from_gate(instruction.gate()).is_ok() {
        return has_plain_qubit_groups(instruction, 1);
    }
    instruction.gate().is_two_qubit_gate()
        && instruction.gate().has_tableau()
        && has_plain_qubit_groups(instruction, 2)
}

fn has_plain_qubit_groups(instruction: &CircuitInstruction, group_size: usize) -> bool {
    instruction
        .target_groups()
        .into_iter()
        .all(|group| group.len() == group_size && group.iter().all(is_plain_qubit_target))
}

fn is_plain_qubit_target(target: &Target) -> bool {
    matches!(
        target,
        Target::Qubit {
            inverted: false,
            ..
        }
    )
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
    use crate::Gate;

    const FIXED_TWO_QUBIT_TABLEAU_GATES: &[&str] = &[
        "II",
        "XCX",
        "XCY",
        "XCZ",
        "YCX",
        "YCY",
        "YCZ",
        "SWAP",
        "ISWAP",
        "ISWAP_DAG",
        "CXSWAP",
        "SWAPCX",
        "CZSWAP",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
    ];

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

    fn undo_circuit_naively(
        tracker: &mut SparseReverseFrameTracker,
        circuit: &Circuit,
    ) -> CircuitResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => tracker.undo_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => undo_repeat_naively(tracker, repeat)?,
            }
        }
        Ok(())
    }

    fn undo_repeat_naively(
        tracker: &mut SparseReverseFrameTracker,
        repeat: &RepeatBlock,
    ) -> CircuitResult<()> {
        for _ in 0..repeat.repeat_count().get() {
            undo_circuit_naively(tracker, repeat.body())?;
        }
        Ok(())
    }

    fn assert_repeat_folding_matches_naive(text: &str, input: &str) {
        let repeat = repeat(text);
        let mut folded = tracker_from_pauli_text(input);
        assert!(
            try_undo_supported_unitary_repeat(&mut folded, &repeat).unwrap(),
            "{text}"
        );

        let mut naive = tracker_from_pauli_text(input);
        undo_repeat_naively(&mut naive, &repeat).unwrap();
        assert_eq!(folded, naive, "{text}");
    }

    fn next_generated_value(state: &mut u64) -> usize {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        usize::try_from(*state >> 32).unwrap()
    }

    fn generated_supported_unitary_repeat(
        seed: u64,
        repeat_count: u64,
        instruction_count: usize,
    ) -> String {
        let single_gates: Vec<_> = SingleQubitClifford::all()
            .map(|gate| gate.canonical_name())
            .collect();
        let mut state = seed;
        let mut text = format!("REPEAT {repeat_count} {{\n");
        for _ in 0..instruction_count {
            if next_generated_value(&mut state).is_multiple_of(3) {
                let gate = single_gates[next_generated_value(&mut state) % single_gates.len()];
                let start = next_generated_value(&mut state) % 6;
                let target_count = 1 + next_generated_value(&mut state) % 3;
                text.push_str("    ");
                text.push_str(gate);
                for offset in 0..target_count {
                    text.push(' ');
                    text.push_str(&((start + offset) % 6).to_string());
                }
                text.push('\n');
            } else {
                let gate = FIXED_TWO_QUBIT_TABLEAU_GATES
                    [next_generated_value(&mut state) % FIXED_TWO_QUBIT_TABLEAU_GATES.len()];
                let pair_start = next_generated_value(&mut state) % 3;
                let pair_count = 1 + next_generated_value(&mut state) % 2;
                text.push_str("    ");
                text.push_str(gate);
                for offset in 0..pair_count {
                    let pair = (pair_start + offset) % 3;
                    text.push(' ');
                    text.push_str(&(pair * 2).to_string());
                    text.push(' ');
                    text.push_str(&(pair * 2 + 1).to_string());
                }
                text.push('\n');
            }
        }
        text.push_str("}\n");
        text
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_mixed_clifford_loop() {
        assert_repeat_folding_matches_naive(
            "
            REPEAT 37 {
                H 0
                S 1
                CX 0 1
                CY 2 0
                CZ 1 2
                C_XYZ 2
                SQRT_X 1
                H_YZ 2
                C_ZYX 0
            }
            ",
            "XYZ",
        );
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_all_single_qubit_cliffords() {
        let mut text = String::from("REPEAT 11 {\n");
        for (index, gate) in SingleQubitClifford::all().enumerate() {
            text.push_str("    ");
            text.push_str(gate.canonical_name());
            text.push(' ');
            text.push_str(&(index % 3).to_string());
            text.push('\n');
        }
        text.push_str("}\n");
        assert_repeat_folding_matches_naive(&text, "XYZ");
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_fixed_two_qubit_cliffords() {
        let mut text = String::from("REPEAT 29 {\n");
        for (index, gate_name) in FIXED_TWO_QUBIT_TABLEAU_GATES.iter().enumerate() {
            let gate = Gate::from_name(gate_name).unwrap();
            assert!(gate.is_two_qubit_gate(), "{gate_name}");
            assert!(gate.has_tableau(), "{gate_name}");
            let left = index % 4;
            let right = (index + 1) % 4;
            text.push_str("    ");
            text.push_str(gate_name);
            text.push(' ');
            text.push_str(&left.to_string());
            text.push(' ');
            text.push_str(&right.to_string());
            text.push('\n');
        }
        text.push_str("    H 0 2\n    S 1 3\n}\n");
        assert_repeat_folding_matches_naive(&text, "XYZY");
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_generated_supported_unitary_loops() {
        for seed in [1, 2, 3, 5, 8, 13, 21] {
            let text = generated_supported_unitary_repeat(seed, 7 + seed % 23, 48);
            assert_repeat_folding_matches_naive(&text, "XYZXYZ");
        }
    }

    #[test]
    fn unitary_repeat_folding_matches_naive_nested_supported_unitary_loops() {
        assert_repeat_folding_matches_naive(
            "
            REPEAT 23 {
                H 0 2 4
                REPEAT 5 {
                    SWAP 0 1 2 3
                    SQRT_YY 1 5
                    C_XYZ 4
                    CXSWAP 2 3
                }
                ISWAP_DAG 4 5
                SQRT_ZZ_DAG 0 2
                S_DAG 3
            }
            ",
            "XYZXYZ",
        );
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
                        CX rec[-1] 1
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
                        CX sweep[0] 1
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
                        SPP X0*X1
                    }
                    "
                ),
            )
            .unwrap()
        );
    }
}
