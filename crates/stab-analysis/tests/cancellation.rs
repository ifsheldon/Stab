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

use stab_analysis::{
    AnalysisError, CircuitFlattenLimits, DemFlattenLimits, DetectingRegionOptions,
    ErrorAnalyzerOptions, FeedbackInliningLimits, FlowGeneratorOptions, MissingDetectorOptions,
    TimeReversedForFlowsOptions, circuit_detecting_regions, circuit_flow_generators_with_options,
    circuit_time_reversed_for_flows_with_options, circuit_to_detector_error_model,
    circuit_with_inlined_feedback_with_limits, flattened_circuit_operations_with_limits,
    flattened_circuit_with_limits, flattened_detector_error_model_with_limits, missing_detectors,
};
use stab_model::{
    Cancellation, Cancelled, Circuit, CircuitTick, DemDetectorId, DetectorErrorModel, ModelError,
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
