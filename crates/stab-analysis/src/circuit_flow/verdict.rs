//! The answer the unsigned flow checker gives for one flow.

use stab_algebra::{FlowMeasurementIndex, PauliString};

/// The answer to one unsigned stabilizer flow query.
///
/// The checker decides every flow of a valid circuit. An invalid circuit, a memory-budget
/// refusal, or a cancellation stops the whole batch with an [`crate::AnalysisError`] instead, so
/// it never reads as a verdict.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowVerdict {
    /// The circuit has the flow, ignoring signs.
    Holds,
    /// The circuit does not have the flow, for the stated reason.
    Violated(FlowViolation),
}

impl FlowVerdict {
    /// Returns true when the circuit has the flow.
    pub const fn holds(&self) -> bool {
        matches!(self, Self::Holds)
    }

    /// Returns why the circuit does not have the flow, when it does not.
    pub const fn violation(&self) -> Option<&FlowViolation> {
        match self {
            Self::Violated(violation) => Some(violation),
            Self::Holds => None,
        }
    }
}

/// Why a circuit does not have an unsigned stabilizer flow.
///
/// The mismatch kind depends only on the query, never on the algorithm that checked it.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowViolation {
    /// A circuit of unitaries, annotations, and noise mapped the input of a Pauli-only flow to a
    /// different output Pauli string. The actual output is the circuit's image of the input, as
    /// wide as the widest of the circuit, the input, and the output.
    OutputMismatch {
        expected_output: PauliString,
        actual_output: PauliString,
    },
    /// A circuit with collapsing or classically controlled operations, or a flow with measurement
    /// or observable terms, pulled the requested output, measurement, and observable terms back to
    /// a different input Pauli string.
    InputMismatch {
        expected_input: PauliString,
        actual_input: PauliString,
    },
    /// A flow measurement term names a measurement record the circuit does not have.
    MeasurementRecordOutOfRange {
        record: FlowMeasurementIndex,
        measurement_count: usize,
    },
    /// A collapse operation anticommutes with the tracked flow region.
    CollapseAnticommutation,
}
