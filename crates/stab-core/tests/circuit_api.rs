#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "PF1 circuit API compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeMap;

use stab_core::{Circuit, CircuitDetectorId, QubitId};

#[test]
fn pf1_circuit_stats_counts_match_owned_upstream_semantics() {
    let circuit = Circuit::from_stim_str(
        "M 0 1\n\
         REPEAT 100 {\n\
             TICK\n\
             M 2\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(2) rec[-1]\n\
             CY sweep[77] 3\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(circuit.len(), 2);
    assert!(!circuit.is_empty());
    assert_eq!(circuit.count_measurements().expect("measurements"), 102);
    assert_eq!(circuit.count_detectors().expect("detectors"), 100);
    assert_eq!(circuit.count_observables().expect("observables"), 3);
    assert_eq!(circuit.count_ticks().expect("ticks"), 100);
    assert_eq!(circuit.count_sweep_bits().expect("sweep bits"), 78);
}

#[test]
fn pf1_circuit_stats_measurement_counts_use_result_groups() {
    let circuit = Circuit::from_stim_str(
        "MPP X0*X1 Y2*Y3 Z4\n\
         MXX 5 6 7 8\n\
         HERALDED_ERASE(0.25) 9 10\n\
         MPAD 0 1 0\n",
    )
    .expect("parse circuit");

    assert_eq!(circuit.count_measurements().expect("measurements"), 10);
}

#[test]
fn pf1_circuit_stats_counts_do_not_unroll_large_repeats() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000000 {\n\
             REPEAT 1000000 {\n\
                 M 0\n\
                 DETECTOR rec[-1]\n\
                 OBSERVABLE_INCLUDE(4) rec[-1]\n\
             }\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit.count_measurements().expect("measurements"),
        1_000_000_000_000
    );
    assert_eq!(
        circuit.count_detectors().expect("detectors"),
        1_000_000_000_000
    );
    assert_eq!(circuit.count_observables().expect("observables"), 5);
}

#[test]
fn pf1_circuit_stats_counts_reject_folded_overflow() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 18446744073709551615 {\n\
             M 0 1\n\
         }\n",
    )
    .expect("parse circuit");

    let error = circuit
        .count_measurements()
        .expect_err("reject count overflow");

    assert!(
        error.to_string().contains("circuit count overflowed"),
        "{error}"
    );
}

#[test]
fn pf1_circuit_stats_final_coordinate_shift_matches_nested_upstream_case() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000 {\n\
             REPEAT 2000 {\n\
                 REPEAT 3000 {\n\
                     SHIFT_COORDS(0, 0, 1)\n\
                 }\n\
                 SHIFT_COORDS(1)\n\
             }\n\
             SHIFT_COORDS(0, 1)\n\
         }\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .final_coordinate_shift()
            .expect("final coordinate shift"),
        vec![2_000_000.0, 1000.0, 6_000_000_000.0]
    );
}

#[test]
fn pf1_circuit_stats_final_qubit_coordinates_apply_shifts_and_repeats() {
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS(1, 2, 3) 0\n\
         QUBIT_COORDS(2) 1\n\
         SHIFT_COORDS(5)\n\
         QUBIT_COORDS(3) 4\n\
         REPEAT 3 {\n\
             SHIFT_COORDS(10, 1)\n\
             QUBIT_COORDS(7) 1\n\
         }\n\
         QUBIT_COORDS(0, 0) 2\n",
    )
    .expect("parse circuit");

    let expected = BTreeMap::from([
        (QubitId::new(0).unwrap(), vec![1.0, 2.0, 3.0]),
        (QubitId::new(1).unwrap(), vec![42.0, 3.0]),
        (QubitId::new(2).unwrap(), vec![35.0, 3.0]),
        (QubitId::new(4).unwrap(), vec![8.0]),
    ]);

    assert_eq!(
        circuit
            .final_qubit_coordinates()
            .expect("final qubit coordinates"),
        expected
    );
}

#[test]
fn pf1_circuit_stats_clear_resets_items_and_counts() {
    let mut circuit = Circuit::from_stim_str("H 0\nM 0\nDETECTOR rec[-1]\n").expect("parse");
    circuit.clear();

    assert!(circuit.is_empty());
    assert_eq!(circuit.len(), 0);
    assert_eq!(circuit.to_stim_string(), "");
    assert_eq!(circuit.count_measurements().expect("measurements"), 0);
    assert_eq!(circuit.count_detectors().expect("detectors"), 0);
}

#[test]
fn pf1_circuit_append_text_preserves_tags_repeats_and_fuses() {
    let mut circuit = Circuit::from_stim_str("H[tag] 0\n").expect("parse base");

    circuit
        .append_from_stim_text(
            "H[tag] 1\n\
             REPEAT[loop] 2 {\n\
                 M[meas] 0\n\
                 DETECTOR[det] rec[-1]\n\
             }\n",
        )
        .expect("append text");

    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "H[tag] 0 1\n",
            "REPEAT[loop] 2 {\n",
            "    M[meas] 0\n",
            "    DETECTOR[det] rec[-1]\n",
            "}\n",
        )
    );
    assert_eq!(circuit.len(), 2);
    assert_eq!(circuit.count_measurements().expect("measurements"), 2);
    assert_eq!(circuit.count_detectors().expect("detectors"), 2);
}

#[test]
fn pf1_circuit_append_text_program_alias_appends() {
    let mut circuit = Circuit::new();

    circuit
        .append_from_stim_program_text(
            "H[test] 0\n\
             CX[test2] 1 2\n",
        )
        .expect("append text");

    assert_eq!(
        circuit.to_stim_string(),
        "H[test] 0\n\
         CX[test2] 1 2\n"
    );
}

#[test]
fn pf1_circuit_append_text_is_atomic_on_parse_error() {
    let mut circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse base");
    let before = circuit.clone();

    let error = circuit
        .append_from_stim_text(
            "H 1\n\
             NOT_A_GATE 2\n",
        )
        .expect_err("reject invalid append text");

    assert!(error.to_string().contains("unknown gate"), "{error}");
    assert_eq!(circuit, before);
}

#[test]
fn pf1_circuit_concat_append_circuit_and_concatenated_fuse_boundary() {
    let mut circuit = Circuit::from_stim_str("H[tag] 0\n").expect("parse base");
    let rhs = Circuit::from_stim_str("H[tag] 1\nM 0\n").expect("parse rhs");

    let concatenated = circuit.concatenated(&rhs);
    circuit.append_circuit(&rhs);

    let expected = "H[tag] 0 1\nM 0\n";
    assert_eq!(circuit.to_stim_string(), expected);
    assert_eq!(concatenated.to_stim_string(), expected);
    assert_eq!(rhs.to_stim_string(), "H[tag] 1\nM 0\n");
}

#[test]
fn pf1_circuit_repeat_matches_upstream_special_cases() {
    let circuit = Circuit::from_stim_str("Y 3\nM 4\n").expect("parse circuit");

    assert_eq!(
        circuit.repeated(0).expect("repeat zero").to_stim_string(),
        ""
    );
    assert_eq!(
        circuit.repeated(1).expect("repeat one").to_stim_string(),
        "Y 3\nM 4\n"
    );
    assert_eq!(
        circuit.repeated(2).expect("repeat two").to_stim_string(),
        concat!("REPEAT 2 {\n", "    Y 3\n", "    M 4\n", "}\n")
    );

    let mut in_place = circuit.clone();
    in_place.repeat_in_place(3).expect("repeat in place");
    assert_eq!(
        in_place.to_stim_string(),
        concat!("REPEAT 3 {\n", "    Y 3\n", "    M 4\n", "}\n")
    );
}

#[test]
fn pf1_circuit_repeat_fuses_single_repeat_block_counts() {
    let circuit =
        Circuit::from_stim_str("REPEAT[tag] 2 {\n    H[tag2] 0\n}\n").expect("parse circuit");

    assert_eq!(
        circuit.repeated(3).expect("repeat nested").to_stim_string(),
        concat!("REPEAT 6 {\n", "    H[tag2] 0\n", "}\n")
    );
}

#[test]
fn pf1_circuit_repeat_rejects_fused_repeat_count_overflow() {
    let circuit =
        Circuit::from_stim_str("REPEAT 1234567890123456789 {\n    H 0\n}\n").expect("parse");

    let error = circuit
        .repeated(16)
        .expect_err("reject repeat count overflow");

    assert_eq!(
        error,
        stab_core::CircuitError::InvalidDomainValue {
            kind: "repetition count",
            value: "overflowed".to_string()
        }
    );
}

#[test]
fn pf1_circuit_stats_coordinate_queries_reject_non_finite_folded_shift() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000000000000 {\n\
             SHIFT_COORDS(1e308)\n\
         }\n",
    )
    .expect("parse circuit");

    let error = circuit
        .final_coordinate_shift()
        .expect_err("reject infinite coordinate shift");

    assert!(
        error.to_string().contains("coordinate shift overflowed"),
        "{error}"
    );
}

#[test]
fn pf1_circuit_detector_coords_include_empty_and_shifted_coordinates() {
    let circuit = Circuit::from_stim_str(
        "M 0\n\
         DETECTOR rec[-1]\n\
         DETECTOR(1, 2, 3) rec[-1]\n\
         REPEAT 3 {\n\
             DETECTOR(42) rec[-1]\n\
             SHIFT_COORDS(100)\n\
         }\n",
    )
    .expect("parse circuit");

    let expected = BTreeMap::from([
        (detector(0), vec![]),
        (detector(1), vec![1.0, 2.0, 3.0]),
        (detector(2), vec![42.0]),
        (detector(3), vec![142.0]),
        (detector(4), vec![242.0]),
    ]);

    assert_eq!(
        circuit.detector_coordinates().expect("all coordinates"),
        expected
    );
    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector zero"),
        vec![]
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([detector(1), detector(3)])
            .expect("selected coordinates"),
        BTreeMap::from([
            (detector(1), vec![1.0, 2.0, 3.0]),
            (detector(3), vec![142.0])
        ])
    );
}

#[test]
fn pf1_circuit_detector_coords_fold_nested_repeat_queries() {
    let circuit = Circuit::from_stim_str(
        "TICK\n\
         REPEAT 1000 {\n\
             REPEAT 2000 {\n\
                 REPEAT 1000 {\n\
                     DETECTOR(0, 0, 0, 4)\n\
                     SHIFT_COORDS(1, 0, 0)\n\
                 }\n\
                 DETECTOR(0, 0, 0, 3)\n\
                 SHIFT_COORDS(0, 1, 0)\n\
             }\n\
             DETECTOR(0, 0, 0, 2)\n\
             SHIFT_COORDS(0, 0, 1)\n\
         }\n\
         DETECTOR(0, 0, 0, 1)\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector 0"),
        vec![0.0, 0.0, 0.0, 4.0]
    );
    assert_eq!(
        circuit
            .coordinates_of_detector(detector(1002))
            .expect("detector 1002"),
        vec![1001.0, 1.0, 0.0, 4.0]
    );
    assert_eq!(
        circuit
            .detector_coordinates_for([
                detector(0),
                detector(1),
                detector(999),
                detector(1000),
                detector(1001),
                detector(1002),
            ])
            .expect("selected coordinates"),
        BTreeMap::from([
            (detector(0), vec![0.0, 0.0, 0.0, 4.0]),
            (detector(1), vec![1.0, 0.0, 0.0, 4.0]),
            (detector(999), vec![999.0, 0.0, 0.0, 4.0]),
            (detector(1000), vec![1000.0, 0.0, 0.0, 3.0]),
            (detector(1001), vec![1000.0, 1.0, 0.0, 4.0]),
            (detector(1002), vec![1001.0, 1.0, 0.0, 4.0]),
        ])
    );
}

#[test]
fn pf1_circuit_detector_coords_skip_detector_free_repeat_shift() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000 {\n\
             SHIFT_COORDS(1)\n\
         }\n\
         DETECTOR(5)\n",
    )
    .expect("parse circuit");

    assert_eq!(
        circuit
            .coordinates_of_detector(detector(0))
            .expect("detector after shift-only repeat"),
        vec![1005.0]
    );
}

#[test]
fn pf1_circuit_detector_coords_reject_missing_detector_id() {
    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").expect("parse");

    let error = circuit
        .coordinates_of_detector(detector(1))
        .expect_err("reject missing detector");

    assert!(error.to_string().contains("Detector index 1 is too big"));
}

fn detector(id: u64) -> CircuitDetectorId {
    CircuitDetectorId::new(id)
}
