use std::hint::black_box;

use stab_core::{
    CodeDistance, DemDetectorId, DemTarget, DetectingRegionOptions, DetectingRegionTargetOptions,
    RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    all_detecting_region_targets, all_detecting_region_ticks, circuit_detecting_regions,
    circuit_detecting_regions_for_targets, generate_repetition_code_circuit,
    generate_surface_code_circuit,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{parse_circuit, stab_runner_error};

#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
const DETECTING_REGIONS_PER_CASE: usize = 2;
const DETECTING_REGIONS_CLIFFORD_CASES: usize = 3;
const DETECTING_REGIONS_SIMPLE: &str = "H 0\n\
                                        TICK\n\
                                        CX 0 1\n\
                                        TICK\n\
                                        MXX 0 1\n\
                                        DETECTOR rec[-1]\n";
const DETECTING_REGIONS_REPEAT: &str = "H 0\n\
                                        REPEAT 2 {\n\
                                            TICK\n\
                                        }\n\
                                        CX 0 1\n\
                                        TICK\n\
                                        MXX 0 1\n\
                                        DETECTOR rec[-1]\n";
const DETECTING_REGIONS_TARGETS: &str = "R 0\n\
                                         TICK\n\
                                         M 0\n\
                                         DETECTOR rec[-1]\n\
                                         OBSERVABLE_INCLUDE(0) rec[-1]\n\
                                         TICK\n\
                                         OBSERVABLE_INCLUDE(1) Z1\n";
const DETECTING_REGIONS_CLIFFORD: &str = "RX 0\n\
                                          TICK\n\
                                          SQRT_X 0\n\
                                          TICK\n\
                                          MX 0\n\
                                          DETECTOR rec[-1]\n\
                                          RX 1\n\
                                          TICK\n\
                                          H_YZ 1\n\
                                          TICK\n\
                                          MX 1\n\
                                          DETECTOR rec[-1]\n\
                                          RY 2\n\
                                          TICK\n\
                                          C_ZYX 2\n\
                                          TICK\n\
                                          MX 2\n\
                                          DETECTOR rec[-1]\n\
                                          R 3\n\
                                          TICK\n\
                                          C_NXYZ 3\n\
                                          TICK\n\
                                          MX 3\n\
                                          DETECTOR rec[-1]\n\
                                          R 4 5\n\
                                          TICK\n\
                                          H 4\n\
                                          CZ 4 5\n\
                                          TICK\n\
                                          MX 4\n\
                                          DETECTOR rec[-1]\n";
const DETECTING_REGIONS_CY: &str = "RX 0\n\
                                    RY 1\n\
                                    TICK\n\
                                    CY 0 1\n\
                                    TICK\n\
                                    MX 0\n\
                                    DETECTOR rec[-1]\n";
const DETECTING_REGIONS_FIXED_TWO_QUBIT_CLIFFORD: &str = "R 0\n\
                                                          RX 1\n\
                                                          TICK\n\
                                                          XCX 0 1\n\
                                                          M 0\n\
                                                          DETECTOR rec[-1]\n\
                                                          R 2 3\n\
                                                          TICK\n\
                                                          SWAP 2 3\n\
                                                          M 2\n\
                                                          DETECTOR rec[-1]\n\
                                                          RY 4\n\
                                                          RX 5\n\
                                                          TICK\n\
                                                          SQRT_XX 4 5\n\
                                                          M 4\n\
                                                          DETECTOR rec[-1]\n";

pub(super) fn run_basic_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_basic(row, "stab_detecting_regions_basic_cases")?,
        measure_basic(row, "stab_detecting_regions_basic_regions")?,
    ])
}

pub(super) fn run_repeat_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_REPEAT)?;
    let detector = DemDetectorId::try_new(0).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![super::measure_stab_iterations(
        "stab_pf5_detecting_regions_repeat_ticks",
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions(
                    &circuit,
                    DetectingRegionOptions {
                        detectors: vec![detector],
                        ticks: vec![0, 1, 2],
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.get(&detector).map_or(0, |regions| regions.len()))
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions repeat benchmark region count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
}

pub(super) fn run_targets_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_TARGETS)?;
    let targets = all_detecting_region_targets(&circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let ticks =
        all_detecting_region_ticks(&circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![super::measure_stab_iterations(
        "stab_pf5_detecting_regions_target_filters",
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions_for_targets(
                    &circuit,
                    DetectingRegionTargetOptions {
                        targets: targets.clone(),
                        ticks: ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.values().map(|regions| regions.len()).sum::<usize>())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions target benchmark region count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
}

pub(super) fn run_clifford_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_CLIFFORD)?;
    let cy_circuit = parse_circuit(&row.id, DETECTING_REGIONS_CY)?;
    let fixed_two_qubit_circuit =
        parse_circuit(&row.id, DETECTING_REGIONS_FIXED_TWO_QUBIT_CLIFFORD)?;
    let targets = all_detecting_region_targets(&circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let ticks =
        all_detecting_region_ticks(&circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    let cy_targets = all_detecting_region_targets(&cy_circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let cy_ticks = all_detecting_region_ticks(&cy_circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let fixed_two_qubit_targets = all_detecting_region_targets(&fixed_two_qubit_circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let fixed_two_qubit_ticks = all_detecting_region_ticks(&fixed_two_qubit_circuit)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![super::measure_stab_iterations(
        "stab_pf5_detecting_regions_clifford_gates",
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions_for_targets(
                    &circuit,
                    DetectingRegionTargetOptions {
                        targets: targets.clone(),
                        ticks: ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                let cy_output = circuit_detecting_regions_for_targets(
                    &cy_circuit,
                    DetectingRegionTargetOptions {
                        targets: cy_targets.clone(),
                        ticks: cy_ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                let fixed_two_qubit_output = circuit_detecting_regions_for_targets(
                    &fixed_two_qubit_circuit,
                    DetectingRegionTargetOptions {
                        targets: fixed_two_qubit_targets.clone(),
                        ticks: fixed_two_qubit_ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.values().map(|regions| regions.len()).sum::<usize>())
                    .and_then(|count| {
                        count.checked_add(
                            cy_output
                                .values()
                                .map(|regions| regions.len())
                                .sum::<usize>(),
                        )
                    })
                    .and_then(|count| {
                        count.checked_add(
                            fixed_two_qubit_output
                                .values()
                                .map(|regions| regions.len())
                                .sum::<usize>(),
                        )
                    })
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions Clifford benchmark region count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
}

pub(super) fn run_generated_repetition_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(3).map_err(|error| stab_runner_error(&row.id, error))?,
        CodeDistance::try_new(3).map_err(|error| stab_runner_error(&row.id, error))?,
        RepetitionCodeTask::Memory,
    )
    .map_err(|error| stab_runner_error(&row.id, error))?;
    let generated = generate_repetition_code_circuit(&params)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let circuit = generated.circuit();
    let targets =
        all_detecting_region_targets(circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    let ticks =
        all_detecting_region_ticks(circuit).map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(vec![super::measure_stab_iterations(
        "stab_pf5_detecting_regions_generated_repetition",
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions_for_targets(
                    circuit,
                    DetectingRegionTargetOptions {
                        targets: targets.clone(),
                        ticks: ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.values().map(|regions| regions.len()).sum::<usize>())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions generated benchmark region count overflowed"
                            .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
}

pub(super) fn run_generated_surface_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3).map_err(|error| stab_runner_error(&row.id, error))?,
        CodeDistance::try_new(3).map_err(|error| stab_runner_error(&row.id, error))?,
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .map_err(|error| stab_runner_error(&row.id, error))?;
    let generated = generate_surface_code_circuit(&params)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let circuit = generated.circuit();
    let targets = vec![
        DemTarget::relative_detector(0).map_err(|error| stab_runner_error(&row.id, error))?,
        DemTarget::relative_detector(4).map_err(|error| stab_runner_error(&row.id, error))?,
        DemTarget::logical_observable(0).map_err(|error| stab_runner_error(&row.id, error))?,
    ];
    let ticks = vec![0, 1, 2, 3, 4, 5];
    Ok(vec![super::measure_stab_iterations(
        "stab_pf5_detecting_regions_generated_surface",
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions_for_targets(
                    circuit,
                    DetectingRegionTargetOptions {
                        targets: targets.clone(),
                        ticks: ticks.clone(),
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                regions = regions
                    .checked_add(output.values().map(|regions| regions.len()).sum::<usize>())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message:
                            "detecting-regions generated surface benchmark region count overflowed"
                                .to_string(),
                    })?;
            }
            black_box(regions);
            Ok(())
        },
    )?])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_cases") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("m9-detecting-regions-basic-batch", "stab_detecting_regions_basic_regions") => Some((
            (UTILITY_BATCH * DETECTING_REGIONS_PER_CASE) as f64,
            "regions/s",
        )),
        ("pf5-detecting-regions-repeat", "stab_pf5_detecting_regions_repeat_ticks") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("pf5-detecting-regions-targets", "stab_pf5_detecting_regions_target_filters") => {
            Some((UTILITY_BATCH as f64, "cases/s"))
        }
        ("pf5-detecting-regions-clifford", "stab_pf5_detecting_regions_clifford_gates") => Some((
            (UTILITY_BATCH * DETECTING_REGIONS_CLIFFORD_CASES) as f64,
            "cases/s",
        )),
        (
            "pf5-detecting-regions-generated-repetition",
            "stab_pf5_detecting_regions_generated_repetition",
        ) => Some((UTILITY_BATCH as f64, "cases/s")),
        (
            "pf5-detecting-regions-generated-surface",
            "stab_pf5_detecting_regions_generated_surface",
        ) => Some((UTILITY_BATCH as f64, "cases/s")),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m9-detecting-regions-basic-batch" => Some(
            "report-only: Stab measures the Rust detecting-regions utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-repeat" => Some(
            "report-only: Stab measures bounded repeat traversal in the Rust detecting-regions utility without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-targets" => Some(
            "report-only: Stab measures detector and logical-observable target filters in the Rust detecting-regions utility without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-clifford" => Some(
            "report-only: Stab measures representative single-qubit and fixed two-qubit Clifford propagation in the Rust detecting-regions utility without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-generated-repetition" => Some(
            "report-only: Stab measures generated repetition-code detecting regions in the Rust utility without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-detecting-regions-generated-surface" => Some(
            "report-only: Stab measures selected generated rotated surface-code detecting regions in the Rust utility without a faithful pinned Stim CLI timing ratio",
        ),
        _ => None,
    }
}

fn measure_basic(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let circuit = parse_circuit(&row.id, DETECTING_REGIONS_SIMPLE)?;
    let detector = DemDetectorId::try_new(0).map_err(|error| stab_runner_error(&row.id, error))?;
    super::measure_stab_iterations(
        measurement_name,
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut regions = 0usize;
            for _ in 0..UTILITY_BATCH {
                let output = circuit_detecting_regions(
                    &circuit,
                    DetectingRegionOptions {
                        detectors: vec![detector],
                        ticks: vec![0, 1],
                        ignore_anticommutation_errors: false,
                    },
                )
                .map_err(|error| stab_runner_error(&row.id, error))?;
                let detector_regions =
                    output
                        .get(&detector)
                        .ok_or_else(|| BenchError::StabRunner {
                            row_id: row.id.clone(),
                            message: "detecting-regions benchmark output omitted detector D0"
                                .to_string(),
                        })?;
                regions = regions.checked_add(detector_regions.len()).ok_or_else(|| {
                    BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "detecting-regions benchmark region count overflowed".to_string(),
                    }
                })?;
            }
            black_box(regions);
            Ok(())
        },
    )
}
