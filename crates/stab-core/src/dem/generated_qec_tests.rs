#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "unit tests use direct assertions for compact compatibility diagnostics"
)]

use std::collections::BTreeSet;

use super::*;
use crate::{
    Circuit, CodeDistance, RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams,
    SurfaceCodeTask, generate_repetition_code_circuit, generate_surface_code_circuit,
};

const GENERATED_DEM_TOLERANCE: f64 = 1e-12;

#[test]
fn generated_qec_dem_repetition_code_semantics_match_pinned_stim() {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("params")
    .with_before_round_data_depolarization(probability(0.0625))
    .with_before_measure_flip_probability(probability(0.03125))
    .with_after_reset_flip_probability(probability(0.015625))
    .with_after_clifford_depolarization(probability(0.0078125));
    let generated =
        generate_repetition_code_circuit(&params).expect("generate repetition code circuit");

    assert_circuit_dem_semantics_match_pinned_stim(
        generated.circuit(),
        concat!(
            "error(0.05784349713673846843375869752890139) D0\n",
            "error(0.05784349713673846843375869752890139) D0 D1\n",
            "error(0.04968261718749993061106096092771622) D0 D2\n",
            "error(0.004166666666666604158797415635717698) D0 D3\n",
            "error(0.04968261718749993061106096092771622) D1 D3\n",
            "error(0.05784349713673846843375869752890139) D1 L0\n",
            "error(0.04548611111111104665649662592841196) D2\n",
            "error(0.04548611111111104665649662592841196) D2 D3\n",
            "error(0.04968261718749993061106096092771622) D2 D4\n",
            "error(0.004166666666666604158797415635717698) D2 D5\n",
            "error(0.04968261718749993061106096092771622) D3 D5\n",
            "error(0.04548611111111104665649662592841196) D3 L0\n",
            "error(0.03320721105344821844074232330967789) D4\n",
            "error(0.03320721105344821844074232330967789) D4 D5\n",
            "error(0.03320721105344821844074232330967789) D5 L0\n",
            "detector(1, 0) D0\n",
            "detector(3, 0) D1\n",
            "shift_detectors(0, 1) 0\n",
            "detector(1, 0) D2\n",
            "detector(3, 0) D3\n",
            "detector(1, 1) D4\n",
            "detector(3, 1) D5\n",
        ),
    );
}

#[test]
fn generated_qec_dem_rotated_surface_code_semantics_match_pinned_stim() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("params")
    .with_before_round_data_depolarization(probability(0.0625))
    .with_before_measure_flip_probability(probability(0.03125))
    .with_after_reset_flip_probability(probability(0.015625))
    .with_after_clifford_depolarization(probability(0.0078125));
    let generated = generate_surface_code_circuit(&params).expect("generate surface code circuit");

    assert_circuit_dem_semantics_match_pinned_stim(
        generated.circuit(),
        concat!(
            "error(0.1081763913599733395454194351259503) D0\n",
            "error(0.001044937790157687233089101042082802) D0 D1\n",
            "error(0.003128266564410596157347344004051592) D0 D1 D2\n",
            "error(0.006236961025425172451541744322867089) D0 D1 L0\n",
            "error(0.05062372448923130319187180248263758) D0 D2\n",
            "error(0.1106278487480839428647172439923452) D0 L0\n",
            "error(0.1457069496702805555532478365421412) D1\n",
            "error(0.001044937790157687233089101042082802) D1 D2\n",
            "error(0.04738388093997307481952674379499513) D1 D2 L0\n",
            "error(0.05021568868351057590704300537254312) D2\n",
            "error(0.04738388093997306094173893598053837) D2 D3\n",
            "error(0.003128266564410596157347344004051592) D2 D3 D4\n",
            "error(0.001044937790157687233089101042082802) D2 D3 L0\n",
            "error(0.05062372448923130319187180248263758) D2 D4\n",
            "error(0.05021568868351059672372471709422825) D2 L0\n",
            "error(0.1457069496702806110643990677999682) D3\n",
            "error(0.006236961025425172451541744322867089) D3 D4\n",
            "error(0.001044937790157687233089101042082802) D3 D4 L0\n",
            "error(0.06602857902625254571393753622032818) D4\n",
            "error(0.06329632803512648397958884061154095) D4 L0\n",
            "detector(2, 2, 0) D0\n",
            "shift_detectors(0, 0, 1) 0\n",
            "detector(2, 0, 0) D1\n",
            "detector(2, 2, 0) D2\n",
            "detector(2, 4, 0) D3\n",
            "detector(2, 2, 1) D4\n",
        ),
    );
}

#[test]
fn semantic_dem_treats_graphlike_decomposition_as_equivalent() {
    let actual = DetectorErrorModel::from_dem_str("error(0.25) D0 D1 ^ D1 D2 L0\n")
        .expect("decomposed DEM")
        .semantic_dem()
        .expect("semantic decomposed DEM");
    let expected = DetectorErrorModel::from_dem_str("error(0.25) D0 D2 L0\n")
        .expect("flat DEM")
        .semantic_dem()
        .expect("semantic flat DEM");

    assert_semantic_dem_close(&actual, &expected);
}

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("probability")
}

fn assert_circuit_dem_semantics_match_pinned_stim(circuit: &Circuit, pinned_stim_dem: &str) {
    let actual = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
        .expect("Stab DEM")
        .semantic_dem()
        .expect("semantic Stab DEM");
    let expected = DetectorErrorModel::from_dem_str(pinned_stim_dem)
        .expect("pinned Stim DEM")
        .semantic_dem()
        .expect("semantic pinned Stim DEM");

    assert_semantic_dem_close(&actual, &expected);
}

trait SemanticDemExt {
    fn semantic_dem(&self) -> CircuitResult<SemanticDem>;
}

impl SemanticDemExt for DetectorErrorModel {
    fn semantic_dem(&self) -> CircuitResult<SemanticDem> {
        let mut out = SemanticDem::default();
        let mut state = DemTraversalState::default();
        collect_semantic_dem(self, &mut state, &mut out)?;
        Ok(out)
    }
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
}

#[derive(Debug)]
struct SemanticDetector {
    detector: u64,
    coordinates: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticTarget {
    Detector(u64),
    LogicalObservable(u64),
}

#[derive(Debug, Default)]
struct DemTraversalState {
    detector_offset: u64,
    coordinate_shift: Vec<f64>,
}

fn collect_semantic_dem(
    dem: &DetectorErrorModel,
    state: &mut DemTraversalState,
    out: &mut SemanticDem,
) -> CircuitResult<()> {
    for item in dem.items() {
        match item {
            DemItem::Instruction(instruction) => {
                collect_semantic_instruction(instruction, state, out)?
            }
            DemItem::RepeatBlock(repeat) => {
                for _ in 0..repeat.repeat_count().get() {
                    collect_semantic_dem(repeat.body(), state, out)?;
                }
            }
        }
    }
    Ok(())
}

fn collect_semantic_instruction(
    instruction: &DemInstruction,
    state: &mut DemTraversalState,
    out: &mut SemanticDem,
) -> CircuitResult<()> {
    match instruction.kind() {
        DemInstructionKind::Error => {
            let probability = instruction.args().first().copied().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "error instruction must have one probability argument",
                )
            })?;
            let targets = semantic_error_targets(instruction.targets(), state.detector_offset)?;
            out.errors.push(SemanticError {
                probability,
                targets,
            });
        }
        DemInstructionKind::Detector => {
            let [DemTarget::RelativeDetector(detector)] = instruction.targets() else {
                return Err(CircuitError::invalid_detector_error_model(
                    "detector instruction must have one detector target",
                ));
            };
            out.detectors.push(SemanticDetector {
                detector: state
                    .detector_offset
                    .checked_add(detector.get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "semantic detector id overflowed",
                        )
                    })?,
                coordinates: shifted_coordinates(instruction.args(), &state.coordinate_shift),
            });
        }
        DemInstructionKind::LogicalObservable => {
            for target in instruction.targets() {
                let DemTarget::LogicalObservable(observable) = target else {
                    return Err(CircuitError::invalid_detector_error_model(
                        "logical_observable instruction must have observable targets",
                    ));
                };
                out.logical_observables.push(observable.get());
            }
        }
        DemInstructionKind::ShiftDetectors => {
            apply_coordinate_shift(&mut state.coordinate_shift, instruction.args());
            state.detector_offset = state
                .detector_offset
                .checked_add(instruction.detector_shift()?)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("detector shift overflowed")
                })?;
        }
    }
    Ok(())
}

fn semantic_error_targets(
    targets: &[DemTarget],
    detector_offset: u64,
) -> CircuitResult<Vec<SemanticTarget>> {
    let mut detectors = BTreeSet::new();
    let mut logical_observables = BTreeSet::new();
    for target in targets {
        match *target {
            DemTarget::RelativeDetector(detector) => {
                let detector_id = detector_offset.checked_add(detector.get()).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("semantic detector id overflowed")
                })?;
                toggle(&mut detectors, detector_id);
            }
            DemTarget::LogicalObservable(observable) => {
                toggle(&mut logical_observables, observable.get());
            }
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "numeric targets are not error targets",
                ));
            }
        }
    }

    let mut out = Vec::new();
    out.extend(detectors.into_iter().map(SemanticTarget::Detector));
    out.extend(
        logical_observables
            .into_iter()
            .map(SemanticTarget::LogicalObservable),
    );
    Ok(out)
}

fn toggle(set: &mut BTreeSet<u64>, value: u64) {
    if !set.insert(value) {
        set.remove(&value);
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
        let coordinate = coordinates
            .get_mut(index)
            .expect("coordinate vector was resized to fit the shift");
        *coordinate += amount;
    }
}

fn assert_semantic_dem_close(actual: &SemanticDem, expected: &SemanticDem) {
    assert_eq!(
        actual.errors.len(),
        expected.errors.len(),
        "DEM error count differs"
    );
    for (index, (actual, expected)) in actual.errors.iter().zip(expected.errors.iter()).enumerate()
    {
        assert_float_close(
            actual.probability,
            expected.probability,
            &format!("error probability #{index}"),
        );
        assert_eq!(
            actual.targets, expected.targets,
            "error targets #{index} differ"
        );
    }

    assert_eq!(
        actual.detectors.len(),
        expected.detectors.len(),
        "DEM detector count differs"
    );
    for (index, (actual, expected)) in actual
        .detectors
        .iter()
        .zip(expected.detectors.iter())
        .enumerate()
    {
        assert_eq!(
            actual.detector, expected.detector,
            "detector target #{index} differs"
        );
        assert_eq!(
            actual.coordinates.len(),
            expected.coordinates.len(),
            "detector coordinate count #{index} differs"
        );
        for (coordinate_index, (actual_coordinate, expected_coordinate)) in actual
            .coordinates
            .iter()
            .zip(expected.coordinates.iter())
            .enumerate()
        {
            assert_float_close(
                *actual_coordinate,
                *expected_coordinate,
                &format!("detector #{index} coordinate #{coordinate_index}"),
            );
        }
    }

    assert_eq!(
        actual.logical_observables, expected.logical_observables,
        "DEM logical observables differ"
    );
}

fn assert_float_close(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= GENERATED_DEM_TOLERANCE,
        "{label} differs: actual={actual:?} expected={expected:?}"
    );
}
