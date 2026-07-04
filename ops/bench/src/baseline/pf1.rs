use std::hint::black_box;

use stab_core::{
    Circuit, CircuitDetectorId, DemDetectorId, DemInstructionKind, DemItem, DemTarget,
    DetectorErrorModel, Flow, Gate, GateArgumentRule, PauliString, Tableau,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{TINY_DIRECT_COMPARE_REPETITIONS, measure_stab_batched, stab_runner_error};

const CIRCUIT_STATS_FIXTURE: &str = r#"
M 0 1
REPEAT 1000000 {
    REPEAT 1000 {
        TICK
        M 2
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(3) rec[-1]
        CY sweep[77] 3
    }
}
"#;

const CIRCUIT_COORDINATE_FIXTURE: &str = r#"
QUBIT_COORDS(1, 2, 3) 0
QUBIT_COORDS(2) 1
SHIFT_COORDS(5)
QUBIT_COORDS(3) 4
REPEAT 1000000 {
    SHIFT_COORDS(10, 1)
    QUBIT_COORDS(7) 1
}
QUBIT_COORDS(0, 0) 2
"#;

const CIRCUIT_DETECTOR_COORDINATE_FIXTURE: &str = r#"
TICK
REPEAT 1000 {
    REPEAT 2000 {
        REPEAT 1000 {
            DETECTOR(0, 0, 0, 4)
            SHIFT_COORDS(1, 0, 0)
        }
        DETECTOR(0, 0, 0, 3)
        SHIFT_COORDS(0, 1, 0)
    }
    DETECTOR(0, 0, 0, 2)
    SHIFT_COORDS(0, 0, 1)
}
DETECTOR(0, 0, 0, 1)
"#;

const DEM_COUNTS_FIXTURE: &str = r#"
shift_detectors(0, 0.5) 100
repeat 1000000 {
    repeat 1000 {
        error(0.125) D0 D2 L7
        detector(1, 2) D0
        logical_observable L5
        shift_detectors(3, 0, 1) 4
    }
}
"#;

const DEM_COORDINATE_FIXTURE: &str = r#"
repeat 200 {
    repeat 100 {
        detector(0, 0, 0, 4) D1
        shift_detectors(1, 0, 0) 10
    }
    detector(0, 0, 0, 3) D2
    shift_detectors(0, 1, 0) 0
}
detector(0, 0, 0, 2) D3
"#;

const DEM_TAGS_FIXTURE: &str = r#"
error[first](0.125) D0 D2 L7
repeat[outer] 1000 {
    repeat[inner] 1000 {
        detector[det](1, 2) D0
        logical_observable[log] L5
        shift_detectors[step](3, 0, 1) 4
    }
}
"#;

pub(super) fn run_circuit_coordinate_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let stats_circuit = Circuit::from_stim_str(CIRCUIT_STATS_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let coordinate_circuit = Circuit::from_stim_str(CIRCUIT_COORDINATE_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let detector_coordinate_circuit = Circuit::from_stim_str(CIRCUIT_DETECTOR_COORDINATE_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let early_detector = CircuitDetectorId::new(1002);
    let late_detector = CircuitDetectorId::new(2_002_001_000);

    Ok(vec![
        measure_stab_batched(
            "stab_circuit_counts_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0_u64;
                checksum ^= stats_circuit
                    .count_measurements()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= stats_circuit
                    .count_detectors()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= stats_circuit
                    .count_observables()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= stats_circuit
                    .count_ticks()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= stats_circuit
                    .count_sweep_bits()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_final_coordinate_shift_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let shift = coordinate_circuit
                    .final_coordinate_shift()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(shift.len());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_final_qubit_coordinates_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let coordinates = coordinate_circuit
                    .final_qubit_coordinates()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(coordinates.len());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_detector_coordinates_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let coordinates = detector_coordinate_circuit
                    .coordinates_of_detector(early_detector)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(float_slice_checksum(&coordinates));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_detector_coordinates_late_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let coordinates = detector_coordinate_circuit
                    .coordinates_of_detector(late_detector)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(float_slice_checksum(&coordinates));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_dem_counts_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let dem = DetectorErrorModel::from_dem_str(DEM_COUNTS_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let coordinate_dem = DetectorErrorModel::from_dem_str(DEM_COORDINATE_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let coordinate_detector =
        DemDetectorId::try_new(1021).map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![
        measure_stab_batched(
            "stab_dem_counts_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0_u64;
                checksum ^= dem
                    .count_detectors()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= dem
                    .count_observables()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= dem
                    .total_detector_shift()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= dem
                    .count_errors()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_dem_final_coordinate_shift_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let shift = dem
                    .final_coordinate_shift()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(float_slice_checksum(&shift));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_dem_detector_coordinates_nested_repeat",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let coordinates = coordinate_dem
                    .coordinates_of_detector(coordinate_detector)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(float_slice_checksum(&coordinates));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_dem_without_tags_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let dem = DetectorErrorModel::from_dem_str(DEM_TAGS_FIXTURE)
        .map_err(|error| stab_runner_error(&row.id, error))?;

    Ok(vec![measure_stab_batched(
        "stab_dem_without_tags_nested_repeat",
        TINY_DIRECT_COMPARE_REPETITIONS,
        || {
            let stripped = dem.without_tags();
            black_box(dem_model_checksum(&stripped));
            Ok(())
        },
    )?])
}

pub(super) fn run_gate_metadata_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let gates = Gate::all().collect::<Vec<_>>();
    let aliases = gates
        .iter()
        .flat_map(|gate| gate.aliases().iter().copied())
        .collect::<Vec<_>>();
    let tableau_gates = gates
        .iter()
        .copied()
        .filter(|gate| gate.has_tableau())
        .collect::<Vec<_>>();
    let flow_gates = gates
        .iter()
        .copied()
        .filter(|gate| gate.has_flows())
        .collect::<Vec<_>>();

    Ok(vec![
        measure_stab_batched(
            "stab_gate_metadata_flags_all_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for gate in &gates {
                    checksum ^= gate.canonical_name().len();
                    checksum ^= gate.aliases().len();
                    checksum ^= gate.category() as usize;
                    checksum ^= argument_rule_checksum(gate.argument_rule());
                    checksum ^= gate.target_rule() as usize;
                    checksum ^= gate.target_group_kind() as usize;
                    checksum ^= usize::from(gate.is_unitary());
                    checksum ^= usize::from(gate.is_reset()) << 1;
                    checksum ^= usize::from(gate.is_noisy()) << 2;
                    checksum ^= usize::from(gate.produces_measurements()) << 3;
                    checksum ^= usize::from(gate.is_single_qubit_gate()) << 4;
                    checksum ^= usize::from(gate.is_two_qubit_gate()) << 5;
                    checksum ^= usize::from(gate.takes_measurement_record_targets()) << 6;
                    checksum ^= usize::from(gate.takes_pauli_targets()) << 7;
                    checksum ^= usize::from(gate.is_symmetric_gate()) << 8;
                    checksum ^= usize::from(gate.can_fuse()) << 9;
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_inverse_all_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for gate in &gates {
                    if let Some(inverse) = gate.inverse() {
                        checksum ^= inverse.canonical_name().len();
                    }
                    let generalized_inverse = gate
                        .generalized_inverse()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= generalized_inverse.canonical_name().len();
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_tableau_supported_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0_u64;
                for gate in &tableau_gates {
                    let tableau = gate
                        .tableau()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= tableau_checksum(&tableau, &row.id)?;
                    black_box(tableau);
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_flows_supported_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0_u64;
                for gate in &flow_gates {
                    let flows = gate
                        .flows()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= flows.len() as u64;
                    for flow in &flows {
                        checksum ^= flow_checksum(flow).rotate_left(3);
                    }
                    black_box(flows);
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_alias_lookup_all_aliases",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for alias in &aliases {
                    let gate = Gate::from_name(alias)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= gate.canonical_name().len();
                }
                black_box(checksum);
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if row_id == "pf1-circuit-coordinate-query" {
        return match name {
            "stab_circuit_counts_nested_repeat"
            | "stab_circuit_final_coordinate_shift_nested_repeat"
            | "stab_circuit_final_qubit_coordinates_nested_repeat"
            | "stab_circuit_detector_coordinates_nested_repeat"
            | "stab_circuit_detector_coordinates_late_nested_repeat" => Some((1.0, "queries/s")),
            _ => None,
        };
    }
    if row_id == "pf1-gate-metadata-lookup" {
        return match name {
            "stab_gate_metadata_flags_all_gates" => Some((Gate::all().len() as f64, "gates/s")),
            "stab_gate_metadata_inverse_all_gates" => Some((Gate::all().len() as f64, "gates/s")),
            "stab_gate_metadata_tableau_supported_gates" => {
                Some((gate_tableau_count() as f64, "tableaus/s"))
            }
            "stab_gate_metadata_flows_supported_gates" => {
                Some((gate_flow_count() as f64, "flows/s"))
            }
            "stab_gate_metadata_alias_lookup_all_aliases" => {
                Some((gate_alias_count() as f64, "lookups/s"))
            }
            _ => None,
        };
    }
    if row_id == "pf1-dem-counts-repeat" {
        return match name {
            "stab_dem_counts_nested_repeat"
            | "stab_dem_final_coordinate_shift_nested_repeat"
            | "stab_dem_detector_coordinates_nested_repeat" => Some((1.0, "queries/s")),
            _ => None,
        };
    }
    if row_id == "pf1-dem-without-tags" {
        return match name {
            "stab_dem_without_tags_nested_repeat" => Some((1.0, "queries/s")),
            _ => None,
        };
    }
    None
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf1-circuit-coordinate-query" => Some(
            "contract-only: Stab measures Rust circuit count, final-coordinate, and detector-coordinate public API queries; pinned Stim exposes similar behavior through C++ and Python APIs but not a faithful Rust direct baseline",
        ),
        "pf1-gate-metadata-lookup" => Some(
            "contract-only: Stab measures Rust gate metadata accessors, tableau metadata reads, tableau-backed flow metadata reads, and alias lookup against the PF1 public API; pinned Stim GateData is a Python binding surface without a faithful Rust direct baseline",
        ),
        "pf1-dem-counts-repeat" => Some(
            "contract-only: Stab measures Rust DEM count, final-coordinate, and detector-coordinate public API queries; pinned Stim exposes similar behavior through C++ and Python APIs but not a faithful Rust direct baseline",
        ),
        "pf1-dem-without-tags" => Some(
            "contract-only: Stab measures Rust DEM recursive tag-stripping public API queries; pinned Stim exposes similar behavior through Python APIs but not a faithful Rust direct baseline",
        ),
        _ => None,
    }
}

fn float_slice_checksum(values: &[f64]) -> u64 {
    values.iter().fold(values.len() as u64, |checksum, value| {
        checksum.rotate_left(7) ^ value.to_bits()
    })
}

fn dem_model_checksum(model: &DetectorErrorModel) -> u64 {
    model
        .items()
        .iter()
        .fold(model.items().len() as u64, |checksum, item| {
            checksum.rotate_left(5) ^ dem_item_checksum(item)
        })
}

fn dem_item_checksum(item: &DemItem) -> u64 {
    match item {
        DemItem::Instruction(instruction) => {
            let mut checksum = dem_instruction_kind_checksum(instruction.kind());
            checksum ^= instruction
                .tag()
                .map_or(0, |tag| tag.len() as u64)
                .rotate_left(3);
            for arg in instruction.args() {
                checksum = checksum.rotate_left(7) ^ arg.to_bits();
            }
            for target in instruction.targets() {
                checksum = checksum.rotate_left(11) ^ dem_target_checksum(target);
            }
            checksum
        }
        DemItem::RepeatBlock(repeat) => {
            let tag_checksum = repeat.tag().map_or(0, |tag| tag.len() as u64);
            repeat.repeat_count().get()
                ^ tag_checksum.rotate_left(13)
                ^ dem_model_checksum(repeat.body())
        }
    }
}

fn dem_instruction_kind_checksum(kind: DemInstructionKind) -> u64 {
    match kind {
        DemInstructionKind::Error => 1,
        DemInstructionKind::Detector => 2,
        DemInstructionKind::LogicalObservable => 3,
        DemInstructionKind::ShiftDetectors => 4,
    }
}

fn dem_target_checksum(target: &DemTarget) -> u64 {
    match target {
        DemTarget::RelativeDetector(id) => 0x10 ^ id.get(),
        DemTarget::LogicalObservable(id) => 0x20 ^ id.get(),
        DemTarget::Separator => 0x30,
        DemTarget::Numeric(value) => 0x40 ^ *value,
    }
}

fn gate_alias_count() -> usize {
    Gate::all().map(|gate| gate.aliases().len()).sum()
}

fn gate_tableau_count() -> usize {
    Gate::all().filter(|gate| gate.has_tableau()).count()
}

fn gate_flow_count() -> usize {
    Gate::all()
        .filter_map(|gate| gate.flows().ok())
        .map(|flows| flows.len())
        .sum()
}

fn tableau_checksum(tableau: &Tableau, row_id: &str) -> Result<u64, BenchError> {
    let mut checksum = tableau.len() as u64;
    for index in 0..tableau.len() {
        let x_output = tableau
            .x_output(index)
            .map_err(|error| stab_runner_error(row_id, error))?;
        let z_output = tableau
            .z_output(index)
            .map_err(|error| stab_runner_error(row_id, error))?;
        checksum ^= pauli_string_checksum(x_output).rotate_left(3);
        checksum ^= pauli_string_checksum(z_output).rotate_left(7);
    }
    Ok(checksum)
}

fn pauli_string_checksum(pauli: &PauliString) -> u64 {
    let mut checksum = (pauli.len() as u64) ^ (u64::from(pauli.sign().is_negative()) << 1);
    for word in pauli.x_bits().iter().chain(pauli.z_bits()) {
        checksum = checksum.rotate_left(5) ^ *word;
    }
    checksum ^ (pauli.weight() as u64)
}

fn flow_checksum(flow: &Flow) -> u64 {
    let mut checksum = pauli_string_checksum(flow.input());
    checksum ^= pauli_string_checksum(flow.output()).rotate_left(7);
    for measurement in flow.measurements() {
        checksum = checksum.rotate_left(11) ^ u64::from(measurement.cast_unsigned());
    }
    for observable in flow.observables() {
        checksum = checksum.rotate_left(13) ^ u64::from(observable);
    }
    checksum
}

fn argument_rule_checksum(rule: GateArgumentRule) -> usize {
    match rule {
        GateArgumentRule::Exact(count) => count,
        GateArgumentRule::Any => 1,
        GateArgumentRule::OptionalProbability => 2,
        GateArgumentRule::ProbabilityList(count) => 3 ^ count,
        GateArgumentRule::AnyProbabilityList => 4,
        GateArgumentRule::UnsignedInteger => 5,
    }
}
