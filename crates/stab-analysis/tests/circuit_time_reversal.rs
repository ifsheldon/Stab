#![allow(
    clippy::expect_used,
    reason = "time-reversal parity fixtures use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;
use std::str::FromStr;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use stab_algebra::Flow;
use stab_analysis::{
    CodeDistance, DetectingRegionTargetOptions, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    TimeReversedForFlowsOptions, all_detecting_region_targets, all_detecting_region_ticks,
    circuit_detecting_regions_for_targets, circuit_flow_generators,
    circuit_has_all_unsigned_stabilizer_flows, circuit_time_reversed_for_flows,
    circuit_time_reversed_for_flows_with_options, generate_surface_code_circuit,
};
use stab_model::{Circuit, CircuitTick, DemTarget};

const GENERATED_REVERSAL_CASES: u32 = 48;
const GENERATED_REVERSAL_SEED: [u8; 32] = [0xB1; 32];

#[test]
fn time_reversal_preserves_generated_surface_code_regions() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        SurfaceCodeTask::RotatedMemoryX,
    )
    .expect("surface-code parameters");
    let generated = generate_surface_code_circuit(&params).expect("generate surface code");
    let input = generated.circuit();

    let targets = all_detecting_region_targets(input).expect("input targets");
    let ticks = all_detecting_region_ticks(input).expect("input ticks");
    let input_regions = circuit_detecting_regions_for_targets(
        input,
        DetectingRegionTargetOptions {
            targets: targets.clone(),
            ticks: ticks.clone(),
            ignore_anticommutation_errors: false,
        },
    )
    .expect("input detecting regions");

    let (inverse, flows) =
        circuit_time_reversed_for_flows(input, &[]).expect("reverse generated surface code");
    assert!(flows.is_empty());
    assert_eq!(inverse.count_ticks().expect("inverse ticks"), 14);
    assert_eq!(inverse.count_detectors().expect("inverse detectors"), 16);
    assert_eq!(
        inverse.count_measurements().expect("inverse measurements"),
        33
    );

    let inverse_regions = circuit_detecting_regions_for_targets(
        &inverse,
        DetectingRegionTargetOptions {
            targets,
            ticks: all_detecting_region_ticks(&inverse).expect("inverse ticks"),
            ignore_anticommutation_errors: false,
        },
    )
    .expect("inverse detecting regions");
    let tick_count = input.count_ticks().expect("input tick count");
    let original_reversed = input_regions
        .values()
        .map(|regions| {
            let mut signature = regions
                .iter()
                .map(|(tick, pauli)| (tick_count - tick.get() - 1, pauli.to_string()))
                .collect::<Vec<_>>();
            signature.sort_by_key(|(tick, _)| *tick);
            signature
        })
        .collect::<BTreeSet<_>>();
    let actual = inverse_regions
        .values()
        .map(|regions| {
            regions
                .iter()
                .map(|(tick, pauli)| (tick.get(), pauli.to_string()))
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(original_reversed.len(), input_regions.len());
    assert_eq!(actual.len(), inverse_regions.len());
    assert_eq!(actual, original_reversed);
    let logical = DemTarget::logical_observable(0).expect("logical observable");
    let inverse_logical = inverse_regions
        .get(&logical)
        .expect("inverse logical region");
    let expected_logical = input_regions
        .get(&logical)
        .expect("input logical region")
        .iter()
        .map(|(tick, pauli)| (CircuitTick::new(tick_count - tick.get() - 1), pauli.clone()))
        .collect();
    assert_eq!(inverse_logical, &expected_logical);
}

#[test]
fn time_reversal_rejects_measurement_feedback() {
    let input = circuit(
        "
        R 1
        M 1
        CX rec[-1] 0
        ",
    );

    let error = circuit_time_reversed_for_flows(&input, &[flow("Z0 -> Z0")])
        .expect_err("feedback remains fail-closed")
        .to_string();

    assert!(error.contains("feedback"), "{error}");
    assert!(error.contains("CX rec[-1] 0"), "{error}");
}

#[test]
fn time_reversal_preserves_observable_pauli_semantics() {
    let input = circuit(
        "
        RX 0
        OBSERVABLE_INCLUDE[test1](2) X0
        OBSERVABLE_INCLUDE[test2](3) Y1
        MY 1
        OBSERVABLE_INCLUDE(3) rec[-1]
        ",
    );
    let expected = circuit(
        "
        RY 1
        OBSERVABLE_INCLUDE[test2](3) Y1
        OBSERVABLE_INCLUDE[test1](2) X0
        MX 0
        OBSERVABLE_INCLUDE(2) rec[-1]
        ",
    );

    let (inverse, flows) =
        circuit_time_reversed_for_flows(&input, &[]).expect("reverse observable Pauli packet");

    assert_eq!(inverse, expected);
    assert!(flows.is_empty());
}

#[test]
fn time_reversal_routes_mpad_flows_and_observables() {
    let input = circuit(
        "
        H 0
        MPAD 0 1
        S 0
        OBSERVABLE_INCLUDE(0) rec[-2]
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    );
    let input_flows = ["1 -> rec[1]", "1 -> -rec[0]", "X -> Z", "Z -> Y"].map(flow);
    let expected = circuit(
        "
        S_DAG 0
        MPAD 1 0
        OBSERVABLE_INCLUDE(0) rec[-2] rec[-1]
        H 0
        ",
    );
    let expected_flows = ["1 -> rec[-2]", "1 -> rec[-1]", "Z -> X", "Y -> Z"].map(flow);

    let (inverse, output_flows) =
        circuit_time_reversed_for_flows(&input, &input_flows).expect("reverse MPAD flow matrix");

    assert_eq!(inverse, expected);
    assert_eq!(output_flows, expected_flows);
}

#[test]
fn time_reversal_bounds_measurement_rich_repeat_expansion() {
    let small = circuit("REPEAT 2 {\n    M 0\n}\n");
    let (inverse, flows) =
        circuit_time_reversed_for_flows(&small, &[]).expect("reverse bounded repeat");
    assert_eq!(inverse, circuit("M 0 0\n"));
    assert!(flows.is_empty());

    let oversized = circuit("REPEAT 1000001 {\n    M 0\n}\n");
    let error = circuit_time_reversed_for_flows(&oversized, &[])
        .expect_err("oversized measurement-rich repeat is rejected")
        .to_string();
    assert!(
        error.contains("1,000,000") || error.contains("1000000"),
        "{error}"
    );
    assert!(
        error.contains("repeat") || error.contains("expanded"),
        "{error}"
    );
}

#[test]
fn time_reversal_skips_instructionless_nested_repeats() {
    let input = circuit(
        "
        REPEAT 1000000000 {
            REPEAT 1 {
            }
        }
        M 0
        ",
    );

    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &[])
        .expect("instruction-empty nested repeat must be skipped");

    assert_eq!(inverse, circuit("M 0\n"));
    assert!(flows.is_empty());
}

#[test]
fn time_reversed_for_flows_matches_stim_heralded_record_reversal() {
    for circuit_text in [
        "HERALDED_ERASE(0.125) 0\n",
        "HERALDED_PAULI_CHANNEL_1(0.1, 0.2, 0.3, 0.4) 0\n",
    ] {
        let input = circuit(circuit_text);
        let (empty_inverse, empty_flows) = circuit_time_reversed_for_flows(&input, &[])
            .expect("reverse heralded instruction without a record-bearing flow");
        assert_eq!(empty_inverse, input, "{circuit_text}");
        assert!(empty_flows.is_empty(), "{circuit_text}");

        let (record_inverse, record_flows) =
            circuit_time_reversed_for_flows(&input, &[flow("1 -> rec[-1]")])
                .expect("reverse heralded instruction with a record-bearing flow");
        assert_eq!(record_inverse, input, "{circuit_text}");
        assert_eq!(record_flows, vec![flow("1 -> 1")], "{circuit_text}");
    }
}

#[test]
fn time_reversal_matches_sweep_control_target_order() {
    for text in [
        "CX sweep[0] 0\n",
        "CY sweep[0] 0\n",
        "CZ sweep[0] 0\n",
        "CZ 0 sweep[0]\n",
        "CZ sweep[0] sweep[1]\n",
        "XCZ 0 sweep[0]\n",
        "YCZ 0 sweep[0]\n",
    ] {
        let input = circuit(text);
        let (inverse, flows) =
            circuit_time_reversed_for_flows(&input, &[]).expect("reverse gate-valid sweep control");
        assert_eq!(inverse, input, "{text}");
        assert!(flows.is_empty(), "{text}");
    }

    for text in [
        "CX 0 sweep[0]\n",
        "CY 0 sweep[0]\n",
        "CX sweep[0] sweep[1]\n",
        "CY sweep[0] sweep[1]\n",
        "XCZ sweep[0] 0\n",
        "YCZ sweep[0] 0\n",
        "XCZ sweep[0] sweep[1]\n",
        "YCZ sweep[0] sweep[1]\n",
    ] {
        let error = circuit_time_reversed_for_flows(&circuit(text), &[])
            .expect_err("gate-invalid sweep target order is rejected")
            .to_string();
        assert!(error.contains("qubit-only side"), "{text}: {error}");
    }
}

#[test]
fn time_reversal_preserves_non_pair_target_order() {
    for (input_text, expected_text) in [
        ("MPP X0*Y1 Z2*X3\n", "MPP X3*Z2 Y1*X0\n"),
        ("SPP X0*Y1 Z2*X3\n", "SPP_DAG X3*Z2 Y1*X0\n"),
        ("E(0.125) X0 Y1 Z2\n", "E(0.125) Z2 Y1 X0\n"),
        ("MXX 0 1 2 3\n", "MXX 2 3 0 1\n"),
    ] {
        let (inverse, flows) = circuit_time_reversed_for_flows(&circuit(input_text), &[])
            .expect("reverse targets using Stim's pair-aware rule");
        assert_eq!(inverse, circuit(expected_text), "{input_text}");
        assert!(flows.is_empty(), "{input_text}");
    }
}

#[test]
fn time_reversal_rejects_inverted_result_synthesis() {
    for (gate, basis) in [("M", "Z"), ("MX", "X"), ("MY", "Y")] {
        let input = circuit(&format!("{gate} !0\n"));
        let input_flow = flow(&format!("{basis}0 -> rec[-1]"));
        let error = circuit_time_reversed_for_flows(&input, &[input_flow])
            .expect_err("Stim rejects the synthesized inverted reset")
            .to_string();
        assert!(
            error.contains('!') || error.contains("inverted"),
            "{gate}: {error}"
        );
    }

    for gate in ["MR", "MRX", "MRY"] {
        let input = circuit(&format!("{gate}(0.125) !0\n"));
        let error = circuit_time_reversed_for_flows(&input, &[])
            .expect_err("Stim rejects inverted ejected reset noise")
            .to_string();
        assert!(
            error.contains('!') || error.contains("inverted"),
            "{gate}: {error}"
        );
    }
}

#[test]
fn time_reversal_avoids_dense_storage_for_sparse_high_qubits() {
    let million_index = circuit("M 1000000\n");
    let (unchanged, flows) = circuit_time_reversed_for_flows(&million_index, &[])
        .expect("reverse a million-index circuit without dense tracker storage");
    assert_eq!(unchanged, million_index);
    assert!(flows.is_empty());
}

#[test]
fn time_reversal_validates_sparse_high_qubit_flows() {
    let input = circuit("M 4095\n");
    let input_flow = flow("Z4095 -> rec[-1]");

    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &[input_flow])
        .expect("reverse and validate a sparse high-index flow");

    assert_eq!(inverse, circuit("R 4095\n"));
    assert_eq!(flows, vec![flow("1 -> Z4095")]);
    assert!(circuit_has_all_unsigned_stabilizer_flows(&inverse, &flows));
}

#[test]
fn time_reversal_uses_sparse_validation_above_dense_tableau_width() {
    let input = circuit("H 1000000\n");

    let (without_flows, empty) = circuit_time_reversed_for_flows(&input, &[])
        .expect("empty-flow unitary reversal must not allocate a dense tableau");
    assert_eq!(without_flows, input);
    assert!(empty.is_empty());

    let idle_flow = flow("Z0 -> Z0");
    let (with_flow, flows) =
        circuit_time_reversed_for_flows(&input, std::slice::from_ref(&idle_flow)).expect(
            "nonempty unitary validation must use the sparse checker above the tableau budget",
        );
    assert_eq!(with_flow, input);
    assert_eq!(flows, vec![idle_flow]);
}

#[test]
fn time_reversal_uses_sparse_validation_at_the_tableau_boundary() {
    let input = circuit("H 512\n");
    let idle_flow = flow("Z0 -> Z0");

    let (inverse, flows) =
        circuit_time_reversed_for_flows(&input, std::slice::from_ref(&idle_flow))
            .expect("reverse above the dense Tableau cap through sparse validation");

    assert_eq!(inverse, input);
    assert_eq!(flows, vec![idle_flow]);
    assert!(circuit_has_all_unsigned_stabilizer_flows(&inverse, &flows));
}

#[test]
fn time_reversal_batches_output_flow_validation() {
    let input = circuit("M 0\n");
    let input_flows = std::iter::repeat_with(|| flow("1 -> Z0 xor rec[-1]"))
        .take(1_024)
        .collect::<Vec<_>>();

    let (inverse, output_flows) = circuit_time_reversed_for_flows(&input, &input_flows)
        .expect("batch reverse and output-flow validation");

    assert_eq!(inverse, input);
    assert_eq!(output_flows.len(), input_flows.len());
    assert!(circuit_has_all_unsigned_stabilizer_flows(
        &inverse,
        &output_flows
    ));
}

#[test]
fn time_reversal_rejects_absolute_relative_record_aliases() {
    let input = circuit("M 0\n");
    let aliased = flow("1 -> rec[-1] xor rec[0]");

    let error = circuit_time_reversed_for_flows(&input, &[aliased])
        .expect_err("pinned Stim rejects distinct terms that alias one record")
        .to_string();

    assert!(
        error.contains("rec[-1]") && error.contains("rec[0]"),
        "{error}"
    );
    assert!(error.contains("alias"), "{error}");
}

#[test]
fn supported_time_reversal_is_semantically_involutive() {
    let config = Config {
        cases: GENERATED_REVERSAL_CASES,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &GENERATED_REVERSAL_SEED);
    let mut runner = TestRunner::new_with_rng(config, rng);
    runner
        .run(&proptest::collection::vec(0_u8..9, 0..10), |operations| {
            let mut text = String::new();
            for operation in operations {
                text.push_str(match operation {
                    0 => "H 0\n",
                    1 => "S 0\n",
                    2 => "H 1\n",
                    3 => "CX 0 1\n",
                    4 => "M 0\n",
                    5 => "MX 1\n",
                    6 => "MZZ 0 1\n",
                    7 => "MPAD 0 1\n",
                    _ => "SPP X0*Z1\n",
                });
                text.push_str("TICK\n");
            }
            let input = Circuit::from_stim_str(&text).map_err(|error| {
                TestCaseError::fail(format!("generated circuit did not parse: {error}\n{text}"))
            })?;
            let input_flows = circuit_flow_generators(&input).map_err(|error| {
                TestCaseError::fail(format!("generated flows failed: {error}\n{text}"))
            })?;
            let options = TimeReversedForFlowsOptions {
                dont_turn_measurements_into_resets: true,
            };

            let (inverse, inverse_flows) =
                circuit_time_reversed_for_flows_with_options(&input, &input_flows, options)
                    .map_err(|error| {
                        TestCaseError::fail(format!("first reversal failed: {error}\n{text}"))
                    })?;
            prop_assert!(
                circuit_has_all_unsigned_stabilizer_flows(&inverse, &inverse_flows),
                "first reversed flows failed the checker\n{}\n{:?}\n{:?}",
                text,
                inverse,
                inverse_flows
            );

            let (round_trip, round_trip_flows) =
                circuit_time_reversed_for_flows_with_options(&inverse, &inverse_flows, options)
                    .map_err(|error| {
                        TestCaseError::fail(format!("second reversal failed: {error}\n{text}"))
                    })?;
            prop_assert_eq!(&round_trip, &input, "round-trip circuit mismatch\n{}", text);
            prop_assert!(
                circuit_has_all_unsigned_stabilizer_flows(&round_trip, &round_trip_flows),
                "round-trip flows failed the checker\n{}\n{:?}",
                text,
                round_trip_flows
            );
            Ok(())
        })
        .expect("deterministic generated reverse-flow corpus");
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid test circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("valid test flow")
}
