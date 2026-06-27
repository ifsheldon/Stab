use crate::{MeasureRecordOffset, PauliBasis};

use super::stabilizer_frame::LocalTableauTransform;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SampleOperation {
    ApplyTableau {
        targets: Vec<usize>,
        transform: LocalTableauTransform,
    },
    Reset {
        qubit: usize,
        basis: PauliBasis,
    },
    Measure {
        qubit: usize,
        basis: PauliBasis,
        inverted: bool,
        flip_probability: f64,
        reset: bool,
    },
    MeasureProduct {
        terms: Vec<(usize, PauliBasis)>,
        inverted: bool,
        flip_probability: f64,
    },
    Pad {
        value: bool,
        flip_probability: f64,
    },
    SingleQubitPauliChannel {
        qubit: usize,
        probabilities: [f64; 3],
    },
    TwoQubitPauliChannel {
        left: usize,
        right: usize,
        probabilities: [f64; 15],
    },
    CorrelatedError {
        else_branch: bool,
        probability: f64,
        terms: Vec<(usize, PauliBasis)>,
    },
    HeraldedPauliChannel {
        qubit: usize,
        probabilities: [f64; 4],
    },
    FeedbackPauli {
        offset: MeasureRecordOffset,
        qubit: usize,
        basis: PauliBasis,
    },
    Repeat {
        count: u64,
        body: Vec<SampleOperation>,
    },
}

pub(super) const SINGLE_QUBIT_PAULI_CHANNEL_BASES: [PauliBasis; 3] =
    [PauliBasis::X, PauliBasis::Y, PauliBasis::Z];

pub(super) const TWO_QUBIT_PAULI_CHANNEL_BASES: [(Option<PauliBasis>, Option<PauliBasis>); 15] = [
    (None, Some(PauliBasis::X)),
    (None, Some(PauliBasis::Y)),
    (None, Some(PauliBasis::Z)),
    (Some(PauliBasis::X), None),
    (Some(PauliBasis::X), Some(PauliBasis::X)),
    (Some(PauliBasis::X), Some(PauliBasis::Y)),
    (Some(PauliBasis::X), Some(PauliBasis::Z)),
    (Some(PauliBasis::Y), None),
    (Some(PauliBasis::Y), Some(PauliBasis::X)),
    (Some(PauliBasis::Y), Some(PauliBasis::Y)),
    (Some(PauliBasis::Y), Some(PauliBasis::Z)),
    (Some(PauliBasis::Z), None),
    (Some(PauliBasis::Z), Some(PauliBasis::X)),
    (Some(PauliBasis::Z), Some(PauliBasis::Y)),
    (Some(PauliBasis::Z), Some(PauliBasis::Z)),
];
