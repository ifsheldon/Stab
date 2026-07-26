use super::{InverseQecOptions, TimeReversedForFlowsOptions};
use crate::{Circuit, CircuitInstruction, CircuitItem, CircuitResult, Flow, RepeatBlock, Tableau};

/// Returns a compact copy of `circuit` with every instruction and repeat-block tag removed.
///
/// Item order, instruction boundaries, arguments, targets, repeat counts, and repeat nesting are
/// preserved. Removing tags does not fuse adjacent instructions that previously had distinct tags.
pub fn circuit_without_tags(circuit: &Circuit) -> Circuit {
    Circuit::from_items(
        circuit
            .items()
            .iter()
            .map(|item| match item {
                CircuitItem::Instruction(instruction) => {
                    CircuitItem::Instruction(instruction.without_tag())
                }
                CircuitItem::RepeatBlock(repeat) => CircuitItem::RepeatBlock(RepeatBlock::new(
                    repeat.repeat_count(),
                    circuit_without_tags(repeat.body()),
                    None,
                )),
            })
            .collect(),
    )
}

// Temporary pre-0.2 method adapters. The owning APIs are the free analysis functions; these
// adapters preserve source compatibility until the model and analysis crates are extracted.
impl Circuit {
    /// Returns a copy of this circuit with all instruction and repeat-block tags removed.
    ///
    /// Item order, instruction boundaries, arguments, targets, repeat counts, and repeat nesting
    /// are preserved. This compatibility method delegates to [`circuit_without_tags`].
    pub fn without_tags(&self) -> Self {
        circuit_without_tags(self)
    }

    /// Converts the currently supported Clifford circuit subset into a tableau.
    ///
    /// The transform supports unitary Clifford operations plus explicit ignore flags for noise,
    /// measurements, and resets. Measurement feedback, detector semantics, and simulator-backed
    /// tableau extraction are outside this helper's current contract.
    pub fn to_tableau(
        &self,
        ignore_noise: bool,
        ignore_measurement: bool,
        ignore_reset: bool,
    ) -> CircuitResult<Tableau> {
        super::circuit_to_tableau(self, ignore_noise, ignore_measurement, ignore_reset)
    }

    /// Returns the inverse of a unitary Clifford circuit.
    ///
    /// Repeat blocks are inverted recursively. Non-unitary instructions such as measurements,
    /// resets, detectors, and noise return an error instead of being skipped or approximated.
    pub fn inverse_unitary(&self) -> CircuitResult<Self> {
        super::circuit_inverse_unitary(self)
    }

    /// Returns the currently supported QEC inverse subset.
    ///
    /// This includes the unitary inverse plus selected reset-measure-detector, detector-flow,
    /// MPP, MPAD, noisy-measurement, measure-reset, and observable-include rewrites. Unsupported
    /// Stim QEC inverse patterns fail closed instead of being approximated.
    pub fn inverse_qec(&self) -> CircuitResult<Self> {
        super::circuit_inverse_qec(self)
    }

    /// Returns the currently implemented QEC inverse subset with explicit options.
    ///
    /// See [`InverseQecOptions`] for the selected option scope and its fail-closed behavior.
    pub fn inverse_qec_with_options(&self, options: InverseQecOptions) -> CircuitResult<Self> {
        super::circuit_inverse_qec_with_options(self, options)
    }

    /// Returns the supported tracker-driven time reversal for unsigned flows.
    ///
    /// Supports the selected Clifford, measurement, reset, measure-reset, detector, observable,
    /// MPAD, coordinate, and ordinary-noise families. Pure unitary repeats stay folded,
    /// measurement-rich expansion is bounded, and feedback, heralded records, and duplicate
    /// collapse targets fail closed.
    pub fn time_reversed_for_flows(&self, flows: &[Flow]) -> CircuitResult<(Self, Vec<Flow>)> {
        super::circuit_time_reversed_for_flows(self, flows)
    }

    /// Returns the selected time-reversal subset with explicit options.
    ///
    /// See [`TimeReversedForFlowsOptions`] for option-specific compatibility and rejection rules.
    pub fn time_reversed_for_flows_with_options(
        &self,
        flows: &[Flow],
        options: TimeReversedForFlowsOptions,
    ) -> CircuitResult<(Self, Vec<Flow>)> {
        super::circuit_time_reversed_for_flows_with_options(self, flows, options)
    }

    /// Returns a circuit rewritten into the current base-gate simplification subset.
    ///
    /// Supported single-qubit Clifford gates and selected two-qubit Clifford gates are
    /// decomposed. Gates outside the selected subset are preserved verbatim.
    pub fn simplified(&self) -> CircuitResult<Self> {
        super::simplified_circuit(self)
    }

    /// Returns this circuit with repeat blocks unrolled and coordinate shifts applied.
    ///
    /// `SHIFT_COORDS` instructions are absorbed into subsequent `QUBIT_COORDS` and `DETECTOR`
    /// instructions. Materialized expansion is bounded; use the lazy flattened iterators when an
    /// owned expanded circuit is unnecessary.
    pub fn flattened(&self) -> CircuitResult<Self> {
        super::flattened_circuit(self)
    }

    /// Returns owned flattened instructions without adjacent-instruction fusion.
    ///
    /// This applies coordinate shifts and unrolls repeats while preserving each yielded
    /// operation as an independent instruction.
    pub fn flattened_operations(&self) -> CircuitResult<Vec<CircuitInstruction>> {
        super::flattened_circuit_operations(self)
    }

    /// Returns a copy of this circuit with noisy behavior removed while preserving records.
    ///
    /// Ordinary noise instructions are dropped, noisy measurement probabilities are stripped,
    /// and heralded noise instructions become deterministic zero `MPAD` results so
    /// measurement-record indexing remains unchanged.
    pub fn without_noise(&self) -> CircuitResult<Self> {
        super::circuit_without_noise(self)
    }

    /// Returns the currently supported H/S/CX/M/R decomposition.
    ///
    /// Unsupported operations return a circuit error instead of being silently preserved.
    pub fn decomposed(&self) -> CircuitResult<Self> {
        super::decomposed_circuit(self)
    }

    /// Returns the currently supported transform with measurement feedback inlined.
    ///
    /// The transform preserves folded repeats where supported, bounds repeat expansion, and
    /// rejects unsupported feedback patterns instead of changing their semantics.
    pub fn with_inlined_feedback(&self) -> CircuitResult<Self> {
        super::circuit_with_inlined_feedback(self)
    }
}
