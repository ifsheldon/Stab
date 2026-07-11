#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "integration parity tests use fixed generated cases and direct invariant diagnostics"
)]

use stab_core::{
    Circuit, DemInstruction, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel,
    ErrorAnalyzerOptions, circuit_to_detector_error_model,
};

const NESTED_CIRCUIT: &str =
    include_str!("../../../oracle/fixtures/inputs/pfm_b5_analyzer_nested_loop.stim");
const NESTED_EXPECTED: &str =
    include_str!("../../../oracle/fixtures/expected/pfm_b5_analyzer_nested_loop.stdout");
const COORDINATE_CIRCUIT: &str =
    include_str!("../../../oracle/fixtures/inputs/pfm_b5_analyzer_coordinate_loop.stim");
const COORDINATE_EXPECTED: &str =
    include_str!("../../../oracle/fixtures/expected/pfm_b5_analyzer_coordinate_loop.stdout");
const GAUGE_CIRCUIT: &str =
    include_str!("../../../oracle/fixtures/inputs/pfm_b5_analyzer_gauge_loop.stim");
const GAUGE_EXPECTED: &str =
    include_str!("../../../oracle/fixtures/expected/pfm_b5_analyzer_gauge_loop.stdout");
const REPETITION_CIRCUIT: &str =
    include_str!("../../../oracle/fixtures/inputs/pfm_b5_analyzer_repetition_code.stim");
const REPETITION_EXPECTED: &str =
    include_str!("../../../oracle/fixtures/expected/pfm_b5_analyzer_repetition_code.stdout");

#[test]
fn pfm_b5_nested_loop_folding() {
    assert_eq!(
        analyze(NESTED_CIRCUIT, ErrorAnalyzerOptions::default()).to_dem_string(),
        NESTED_EXPECTED
    );
}

#[test]
fn pfm_b5_coordinate_loop() {
    assert_eq!(
        analyze(COORDINATE_CIRCUIT, ErrorAnalyzerOptions::default()).to_dem_string(),
        COORDINATE_EXPECTED
    );
}

#[test]
fn pfm_b5_gauge_loop_bounded() {
    assert_eq!(
        analyze(
            GAUGE_CIRCUIT,
            ErrorAnalyzerOptions {
                allow_gauge_detectors: true,
                ..ErrorAnalyzerOptions::default()
            },
        )
        .to_dem_string(),
        GAUGE_EXPECTED
    );
}

#[test]
fn pfm_b5_repetition_code_loop() {
    let actual = analyze(
        REPETITION_CIRCUIT,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    );
    let expected = DetectorErrorModel::from_dem_str(REPETITION_EXPECTED).expect("pinned Stim DEM");
    assert_models_match_with_probability_tolerance(&actual, &expected, 1e-15);
}

#[test]
fn pfm_b5_generated_folded_matches_unrolled_clifford_measurement_loops() {
    let gates = [
        "I 0",
        "S 0",
        "Z 1",
        "CX 0 1",
        "CZ 0 1",
        "SWAP 0 1",
        "X 0\n    X 0",
    ];
    let probabilities = ["0.001", "0.01", "0.05", "0.125"];
    let mut seed = 0x5a17_9d3c_e241_b607_u64;

    for case_index in 0..16 {
        let repeat_count = 5 + usize::try_from(next_seeded(&mut seed) % 8).expect("small count");
        let gate = gates
            [usize::try_from(next_seeded(&mut seed) % gates.len() as u64).expect("gate index")];
        let x_probability =
            probabilities[usize::try_from(next_seeded(&mut seed) % probabilities.len() as u64)
                .expect("probability index")];
        let z_probability =
            probabilities[usize::try_from(next_seeded(&mut seed) % probabilities.len() as u64)
                .expect("probability index")];
        let circuit = format!(
            "R 0 1\nREPEAT {repeat_count} {{\n    {gate}\n    X_ERROR({x_probability}) 0\n    Z_ERROR({z_probability}) 1\n    M 0 1\n    DETECTOR({case_index}, 0) rec[-2]\n    DETECTOR({case_index}, 1) rec[-1]\n    OBSERVABLE_INCLUDE(0) rec[-2] rec[-1]\n    SHIFT_COORDS(0, 1)\n    R 0 1\n}}\n"
        );
        assert_folded_matches_unrolled(&circuit, ErrorAnalyzerOptions::default());
    }
}

#[test]
fn pfm_b5_generated_folded_matches_unrolled_nested_coordinate_loop() {
    let circuit = "\
R 0
REPEAT 7 {
    REPEAT 3 {
        X_ERROR(0.05) 0
        M 0
        DETECTOR(1, 2) rec[-1]
        R 0
        SHIFT_COORDS(0.25, 0.5)
    }
    OBSERVABLE_INCLUDE(0) rec[-1]
}
";
    assert_folded_matches_unrolled(circuit, ErrorAnalyzerOptions::default());
}

fn analyze(text: &str, options: ErrorAnalyzerOptions) -> DetectorErrorModel {
    analyze_with_fold(text, options, true)
}

fn analyze_with_fold(
    text: &str,
    mut options: ErrorAnalyzerOptions,
    fold_loops: bool,
) -> DetectorErrorModel {
    options.fold_loops = fold_loops;
    let circuit = Circuit::from_stim_str(text).expect("valid analyzer fixture");
    circuit_to_detector_error_model(&circuit, options).expect("folded detector error model")
}

fn assert_folded_matches_unrolled(text: &str, options: ErrorAnalyzerOptions) {
    let folded = semantic_dem(&analyze_with_fold(text, options, true));
    let unrolled = semantic_dem(&analyze_with_fold(text, options, false));
    assert_semantic_dems_match(&folded, &unrolled);
}

fn next_seeded(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[derive(Debug, Default)]
struct SemanticDem {
    errors: Vec<SemanticError>,
    detectors: Vec<SemanticDetector>,
    logical_observables: Vec<u64>,
}

#[derive(Debug)]
struct SemanticError {
    probability: f64,
    targets: Vec<SemanticTarget>,
    tag: Option<String>,
}

#[derive(Debug)]
struct SemanticDetector {
    detector: u64,
    coordinates: Vec<f64>,
    tag: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticTarget {
    Detector(u64),
    LogicalObservable(u64),
}

#[derive(Debug, Default)]
struct SemanticState {
    detector_offset: u64,
    coordinate_shift: Vec<f64>,
}

fn semantic_dem(model: &DetectorErrorModel) -> SemanticDem {
    let mut result = SemanticDem::default();
    let mut state = SemanticState::default();
    collect_semantic_dem(model, &mut state, &mut result);
    result
}

fn collect_semantic_dem(
    model: &DetectorErrorModel,
    state: &mut SemanticState,
    result: &mut SemanticDem,
) {
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => {
                collect_semantic_instruction(instruction, state, result);
            }
            DemItem::RepeatBlock(repeat) => {
                for _ in 0..repeat.repeat_count().get() {
                    collect_semantic_dem(repeat.body(), state, result);
                }
            }
        }
    }
}

fn collect_semantic_instruction(
    instruction: &DemInstruction,
    state: &mut SemanticState,
    result: &mut SemanticDem,
) {
    match instruction.kind() {
        DemInstructionKind::Error => {
            let probability = *instruction
                .args()
                .first()
                .expect("error instruction should have a probability");
            let mut targets = Vec::new();
            for target in instruction.targets() {
                match *target {
                    DemTarget::RelativeDetector(detector) => {
                        targets.push(SemanticTarget::Detector(
                            state
                                .detector_offset
                                .checked_add(detector.get())
                                .expect("semantic detector id should fit"),
                        ));
                    }
                    DemTarget::LogicalObservable(observable) => {
                        targets.push(SemanticTarget::LogicalObservable(observable.get()));
                    }
                    DemTarget::Separator => {}
                    DemTarget::Numeric(_) => panic!("error target should not be numeric"),
                }
            }
            result.errors.push(SemanticError {
                probability,
                targets,
                tag: instruction.tag().map(ToOwned::to_owned),
            });
        }
        DemInstructionKind::Detector => {
            let [DemTarget::RelativeDetector(detector)] = instruction.targets() else {
                panic!("detector declaration should have one detector target");
            };
            result.detectors.push(SemanticDetector {
                detector: state
                    .detector_offset
                    .checked_add(detector.get())
                    .expect("semantic detector id should fit"),
                coordinates: shifted_coordinates(instruction.args(), &state.coordinate_shift),
                tag: instruction.tag().map(ToOwned::to_owned),
            });
        }
        DemInstructionKind::LogicalObservable => {
            for target in instruction.targets() {
                let DemTarget::LogicalObservable(observable) = target else {
                    panic!("logical observable declaration should have observable targets");
                };
                result.logical_observables.push(observable.get());
            }
        }
        DemInstructionKind::ShiftDetectors => {
            apply_coordinate_shift(&mut state.coordinate_shift, instruction.args());
            let [DemTarget::Numeric(detector_shift)] = instruction.targets() else {
                panic!("shift_detectors should have one numeric target");
            };
            state.detector_offset = state
                .detector_offset
                .checked_add(*detector_shift)
                .expect("semantic detector offset should fit");
        }
    }
}

fn shifted_coordinates(coordinates: &[f64], shift: &[f64]) -> Vec<f64> {
    let len = coordinates.len().max(shift.len());
    (0..len)
        .map(|index| {
            coordinates.get(index).copied().unwrap_or(0.0)
                + shift.get(index).copied().unwrap_or(0.0)
        })
        .collect()
}

fn apply_coordinate_shift(coordinates: &mut Vec<f64>, shift: &[f64]) {
    if coordinates.len() < shift.len() {
        coordinates.resize(shift.len(), 0.0);
    }
    for (index, amount) in shift.iter().copied().enumerate() {
        coordinates[index] += amount;
    }
}

fn assert_semantic_dems_match(actual: &SemanticDem, expected: &SemanticDem) {
    assert_eq!(actual.errors.len(), expected.errors.len());
    for (actual, expected) in actual.errors.iter().zip(&expected.errors) {
        assert!((actual.probability - expected.probability).abs() <= 1e-15);
        assert_eq!(actual.targets, expected.targets);
        assert_eq!(actual.tag, expected.tag);
    }

    assert_eq!(actual.detectors.len(), expected.detectors.len());
    for (actual, expected) in actual.detectors.iter().zip(&expected.detectors) {
        assert_eq!(actual.detector, expected.detector);
        assert_eq!(actual.coordinates.len(), expected.coordinates.len());
        for (actual, expected) in actual.coordinates.iter().zip(&expected.coordinates) {
            assert!((actual - expected).abs() <= 1e-12);
        }
        assert_eq!(actual.tag, expected.tag);
    }
    assert_eq!(actual.logical_observables, expected.logical_observables);
}

fn assert_models_match_with_probability_tolerance(
    actual: &DetectorErrorModel,
    expected: &DetectorErrorModel,
    tolerance: f64,
) {
    assert_eq!(actual.items().len(), expected.items().len());
    for (actual, expected) in actual.items().iter().zip(expected.items()) {
        match (actual, expected) {
            (DemItem::Instruction(actual), DemItem::Instruction(expected)) => {
                assert_eq!(actual.kind(), expected.kind());
                assert_eq!(actual.targets(), expected.targets());
                assert_eq!(actual.tag(), expected.tag());
                assert_eq!(actual.args().len(), expected.args().len());
                for (actual_arg, expected_arg) in actual.args().iter().zip(expected.args()) {
                    if actual.kind() == DemInstructionKind::Error {
                        assert!(
                            (actual_arg - expected_arg).abs() <= tolerance,
                            "error probability {actual_arg} differs from {expected_arg}"
                        );
                    } else {
                        assert_eq!(actual_arg, expected_arg);
                    }
                }
            }
            (DemItem::RepeatBlock(actual), DemItem::RepeatBlock(expected)) => {
                assert_eq!(actual.repeat_count(), expected.repeat_count());
                assert_eq!(actual.tag(), expected.tag());
                assert_models_match_with_probability_tolerance(
                    actual.body(),
                    expected.body(),
                    tolerance,
                );
            }
            _ => panic!("DEM item kinds differ: {actual:?} != {expected:?}"),
        }
    }
}
