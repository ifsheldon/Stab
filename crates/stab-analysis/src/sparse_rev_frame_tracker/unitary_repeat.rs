use std::collections::BTreeSet;

use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, DemTarget, DemTargetKind, QubitId, RepeatBlock,
    Target, TargetKind,
};

use super::budget::slot_transform_bytes;
use super::{
    AnticommutationPolicy, SparseReverseFrameTracker, TargetXorSet, qubit_index, replace_qubit_set,
};
use crate::AnalysisOperation;
use crate::{
    AnalysisError, AnalysisResult, single_qubit_clifford_for_gate, two_qubit_clifford_for_gate,
};

/// One supported unitary repeat being folded into a qubit-slot transform.
///
/// The basis tracker seeds every X and Z slot of the body's touched qubits with its own
/// observable target. The reverse undo stack undoes the body once on that tracker, folding nested
/// unitary repeats the same way on their own frames, so a nesting chain costs one body undo per
/// level and no native recursion. Each target then marks the slots its source slot maps to.
#[derive(Debug)]
pub(super) struct UnitaryFold<'a> {
    repeat: &'a RepeatBlock,
    qubits: Vec<usize>,
    basis: SparseReverseFrameTracker,
}

impl<'a> UnitaryFold<'a> {
    /// Starts folding `repeat` when its count is above one and its body is a supported unitary.
    pub(super) fn start(
        parent: &SparseReverseFrameTracker,
        repeat: &'a RepeatBlock,
    ) -> AnalysisResult<Option<Self>> {
        if repeat.repeat_count().get() <= 1 || !is_supported_unitary_circuit(repeat.body()) {
            return Ok(None);
        }
        let qubits = touched_qubits(repeat.body());
        if let Some(&qubit) = qubits.iter().find(|&&qubit| qubit >= parent.qubit_count) {
            return Err(AnalysisError::internal(
                AnalysisOperation::ReverseFrameTracking,
                format!("unitary repeat touches qubit {qubit} outside the sparse reverse tracker"),
            ));
        }
        let slot_count = qubits.len().checked_mul(2).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::ReverseFrameTracking,
                "sparse reverse unitary repeat slot count overflowed",
            )
        })?;
        // Every seeded slot holds one target and one map entry.
        parent.check_live_budget_with_extra(super::budget::seeded_slot_bytes(slot_count))?;
        let mut basis =
            SparseReverseFrameTracker::new(parent.qubit_count, 0, 0, AnticommutationPolicy::Fail);
        basis.cancellation = parent.cancellation.clone();
        basis.meter.policy = parent.meter.policy;
        basis.meter.base_bytes = parent.meter.base_bytes;
        for (local, &qubit) in qubits.iter().enumerate() {
            let qubit = qubit_id_from_index(qubit)?;
            basis.toggle_xs(qubit, &[slot_target(local)?])?;
            let z_slot = local.checked_add(qubits.len()).ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::ReverseFrameTracking,
                    "sparse reverse unitary repeat slot overflowed",
                )
            })?;
            basis.toggle_zs(qubit, &[slot_target(z_slot)?])?;
        }
        Ok(Some(Self {
            repeat,
            qubits,
            basis,
        }))
    }

    pub(super) fn body(&self) -> &'a Circuit {
        self.repeat.body()
    }

    pub(super) fn basis(&self) -> &SparseReverseFrameTracker {
        &self.basis
    }

    pub(super) fn basis_mut(&mut self) -> &mut SparseReverseFrameTracker {
        &mut self.basis
    }

    /// Reads the body's slot transform off the basis tracker, raises it to the repeat count, and
    /// applies it to `parent`.
    pub(super) fn finish(self, parent: &mut SparseReverseFrameTracker) -> AnalysisResult<()> {
        let Self {
            repeat,
            qubits,
            basis,
        } = self;
        let transform = SlotTransform::from_basis(qubits, &basis, parent)?;
        drop(basis);
        // `pow` holds the accumulated result, the squared base, and one composition at a time.
        parent.check_live_budget_with_extra(transform.retained_bytes().saturating_mul(3))?;
        transform
            .pow(repeat.repeat_count().get(), parent)?
            .apply_to(parent)
    }
}

/// Folds one supported unitary repeat on its own undo stack, reporting whether it applied.
#[cfg(test)]
pub(super) fn try_undo_supported_unitary_repeat(
    tracker: &mut SparseReverseFrameTracker,
    repeat: &RepeatBlock,
) -> AnalysisResult<bool> {
    let Some(fold) = UnitaryFold::start(tracker, repeat)? else {
        return Ok(false);
    };
    super::shifted_repeat::undo_unitary_fold(tracker, fold)?;
    Ok(true)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SlotTransform {
    qubits: Vec<usize>,
    destinations: Vec<BTreeSet<usize>>,
}

impl SlotTransform {
    fn identity(&self) -> Self {
        Self {
            qubits: self.qubits.clone(),
            destinations: (0..self.destinations.len())
                .map(|slot| BTreeSet::from([slot]))
                .collect(),
        }
    }

    fn from_basis(
        qubits: Vec<usize>,
        basis: &SparseReverseFrameTracker,
        parent: &SparseReverseFrameTracker,
    ) -> AnalysisResult<Self> {
        let active_qubit_count = qubits.len();
        let slot_count = active_qubit_count.checked_mul(2).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::ReverseFrameTracking,
                "sparse reverse unitary repeat slot count overflowed",
            )
        })?;
        let mut destinations = vec![BTreeSet::new(); slot_count];
        let mut retained = 0_u64;
        for (sets, offset) in [(&basis.xs, 0), (&basis.zs, active_qubit_count)] {
            for (qubit, targets) in sets {
                parent.check_cancelled()?;
                let local = qubits.binary_search(&qubit_index(*qubit)?).map_err(|_| {
                    AnalysisError::internal(
                        AnalysisOperation::ReverseFrameTracking,
                        format!(
                            "unitary repeat moved a slot onto untouched qubit {}",
                            qubit.get()
                        ),
                    )
                })?;
                let slot = local.checked_add(offset).ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::ReverseFrameTracking,
                        "unitary repeat target slot overflowed",
                    )
                })?;
                retained = retained.saturating_add(slot_transform_bytes(targets.len()));
                parent.check_live_budget_with_extra(retained.saturating_add(basis.live_bytes()))?;
                for target in targets {
                    let source = slot_from_target(*target)?;
                    destinations
                        .get_mut(source)
                        .ok_or_else(|| {
                            AnalysisError::internal(
                                AnalysisOperation::ReverseFrameTracking,
                                format!("unitary repeat source slot {source} is out of bounds"),
                            )
                        })?
                        .insert(slot);
                }
            }
        }
        Ok(Self {
            qubits,
            destinations,
        })
    }

    fn retained_bytes(&self) -> u64 {
        self.destinations.iter().fold(0_u64, |acc, slots| {
            acc.saturating_add(slot_transform_bytes(slots.len()))
        })
    }

    fn pow(&self, mut exponent: u64, parent: &SparseReverseFrameTracker) -> AnalysisResult<Self> {
        let mut result = self.identity();
        let mut base = self.clone();
        while exponent > 0 {
            parent.check_cancelled()?;
            if exponent & 1 == 1 {
                result = result.then(&base)?;
            }
            exponent >>= 1;
            if exponent > 0 {
                base = base.then(&base)?;
            }
        }
        Ok(result)
    }

    fn then(&self, next: &Self) -> AnalysisResult<Self> {
        if self.qubits != next.qubits {
            return Err(AnalysisError::internal(
                AnalysisOperation::ReverseFrameTracking,
                "unitary repeat transforms have different active qubit mappings",
            ));
        }
        let destinations = self
            .destinations
            .iter()
            .map(|middle_slots| {
                let mut result = BTreeSet::new();
                for middle in middle_slots {
                    let next_slots = next.destinations.get(*middle).ok_or_else(|| {
                        AnalysisError::internal(
                            AnalysisOperation::ReverseFrameTracking,
                            format!("unitary repeat transform slot {middle} is out of bounds"),
                        )
                    })?;
                    toggle_slots(&mut result, next_slots.iter().copied());
                }
                Ok(result)
            })
            .collect::<AnalysisResult<Vec<_>>>()?;
        Ok(Self {
            qubits: self.qubits.clone(),
            destinations,
        })
    }

    fn apply_to(&self, tracker: &mut SparseReverseFrameTracker) -> AnalysisResult<()> {
        let active_qubit_count = self.qubits.len();
        let slot_count = self.destinations.len();
        let mut old_slots = Vec::with_capacity(slot_count);
        for &qubit in &self.qubits {
            old_slots.push(tracker.xs_for(qubit_id_from_index(qubit)?)?);
        }
        for &qubit in &self.qubits {
            old_slots.push(tracker.zs_for(qubit_id_from_index(qubit)?)?);
        }
        // A destination set is at most the sum of the sets mapped onto it, and every new set
        // is live alongside the sets it replaces until it is stored.
        let mut new_targets = 0_u64;
        for (source_targets, destination_slots) in old_slots.iter().zip(&self.destinations) {
            new_targets = new_targets.saturating_add(
                (source_targets.len() as u64).saturating_mul(destination_slots.len() as u64),
            );
        }
        tracker.check_live_budget_with_extra(super::budget::target_run_bytes(
            new_targets,
            slot_count,
        ))?;
        let old_slots = old_slots
            .into_iter()
            .map(<[DemTarget]>::to_vec)
            .collect::<Vec<_>>();

        let mut new_slots = vec![TargetXorSet::new(); slot_count];
        let mut merge = Vec::new();
        for (source_slot, source_targets) in old_slots.iter().enumerate() {
            if source_targets.is_empty() {
                continue;
            }
            let destination_slots = self.destinations.get(source_slot).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::ReverseFrameTracking,
                    format!("unitary repeat source slot {source_slot} is out of bounds"),
                )
            })?;
            for destination_slot in destination_slots {
                let destination_targets =
                    new_slots.get_mut(*destination_slot).ok_or_else(|| {
                        AnalysisError::internal(AnalysisOperation::ReverseFrameTracking, format!(
                            "unitary repeat destination slot {destination_slot} is out of bounds"
                        ))
                    })?;
                destination_targets.xor_sorted(source_targets, &mut merge);
            }
        }
        drop(old_slots);
        drop(merge);

        for (local, &qubit) in self.qubits.iter().enumerate() {
            let value = new_slots
                .get_mut(local)
                .map(std::mem::take)
                .ok_or_else(|| active_slot_error("X transform", qubit))?;
            replace_qubit_set(
                &mut tracker.xs,
                qubit_id_from_index(qubit)?,
                value,
                &mut tracker.scratch,
                &mut tracker.meter,
            );
        }
        for (local, &qubit) in self.qubits.iter().enumerate() {
            let value = new_slots
                .get_mut(active_qubit_count + local)
                .map(std::mem::take)
                .ok_or_else(|| active_slot_error("Z transform", qubit))?;
            replace_qubit_set(
                &mut tracker.zs,
                qubit_id_from_index(qubit)?,
                value,
                &mut tracker.scratch,
                &mut tracker.meter,
            );
        }
        tracker.check_live_budget()
    }
}

fn active_slot_error(basis: &str, qubit: usize) -> AnalysisError {
    AnalysisError::internal(
        AnalysisOperation::ReverseFrameTracking,
        format!("missing active {basis} slot for qubit {qubit} during unitary repeat folding"),
    )
}

fn touched_qubits(circuit: &Circuit) -> Vec<usize> {
    let mut qubits = BTreeSet::new();
    let mut stack = vec![circuit.items().iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        match item {
            CircuitItem::Instruction(instruction) => {
                qubits.extend(
                    instruction
                        .targets()
                        .iter()
                        .filter_map(Target::qubit_id)
                        .map(|qubit| qubit.get() as usize),
                );
            }
            CircuitItem::RepeatBlock(repeat) => stack.push(repeat.body().items().iter()),
        }
    }
    qubits.into_iter().collect()
}

/// Reports whether a repeat body is folded as one qubit-slot transform raised to the repeat count
/// instead of being undone once per iteration.
pub(crate) fn is_supported_unitary_circuit(circuit: &Circuit) -> bool {
    let mut stack = vec![circuit.items().iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        match item {
            CircuitItem::Instruction(instruction) => {
                if !is_supported_unitary_instruction(instruction) {
                    return false;
                }
            }
            CircuitItem::RepeatBlock(repeat) => stack.push(repeat.body().items().iter()),
        }
    }
    true
}

fn is_supported_unitary_instruction(instruction: &CircuitInstruction) -> bool {
    if single_qubit_clifford_for_gate(instruction.gate()).is_some() {
        return has_plain_qubit_groups(instruction, 1);
    }
    two_qubit_clifford_for_gate(instruction.gate()).is_some()
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
        target.kind(),
        TargetKind::Qubit {
            inverted: false,
            ..
        }
    )
}

fn qubit_id_from_index(index: usize) -> AnalysisResult<QubitId> {
    Ok(QubitId::new(u32::try_from(index).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::ReverseFrameTracking,
            format!("unitary repeat qubit index {index} does not fit u32"),
        )
    })?)?)
}

fn slot_target(slot: usize) -> AnalysisResult<DemTarget> {
    let observable = u64::try_from(slot).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::ReverseFrameTracking,
            "sparse reverse unitary repeat slot does not fit u64",
        )
    })?;
    DemTarget::logical_observable(observable).map_err(Into::into)
}

fn slot_from_target(target: DemTarget) -> AnalysisResult<usize> {
    match target.kind() {
        DemTargetKind::LogicalObservable(slot) => usize::try_from(slot.get()).map_err(|_| {
            AnalysisError::count_overflow(
                AnalysisOperation::ReverseFrameTracking,
                "sparse reverse unitary repeat slot does not fit usize",
            )
        }),
        DemTargetKind::RelativeDetector(_) | DemTargetKind::Separator => {
            Err(AnalysisError::internal(
                AnalysisOperation::ReverseFrameTracking,
                format!("unitary repeat basis tracker holds non-slot target {target}"),
            ))
        }
    }
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
    use stab_algebra::SingleQubitClifford;
    use stab_model::Gate;

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
        let mut tracker =
            SparseReverseFrameTracker::new(text.len(), 0, 0, AnticommutationPolicy::Fail);
        let sensitivity = [DemTarget::logical_observable(0).unwrap()];
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
    ) -> AnalysisResult<()> {
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
    ) -> AnalysisResult<()> {
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
            assert!(two_qubit_clifford_for_gate(gate).is_some(), "{gate_name}");
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
    fn unitary_repeat_folding_matches_naive_multi_level_nesting() {
        // Every level mixes its own gates with a nested fold, including sibling repeats, a
        // count-one level that is not folded itself, and qubits only inner levels touch.
        assert_repeat_folding_matches_naive(
            "
            REPEAT 3 {
                CX 0 1
                REPEAT 2 {
                    H 1
                    REPEAT 1 {
                        REPEAT 5 {
                            ISWAP 2 3
                            REPEAT 2 {
                                C_XYZ 3
                                SQRT_XX 3 4
                            }
                            S 2
                        }
                        CZSWAP 1 2
                    }
                    REPEAT 3 {
                        SWAPCX 0 4
                    }
                }
                SQRT_Y_DAG 0
            }
            ",
            "XYZXZ",
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
    fn unitary_repeat_folding_keeps_wide_idle_suffix_implicit() {
        const HIGH_QUBIT: u32 = 65_535;
        let repeat = repeat("REPEAT 1000001 {\n    H 0\n}\n");
        let target = DemTarget::logical_observable(0).unwrap();
        let mut tracker = SparseReverseFrameTracker::new(
            HIGH_QUBIT as usize + 1,
            0,
            0,
            AnticommutationPolicy::Fail,
        );
        tracker
            .toggle_xs(QubitId::new(HIGH_QUBIT).unwrap(), &[target])
            .unwrap();

        assert!(try_undo_supported_unitary_repeat(&mut tracker, &repeat).unwrap());
        assert!(
            tracker
                .xs_for(QubitId::new(HIGH_QUBIT).unwrap())
                .unwrap()
                .contains(&target)
        );
        assert!(
            !tracker
                .zs_for(QubitId::new(HIGH_QUBIT).unwrap())
                .unwrap()
                .contains(&target)
        );
    }

    #[test]
    fn unitary_repeat_folding_remaps_sparse_high_active_qubit() {
        const HIGH_QUBIT: u32 = 65_535;
        let repeat = repeat(&format!("REPEAT 1000001 {{\n    H {HIGH_QUBIT}\n}}\n"));
        let target = DemTarget::logical_observable(0).unwrap();
        let mut tracker = SparseReverseFrameTracker::new(
            HIGH_QUBIT as usize + 1,
            0,
            0,
            AnticommutationPolicy::Fail,
        );
        tracker
            .toggle_xs(QubitId::new(HIGH_QUBIT).unwrap(), &[target])
            .unwrap();

        assert!(try_undo_supported_unitary_repeat(&mut tracker, &repeat).unwrap());
        assert!(
            !tracker
                .xs_for(QubitId::new(HIGH_QUBIT).unwrap())
                .unwrap()
                .contains(&target)
        );
        assert!(
            tracker
                .zs_for(QubitId::new(HIGH_QUBIT).unwrap())
                .unwrap()
                .contains(&target)
        );
    }

    #[test]
    fn unitary_repeat_folding_declines_non_unitary_and_unsupported_gates() {
        let mut tracker = SparseReverseFrameTracker::new(2, 0, 0, AnticommutationPolicy::Fail);
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
