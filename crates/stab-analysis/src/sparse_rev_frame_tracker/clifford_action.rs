//! Precomputed reverse actions of unitary Cliffords on tracked sensitivity slots.
//!
//! Undoing a Clifford conjugates every tracked Pauli by the gate's inverse. Conjugation is
//! linear over GF(2) on the X and Z bits, so each output slot (one qubit's X or Z sensitivity
//! set) is the XOR of a fixed subset of the input slots. The subsets depend only on the gate,
//! so they are derived once from the gate tableau for every catalog gate.

use std::sync::LazyLock;

use stab_algebra::{PauliBasis, Tableau};
use stab_model::{DemTarget, Gate, GateKind, QubitId};

use super::budget::{TrackerMeter, buffer_bytes};
use super::sets::{buffer_growth_bytes, unknown_buffer_growth_bytes};
use super::{
    SparseReverseFrameTracker, TargetXorSet, TrackerScratch, replace_qubit_set, sorted_slice,
};
use crate::{AnalysisError, AnalysisResult, gate_tableau, single_qubit_clifford_for_gate};

/// Input slots XORed into each output slot, as bitmasks over input slot indices.
///
/// One-qubit slots are `[x, z]`; two-qubit slots are `[left x, left z, right x, right z]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CliffordAction<const SLOTS: usize> {
    sources: [u8; SLOTS],
}

impl<const SLOTS: usize> CliffordAction<SLOTS> {
    /// Builds the action from the images of each input slot's generator.
    fn from_images(images: [[bool; SLOTS]; SLOTS]) -> Self {
        let mut sources = [0_u8; SLOTS];
        for (input, image) in images.iter().enumerate() {
            for (output, &bit) in image.iter().enumerate() {
                if bit && let Some(mask) = sources.get_mut(output) {
                    *mask |= 1 << input;
                }
            }
        }
        Self { sources }
    }

    /// Returns the output slot bits produced by input slot bits, for tests and reference use.
    #[cfg(test)]
    pub(super) fn apply_bits(self, input: [bool; SLOTS]) -> [bool; SLOTS] {
        self.sources.map(|mask| {
            input.iter().enumerate().fold(false, |bit, (slot, &set)| {
                bit ^ (set && mask & (1 << slot) != 0)
            })
        })
    }

    /// Worst-case bytes computing every output allocates beyond the buffers' current storage.
    ///
    /// An output is at most the sum of the inputs XORed into it. An output fed by one input is
    /// copied into its own buffer; one fed by several merges through the scratch buffer, trading
    /// storage with it, so both buffers may grow to hold that output.
    fn output_growth_bytes(self, inputs: &[&[DemTarget]; SLOTS], outputs: &[TargetXorSet]) -> u64 {
        self.sources
            .iter()
            .zip(outputs)
            .fold(0_u64, |growth, (mask, output)| {
                let (count, needed) = inputs
                    .iter()
                    .enumerate()
                    .filter(|(slot, input)| mask & (1 << slot) != 0 && !input.is_empty())
                    .fold((0_usize, 0_usize), |(count, needed), (_, input)| {
                        (count + 1, needed.saturating_add(input.len()))
                    });
                growth.saturating_add(match count {
                    0 => 0,
                    1 => buffer_growth_bytes(output.capacity(), needed),
                    _ => unknown_buffer_growth_bytes(needed).saturating_mul(2),
                })
            })
    }

    /// Writes every output slot from borrowed input slots into `outputs`.
    fn compute(
        self,
        inputs: [&[DemTarget]; SLOTS],
        outputs: &mut [TargetXorSet; SLOTS],
        merge: &mut Vec<DemTarget>,
    ) {
        for (output, mask) in outputs.iter_mut().zip(self.sources) {
            output.clear();
            for (slot, input) in inputs.iter().enumerate() {
                if mask & (1 << slot) != 0 {
                    output.xor_sorted(input, merge);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TrackerClifford {
    One(CliffordAction<2>),
    Two(CliffordAction<4>),
}

/// Reverse actions of every catalog Clifford, indexed by gate kind.
static TRACKER_CLIFFORDS: LazyLock<[Option<TrackerClifford>; GateKind::COUNT]> =
    LazyLock::new(|| {
        GateKind::ALL.map(|kind| {
            let gate = kind.gate();
            if single_qubit_clifford_for_gate(gate).is_ok() {
                derive_single_qubit_action(gate)
                    .ok()
                    .map(TrackerClifford::One)
            } else if gate.is_two_qubit_gate() {
                derive_two_qubit_action(gate).ok().map(TrackerClifford::Two)
            } else {
                None
            }
        })
    });

fn cached_action(gate: Gate) -> Option<TrackerClifford> {
    TRACKER_CLIFFORDS
        .get(gate.kind() as usize)
        .copied()
        .flatten()
}

/// Returns the reverse action of a single-qubit Clifford gate.
pub(super) fn single_qubit_action(gate: Gate) -> AnalysisResult<CliffordAction<2>> {
    match cached_action(gate) {
        Some(TrackerClifford::One(action)) => Ok(action),
        // Gates without a cached action re-derive it to report the original failure.
        _ => derive_single_qubit_action(gate),
    }
}

/// Returns the reverse action of a two-qubit Clifford gate.
pub(super) fn two_qubit_action(gate: Gate) -> AnalysisResult<CliffordAction<4>> {
    match cached_action(gate) {
        Some(TrackerClifford::Two(action)) => Ok(action),
        _ => derive_two_qubit_action(gate),
    }
}

fn derive_single_qubit_action(gate: Gate) -> AnalysisResult<CliffordAction<2>> {
    let name = gate.canonical_name();
    let clifford = single_qubit_clifford_for_gate(gate).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse frame tracker does not support tableau gate {name}"
        ))
    })?;
    let inverse = clifford.inverse();
    let image = |basis| {
        let output = inverse.apply_basis(basis);
        [output.x_bit(), output.z_bit()]
    };
    Ok(CliffordAction::from_images([
        image(PauliBasis::X),
        image(PauliBasis::Z),
    ]))
}

fn derive_two_qubit_action(gate: Gate) -> AnalysisResult<CliffordAction<4>> {
    let name = gate.canonical_name();
    let inverse_tableau = two_qubit_inverse_tableau(gate)?;
    let bits = |output: Result<stab_algebra::PauliString, _>| {
        let output = output.map_err(|error: stab_algebra::StabilizerError| {
            AnalysisError::invalid_detector_error_model(format!(
                "failed to apply inverse tableau for {name} during sparse reverse tracking: {error}"
            ))
        })?;
        let left = output.get(0).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "inverse tableau for {name} did not return left output basis"
            ))
        })?;
        let right = output.get(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "inverse tableau for {name} did not return right output basis"
            ))
        })?;
        Ok::<_, AnalysisError>([left.x_bit(), left.z_bit(), right.x_bit(), right.z_bit()])
    };
    Ok(CliffordAction::from_images([
        bits(inverse_tableau.x_output(0))?,
        bits(inverse_tableau.z_output(0))?,
        bits(inverse_tableau.x_output(1))?,
        bits(inverse_tableau.z_output(1))?,
    ]))
}

pub(super) fn two_qubit_inverse_tableau(gate: Gate) -> AnalysisResult<Tableau> {
    let gate_name = gate.canonical_name();
    let inverse = gate.inverse().ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!(
            "{gate_name} does not have a unitary inverse for sparse reverse tracking"
        ))
    })?;
    let inverse_tableau = gate_tableau(inverse).map_err(|error| {
        AnalysisError::invalid_detector_error_model(format!(
            "failed to load inverse tableau for {gate_name} during sparse reverse tracking: {error}"
        ))
    })?;
    if inverse_tableau.len() != 2 {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "{gate_name} expected a two-qubit tableau during sparse reverse tracking"
        )));
    }
    Ok(inverse_tableau)
}

impl TrackerScratch {
    fn take_sets<const N: usize>(&mut self) -> [TargetXorSet; N] {
        std::array::from_fn(|_| self.take_set())
    }

    /// Admits `growth` for computing `outputs`, which are out of the pool while they are
    /// computed, so their current storage is admitted with the growth.
    fn admit_action_outputs(
        &self,
        meter: &TrackerMeter,
        outputs: &[TargetXorSet],
        growth: u64,
    ) -> AnalysisResult<()> {
        if growth == 0 {
            return Ok(());
        }
        let in_flight = outputs.iter().fold(0_u64, |sum, output| {
            sum.saturating_add(buffer_bytes(output.capacity()))
        });
        self.admit_growth(meter, growth.saturating_add(in_flight))
    }
}

impl SparseReverseFrameTracker {
    pub(super) fn apply_single_qubit_action(
        &mut self,
        action: CliffordAction<2>,
        qubit: QubitId,
    ) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        let inputs = [
            sorted_slice(self.xs.get(&qubit)),
            sorted_slice(self.zs.get(&qubit)),
        ];
        if inputs.iter().all(|input| input.is_empty()) {
            return Ok(());
        }
        let mut outputs = self.scratch.take_sets::<2>();
        let growth = action.output_growth_bytes(&inputs, &outputs);
        self.scratch
            .admit_action_outputs(&self.meter, &outputs, growth)?;
        action.compute(inputs, &mut outputs, &mut self.scratch.merge);
        let [x, z] = outputs;
        replace_qubit_set(&mut self.xs, qubit, x, &mut self.scratch, &mut self.meter);
        replace_qubit_set(&mut self.zs, qubit, z, &mut self.scratch, &mut self.meter);
        Ok(())
    }

    pub(super) fn apply_two_qubit_action(
        &mut self,
        action: CliffordAction<4>,
        left: QubitId,
        right: QubitId,
    ) -> AnalysisResult<()> {
        self.validate_qubit(left)?;
        self.validate_qubit(right)?;
        let inputs = [
            sorted_slice(self.xs.get(&left)),
            sorted_slice(self.zs.get(&left)),
            sorted_slice(self.xs.get(&right)),
            sorted_slice(self.zs.get(&right)),
        ];
        if inputs.iter().all(|input| input.is_empty()) {
            return Ok(());
        }
        let mut outputs = self.scratch.take_sets::<4>();
        let growth = action.output_growth_bytes(&inputs, &outputs);
        self.scratch
            .admit_action_outputs(&self.meter, &outputs, growth)?;
        action.compute(inputs, &mut outputs, &mut self.scratch.merge);
        let [left_x, left_z, right_x, right_z] = outputs;
        replace_qubit_set(
            &mut self.xs,
            left,
            left_x,
            &mut self.scratch,
            &mut self.meter,
        );
        replace_qubit_set(
            &mut self.zs,
            left,
            left_z,
            &mut self.scratch,
            &mut self.meter,
        );
        replace_qubit_set(
            &mut self.xs,
            right,
            right_x,
            &mut self.scratch,
            &mut self.meter,
        );
        replace_qubit_set(
            &mut self.zs,
            right,
            right_z,
            &mut self.scratch,
            &mut self.meter,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "catalog sweeps fail with the offending gate name"
    )]

    use stab_algebra::{PauliBasis, PauliSign};
    use stab_model::Gate;

    use super::two_qubit_inverse_tableau;
    use super::{derive_two_qubit_action, single_qubit_action, two_qubit_action};
    use crate::{gate_has_tableau, single_qubit_clifford_for_gate};

    const BASES: [PauliBasis; 4] = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];

    #[test]
    fn cached_actions_match_tableau_conjugation_for_every_catalog_clifford() {
        let mut single = 0;
        let mut double = 0;
        for gate in Gate::all() {
            let name = gate.canonical_name();
            if let Ok(clifford) = single_qubit_clifford_for_gate(gate) {
                let inverse = clifford.inverse();
                let action = single_qubit_action(gate).expect("single-qubit action");
                for basis in [PauliBasis::X, PauliBasis::Y, PauliBasis::Z] {
                    let expected = inverse.apply_basis(basis);
                    let actual = action.apply_bits([basis.x_bit(), basis.z_bit()]);
                    assert_eq!(
                        actual,
                        [expected.x_bit(), expected.z_bit()],
                        "{name} on {basis:?}"
                    );
                }
                single += 1;
            } else if gate.is_two_qubit_gate() && gate_has_tableau(gate) {
                let tableau = two_qubit_inverse_tableau(gate).expect("two-qubit inverse tableau");
                let action = two_qubit_action(gate).expect("two-qubit action");
                assert_eq!(Some(action), derive_two_qubit_action(gate).ok(), "{name}");
                for left in BASES {
                    for right in BASES {
                        let input = stab_algebra::advanced::pauli_from_bases_unbudgeted(
                            PauliSign::Plus,
                            [left, right],
                        );
                        let output = tableau.apply(&input).expect("apply inverse tableau");
                        let (Some(out_left), Some(out_right)) = (output.get(0), output.get(1))
                        else {
                            panic!("{name} inverse tableau lost an output qubit");
                        };
                        let actual = action.apply_bits([
                            left.x_bit(),
                            left.z_bit(),
                            right.x_bit(),
                            right.z_bit(),
                        ]);
                        assert_eq!(
                            actual,
                            [
                                out_left.x_bit(),
                                out_left.z_bit(),
                                out_right.x_bit(),
                                out_right.z_bit()
                            ],
                            "{name} on {left:?}{right:?}"
                        );
                    }
                }
                double += 1;
            }
        }
        assert_eq!(single, 24, "every single-qubit Clifford is covered");
        assert!(
            double >= 22,
            "every two-qubit tableau gate is covered, saw {double}"
        );
    }
}
