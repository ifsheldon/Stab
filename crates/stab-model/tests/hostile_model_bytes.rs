#![allow(
    clippy::panic,
    reason = "hostile-input properties report the exact accepted bytes when a printed form fails"
)]

//! Hostile-byte properties for the `.stim` and `.dem` parsers.
//!
//! Arbitrary bytes, grammar-token soup, and byte-level mutations of valid models must produce
//! either a model or a typed `ModelError`, never a panic. Anything accepted must also print to
//! canonical text that reparses, and folded count queries must stay checked.

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_model::{Circuit, DetectorErrorModel};

const CIRCUIT_TOKENS: &[&str] = &[
    "H",
    "CX",
    "cnot",
    "MPP",
    "M",
    "MR",
    "MXX",
    "MPAD",
    "R",
    "TICK",
    "DETECTOR",
    "OBSERVABLE_INCLUDE",
    "QUBIT_COORDS",
    "SHIFT_COORDS",
    "X_ERROR",
    "DEPOLARIZE2",
    "PAULI_CHANNEL_1",
    "PAULI_CHANNEL_2",
    "E",
    "ELSE_CORRELATED_ERROR",
    "HERALDED_PAULI_CHANNEL_1",
    "SPP",
    "SPP_DAG",
    "REPEAT",
    "repeat",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "\\",
    "\\C",
    "\\B",
    "\\n",
    "\\x",
    "!",
    "*",
    "rec[-",
    "rec[-0]",
    "rec[-16777216]",
    "sweep[",
    "sweep[16777215]",
    "X",
    "Y",
    "Z",
    "x1",
    "!Z2",
    "#",
    "\n",
    "\r\n",
    "\r",
    " ",
    "\t",
    ",",
    "0",
    "1",
    "16777215",
    "16777216",
    "-1",
    "-0",
    "0.5",
    "1e308",
    "1e999",
    "1e-400",
    "nan",
    "inf",
    "-inf",
    "0x10",
    "9223372036854775807",
    "18446744073709551616",
    "\u{fffd}",
    "\u{0}",
    "\u{ff}",
];

const DEM_TOKENS: &[&str] = &[
    "error",
    "ERROR",
    "detector",
    "logical_observable",
    "shift_detectors",
    "repeat",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "\\",
    "\\C",
    "\\B",
    "\\r",
    "\\q",
    "^",
    "D",
    "L",
    "D0",
    "L0",
    "D1152921504606846975",
    "D1152921504606846976",
    "L4294967295",
    "L4294967296",
    "#",
    "\n",
    "\r\n",
    "\r",
    " ",
    "\t",
    ",",
    "0",
    "1",
    "-1",
    "-0",
    "0.25",
    "1.5",
    "1e308",
    "1e999",
    "1e-400",
    "nan",
    "inf",
    "1152921504606846975",
    "18446744073709551615",
    "18446744073709551616",
    "\u{fffd}",
    "\u{0}",
];

const CIRCUIT_SEEDS: &[&[u8]] = &[
    b"H 0\nCX 0 1\nM(0.125) !0 1\nDETECTOR(1, -0) rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    b"REPEAT[loop\\C] 1000000000000 {\n    MPP !X0*Y1 Z2\n    CX rec[-1] 3 sweep[2] 4\n}\n",
    b"PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\nE[t](0.5) X0 Y1\nELSE_CORRELATED_ERROR(0.25) Z2\nMPAD 0 1\n",
    b"QUBIT_COORDS(1.5, 2) 0\nSHIFT_COORDS(0, 0, 1)\nTICK\nMXX !0 1\nHERALDED_ERASE(0.01) 3\n",
];

const DEM_SEEDS: &[&[u8]] = &[
    b"error(0.125) D0 D1 ^ L0\ndetector(1, -0) D2\nlogical_observable L3\n",
    b"repeat[loop\\C] 1152921504606846975 {\n    error[x](0.25) D0 L1 ^ D2\n    shift_detectors(1, 2) 1152921504606846975\n}\n",
    b"shift_detectors 5\nrepeat 3 {\n    repeat 2 {\n        detector D0\n    }\n}\n",
];

#[derive(Clone, Debug)]
enum Edit {
    Insert(usize, u8),
    Delete(usize),
    Replace(usize, u8),
    Truncate(usize),
}

fn token_soup(tokens: &'static [&'static str]) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop::sample::select(tokens), 0..48)
        .prop_map(|tokens| tokens.concat().into_bytes())
}

fn mutated(seeds: &'static [&'static [u8]]) -> impl Strategy<Value = Vec<u8>> {
    let edit = prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(at, byte)| Edit::Insert(at, byte)),
        any::<usize>().prop_map(Edit::Delete),
        (any::<usize>(), any::<u8>()).prop_map(|(at, byte)| Edit::Replace(at, byte)),
        any::<usize>().prop_map(Edit::Truncate),
    ];
    (
        prop::sample::select(seeds),
        prop::collection::vec(edit, 1..6),
    )
        .prop_map(|(seed, edits)| {
            let mut bytes = seed.to_vec();
            for edit in edits {
                let len = bytes.len();
                match edit {
                    Edit::Insert(at, byte) => bytes.insert(at % (len + 1), byte),
                    Edit::Delete(at) if len > 0 => {
                        bytes.remove(at % len);
                    }
                    Edit::Replace(at, byte) if len > 0 => {
                        if let Some(slot) = bytes.get_mut(at % len) {
                            *slot = byte;
                        }
                    }
                    Edit::Truncate(at) => bytes.truncate(at % (len + 1)),
                    Edit::Delete(_) | Edit::Replace(..) => {}
                }
            }
            bytes
        })
}

fn hostile(
    tokens: &'static [&'static str],
    seeds: &'static [&'static [u8]],
) -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..256),
        token_soup(tokens),
        mutated(seeds),
    ]
}

fn config(seed: u64) -> Config {
    Config {
        cases: 2048,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    }
}

fn check_circuit(input: &[u8]) {
    let parsed = Circuit::from_stim_bytes(input);
    if let Ok(text) = std::str::from_utf8(input) {
        let from_str = Circuit::from_stim_str(text);
        assert_eq!(
            from_str.is_ok(),
            parsed.is_ok(),
            "str and byte circuit parsers disagree on acceptance"
        );
    }
    let Ok(circuit) = parsed else {
        return;
    };
    let printed = circuit.to_stim_bytes();
    if let Err(error) = Circuit::from_stim_bytes(&printed) {
        panic!(
            "accepted circuit printed unparseable text: {error}\ninput: {:?}\nprinted:\n{}",
            String::from_utf8_lossy(input),
            String::from_utf8_lossy(&printed)
        );
    }
    let _fingerprint = circuit.fingerprint();
    let _measurements = circuit.count_measurements();
    let _detectors = circuit.count_detectors();
    let _observables = circuit.count_observables();
    let _ticks = circuit.count_ticks();
    let _sweep_bits = circuit.count_sweep_bits();
    let _qubits = circuit.count_qubits();
}

fn check_dem(input: &[u8]) {
    let parsed = DetectorErrorModel::from_dem_bytes(input);
    if let Ok(text) = std::str::from_utf8(input) {
        let from_str = DetectorErrorModel::from_dem_str(text);
        assert_eq!(
            from_str.is_ok(),
            parsed.is_ok(),
            "str and byte DEM parsers disagree on acceptance"
        );
    }
    let Ok(model) = parsed else {
        return;
    };
    let printed = model.to_dem_bytes();
    match DetectorErrorModel::from_dem_bytes(&printed) {
        Ok(reparsed) => assert!(
            reparsed == model,
            "accepted DEM changed through canonical text: {:?}",
            String::from_utf8_lossy(input)
        ),
        Err(error) => panic!(
            "accepted DEM printed unparseable text: {error}\ninput: {:?}\nprinted:\n{}",
            String::from_utf8_lossy(input),
            String::from_utf8_lossy(&printed)
        ),
    }
    let _fingerprint = model.fingerprint();
    let _shift = model.total_detector_shift();
    let _detectors = model.count_detectors();
    let _observables = model.count_observables();
}

proptest! {
    #![proptest_config(config(0x4057_11e0_0001))]

    #[test]
    fn hostile_circuit_bytes_never_panic(input in hostile(CIRCUIT_TOKENS, CIRCUIT_SEEDS)) {
        check_circuit(&input);
    }

    #[test]
    fn hostile_dem_bytes_never_panic(input in hostile(DEM_TOKENS, DEM_SEEDS)) {
        check_dem(&input);
    }
}
