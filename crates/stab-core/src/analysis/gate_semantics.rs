use crate::{Gate, SingleQubitClifford, StabilizerError, StabilizerResult};

/// Resolves a closed-dialect gate into its single-qubit Clifford algebra value.
///
/// This semantic adapter owns the dependency between the Stim gate model and the algebra layer.
/// Algebra values remain usable without importing or constructing a [`Gate`].
pub fn single_qubit_clifford_for_gate(gate: Gate) -> StabilizerResult<SingleQubitClifford> {
    let clifford = match gate.canonical_name() {
        "I" => SingleQubitClifford::I,
        "X" => SingleQubitClifford::X,
        "Y" => SingleQubitClifford::Y,
        "Z" => SingleQubitClifford::Z,
        "H" => SingleQubitClifford::H,
        "SQRT_Y_DAG" => SingleQubitClifford::SqrtYDag,
        "H_NXZ" => SingleQubitClifford::Hnxz,
        "SQRT_Y" => SingleQubitClifford::SqrtY,
        "S" => SingleQubitClifford::S,
        "H_XY" => SingleQubitClifford::Hxy,
        "H_NXY" => SingleQubitClifford::Hnxy,
        "S_DAG" => SingleQubitClifford::SDag,
        "SQRT_X_DAG" => SingleQubitClifford::SqrtXDag,
        "SQRT_X" => SingleQubitClifford::SqrtX,
        "H_NYZ" => SingleQubitClifford::Hnyz,
        "H_YZ" => SingleQubitClifford::Hyz,
        "C_XYZ" => SingleQubitClifford::Cxyz,
        "C_XYNZ" => SingleQubitClifford::Cxynz,
        "C_NXYZ" => SingleQubitClifford::Cnxyz,
        "C_XNYZ" => SingleQubitClifford::Cxnyz,
        "C_ZYX" => SingleQubitClifford::Czyx,
        "C_ZNYX" => SingleQubitClifford::Cznyx,
        "C_NZYX" => SingleQubitClifford::Cnzyx,
        "C_ZYNX" => SingleQubitClifford::Czynx,
        _ => {
            return Err(StabilizerError::InvalidSingleQubitCliffordGate {
                gate: gate.canonical_name().to_owned(),
            });
        }
    };
    Ok(clifford)
}
