//! Pinned Stim v1.16.0 stabilizer flows of the non-unitary gates, as typed table data.
//!
//! Unitary gates have no entry here: their flows are the images of their tableau, which the
//! algebra layer owns.

use std::fmt::{self, Display, Formatter};

use super::GateKind;
use crate::{MeasureRecordOffset, Pauli};

/// A signed Pauli product over a gate's local qubits, as plain table data.
///
/// Position `q` holds the Pauli on local qubit `q`, or `None` for identity. An empty product is
/// the identity `1` of a flow side that touches no qubit, such as the input of a reset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatePauliString {
    negative: bool,
    paulis: &'static [Option<Pauli>],
}

impl GatePauliString {
    const fn new(negative: bool, paulis: &'static [Option<Pauli>]) -> Self {
        Self { negative, paulis }
    }

    /// Whether the product carries a `-1` sign.
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.negative
    }

    /// The Pauli on each local qubit, `None` meaning identity.
    #[inline]
    pub const fn paulis(self) -> &'static [Option<Pauli>] {
        self.paulis
    }
}

/// Renders the dense Stim form, such as `-YYZ` or `X__`; the empty product renders as `1`.
impl Display for GatePauliString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        if self.paulis.is_empty() {
            return f.write_str("1");
        }
        for pauli in self.paulis {
            match pauli {
                Some(pauli) => pauli.fmt(f)?,
                None => f.write_str("_")?,
            }
        }
        Ok(())
    }
}

/// One stabilizer flow of a non-unitary gate: `input -> output xor rec[...]`.
///
/// The measurement terms are record offsets relative to the end of the gate's own results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GateFlowDescriptor {
    input: GatePauliString,
    output: GatePauliString,
    measurements: &'static [MeasureRecordOffset],
}

impl GateFlowDescriptor {
    const fn new(
        input: GatePauliString,
        output: GatePauliString,
        measurements: &'static [MeasureRecordOffset],
    ) -> Self {
        Self {
            input,
            output,
            measurements,
        }
    }

    #[inline]
    pub const fn input(self) -> GatePauliString {
        self.input
    }

    #[inline]
    pub const fn output(self) -> GatePauliString {
        self.output
    }

    /// The measurement records the flow's output includes, as offsets into the gate's results.
    #[inline]
    pub const fn measurements(self) -> &'static [MeasureRecordOffset] {
        self.measurements
    }
}

/// Renders Stim's flow text, such as `Z -> rec[-1]`, `1 -> Z`, or `Z__ -> -YYZ`.
impl Display for GateFlowDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> ", self.input)?;
        let mut has_output = !self.output.paulis.is_empty();
        if has_output {
            self.output.fmt(f)?;
        }
        for measurement in self.measurements {
            if has_output {
                f.write_str(" xor ")?;
            }
            write!(f, "rec[{}]", measurement.get())?;
            has_output = true;
        }
        if !has_output {
            f.write_str("1")?;
        }
        Ok(())
    }
}

const I: Option<Pauli> = None;
const X: Option<Pauli> = Some(Pauli::X);
const Y: Option<Pauli> = Some(Pauli::Y);
const Z: Option<Pauli> = Some(Pauli::Z);

/// The identity side of a flow.
const ONE: GatePauliString = GatePauliString::new(false, &[]);
const REC_1: &[MeasureRecordOffset] = &[MeasureRecordOffset::from_packed_distance(1)];
const REC_2: &[MeasureRecordOffset] = &[MeasureRecordOffset::from_packed_distance(2)];

const fn plus(paulis: &'static [Option<Pauli>]) -> GatePauliString {
    GatePauliString::new(false, paulis)
}

const fn minus(paulis: &'static [Option<Pauli>]) -> GatePauliString {
    GatePauliString::new(true, paulis)
}

/// `input -> output`.
const fn maps(input: &'static [Option<Pauli>], output: GatePauliString) -> GateFlowDescriptor {
    GateFlowDescriptor::new(plus(input), output, &[])
}

/// `input -> input`.
const fn keeps(paulis: &'static [Option<Pauli>]) -> GateFlowDescriptor {
    maps(paulis, plus(paulis))
}

/// `input -> rec[...]`.
const fn measures(
    input: &'static [Option<Pauli>],
    record: &'static [MeasureRecordOffset],
) -> GateFlowDescriptor {
    GateFlowDescriptor::new(plus(input), ONE, record)
}

/// `1 -> output`.
const fn prepares(output: &'static [Option<Pauli>]) -> GateFlowDescriptor {
    GateFlowDescriptor::new(ONE, plus(output), &[])
}

pub(crate) const fn gate_flow_descriptors(kind: GateKind) -> Option<&'static [GateFlowDescriptor]> {
    Some(match kind {
        GateKind::M => const { &[measures(&[Z], REC_1), keeps(&[Z])] },
        GateKind::MX => const { &[measures(&[X], REC_1), keeps(&[X])] },
        GateKind::MY => const { &[measures(&[Y], REC_1), keeps(&[Y])] },
        GateKind::R => const { &[prepares(&[Z])] },
        GateKind::RX => const { &[prepares(&[X])] },
        GateKind::RY => const { &[prepares(&[Y])] },
        GateKind::MR => const { &[measures(&[Z], REC_1), prepares(&[Z])] },
        GateKind::MRX => const { &[measures(&[X], REC_1), prepares(&[X])] },
        GateKind::MRY => const { &[measures(&[Y], REC_1), prepares(&[Y])] },
        GateKind::MXX => {
            const {
                &[
                    keeps(&[X, I]),
                    keeps(&[I, X]),
                    keeps(&[Z, Z]),
                    measures(&[X, X], REC_1),
                ]
            }
        }
        GateKind::MYY => {
            const {
                &[
                    keeps(&[X, X]),
                    keeps(&[Y, I]),
                    keeps(&[I, Y]),
                    measures(&[Y, Y], REC_1),
                ]
            }
        }
        GateKind::MZZ => {
            const {
                &[
                    keeps(&[X, X]),
                    keeps(&[Z, I]),
                    keeps(&[I, Z]),
                    measures(&[Z, Z], REC_1),
                ]
            }
        }
        GateKind::MPP => {
            const {
                &[
                    measures(&[X, Y, Z, I, I], REC_2),
                    measures(&[I, I, I, X, X], REC_1),
                    keeps(&[X, I, I, I, I]),
                    keeps(&[I, Y, I, I, I]),
                    keeps(&[I, I, Z, I, I]),
                    keeps(&[I, I, I, X, I]),
                    keeps(&[I, I, I, I, X]),
                    keeps(&[Z, Z, I, I, I]),
                    keeps(&[I, X, X, I, I]),
                    keeps(&[I, I, I, Z, Z]),
                ]
            }
        }
        GateKind::SPP => {
            const {
                &[
                    keeps(&[X, I, I]),
                    maps(&[Z, I, I], minus(&[Y, Y, Z])),
                    maps(&[I, X, I], minus(&[X, Z, Z])),
                    maps(&[I, Z, I], plus(&[X, X, Z])),
                    maps(&[I, I, X], plus(&[X, Y, Y])),
                    keeps(&[I, I, Z]),
                ]
            }
        }
        GateKind::SPP_DAG => {
            const {
                &[
                    keeps(&[X, I, I]),
                    maps(&[Z, I, I], plus(&[Y, Y, Z])),
                    maps(&[I, X, I], plus(&[X, Z, Z])),
                    maps(&[I, Z, I], minus(&[X, X, Z])),
                    maps(&[I, I, X], minus(&[X, Y, Y])),
                    keeps(&[I, I, Z]),
                ]
            }
        }
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => return None,
    })
}
