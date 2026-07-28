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

pub(crate) fn gate_decomposition_text(gate_name: &str) -> Option<&'static str> {
    match gate_name {
        "MX" => Some(
            r#"
H 0
M 0
H 0
"#,
        ),
        "MY" => Some(
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
        "M" => Some(
            r#"
M 0
"#,
        ),
        "MRX" => Some(
            r#"
H 0
M 0
R 0
H 0
"#,
        ),
        "MRY" => Some(
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
        "MR" => Some(
            r#"
M 0
R 0
"#,
        ),
        "RX" => Some(
            r#"
R 0
H 0
"#,
        ),
        "RY" => Some(
            r#"
R 0
H 0
S 0
"#,
        ),
        "R" => Some(
            r#"
R 0
"#,
        ),
        "XCX" => Some(
            r#"
H 0
CNOT 0 1
H 0
"#,
        ),
        "XCY" => Some(
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
        "XCZ" => Some(
            r#"
CNOT 1 0
"#,
        ),
        "YCX" => Some(
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
        "YCY" => Some(
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
        "YCZ" => Some(
            r#"
S 0
S 0
S 0
CNOT 1 0
S 0
"#,
        ),
        "CX" => Some(
            r#"
CNOT 0 1
"#,
        ),
        "CY" => Some(
            r#"
S 1
S 1
S 1
CNOT 0 1
S 1
"#,
        ),
        "CZ" => Some(
            r#"
H 1
CNOT 0 1
H 1
"#,
        ),
        "H" => Some(
            r#"
H 0
"#,
        ),
        "H_XY" => Some(
            r#"
H 0
S 0
S 0
H 0
S 0
"#,
        ),
        "H_YZ" => Some(
            r#"
H 0
S 0
H 0
S 0
S 0
"#,
        ),
        "H_NXY" => Some(
            r#"
S 0
H 0
S 0
S 0
H 0
"#,
        ),
        "H_NXZ" => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
"#,
        ),
        "H_NYZ" => Some(
            r#"
S 0
S 0
H 0
S 0
H 0
"#,
        ),
        "MXX" => Some(
            r#"
CX 0 1
H 0
M 0
H 0
CX 0 1
"#,
        ),
        "MYY" => Some(
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
        "MZZ" => Some(
            r#"
CX 0 1
M 1
CX 0 1
"#,
        ),
        "I" => Some(
            r#"
# (no operations)
"#,
        ),
        "X" => Some(
            r#"
H 0
S 0
S 0
H 0
"#,
        ),
        "Y" => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
H 0
"#,
        ),
        "Z" => Some(
            r#"
S 0
S 0
"#,
        ),
        "MPP" => Some(
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
        "SPP" => Some(
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
        "SPP_DAG" => Some(
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
        "C_XYZ" => Some(
            r#"
S 0
S 0
S 0
H 0
"#,
        ),
        "C_NXYZ" => Some(
            r#"
S 0
S 0
S 0
H 0
S 0
S 0
"#,
        ),
        "C_XNYZ" => Some(
            r#"
S 0
H 0
"#,
        ),
        "C_XYNZ" => Some(
            r#"
S 0
H 0
S 0
S 0
"#,
        ),
        "C_ZYX" => Some(
            r#"
H 0
S 0
"#,
        ),
        "C_ZYNX" => Some(
            r#"
S 0
S 0
H 0
S 0
"#,
        ),
        "C_ZNYX" => Some(
            r#"
H 0
S 0
S 0
S 0
"#,
        ),
        "C_NZYX" => Some(
            r#"
S 0
S 0
H 0
S 0
S 0
S 0
"#,
        ),
        "SQRT_X" => Some(
            r#"
H 0
S 0
H 0
"#,
        ),
        "SQRT_X_DAG" => Some(
            r#"
S 0
H 0
S 0
"#,
        ),
        "SQRT_Y" => Some(
            r#"
S 0
S 0
H 0
"#,
        ),
        "SQRT_Y_DAG" => Some(
            r#"
H 0
S 0
S 0
"#,
        ),
        "S" => Some(
            r#"
S 0
"#,
        ),
        "S_DAG" => Some(
            r#"
S 0
S 0
S 0
"#,
        ),
        "II" => Some(
            r#"
"#,
        ),
        "SQRT_XX" => Some(
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
        "SQRT_XX_DAG" => Some(
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
        "SQRT_YY" => Some(
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
        "SQRT_YY_DAG" => Some(
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
        "SQRT_ZZ" => Some(
            r#"
H 1
CNOT 0 1
H 1
S 0
S 1
"#,
        ),
        "SQRT_ZZ_DAG" => Some(
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
        "SWAP" => Some(
            r#"
CNOT 0 1
CNOT 1 0
CNOT 0 1
"#,
        ),
        "ISWAP" => Some(
            r#"
H 0
CNOT 0 1
CNOT 1 0
H 1
S 1
S 0
"#,
        ),
        "ISWAP_DAG" => Some(
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
        "CXSWAP" => Some(
            r#"
CNOT 1 0
CNOT 0 1
"#,
        ),
        "SWAPCX" => Some(
            r#"
CNOT 0 1
CNOT 1 0
"#,
        ),
        "CZSWAP" => Some(
            r#"
H 0
CX 0 1
CX 1 0
H 1
"#,
        ),
        _ => None,
    }
}
