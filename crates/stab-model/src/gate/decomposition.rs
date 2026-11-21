//! Pinned Stim v1.16.0 decompositions into the H/S/CX/M/R basis, as typed table data.

use std::fmt::{self, Display, Formatter};

use super::{Gate, GateKind};
use crate::circuit::Circuit;
use crate::{CircuitInstruction, QubitId, Target};

/// One instruction of an H/S/CX/M/R decomposition, over the gate's local qubit indices.
///
/// Each step is one instruction of Stim's decomposition text, so a step may repeat a qubit,
/// such as `S 1 1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecompositionStep {
    H(&'static [u8]),
    S(&'static [u8]),
    /// Control and target pairs.
    Cx(&'static [[u8; 2]]),
    M(&'static [u8]),
    R(&'static [u8]),
}

impl DecompositionStep {
    /// Returns the base gate this step applies.
    #[inline]
    pub fn gate(self) -> Gate {
        match self {
            Self::H(_) => Gate::H,
            Self::S(_) => Gate::S,
            Self::Cx(_) => Gate::CX,
            Self::M(_) => Gate::M,
            Self::R(_) => GateKind::R.gate(),
        }
    }

    /// Returns the local qubits in target order, flattening `CX` pairs.
    #[inline]
    pub fn qubits(self) -> &'static [u8] {
        match self {
            Self::H(qubits) | Self::S(qubits) | Self::M(qubits) | Self::R(qubits) => qubits,
            Self::Cx(pairs) => pairs.as_flattened(),
        }
    }

    fn to_instruction(self) -> CircuitInstruction {
        let targets = self
            .qubits()
            .iter()
            .map(|&qubit| Target::qubit(QubitId::from_packed(u32::from(qubit)), false))
            .collect();
        // Every table step names its base gate's target shape: single qubits, or `CX` pairs of
        // distinct qubits, which the decomposition test checks against Stim's parsed text.
        CircuitInstruction::from_validated_parts(self.gate(), Vec::new(), targets, None)
    }
}

/// Renders the step as one Stim instruction, such as `CX 0 1 1 0`.
impl Display for DecompositionStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.gate().canonical_name())?;
        for qubit in self.qubits() {
            write!(f, " {qubit}")?;
        }
        Ok(())
    }
}

/// Stim v1.16.0 gate decomposition metadata into the H/S/CX/M/R basis.
///
/// The steps act on local qubit indices: `0` for a single-qubit gate, `0` and `1` for a pair
/// gate, and the qubits of Stim's example targets for `MPP`, `SPP`, and `SPP_DAG`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GateDecomposition {
    steps: &'static [DecompositionStep],
}

impl GateDecomposition {
    /// Returns the decomposition's instructions in execution order.
    #[inline]
    pub const fn steps(self) -> &'static [DecompositionStep] {
        self.steps
    }

    /// Builds the decomposition as a circuit, fusing adjacent steps of one gate as Stim's parser
    /// does for the decomposition text.
    pub fn to_circuit(self) -> Circuit {
        let mut circuit = Circuit::new();
        for step in self.steps {
            circuit.append_instruction(step.to_instruction());
        }
        circuit
    }
}

/// Renders one instruction per line; Stim's text may spell `CX` as `CNOT` or split and merge
/// instructions differently, but it parses to the same circuit.
impl Display for GateDecomposition {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for step in self.steps {
            writeln!(f, "{step}")?;
        }
        Ok(())
    }
}

pub(crate) fn gate_decomposition(kind: GateKind) -> Option<GateDecomposition> {
    use DecompositionStep::{Cx, H, M, R, S};
    let steps: &'static [DecompositionStep] = match kind {
        GateKind::MX => &[H(&[0]), M(&[0]), H(&[0])],
        GateKind::MY => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            H(&[0]),
            M(&[0]),
            H(&[0]),
            S(&[0]),
        ],
        GateKind::M => &[M(&[0])],
        GateKind::MRX => &[H(&[0]), M(&[0]), R(&[0]), H(&[0])],
        GateKind::MRY => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            H(&[0]),
            M(&[0]),
            R(&[0]),
            H(&[0]),
            S(&[0]),
        ],
        GateKind::MR => &[M(&[0]), R(&[0])],
        GateKind::RX => &[R(&[0]), H(&[0])],
        GateKind::RY => &[R(&[0]), H(&[0]), S(&[0])],
        GateKind::R => &[R(&[0])],
        GateKind::XCX => &[H(&[0]), Cx(&[[0, 1]]), H(&[0])],
        GateKind::XCY => &[
            H(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
            Cx(&[[0, 1]]),
            H(&[0]),
            S(&[1]),
        ],
        GateKind::XCZ => &[Cx(&[[1, 0]])],
        GateKind::YCX => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            H(&[1]),
            Cx(&[[1, 0]]),
            S(&[0]),
            H(&[1]),
        ],
        GateKind::YCY => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
            H(&[0]),
            Cx(&[[0, 1]]),
            H(&[0]),
            S(&[0]),
            S(&[1]),
        ],
        GateKind::YCZ => &[S(&[0]), S(&[0]), S(&[0]), Cx(&[[1, 0]]), S(&[0])],
        GateKind::CX => &[Cx(&[[0, 1]])],
        GateKind::CY => &[S(&[1]), S(&[1]), S(&[1]), Cx(&[[0, 1]]), S(&[1])],
        GateKind::CZ => &[H(&[1]), Cx(&[[0, 1]]), H(&[1])],
        GateKind::H => &[H(&[0])],
        GateKind::H_XY => &[H(&[0]), S(&[0]), S(&[0]), H(&[0]), S(&[0])],
        GateKind::H_YZ => &[H(&[0]), S(&[0]), H(&[0]), S(&[0]), S(&[0])],
        GateKind::H_NXY => &[S(&[0]), H(&[0]), S(&[0]), S(&[0]), H(&[0])],
        GateKind::H_NXZ => &[S(&[0]), S(&[0]), H(&[0]), S(&[0]), S(&[0])],
        GateKind::H_NYZ => &[S(&[0]), S(&[0]), H(&[0]), S(&[0]), H(&[0])],
        GateKind::MXX => &[Cx(&[[0, 1]]), H(&[0]), M(&[0]), H(&[0]), Cx(&[[0, 1]])],
        GateKind::MYY => &[
            S(&[0, 1]),
            Cx(&[[0, 1]]),
            H(&[0]),
            M(&[0]),
            S(&[1, 1]),
            H(&[0]),
            Cx(&[[0, 1]]),
            S(&[0, 1]),
        ],
        GateKind::MZZ => &[Cx(&[[0, 1]]), M(&[1]), Cx(&[[0, 1]])],
        GateKind::X => &[H(&[0]), S(&[0]), S(&[0]), H(&[0])],
        GateKind::Y => &[S(&[0]), S(&[0]), H(&[0]), S(&[0]), S(&[0]), H(&[0])],
        GateKind::Z => &[S(&[0]), S(&[0])],
        GateKind::MPP => &[
            S(&[1, 1, 1]),
            H(&[0, 1, 3, 4]),
            Cx(&[[2, 0], [1, 0], [4, 3]]),
            M(&[0, 3]),
            Cx(&[[2, 0], [1, 0], [4, 3]]),
            H(&[0, 1, 3, 4]),
            S(&[1]),
        ],
        GateKind::SPP => &[
            Cx(&[[2, 1]]),
            Cx(&[[1, 0]]),
            S(&[1]),
            S(&[1]),
            H(&[1]),
            Cx(&[[1, 0]]),
            Cx(&[[2, 1]]),
        ],
        GateKind::SPP_DAG => &[
            Cx(&[[2, 1]]),
            Cx(&[[1, 0]]),
            H(&[1]),
            S(&[1]),
            S(&[1]),
            Cx(&[[1, 0]]),
            Cx(&[[2, 1]]),
        ],
        GateKind::C_XYZ => &[S(&[0]), S(&[0]), S(&[0]), H(&[0])],
        GateKind::C_NXYZ => &[S(&[0]), S(&[0]), S(&[0]), H(&[0]), S(&[0]), S(&[0])],
        GateKind::C_XNYZ => &[S(&[0]), H(&[0])],
        GateKind::C_XYNZ => &[S(&[0]), H(&[0]), S(&[0]), S(&[0])],
        GateKind::C_ZYX => &[H(&[0]), S(&[0])],
        GateKind::C_ZYNX => &[S(&[0]), S(&[0]), H(&[0]), S(&[0])],
        GateKind::C_ZNYX => &[H(&[0]), S(&[0]), S(&[0]), S(&[0])],
        GateKind::C_NZYX => &[S(&[0]), S(&[0]), H(&[0]), S(&[0]), S(&[0]), S(&[0])],
        GateKind::SQRT_X => &[H(&[0]), S(&[0]), H(&[0])],
        GateKind::SQRT_X_DAG => &[S(&[0]), H(&[0]), S(&[0])],
        GateKind::SQRT_Y => &[S(&[0]), S(&[0]), H(&[0])],
        GateKind::SQRT_Y_DAG => &[H(&[0]), S(&[0]), S(&[0])],
        GateKind::S => &[S(&[0])],
        GateKind::S_DAG => &[S(&[0]), S(&[0]), S(&[0])],
        GateKind::SQRT_XX => &[
            H(&[0]),
            Cx(&[[0, 1]]),
            H(&[1]),
            S(&[0]),
            S(&[1]),
            H(&[0]),
            H(&[1]),
        ],
        GateKind::SQRT_XX_DAG => &[
            H(&[0]),
            Cx(&[[0, 1]]),
            H(&[1]),
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
            H(&[0]),
            H(&[1]),
        ],
        GateKind::SQRT_YY => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
            H(&[0]),
            Cx(&[[0, 1]]),
            H(&[1]),
            S(&[0]),
            S(&[1]),
            H(&[0]),
            H(&[1]),
            S(&[0]),
            S(&[1]),
        ],
        GateKind::SQRT_YY_DAG => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            H(&[0]),
            Cx(&[[0, 1]]),
            H(&[1]),
            S(&[0]),
            S(&[1]),
            H(&[0]),
            H(&[1]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
        ],
        GateKind::SQRT_ZZ => &[H(&[1]), Cx(&[[0, 1]]), H(&[1]), S(&[0]), S(&[1])],
        GateKind::SQRT_ZZ_DAG => &[
            H(&[1]),
            Cx(&[[0, 1]]),
            H(&[1]),
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
        ],
        GateKind::SWAP => &[Cx(&[[0, 1]]), Cx(&[[1, 0]]), Cx(&[[0, 1]])],
        GateKind::ISWAP => &[
            H(&[0]),
            Cx(&[[0, 1]]),
            Cx(&[[1, 0]]),
            H(&[1]),
            S(&[1]),
            S(&[0]),
        ],
        GateKind::ISWAP_DAG => &[
            S(&[0]),
            S(&[0]),
            S(&[0]),
            S(&[1]),
            S(&[1]),
            S(&[1]),
            H(&[1]),
            Cx(&[[1, 0]]),
            Cx(&[[0, 1]]),
            H(&[0]),
        ],
        GateKind::CXSWAP => &[Cx(&[[1, 0]]), Cx(&[[0, 1]])],
        GateKind::SWAPCX => &[Cx(&[[0, 1]]), Cx(&[[1, 0]])],
        GateKind::CZSWAP => &[H(&[0]), Cx(&[[0, 1]]), Cx(&[[1, 0]]), H(&[1])],
        GateKind::I | GateKind::II => &[],
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
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
        | GateKind::HERALDED_PAULI_CHANNEL_1 => return None,
    };
    Some(GateDecomposition { steps })
}
