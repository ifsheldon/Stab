use super::GateKind;

/// Stim v1.16.0 gate decomposition metadata into the H/S/CX/M/R basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GateDecomposition {
    stim_text: &'static str,
}

impl GateDecomposition {
    pub(crate) const fn new(stim_text: &'static str) -> Self {
        Self { stim_text }
    }

    /// Returns the pinned Stim v1.16.0 `.stim` text for this gate decomposition.
    pub fn as_stim_str(self) -> &'static str {
        self.stim_text
    }
}

impl AsRef<str> for GateDecomposition {
    fn as_ref(&self) -> &str {
        self.stim_text
    }
}

pub(crate) fn gate_decomposition_text(kind: GateKind) -> Option<&'static str> {
    match kind {
        GateKind::MX => Some(
            r#"
H 0
M 0
H 0
"#,
        ),
        GateKind::MY => Some(
            r#"
S 0
S 0
S 0
H 0
M 0
H 0
S 0
"#,
        ),
        GateKind::M => Some(
            r#"
M 0
"#,
        ),
        GateKind::MRX => Some(
            r#"
H 0
M 0
R 0
H 0
"#,
        ),
        GateKind::MRY => Some(
            r#"
S 0
S 0
S 0
H 0
M 0
R 0
H 0
S 0
"#,
        ),
        GateKind::MR => Some(
            r#"
M 0
R 0
"#,
        ),
        GateKind::RX => Some(
            r#"
R 0
H 0
"#,
        ),
        GateKind::RY => Some(
            r#"
R 0
H 0
S 0
"#,
        ),
        GateKind::R => Some(
            r#"
R 0
"#,
        ),
        GateKind::XCX => Some(
            r#"
H 0
CNOT 0 1
H 0
"#,
        ),
        GateKind::XCY => Some(
            r#"
H 0
S 1
S 1
S 1
CNOT 0 1
H 0
S 1
"#,
        ),
        GateKind::XCZ => Some(
            r#"
CNOT 1 0
"#,
        ),
        GateKind::YCX => Some(
            r#"
S 0
S 0
S 0
H 1
CNOT 1 0
S 0
H 1
"#,
        ),
        GateKind::YCY => Some(
            r#"
S 0
S 0
S 0
S 1
S 1
S 1
H 0
CNOT 0 1
H 0
S 0
S 1
"#,
        ),
        GateKind::YCZ => Some(
            r#"
S 0
S 0
S 0
CNOT 1 0
S 0
"#,
        ),
        GateKind::CX => Some(
            r#"
CNOT 0 1
"#,
        ),
        GateKind::CY => Some(
            r#"
S 1
S 1
S 1
CNOT 0 1
S 1
"#,
        ),
        GateKind::CZ => Some(
            r#"
H 1
CNOT 0 1
H 1
"#,
        ),
        GateKind::H => Some(
            r#"
H 0
"#,
        ),
        GateKind::H_XY => Some(
            r#"
H 0
S 0
S 0
H 0
S 0
"#,
        ),
        GateKind::H_YZ => Some(
            r#"
H 0
S 0
H 0
S 0
S 0
"#,
        ),
        GateKind::H_NXY => Some(
            r#"
S 0
H 0
S 0
S 0
H 0
"#,
        ),
        GateKind::H_NXZ => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
"#,
        ),
        GateKind::H_NYZ => Some(
            r#"
S 0
S 0
H 0
S 0
H 0
"#,
        ),
        GateKind::MXX => Some(
            r#"
CX 0 1
H 0
M 0
H 0
CX 0 1
"#,
        ),
        GateKind::MYY => Some(
            r#"
S 0 1
CX 0 1
H 0
M 0
S 1 1
H 0
CX 0 1
S 0 1
"#,
        ),
        GateKind::MZZ => Some(
            r#"
CX 0 1
M 1
CX 0 1
"#,
        ),
        GateKind::I => Some(
            r#"
# (no operations)
"#,
        ),
        GateKind::X => Some(
            r#"
H 0
S 0
S 0
H 0
"#,
        ),
        GateKind::Y => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
H 0
"#,
        ),
        GateKind::Z => Some(
            r#"
S 0
S 0
"#,
        ),
        GateKind::MPP => Some(
            r#"
S 1 1 1
H 0 1 3 4
CX 2 0 1 0 4 3
M 0 3
CX 2 0 1 0 4 3
H 0 1 3 4
S 1
"#,
        ),
        GateKind::SPP => Some(
            r#"
CX 2 1
CX 1 0
S 1
S 1
H 1
CX 1 0
CX 2 1
"#,
        ),
        GateKind::SPP_DAG => Some(
            r#"
CX 2 1
CX 1 0
H 1
S 1
S 1
CX 1 0
CX 2 1
"#,
        ),
        GateKind::C_XYZ => Some(
            r#"
S 0
S 0
S 0
H 0
"#,
        ),
        GateKind::C_NXYZ => Some(
            r#"
S 0
S 0
S 0
H 0
S 0
S 0
"#,
        ),
        GateKind::C_XNYZ => Some(
            r#"
S 0
H 0
"#,
        ),
        GateKind::C_XYNZ => Some(
            r#"
S 0
H 0
S 0
S 0
"#,
        ),
        GateKind::C_ZYX => Some(
            r#"
H 0
S 0
"#,
        ),
        GateKind::C_ZYNX => Some(
            r#"
S 0
S 0
H 0
S 0
"#,
        ),
        GateKind::C_ZNYX => Some(
            r#"
H 0
S 0
S 0
S 0
"#,
        ),
        GateKind::C_NZYX => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
S 0
"#,
        ),
        GateKind::SQRT_X => Some(
            r#"
H 0
S 0
H 0
"#,
        ),
        GateKind::SQRT_X_DAG => Some(
            r#"
S 0
H 0
S 0
"#,
        ),
        GateKind::SQRT_Y => Some(
            r#"
S 0
S 0
H 0
"#,
        ),
        GateKind::SQRT_Y_DAG => Some(
            r#"
H 0
S 0
S 0
"#,
        ),
        GateKind::S => Some(
            r#"
S 0
"#,
        ),
        GateKind::S_DAG => Some(
            r#"
S 0
S 0
S 0
"#,
        ),
        GateKind::II => Some(
            r#"
"#,
        ),
        GateKind::SQRT_XX => Some(
            r#"
H 0
CNOT 0 1
H 1
S 0
S 1
H 0
H 1
"#,
        ),
        GateKind::SQRT_XX_DAG => Some(
            r#"
H 0
CNOT 0 1
H 1
S 0
S 0
S 0
S 1
S 1
S 1
H 0
H 1
"#,
        ),
        GateKind::SQRT_YY => Some(
            r#"
S 0
S 0
S 0
S 1
S 1
S 1
H 0
CNOT 0 1
H 1
S 0
S 1
H 0
H 1
S 0
S 1
"#,
        ),
        GateKind::SQRT_YY_DAG => Some(
            r#"
S 0
S 0
S 0
S 1
H 0
CNOT 0 1
H 1
S 0
S 1
H 0
H 1
S 0
S 1
S 1
S 1
"#,
        ),
        GateKind::SQRT_ZZ => Some(
            r#"
H 1
CNOT 0 1
H 1
S 0
S 1
"#,
        ),
        GateKind::SQRT_ZZ_DAG => Some(
            r#"
H 1
CNOT 0 1
H 1
S 0
S 0
S 0
S 1
S 1
S 1
"#,
        ),
        GateKind::SWAP => Some(
            r#"
CNOT 0 1
CNOT 1 0
CNOT 0 1
"#,
        ),
        GateKind::ISWAP => Some(
            r#"
H 0
CNOT 0 1
CNOT 1 0
H 1
S 1
S 0
"#,
        ),
        GateKind::ISWAP_DAG => Some(
            r#"
S 0
S 0
S 0
S 1
S 1
S 1
H 1
CNOT 1 0
CNOT 0 1
H 0
"#,
        ),
        GateKind::CXSWAP => Some(
            r#"
CNOT 1 0
CNOT 0 1
"#,
        ),
        GateKind::SWAPCX => Some(
            r#"
CNOT 0 1
CNOT 1 0
"#,
        ),
        GateKind::CZSWAP => Some(
            r#"
H 0
CX 0 1
CX 1 0
H 1
"#,
        ),
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
        | GateKind::HERALDED_PAULI_CHANNEL_1 => None,
    }
}
