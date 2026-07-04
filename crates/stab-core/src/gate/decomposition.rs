use crate::{Circuit, CircuitResult};

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

    /// Parses the decomposition text into a Stab circuit.
    pub fn to_circuit(self) -> CircuitResult<Circuit> {
        Circuit::from_stim_str(self.stim_text)
    }
}

impl AsRef<str> for GateDecomposition {
    fn as_ref(&self) -> &str {
        self.stim_text
    }
}

pub(crate) fn gate_decomposition(gate_name: &str) -> Option<GateDecomposition> {
    match gate_name {
        "MX" => Some(GateDecomposition::new(
            r#"
H 0
M 0
H 0
"#,
        )),
        "MY" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
H 0
M 0
H 0
S 0
"#,
        )),
        "M" => Some(GateDecomposition::new(
            r#"
M 0
"#,
        )),
        "MRX" => Some(GateDecomposition::new(
            r#"
H 0
M 0
R 0
H 0
"#,
        )),
        "MRY" => Some(GateDecomposition::new(
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
        )),
        "MR" => Some(GateDecomposition::new(
            r#"
M 0
R 0
"#,
        )),
        "RX" => Some(GateDecomposition::new(
            r#"
R 0
H 0
"#,
        )),
        "RY" => Some(GateDecomposition::new(
            r#"
R 0
H 0
S 0
"#,
        )),
        "R" => Some(GateDecomposition::new(
            r#"
R 0
"#,
        )),
        "XCX" => Some(GateDecomposition::new(
            r#"
H 0
CNOT 0 1
H 0
"#,
        )),
        "XCY" => Some(GateDecomposition::new(
            r#"
H 0
S 1
S 1
S 1
CNOT 0 1
H 0
S 1
"#,
        )),
        "XCZ" => Some(GateDecomposition::new(
            r#"
CNOT 1 0
"#,
        )),
        "YCX" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
H 1
CNOT 1 0
S 0
H 1
"#,
        )),
        "YCY" => Some(GateDecomposition::new(
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
        )),
        "YCZ" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
CNOT 1 0
S 0
"#,
        )),
        "CX" => Some(GateDecomposition::new(
            r#"
CNOT 0 1
"#,
        )),
        "CY" => Some(GateDecomposition::new(
            r#"
S 1
S 1
S 1
CNOT 0 1
S 1
"#,
        )),
        "CZ" => Some(GateDecomposition::new(
            r#"
H 1
CNOT 0 1
H 1
"#,
        )),
        "H" => Some(GateDecomposition::new(
            r#"
H 0
"#,
        )),
        "H_XY" => Some(GateDecomposition::new(
            r#"
H 0
S 0
S 0
H 0
S 0
"#,
        )),
        "H_YZ" => Some(GateDecomposition::new(
            r#"
H 0
S 0
H 0
S 0
S 0
"#,
        )),
        "H_NXY" => Some(GateDecomposition::new(
            r#"
S 0
H 0
S 0
S 0
H 0
"#,
        )),
        "H_NXZ" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
S 0
S 0
"#,
        )),
        "H_NYZ" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
S 0
H 0
"#,
        )),
        "MXX" => Some(GateDecomposition::new(
            r#"
CX 0 1
H 0
M 0
H 0
CX 0 1
"#,
        )),
        "MYY" => Some(GateDecomposition::new(
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
        )),
        "MZZ" => Some(GateDecomposition::new(
            r#"
CX 0 1
M 1
CX 0 1
"#,
        )),
        "I" => Some(GateDecomposition::new(
            r#"
# (no operations)
"#,
        )),
        "X" => Some(GateDecomposition::new(
            r#"
H 0
S 0
S 0
H 0
"#,
        )),
        "Y" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
S 0
S 0
H 0
"#,
        )),
        "Z" => Some(GateDecomposition::new(
            r#"
S 0
S 0
"#,
        )),
        "MPP" => Some(GateDecomposition::new(
            r#"
S 1 1 1
H 0 1 3 4
CX 2 0 1 0 4 3
M 0 3
CX 2 0 1 0 4 3
H 0 1 3 4
S 1
"#,
        )),
        "SPP" => Some(GateDecomposition::new(
            r#"
CX 2 1
CX 1 0
S 1
S 1
H 1
CX 1 0
CX 2 1
"#,
        )),
        "SPP_DAG" => Some(GateDecomposition::new(
            r#"
CX 2 1
CX 1 0
H 1
S 1
S 1
CX 1 0
CX 2 1
"#,
        )),
        "C_XYZ" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
H 0
"#,
        )),
        "C_NXYZ" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
H 0
S 0
S 0
"#,
        )),
        "C_XNYZ" => Some(GateDecomposition::new(
            r#"
S 0
H 0
"#,
        )),
        "C_XYNZ" => Some(GateDecomposition::new(
            r#"
S 0
H 0
S 0
S 0
"#,
        )),
        "C_ZYX" => Some(GateDecomposition::new(
            r#"
H 0
S 0
"#,
        )),
        "C_ZYNX" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
S 0
"#,
        )),
        "C_ZNYX" => Some(GateDecomposition::new(
            r#"
H 0
S 0
S 0
S 0
"#,
        )),
        "C_NZYX" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
S 0
S 0
S 0
"#,
        )),
        "SQRT_X" => Some(GateDecomposition::new(
            r#"
H 0
S 0
H 0
"#,
        )),
        "SQRT_X_DAG" => Some(GateDecomposition::new(
            r#"
S 0
H 0
S 0
"#,
        )),
        "SQRT_Y" => Some(GateDecomposition::new(
            r#"
S 0
S 0
H 0
"#,
        )),
        "SQRT_Y_DAG" => Some(GateDecomposition::new(
            r#"
H 0
S 0
S 0
"#,
        )),
        "S" => Some(GateDecomposition::new(
            r#"
S 0
"#,
        )),
        "S_DAG" => Some(GateDecomposition::new(
            r#"
S 0
S 0
S 0
"#,
        )),
        "II" => Some(GateDecomposition::new(
            r#"
"#,
        )),
        "SQRT_XX" => Some(GateDecomposition::new(
            r#"
H 0
CNOT 0 1
H 1
S 0
S 1
H 0
H 1
"#,
        )),
        "SQRT_XX_DAG" => Some(GateDecomposition::new(
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
        )),
        "SQRT_YY" => Some(GateDecomposition::new(
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
        )),
        "SQRT_YY_DAG" => Some(GateDecomposition::new(
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
        )),
        "SQRT_ZZ" => Some(GateDecomposition::new(
            r#"
H 1
CNOT 0 1
H 1
S 0
S 1
"#,
        )),
        "SQRT_ZZ_DAG" => Some(GateDecomposition::new(
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
        )),
        "SWAP" => Some(GateDecomposition::new(
            r#"
CNOT 0 1
CNOT 1 0
CNOT 0 1
"#,
        )),
        "ISWAP" => Some(GateDecomposition::new(
            r#"
H 0
CNOT 0 1
CNOT 1 0
H 1
S 1
S 0
"#,
        )),
        "ISWAP_DAG" => Some(GateDecomposition::new(
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
        )),
        "CXSWAP" => Some(GateDecomposition::new(
            r#"
CNOT 1 0
CNOT 0 1
"#,
        )),
        "SWAPCX" => Some(GateDecomposition::new(
            r#"
CNOT 0 1
CNOT 1 0
"#,
        )),
        "CZSWAP" => Some(GateDecomposition::new(
            r#"
H 0
CX 0 1
CX 1 0
H 1
"#,
        )),
        _ => None,
    }
}

pub(crate) fn gate_has_decomposition(gate_name: &str) -> bool {
    gate_decomposition(gate_name).is_some()
}
