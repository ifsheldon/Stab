#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "M4 compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;

use stab_core::Gate;

const STIM_V116_GATE_HASHES: [(&str, usize); 82] = [
    ("NOT_A_GATE", 143),
    ("DETECTOR", 381),
    ("OBSERVABLE_INCLUDE", 157),
    ("TICK", 464),
    ("QUBIT_COORDS", 439),
    ("SHIFT_COORDS", 14),
    ("REPEAT", 21),
    ("MPAD", 192),
    ("MX", 476),
    ("MY", 119),
    ("M", 310),
    ("MRX", 115),
    ("MRY", 360),
    ("MR", 58),
    ("RX", 358),
    ("RY", 1),
    ("R", 451),
    ("XCX", 94),
    ("XCY", 197),
    ("XCZ", 440),
    ("YCX", 448),
    ("YCY", 183),
    ("YCZ", 386),
    ("CX", 208),
    ("CY", 363),
    ("CZ", 6),
    ("H", 169),
    ("H_XY", 350),
    ("H_YZ", 333),
    ("H_NXY", 461),
    ("H_NXZ", 454),
    ("H_NYZ", 114),
    ("DEPOLARIZE1", 216),
    ("DEPOLARIZE2", 63),
    ("X_ERROR", 209),
    ("Y_ERROR", 239),
    ("Z_ERROR", 269),
    ("I_ERROR", 79),
    ("II_ERROR", 215),
    ("PAULI_CHANNEL_1", 288),
    ("PAULI_CHANNEL_2", 443),
    ("E", 494),
    ("ELSE_CORRELATED_ERROR", 364),
    ("HERALDED_ERASE", 314),
    ("HERALDED_PAULI_CHANNEL_1", 388),
    ("I", 402),
    ("X", 313),
    ("Y", 34),
    ("Z", 267),
    ("C_XYZ", 48),
    ("C_ZYX", 502),
    ("C_NXYZ", 245),
    ("C_XNYZ", 161),
    ("C_XYNZ", 253),
    ("C_NZYX", 297),
    ("C_ZNYX", 49),
    ("C_ZYNX", 237),
    ("SQRT_X", 319),
    ("SQRT_X_DAG", 342),
    ("SQRT_Y", 445),
    ("SQRT_Y_DAG", 143),
    ("S", 172),
    ("S_DAG", 121),
    ("II", 399),
    ("SQRT_XX", 318),
    ("SQRT_XX_DAG", 341),
    ("SQRT_YY", 394),
    ("SQRT_YY_DAG", 140),
    ("SQRT_ZZ", 106),
    ("SQRT_ZZ_DAG", 231),
    ("MPP", 501),
    ("SPP", 425),
    ("SPP_DAG", 223),
    ("SWAP", 81),
    ("ISWAP", 24),
    ("CXSWAP", 390),
    ("SWAPCX", 47),
    ("CZSWAP", 76),
    ("ISWAP_DAG", 332),
    ("MXX", 5),
    ("MYY", 409),
    ("MZZ", 281),
];

#[test]
fn gate_name_hash_matches_exact_stim_v116_gate_table() {
    let actual_names = std::iter::once("NOT_A_GATE")
        .chain(Gate::all().map(Gate::canonical_name))
        .collect::<Vec<_>>();
    let expected_names = STIM_V116_GATE_HASHES
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    assert_eq!(actual_names, expected_names);

    for (name, expected_hash) in STIM_V116_GATE_HASHES {
        assert_eq!(Gate::stim_name_hash(name), expected_hash, "{name}");
    }

    let hashes = Gate::all()
        .map(|gate| Gate::stim_name_hash(gate.canonical_name()))
        .collect::<BTreeSet<_>>();
    assert_eq!(hashes.len(), Gate::all().count());
}
