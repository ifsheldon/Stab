use super::GateKind;

pub(crate) fn gate_flow_descriptors(kind: GateKind) -> Option<&'static [&'static str]> {
    Some(match kind {
        GateKind::M => &["Z -> rec[-1]", "Z -> Z"][..],
        GateKind::MX => &["X -> rec[-1]", "X -> X"],
        GateKind::MY => &["Y -> rec[-1]", "Y -> Y"],
        GateKind::R => &["1 -> Z"],
        GateKind::RX => &["1 -> X"],
        GateKind::RY => &["1 -> Y"],
        GateKind::MR => &["Z -> rec[-1]", "1 -> Z"],
        GateKind::MRX => &["X -> rec[-1]", "1 -> X"],
        GateKind::MRY => &["Y -> rec[-1]", "1 -> Y"],
        GateKind::MXX => &["X_ -> X_", "_X -> _X", "ZZ -> ZZ", "XX -> rec[-1]"],
        GateKind::MYY => &["XX -> XX", "Y_ -> Y_", "_Y -> _Y", "YY -> rec[-1]"],
        GateKind::MZZ => &["XX -> XX", "Z_ -> Z_", "_Z -> _Z", "ZZ -> rec[-1]"],
        GateKind::MPP => &[
            "XYZ__ -> rec[-2]",
            "___XX -> rec[-1]",
            "X____ -> X____",
            "_Y___ -> _Y___",
            "__Z__ -> __Z__",
            "___X_ -> ___X_",
            "____X -> ____X",
            "ZZ___ -> ZZ___",
            "_XX__ -> _XX__",
            "___ZZ -> ___ZZ",
        ],
        GateKind::SPP => &[
            "X__ -> X__",
            "Z__ -> -YYZ",
            "_X_ -> -XZZ",
            "_Z_ -> XXZ",
            "__X -> XYY",
            "__Z -> __Z",
        ],
        GateKind::SPP_DAG => &[
            "X__ -> X__",
            "Z__ -> YYZ",
            "_X_ -> XZZ",
            "_Z_ -> -XXZ",
            "__X -> -XYY",
            "__Z -> __Z",
        ],
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
