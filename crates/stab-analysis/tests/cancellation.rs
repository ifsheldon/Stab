#![allow(
    clippy::expect_used,
    reason = "cancellation tests use compact fixture setup"
)]

//! Cooperative cancellation of the analysis operations whose work scales with repeat counts.
//!
//! Every operation polls its signal inside the loops that expand repeats, so a signal set before
//! the call stops it at the first poll. Fixtures that repeat 10^12 times fit the memory budget
//! and can only return by observing the signal; operations whose expanded work is bounded by
//! their admitted output use smaller fixtures that would otherwise succeed.

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use stab_algebra::{Flow, PauliString};
use stab_analysis::{
    AnalysisError, CircuitFlattenLimits, CircuitPass, CircuitPassContext, CircuitPassError,
    CircuitPassInput, CircuitPassOutput, CircuitPassResources, CodeDistance, DemFlattenLimits,
    DetectingRegionOptions, ErrorAnalyzerOptions, FeedbackInliningLimits, FlowGeneratorOptions,
    LogicalErrorSearchLimits, MissingDetectorOptions, PauliConjugationOptions, RoundCount,
    SatMaterializationLimits, SurfaceCodeParams, SurfaceCodeTask, TimeReversedForFlowsOptions,
    UnsignedFlowCheckOptions, WithoutNoiseOptions, WithoutNoisePass,
    check_if_circuit_has_unsigned_stabilizer_flows_with_options,
    check_unsigned_stabilizer_flows_with_diagnostics_with_options, circuit_detecting_regions,
    circuit_flow_generators_with_options, circuit_has_all_unsigned_stabilizer_flows_with_options,
    circuit_has_unsigned_stabilizer_flow_with_options,
    circuit_time_reversed_for_flows_with_options, circuit_to_detector_error_model,
    circuit_with_inlined_feedback_with_limits, find_undetectable_logical_error_with_limits,
    flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
    flattened_detector_error_model_with_limits, generate_surface_code_circuit,
    likeliest_error_sat_problem_with_limits, missing_detectors, pauli_after_circuit_with_options,
    pauli_before_circuit_with_options, run_circuit_pass, shortest_error_sat_problem_with_limits,
    shortest_graphlike_undetectable_logical_error_with_limits,
};
use stab_model::advanced::CancellationCountdown;
use stab_model::{
    Cancellation, Cancelled, Circuit, CircuitTick, DemDetectorId, DetectorErrorModel, ModelError,
    Probability,
};

const ENDLESS_MEASUREMENTS: &str = "REPEAT 1000000000000 {\n    M 0\n}\n";
const ENDLESS_UNITARY_THEN_MEASURE: &str = "REPEAT 1000000000000 {\n    H 0\n}\nM 0\n";

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn cancelled() -> Cancellation {
    let cancellation = Cancellation::new();
    cancellation.cancel();
    cancellation
}

fn assert_cancelled<T: std::fmt::Debug>(result: Result<T, AnalysisError>) {
    assert_eq!(
        result.expect_err("a set signal stops the operation"),
        AnalysisError::Cancelled(Cancelled)
    );
}

#[test]
fn unrolled_error_analysis_of_an_astronomical_repeat_is_cancellable() {
    let options = ErrorAnalyzerOptions {
        cancellation: cancelled(),
        ..ErrorAnalyzerOptions::default()
    };
    assert_cancelled(circuit_to_detector_error_model(
        &circuit(ENDLESS_MEASUREMENTS),
        options,
    ));
}

#[test]
fn flow_generation_and_missing_detectors_over_an_astronomical_repeat_are_cancellable() {
    let fixture = circuit(ENDLESS_UNITARY_THEN_MEASURE);
    assert_cancelled(circuit_flow_generators_with_options(
        &fixture,
        &FlowGeneratorOptions {
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: cancelled(),
        },
    ));
    assert_cancelled(missing_detectors(
        &fixture,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: cancelled(),
        },
    ));
}

#[test]
fn dem_flattening_of_an_astronomical_empty_expansion_is_cancellable() {
    // The outer body yields nothing, so the budget admits it while the walk visits 10^12 bodies.
    let model = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n    repeat 0 {\n        error(0.1) D0\n    }\n}\n",
    )
    .expect("parse DEM fixture");
    assert_cancelled(flattened_detector_error_model_with_limits(
        &model,
        DemFlattenLimits::default().with_cancellation(cancelled()),
    ));
    let iterated = model
        .iter_flattened_instructions()
        .with_cancellation(cancelled())
        .next()
        .expect("the iterator reports cancellation");
    assert_eq!(iterated, Err(ModelError::Cancelled(Cancelled)));
}

#[test]
fn output_bounded_expansions_honor_a_set_signal() {
    let fixture = circuit(
        "REPEAT 100000 {\n    H 0\n    M 0\n    CX rec[-1] 1\n    TICK\n    DETECTOR rec[-1]\n}\n",
    );
    let limits = CircuitFlattenLimits::default().with_cancellation(cancelled());
    assert_cancelled(flattened_circuit_with_limits(&fixture, limits.clone()));
    assert_cancelled(flattened_circuit_operations_with_limits(&fixture, limits));
    assert_cancelled(circuit_with_inlined_feedback_with_limits(
        &fixture,
        FeedbackInliningLimits::default().with_cancellation(cancelled()),
    ));
    let measurements = circuit("REPEAT 100000 {\n    H 0\n    M 0\n}\n");
    assert_cancelled(circuit_time_reversed_for_flows_with_options(
        &measurements,
        &[],
        TimeReversedForFlowsOptions {
            dont_turn_measurements_into_resets: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: cancelled(),
        },
    ));
    assert_cancelled(circuit_detecting_regions(
        &fixture,
        DetectingRegionOptions {
            detectors: vec![DemDetectorId::try_new(0).expect("detector id")],
            ticks: vec![CircuitTick::new(0)],
            ignore_anticommutation_errors: true,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: cancelled(),
        },
    ));
}

/// Sets `cancellation` from another thread once the operation under test has started, so the
/// signal is observed by a poll inside the operation's loop rather than by an entry check.
fn cancel_soon(cancellation: &Cancellation) -> JoinHandle<()> {
    let signal = cancellation.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        signal.cancel();
    })
}

/// Runs `operation` with a signal set shortly after it starts and checks that it stops promptly.
fn assert_cancelled_in_flight<T: std::fmt::Debug>(
    operation: impl FnOnce(Cancellation) -> Result<T, AnalysisError>,
) {
    let cancellation = Cancellation::new();
    let canceller = cancel_soon(&cancellation);
    let started = Instant::now();
    assert_cancelled(operation(cancellation));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    canceller.join().expect("canceller thread");
}

fn flows(texts: &[&str]) -> Vec<Flow> {
    texts
        .iter()
        .map(|text| text.parse().expect("parse flow fixture"))
        .collect()
}

#[test]
fn flow_checks_report_cancellation_instead_of_failing_flows_closed() {
    // A unitary query the dense Tableau can check and a measurement query only the sparse
    // tracker can check; both hold, so a `false` could only come from the signal.
    for (text, flow_texts) in [
        ("H 0\nCX 0 1\nS 1\n", &["X_ -> Z_", "_Z -> ZZ"][..]),
        (
            "R 0\nH 0\nH 0\nM 0\n",
            &["1 -> rec[-1]", "1 -> Z xor rec[-1]", "1 -> Z"][..],
        ),
    ] {
        let fixture = circuit(text);
        let flows = flows(flow_texts);
        let unset = UnsignedFlowCheckOptions {
            cancellation: Cancellation::new(),
            ..UnsignedFlowCheckOptions::default()
        };
        assert_eq!(
            check_if_circuit_has_unsigned_stabilizer_flows_with_options(&fixture, &flows, &unset),
            Ok(vec![true; flows.len()]),
            "{text}"
        );
        let set = UnsignedFlowCheckOptions {
            cancellation: cancelled(),
            ..UnsignedFlowCheckOptions::default()
        };
        assert_cancelled(check_if_circuit_has_unsigned_stabilizer_flows_with_options(
            &fixture, &flows, &set,
        ));
        assert_cancelled(
            check_unsigned_stabilizer_flows_with_diagnostics_with_options(&fixture, &flows, &set),
        );
        assert_cancelled(circuit_has_unsigned_stabilizer_flow_with_options(
            &fixture,
            flows.first().expect("fixture flow"),
            &set,
        ));
        assert_cancelled(circuit_has_all_unsigned_stabilizer_flows_with_options(
            &fixture, &flows, &set,
        ));
    }
}

#[test]
fn pauli_conjugation_through_an_astronomical_measured_repeat_is_cancellable() {
    // The measurement makes the circuit non-unitary, so conjugation walks 2 * 10^12 instructions.
    let fixture = circuit("REPEAT 1000000000000 {\n    H 0\n    M 1\n}\n");
    let pauli: PauliString = "X_".parse().expect("parse Pauli fixture");
    let options = |cancellation| PauliConjugationOptions {
        cancellation,
        ..PauliConjugationOptions::default()
    };
    assert_cancelled_in_flight(|signal| {
        pauli_after_circuit_with_options(&pauli, &fixture, &options(signal))
    });
    assert_cancelled_in_flight(|signal| {
        pauli_before_circuit_with_options(&pauli, &fixture, &options(signal))
    });
}

/// A pass that never finishes unless it observes the context's signal, which it reports through
/// its own diagnostic type.
struct EndlessPass;

#[derive(Debug, thiserror::Error)]
#[error("the endless pass stopped")]
struct EndlessPassStopped;

impl CircuitPass for EndlessPass {
    type Options = ();
    type Report = ();
    type Diagnostic = EndlessPassStopped;

    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        _options: &(),
    ) -> Result<CircuitPassResources, EndlessPassStopped> {
        Ok(input.resources())
    }

    fn run(
        &self,
        input: CircuitPassInput<'_>,
        _options: &(),
    ) -> Result<CircuitPassOutput<()>, EndlessPassStopped> {
        let mut countdown = CancellationCountdown::new();
        loop {
            countdown
                .tick(input.context().cancellation())
                .map_err(|_| EndlessPassStopped)?;
        }
    }
}

#[test]
fn circuit_passes_report_the_context_signal() {
    let fixture = circuit("X_ERROR(0.1) 0\nREPEAT 3 {\n    H 0\n    DEPOLARIZE1(0.1) 0\n}\n");
    let context = CircuitPassContext::default().with_cancellation(cancelled());
    let error = run_circuit_pass(&WithoutNoisePass, &fixture, &WithoutNoiseOptions, &context)
        .expect_err("a set signal stops the pass");
    assert!(
        matches!(error, CircuitPassError::Cancelled(Cancelled)),
        "{error}"
    );

    let cancellation = Cancellation::new();
    let canceller = cancel_soon(&cancellation);
    let context = CircuitPassContext::default().with_cancellation(cancellation);
    let error = run_circuit_pass(&EndlessPass, &fixture, &(), &context)
        .expect_err("only the signal stops the endless pass");
    assert!(
        matches!(error, CircuitPassError::Cancelled(Cancelled)),
        "{error}"
    );
    canceller.join().expect("canceller thread");
}

fn surface_code_model(distance: u32) -> DetectorErrorModel {
    let noise = Probability::try_new(0.001).expect("probability");
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(u64::from(distance)).expect("rounds"),
        CodeDistance::try_new(distance).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface code parameters")
    .with_after_clifford_depolarization(noise)
    .with_before_measure_flip_probability(noise);
    let generated = generate_surface_code_circuit(&params).expect("surface code circuit");
    circuit_to_detector_error_model(
        generated.circuit(),
        ErrorAnalyzerOptions {
            decompose_errors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("surface code model")
}

#[test]
fn logical_error_searches_are_cancellable() {
    let model = surface_code_model(5);
    let limits = || LogicalErrorSearchLimits::default().with_cancellation(cancelled());
    assert_cancelled(shortest_graphlike_undetectable_logical_error_with_limits(
        &model,
        false,
        limits(),
    ));
    assert_cancelled(find_undetectable_logical_error_with_limits(
        &model,
        4,
        4,
        false,
        limits(),
    ));
    // The weight-4 hypergraph search of a distance-5 surface code examines millions of
    // transitions, far more than the signal's delay.
    assert_cancelled_in_flight(|signal| {
        find_undetectable_logical_error_with_limits(
            &model,
            4,
            4,
            false,
            LogicalErrorSearchLimits::default().with_cancellation(signal),
        )
    });
}

#[test]
fn sat_generation_over_an_astronomical_repeat_is_cancellable() {
    // With the traversal limits lifted, admission visits 10^12 iterations before any output.
    let model = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n    error(0.1) D0 L0\n    shift_detectors 1\n}\n",
    )
    .expect("parse DEM fixture");
    let unbounded = |signal| {
        SatMaterializationLimits::default()
            .with_max_expanded_instructions(u64::MAX)
            .with_max_repeat_iterations(u64::MAX)
            .with_max_error_mechanisms(usize::MAX)
            .with_max_target_occurrences(usize::MAX)
            .with_cancellation(signal)
    };
    assert_cancelled_in_flight(|signal| {
        shortest_error_sat_problem_with_limits(&model, unbounded(signal))
    });
    assert_cancelled_in_flight(|signal| {
        likeliest_error_sat_problem_with_limits(&model, 100, unbounded(signal))
    });
}
