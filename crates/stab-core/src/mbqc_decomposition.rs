use crate::{Circuit, CircuitResult, Gate};

pub fn mbqc_decomposition(gate: Gate) -> CircuitResult<Option<Circuit>> {
    mbqc_decomposition_text(gate)
        .map(Circuit::from_stim_str)
        .transpose()
}

fn mbqc_decomposition_text(gate: Gate) -> Option<&'static str> {
    match gate.canonical_name() {
        "DETECTOR"
        | "OBSERVABLE_INCLUDE"
        | "TICK"
        | "QUBIT_COORDS"
        | "SHIFT_COORDS"
        | "REPEAT"
        | "MPAD"
        | "DEPOLARIZE1"
        | "DEPOLARIZE2"
        | "X_ERROR"
        | "Y_ERROR"
        | "Z_ERROR"
        | "I_ERROR"
        | "II_ERROR"
        | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2"
        | "E"
        | "ELSE_CORRELATED_ERROR"
        | "HERALDED_ERASE"
        | "HERALDED_PAULI_CHANNEL_1" => None,
        "MX" => Some(
            "
            MX 0
        ",
        ),
        "MY" => Some(
            "
            MY 0
        ",
        ),
        "M" => Some(
            "
            MZ 0
        ",
        ),
        "MRX" | "RX" => Some(
            "
            MX 0
            CZ rec[-1] 0
        ",
        ),
        "MRY" | "RY" => Some(
            "
            MY 0
            CX rec[-1] 0
        ",
        ),
        "MR" | "R" => Some(
            "
            MZ 0
            CX rec[-1] 0
        ",
        ),
        "H_XY" => Some(
            "
            MX 1
            MZZ 0 1
            MY 1
            X 0
            CZ rec[-3] 0 rec[-2] 0 rec[-1] 0
        ",
        ),
        "S" => Some(
            "
            MY 1
            MZZ 0 1
            MX 1
            CZ rec[-3] 0 rec[-2] 0 rec[-1] 0
        ",
        ),
        "I" | "II" => Some(""),
        "X" => Some(
            "
            X 0
        ",
        ),
        "Y" => Some(
            "
            Y 0
        ",
        ),
        "Z" => Some(
            "
            Z 0
        ",
        ),
        "CX" => Some(
            "
            MX 2
            MZZ 0 2
            MXX 1 2
            MZ 2
            CX rec[-3] 1 rec[-1] 1
            CZ rec[-4] 0 rec[-2] 0
        ",
        ),
        _ => None,
    }
}
