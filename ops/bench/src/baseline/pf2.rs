use std::hint::black_box;
use std::str::FromStr;

use stab_core::{
    Circuit, CircuitInstruction, CircuitItem, CodeDistance, Flow, RoundCount, SurfaceCodeParams,
    SurfaceCodeTask, Target, TimeReversedForFlowsOptions, generate_surface_code_circuit,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_batched, stab_runner_error};

#[cfg(not(test))]
const FLATTEN_REPETITIONS: u64 = 4096;
#[cfg(test)]
const FLATTEN_REPETITIONS: u64 = 2;
#[cfg(not(test))]
const WITHOUT_NOISE_GROUPS: usize = 2048;
#[cfg(test)]
const WITHOUT_NOISE_GROUPS: usize = 2;
#[cfg(not(test))]
const TRANSFORM_REPETITIONS: usize = 8;
#[cfg(test)]
const TRANSFORM_REPETITIONS: usize = 1;

const FLATTEN_OUTPUTS_PER_REPETITION: u64 = 3;
const FLATTEN_FIXED_OUTPUTS: u64 = 2;
const WITHOUT_NOISE_SOURCE_INSTRUCTIONS_PER_GROUP: usize = 5;
const FEEDBACK_INLINE_MPP: &str = "RX 0\n\
                                  RY 1\n\
                                  RZ 2\n\
                                  MPP X0*Y1*Z2 Z5\n\
                                  CX rec[-2] 3\n\
                                  M 3\n\
                                  DETECTOR rec[-1]\n";
const FEEDBACK_INLINE_REPEAT_LOOP: &str = "R 0 1\n\
                                          X_ERROR(0.125) 0 1\n\
                                          CX 0 1\n\
                                          M 1\n\
                                          CX rec[-1] 1\n\
                                          DETECTOR rec[-1]\n\
                                          REPEAT 30 {\n\
                                              X_ERROR(0.125) 0 1\n\
                                              CX 0 1\n\
                                              M 1\n\
                                              CX rec[-1] 1\n\
                                              DETECTOR rec[-1] rec[-2]\n\
                                          }\n\
                                          M 0\n\
                                          DETECTOR rec[-1] rec[-2]\n";
const FEEDBACK_INLINE_XCZ_YCZ: &str = "R 0 1 2\n\
                                      X_ERROR(0.125) 0\n\
                                      M 0\n\
                                      XCZ 1 rec[-1]\n\
                                      YCZ 2 rec[-1]\n\
                                      M 1 2\n\
                                      DETECTOR rec[-2]\n\
                                      DETECTOR rec[-1]\n";
const DECOMPOSE_MPP_SPP: &str = "ISWAP 0 1 2 1\n\
                                 TICK\n\
                                 MPP X1*Z2*Y3 X4*!X4\n\
                                 SPP Y0\n\
                                 SPP_DAG !Z5\n\
                                 MXX 6 7 6 8\n\
                                 X_ERROR(0.25) 0\n\
                                 DETECTOR rec[-1]\n";
const TIME_REVERSE_FLOW_UNITARY: &str = "H 2\n";
const TIME_REVERSE_FLOW_TEXTS: [&str; 4] = [
    "X300 -> X300",
    "X2*Z301 -> Z2*Z301",
    "Z2*X301 -> X2*X301",
    "Y2*Y301 -> Y2*Y301",
];
const TIME_REVERSE_FLOW_MEASUREMENT_MZZ_TEXTS: [&str; 4] = [
    "X0*X1 -> Y0*Y1 xor rec[-1]",
    "X0*X1 -> X0*X1",
    "Z0 -> Z1 xor rec[-1]",
    "Z0 -> Z0",
];
const TIME_REVERSE_FLOW_MEASUREMENT_MZZ_SUFFIX_TEXTS: [&str; 4] = [
    "X0*X1 -> X0*Z1 xor rec[-1]",
    "X0*X1 -> Z0*Y1",
    "Z0 -> Z0*Z1 xor rec[-1]",
    "Z0 -> X0*Y1",
];
const TIME_REVERSE_FLOW_MEASUREMENT_M_MULTI_TEXTS: [&str; 2] =
    ["1 -> Z0 xor rec[-2]", "1 -> Z1 xor rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MZZ_MULTI_TEXTS: [&str; 2] =
    ["1 -> Z0*Z1 xor rec[-2]", "1 -> Z2*Z3 xor rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_M_TEXTS: [&str; 1] = ["Z0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MX_TEXTS: [&str; 1] = ["X0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MY_TEXTS: [&str; 1] = ["Y0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_R_TEXTS: [&str; 1] = ["1 -> Z0"];
const TIME_REVERSE_FLOW_MEASUREMENT_RX_TEXTS: [&str; 1] = ["1 -> X0"];
const TIME_REVERSE_FLOW_MEASUREMENT_RY_TEXTS: [&str; 1] = ["1 -> Y0"];
const TIME_REVERSE_FLOW_MEASUREMENT_R_MULTI_TEXTS: [&str; 3] = ["1 -> Z0", "1 -> Z1", "1 -> Z0*Z1"];
const TIME_REVERSE_FLOW_MEASUREMENT_RX_MULTI_TEXTS: [&str; 1] = ["1 -> X0*X1"];
const TIME_REVERSE_FLOW_MEASUREMENT_RY_MULTI_TEXTS: [&str; 1] = ["1 -> Y0*Y1"];
const TIME_REVERSE_FLOW_MEASUREMENT_MR_TEXTS: [&str; 2] = ["1 -> Z0", "Z0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MRX_TEXTS: [&str; 2] = ["1 -> X0", "X0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MRY_TEXTS: [&str; 2] = ["1 -> Y0", "Y0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MR_MULTI_TEXTS: [&str; 5] = [
    "Z0 -> rec[-2]",
    "Z1 -> rec[-1]",
    "1 -> Z0",
    "1 -> Z1",
    "1 -> Z0*Z1",
];
const TIME_REVERSE_FLOW_MEASUREMENT_MRX_MULTI_TEXTS: [&str; 1] = ["1 -> X0*X1"];
const TIME_REVERSE_FLOW_MEASUREMENT_MRY_MULTI_TEXTS: [&str; 1] = ["1 -> Y0*Y1"];
const TIME_REVERSE_FLOW_MEASUREMENT_MR_INVERTED_TEXTS: [&str; 3] =
    ["Z0*Z1 -> rec[-2] xor rec[-1]", "1 -> Z0", "1 -> Z1"];
const TIME_REVERSE_FLOW_MEASUREMENT_MRX_INVERTED_TEXTS: [&str; 2] = ["1 -> X0", "X0 -> rec[-1]"];
const TIME_REVERSE_FLOW_MEASUREMENT_MRY_INVERTED_TEXTS: [&str; 2] =
    ["Y0*Y1 -> rec[-2] xor rec[-1]", "1 -> Y0*Y1"];
const TIME_REVERSE_FLOW_MEASUREMENT_FLOW_FLIP_TEXTS: [&str; 4] = [
    "Y0*Z1 -> rec[-3] xor rec[-1]",
    "1 -> Z0*Z1",
    "1 -> Z1",
    "1 -> Z0",
];
const TIME_REVERSE_FLOW_MPAD_MATRIX: &str = "H 0\n\
                                             MPAD 0 1\n\
                                             S 0\n\
                                             OBSERVABLE_INCLUDE(0) rec[-2]\n\
                                             OBSERVABLE_INCLUDE(0) rec[-1]\n";
const TIME_REVERSE_FLOW_MPAD_MATRIX_TEXTS: [&str; 7] = [
    "1 -> rec[1]",
    "1 -> -rec[0]",
    "X -> Z",
    "Z -> Y",
    "1 -> obs[0]",
    "1 -> rec[-2] xor obs[0]",
    "1 -> rec[-1] xor obs[0]",
];
const TIME_REVERSE_FLOW_GENERATED_SURFACE_CASES: [(&str, u64, u32, usize); 3] = [
    ("d3_r2", 2, 3, 66),
    ("d5_r2", 2, 5, 130),
    ("d7_r2", 2, 7, 226),
];
const TIME_REVERSE_FLOW_MPAD_SCALE_SIZES: [usize; 3] = [1, 8, 64];
const TIME_REVERSE_FLOW_SMALL_REPEAT_BODY: &str = "H 0\nS 0\nS_DAG 0\nH 0\n";
const TIME_REVERSE_FLOW_SMALL_REPEAT_BODY_OPERATIONS: usize = 4;
const TIME_REVERSE_FLOW_WIDE_REPEAT_QUBITS: u32 = 8;
const TIME_REVERSE_FLOW_WIDE_REPEAT_BODY_OPERATIONS: usize = 56;
const TIME_REVERSE_FLOW_REPEAT_COUNTS: [(&str, u64); 3] = [
    ("count_1", 1),
    ("count_1024", 1_024),
    ("count_1b", 1_000_000_000),
];
const TIME_REVERSE_FLOW_MEASUREMENT_CASES: [(&str, &[&str], bool); 24] = [
    ("MZZ 0 1\n", &TIME_REVERSE_FLOW_MEASUREMENT_MZZ_TEXTS, false),
    (
        "MZZ 0 1\nH 0\nCX 0 1\nS 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MZZ_SUFFIX_TEXTS,
        false,
    ),
    (
        "M 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_M_MULTI_TEXTS,
        false,
    ),
    (
        "MZZ 0 1 2 3\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MZZ_MULTI_TEXTS,
        false,
    ),
    ("M 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_M_TEXTS, false),
    ("M 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_M_TEXTS, true),
    ("MX 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_MX_TEXTS, false),
    ("MY 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_MY_TEXTS, false),
    ("R 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_R_TEXTS, false),
    ("RX 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_RX_TEXTS, false),
    ("RY 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_RY_TEXTS, false),
    (
        "R 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_R_MULTI_TEXTS,
        false,
    ),
    (
        "RX 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_RX_MULTI_TEXTS,
        false,
    ),
    (
        "RY 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_RY_MULTI_TEXTS,
        false,
    ),
    ("MR 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_MR_TEXTS, false),
    ("MRX 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_MRX_TEXTS, false),
    ("MRY 0\n", &TIME_REVERSE_FLOW_MEASUREMENT_MRY_TEXTS, false),
    (
        "MR 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MR_MULTI_TEXTS,
        false,
    ),
    (
        "MRX 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MRX_MULTI_TEXTS,
        false,
    ),
    (
        "MRY 0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MRY_MULTI_TEXTS,
        false,
    ),
    (
        "MR !0 1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MR_INVERTED_TEXTS,
        false,
    ),
    (
        "MRX !0\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MRX_INVERTED_TEXTS,
        false,
    ),
    (
        "MRY 0 !1\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_MRY_INVERTED_TEXTS,
        false,
    ),
    (
        "MY 0\nMRX 0\nMR 1\nR 0\n",
        &TIME_REVERSE_FLOW_MEASUREMENT_FLOW_FLIP_TEXTS,
        false,
    ),
];

pub(super) fn run_circuit_flatten_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, &flatten_repeat_fixture())?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_flatten_repeat_shifted_coords",
        TRANSFORM_REPETITIONS,
        || {
            let flattened = circuit
                .flattened()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(circuit_checksum(&flattened));
            Ok(())
        },
    )?])
}

pub(super) fn run_circuit_without_noise_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, &without_noise_fixture())?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_without_noise_top_level",
        TRANSFORM_REPETITIONS,
        || {
            let noiseless = circuit
                .without_noise()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(circuit_checksum(&noiseless));
            Ok(())
        },
    )?])
}

pub(super) fn run_feedback_inline_batch_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let mpp_circuit = parse_circuit(&row.id, FEEDBACK_INLINE_MPP)?;
    let repeat_loop_circuit = parse_circuit(&row.id, FEEDBACK_INLINE_REPEAT_LOOP)?;
    let xcz_ycz_circuit = parse_circuit(&row.id, FEEDBACK_INLINE_XCZ_YCZ)?;
    Ok(vec![
        measure_stab_batched(
            "stab_circuit_with_inlined_feedback_mpp",
            TRANSFORM_REPETITIONS,
            || {
                let inlined = mpp_circuit
                    .with_inlined_feedback()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(circuit_checksum(&inlined));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_with_inlined_feedback_repeat_loop",
            TRANSFORM_REPETITIONS,
            || {
                let inlined = repeat_loop_circuit
                    .with_inlined_feedback()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(circuit_checksum(&inlined));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_with_inlined_feedback_xcz_ycz",
            TRANSFORM_REPETITIONS,
            || {
                let inlined = xcz_ycz_circuit
                    .with_inlined_feedback()
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(circuit_checksum(&inlined));
                Ok(())
            },
        )?,
    ])
}

pub(super) fn run_circuit_decompose_mpp_spp_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, DECOMPOSE_MPP_SPP)?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_decompose_mpp_spp",
        TRANSFORM_REPETITIONS,
        || {
            let decomposed = circuit
                .decomposed()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(circuit_checksum(&decomposed));
            Ok(())
        },
    )?])
}

pub(super) fn run_time_reverse_flow_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, TIME_REVERSE_FLOW_UNITARY)?;
    let flows = parse_flows(&row.id, TIME_REVERSE_FLOW_TEXTS)?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_time_reversed_for_flows_unitary",
        TRANSFORM_REPETITIONS,
        || {
            let (reversed, reversed_flows) = circuit
                .time_reversed_for_flows(&flows)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box((circuit_checksum(&reversed), reversed_flows.len()));
            Ok(())
        },
    )?])
}

pub(super) fn run_time_reverse_flow_measurement_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let cases = parse_time_reverse_measurement_cases(&row.id)?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_time_reversed_for_flows_measurement",
        TRANSFORM_REPETITIONS,
        || {
            let mut checksum = 0_u64;
            let mut reversed_flow_count = 0_usize;
            for (circuit, flows, options) in &cases {
                let (reversed, reversed_flows) = circuit
                    .time_reversed_for_flows_with_options(flows, *options)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= circuit_checksum(&reversed);
                reversed_flow_count += reversed_flows.len();
            }
            black_box((checksum, reversed_flow_count));
            Ok(())
        },
    )?])
}

pub(super) fn run_time_reverse_flow_generated_surface_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let mut measurements = Vec::with_capacity(TIME_REVERSE_FLOW_GENERATED_SURFACE_CASES.len());
    for (suffix, rounds, distance, expected_operations) in TIME_REVERSE_FLOW_GENERATED_SURFACE_CASES
    {
        let params = SurfaceCodeParams::new(
            RoundCount::try_new(rounds).map_err(|error| stab_runner_error(&row.id, error))?,
            CodeDistance::try_new(distance).map_err(|error| stab_runner_error(&row.id, error))?,
            SurfaceCodeTask::RotatedMemoryX,
        )
        .map_err(|error| stab_runner_error(&row.id, error))?;
        let generated = generate_surface_code_circuit(&params)
            .map_err(|error| stab_runner_error(&row.id, error))?;
        let circuit = generated.circuit().clone();
        require_no_repeat_blocks(row, &circuit, &format!("generated-surface {suffix}"))?;
        require_time_reverse_work(
            row,
            &format!("generated-surface {suffix} compact source instructions"),
            compact_instruction_count(&circuit),
            expected_operations,
        )?;
        measurements.push(measure_stab_batched(
            &format!("stab_circuit_time_reversed_for_flows_generated_surface_{suffix}"),
            TRANSFORM_REPETITIONS,
            || {
                let (reversed, reversed_flows) = circuit
                    .time_reversed_for_flows(&[])
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((circuit_checksum(&reversed), reversed_flows.len()));
                Ok(())
            },
        )?);
    }
    Ok(measurements)
}

pub(super) fn run_time_reverse_flow_mpad_matrix_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let circuit = parse_circuit(&row.id, TIME_REVERSE_FLOW_MPAD_MATRIX)?;
    let flows = parse_flows(&row.id, TIME_REVERSE_FLOW_MPAD_MATRIX_TEXTS)?;
    let mut measurements = vec![measure_stab_batched(
        "stab_circuit_time_reversed_for_flows_mpad_matrix",
        TRANSFORM_REPETITIONS,
        || {
            let (reversed, reversed_flows) = circuit
                .time_reversed_for_flows(&flows)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box((circuit_checksum(&reversed), reversed_flows.len()));
            Ok(())
        },
    )?];
    for size in TIME_REVERSE_FLOW_MPAD_SCALE_SIZES {
        let (circuit, flows) = mpad_scaling_case(&row.id, size)?;
        measurements.push(measure_stab_batched(
            &format!("stab_circuit_time_reversed_for_flows_mpad_scale_{size}"),
            TRANSFORM_REPETITIONS,
            || {
                let (reversed, reversed_flows) = circuit
                    .time_reversed_for_flows(&flows)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((circuit_checksum(&reversed), reversed_flows.len()));
                Ok(())
            },
        )?);
    }
    Ok(measurements)
}

pub(super) fn run_time_reverse_flow_large_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let small_flows = parse_flows(&row.id, ["X -> X"])?;
    let mut measurements = Vec::with_capacity(TIME_REVERSE_FLOW_REPEAT_COUNTS.len() + 1);
    for (suffix, repeat_count) in TIME_REVERSE_FLOW_REPEAT_COUNTS {
        let circuit = time_reverse_repeat_circuit(
            row,
            repeat_count,
            TIME_REVERSE_FLOW_SMALL_REPEAT_BODY,
            TIME_REVERSE_FLOW_SMALL_REPEAT_BODY_OPERATIONS,
        )?;
        measurements.push(measure_stab_batched(
            &format!("stab_circuit_time_reversed_for_flows_unitary_repeat_{suffix}"),
            TRANSFORM_REPETITIONS,
            || {
                let (reversed, reversed_flows) = circuit
                    .time_reversed_for_flows(&small_flows)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((circuit_checksum(&reversed), reversed_flows.len()));
                Ok(())
            },
        )?);
    }

    let wide_body = wide_identity_repeat_body();
    let wide_circuit = time_reverse_repeat_circuit(
        row,
        1_000_000_000,
        &wide_body,
        TIME_REVERSE_FLOW_WIDE_REPEAT_BODY_OPERATIONS,
    )?;
    let wide_flows = parse_flows(&row.id, ["XXXXXXXX -> XXXXXXXX"])?;
    measurements.push(measure_stab_batched(
        "stab_circuit_time_reversed_for_flows_unitary_repeat_wide_body_1b",
        TRANSFORM_REPETITIONS,
        || {
            let (reversed, reversed_flows) = wide_circuit
                .time_reversed_for_flows(&wide_flows)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box((circuit_checksum(&reversed), reversed_flows.len()));
            Ok(())
        },
    )?);
    Ok(measurements)
}

pub(super) fn run_time_reverse_flow_sparse_high_qubit_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let low = parse_circuit(&row.id, "H 0\n")?;
    let high = parse_circuit(&row.id, "H 1000000\n")?;
    let flows = parse_flows(&row.id, ["Z1 -> Z1"])?;
    Ok(vec![
        measure_stab_batched(
            "stab_circuit_time_reversed_for_flows_sparse_qubit_0",
            TRANSFORM_REPETITIONS,
            || {
                let (reversed, reversed_flows) = low
                    .time_reversed_for_flows(&flows)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((circuit_checksum(&reversed), reversed_flows.len()));
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_circuit_time_reversed_for_flows_sparse_qubit_1000000",
            TRANSFORM_REPETITIONS,
            || {
                let (reversed, reversed_flows) = high
                    .time_reversed_for_flows(&flows)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box((circuit_checksum(&reversed), reversed_flows.len()));
                Ok(())
            },
        )?,
    ])
}

fn time_reverse_repeat_circuit(
    row: &BenchmarkRow,
    repeat_count: u64,
    body: &str,
    expected_body_operations: usize,
) -> Result<Circuit, BenchError> {
    let circuit = parse_circuit(&row.id, &format!("REPEAT {repeat_count} {{\n{body}}}\n"))?;
    let [CircuitItem::RepeatBlock(repeat)] = circuit.items() else {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: "unitary-repeat fixture did not parse as one repeat block".to_string(),
        });
    };
    if repeat.repeat_count().get() != repeat_count {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "unitary-repeat count drifted: expected {repeat_count}, got {}",
                repeat.repeat_count().get()
            ),
        });
    }
    require_time_reverse_work(
        row,
        "unitary-repeat body operations",
        repeat.body().iter_flattened_instructions().count(),
        expected_body_operations,
    )?;
    Ok(circuit)
}

fn mpad_scaling_case(row_id: &str, size: usize) -> Result<(Circuit, Vec<Flow>), BenchError> {
    let mut circuit_text = String::from("MPAD");
    for _ in 0..size {
        circuit_text.push_str(" 1");
    }
    circuit_text.push('\n');
    let circuit = parse_circuit(row_id, &circuit_text)?;
    let flows = (0..size)
        .map(|index| {
            Flow::from_str(&format!("1 -> rec[{index}]"))
                .map_err(|error| stab_runner_error(row_id, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((circuit, flows))
}

fn compact_instruction_count(circuit: &Circuit) -> usize {
    circuit
        .items()
        .iter()
        .map(|item| match item {
            CircuitItem::Instruction(_) => 1,
            CircuitItem::RepeatBlock(repeat) => compact_instruction_count(repeat.body()),
        })
        .sum()
}

fn require_no_repeat_blocks(
    row: &BenchmarkRow,
    circuit: &Circuit,
    label: &str,
) -> Result<(), BenchError> {
    let has_repeat = circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(_) => false,
        CircuitItem::RepeatBlock(_) => true,
    });
    if has_repeat {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "{label} unexpectedly contains a repeat block; the generated-surface resource row requires compact source work without repeat-count expansion"
            ),
        });
    }
    Ok(())
}

fn wide_identity_repeat_body() -> String {
    let mut body = String::new();
    for qubit in 0..TIME_REVERSE_FLOW_WIDE_REPEAT_QUBITS {
        let gates = if qubit & 1 == 0 {
            ["X", "H", "Z", "Y", "Y", "Z", "H", "X"]
        } else {
            ["H", "X", "Z", "Y", "Y", "Z", "X", "H"]
        };
        for gate in gates {
            body.push_str(gate);
            body.push(' ');
            body.push_str(&qubit.to_string());
            body.push('\n');
        }
    }
    body
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if row_id == "pfm-b1-time-reverse-generated-surface"
        && let Some(suffix) =
            name.strip_prefix("stab_circuit_time_reversed_for_flows_generated_surface_")
    {
        return TIME_REVERSE_FLOW_GENERATED_SURFACE_CASES
            .iter()
            .find(|(candidate, _, _, _)| *candidate == suffix)
            .map(|(_, _, _, operations)| (*operations as f64, "source-instructions/s"));
    }
    match (row_id, name) {
        ("pf2-circuit-flatten-repeat", "stab_circuit_flatten_repeat_shifted_coords") => {
            let operations =
                FLATTEN_FIXED_OUTPUTS + FLATTEN_REPETITIONS * FLATTEN_OUTPUTS_PER_REPETITION;
            Some((operations as f64, "operations/s"))
        }
        ("pf2-circuit-without-noise", "stab_circuit_without_noise_top_level") => Some((
            (WITHOUT_NOISE_GROUPS * WITHOUT_NOISE_SOURCE_INSTRUCTIONS_PER_GROUP) as f64,
            "source-instructions/s",
        )),
        ("pf2-feedback-inline-batch", "stab_circuit_with_inlined_feedback_mpp") => {
            Some((1.0, "transforms/s"))
        }
        ("pf2-feedback-inline-batch", "stab_circuit_with_inlined_feedback_repeat_loop") => {
            Some((30.0, "repeat-iterations/s"))
        }
        ("pf2-feedback-inline-batch", "stab_circuit_with_inlined_feedback_xcz_ycz") => {
            Some((1.0, "transforms/s"))
        }
        ("pf2-circuit-decompose-mpp-spp", "stab_circuit_decompose_mpp_spp") => {
            Some((8.0, "source-instructions/s"))
        }
        ("pf2-time-reverse-flow", "stab_circuit_time_reversed_for_flows_unitary") => {
            Some((TIME_REVERSE_FLOW_TEXTS.len() as f64, "flows/s"))
        }
        (
            "pf2-time-reverse-flow-measurement",
            "stab_circuit_time_reversed_for_flows_measurement",
        ) => Some((time_reverse_measurement_flow_count() as f64, "flows/s")),
        ("pfm-b1-time-reverse-mpad-matrix", "stab_circuit_time_reversed_for_flows_mpad_matrix") => {
            Some((TIME_REVERSE_FLOW_MPAD_MATRIX_TEXTS.len() as f64, "flows/s"))
        }
        ("pfm-b1-time-reverse-mpad-matrix", name)
            if name.starts_with("stab_circuit_time_reversed_for_flows_mpad_scale_") =>
        {
            name.rsplit('_')
                .next()
                .and_then(|size| size.parse::<usize>().ok())
                .filter(|size| TIME_REVERSE_FLOW_MPAD_SCALE_SIZES.contains(size))
                .map(|size| (size as f64, "flows/s"))
        }
        (
            "pfm-b1-time-reverse-large-unitary-repeat",
            "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1"
            | "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1024"
            | "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1b"
            | "stab_circuit_time_reversed_for_flows_unitary_repeat_wide_body_1b",
        ) => Some((1.0, "transforms/s")),
        (
            "pfm-b1-time-reverse-sparse-high-qubit",
            "stab_circuit_time_reversed_for_flows_sparse_qubit_0"
            | "stab_circuit_time_reversed_for_flows_sparse_qubit_1000000",
        ) => Some((1.0, "transforms/s")),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf2-circuit-flatten-repeat" => Some(
            "contract-only: Stab measures Rust Circuit::flattened repeat unrolling and coordinate-shift application; pinned Stim has equivalent API behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-circuit-without-noise" => Some(
            "contract-only: Stab measures Rust Circuit::without_noise over top-level noisy, heralded, measurement, detector, and annotation instructions; pinned Stim has equivalent API behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-feedback-inline-batch" => Some(
            "contract-only: Stab measures Rust Circuit::with_inlined_feedback on the scoped MPP feedback subset, selected bounded repeat-loop refolding case, and selected XCZ/YCZ feedback case; pinned Stim has equivalent transform behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-circuit-decompose-mpp-spp" => Some(
            "contract-only: Stab measures Rust Circuit::decomposed over ISWAP, MPP, SPP, pair-measurement, noise, and annotation operations; pinned Stim has equivalent API behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-time-reverse-flow" => Some(
            "contract-only: Stab measures the scoped Rust Circuit::time_reversed_for_flows unitary subset; broader measurement-rich QEC inverse rewrites remain active follow-up work and pinned Stim has no faithful Rust direct baseline in this harness",
        ),
        "pf2-time-reverse-flow-measurement" => Some(
            "contract-only: Stab measures the historical pinned measurement-rich Rust Circuit::time_reversed_for_flows corpus through the shared gate-family engine; pinned Stim has no faithful in-process Rust baseline in this harness",
        ),
        "pfm-b1-time-reverse-generated-surface" => Some(
            "contract-only: Stab measures a no-repeat distance matrix of rotated-memory-X reverse-flow transforms with fixture generation, repeat absence, and literal compact source-instruction validation outside each sample; pinned Stim has no faithful in-process Rust baseline in this harness",
        ),
        "pfm-b1-time-reverse-mpad-matrix" => Some(
            "contract-only: Stab measures the seven-flow MPAD semantic matrix plus 1, 8, and 64 independent MPAD record-flow scaling points; pinned Stim has no faithful in-process Rust baseline in this harness",
        ),
        "pfm-b1-time-reverse-large-unitary-repeat" => Some(
            "contract-only: Stab measures one transform per sample across repeat-count and repeat-body/state-size matrices; repeat count and compact body work are validated outside timing and no expanded-operation throughput is claimed; pinned Stim has no faithful in-process Rust baseline in this harness",
        ),
        "pfm-b1-time-reverse-sparse-high-qubit" => Some(
            "contract-only: Stab compares otherwise identical unitary reversals with a nonempty low-width validation flow at low and million-scale maximum qubit ids to guard allocation against maximum-index amplification; pinned Stim has no faithful in-process Rust baseline in this harness",
        ),
        _ => None,
    }
}

fn require_time_reverse_work(
    row: &BenchmarkRow,
    label: &str,
    actual: usize,
    expected: usize,
) -> Result<(), BenchError> {
    if actual == expected {
        return Ok(());
    }
    Err(BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!("{label} drifted: expected {expected}, got {actual}"),
    })
}

fn flatten_repeat_fixture() -> String {
    format!(
        "\
SHIFT_COORDS(5, 0)
QUBIT_COORDS(1, 2, 3) 0
REPEAT {FLATTEN_REPETITIONS} {{
    MR 0 1
    DETECTOR(0, 0) rec[-2]
    DETECTOR(1, 0) rec[-1]
    SHIFT_COORDS(0, 1)
}}
OBSERVABLE_INCLUDE(2) rec[-1]
"
    )
}

fn without_noise_fixture() -> String {
    let mut text = String::new();
    for qubit in 0..WITHOUT_NOISE_GROUPS {
        text.push_str(&format!(
            "\
H {qubit}
X_ERROR(0.1) {qubit}
M(0.05) {qubit}
HERALDED_ERASE(0.01) {qubit}
DETECTOR rec[-1]
"
        ));
    }
    text
}

fn parse_circuit(row_id: &str, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(row_id, error))
}

fn parse_flows<const N: usize>(row_id: &str, texts: [&str; N]) -> Result<Vec<Flow>, BenchError> {
    texts
        .into_iter()
        .map(|text| Flow::from_str(text).map_err(|error| stab_runner_error(row_id, error)))
        .collect()
}

fn parse_flow_slice(row_id: &str, texts: &[&str]) -> Result<Vec<Flow>, BenchError> {
    texts
        .iter()
        .map(|text| Flow::from_str(text).map_err(|error| stab_runner_error(row_id, error)))
        .collect()
}

fn parse_time_reverse_measurement_cases(
    row_id: &str,
) -> Result<Vec<(Circuit, Vec<Flow>, TimeReversedForFlowsOptions)>, BenchError> {
    TIME_REVERSE_FLOW_MEASUREMENT_CASES
        .iter()
        .map(
            |(circuit_text, flow_texts, dont_turn_measurements_into_resets)| {
                Ok((
                    parse_circuit(row_id, circuit_text)?,
                    parse_flow_slice(row_id, flow_texts)?,
                    TimeReversedForFlowsOptions {
                        dont_turn_measurements_into_resets: *dont_turn_measurements_into_resets,
                    },
                ))
            },
        )
        .collect()
}

fn time_reverse_measurement_flow_count() -> usize {
    TIME_REVERSE_FLOW_MEASUREMENT_CASES
        .iter()
        .map(|(_, flows, _)| flows.len())
        .sum()
}

fn circuit_checksum(circuit: &Circuit) -> u64 {
    circuit
        .items()
        .iter()
        .fold(circuit.items().len() as u64, |checksum, item| {
            checksum.rotate_left(5) ^ circuit_item_checksum(item)
        })
}

fn circuit_item_checksum(item: &CircuitItem) -> u64 {
    match item {
        CircuitItem::Instruction(instruction) => circuit_instruction_checksum(instruction),
        CircuitItem::RepeatBlock(repeat) => {
            repeat.repeat_count().get()
                ^ repeat
                    .tag()
                    .map_or(0, |tag| tag.len() as u64)
                    .rotate_left(7)
                ^ circuit_checksum(repeat.body()).rotate_left(13)
        }
    }
}

fn circuit_instruction_checksum(instruction: &CircuitInstruction) -> u64 {
    let mut checksum = instruction.gate().canonical_name().len() as u64;
    checksum ^= instruction
        .tag()
        .map_or(0, |tag| tag.len() as u64)
        .rotate_left(3);
    for arg in instruction.args() {
        checksum = checksum.rotate_left(7) ^ arg.to_bits();
    }
    for target in instruction.targets() {
        checksum = checksum.rotate_left(11) ^ target_checksum(target);
    }
    checksum
}

fn target_checksum(target: &Target) -> u64 {
    match target {
        Target::Qubit { id, inverted } => u64::from(id.get()) ^ u64::from(*inverted).rotate_left(1),
        Target::MeasurementRecord { offset } => {
            i64::from(offset.get()).cast_unsigned().rotate_left(3)
        }
        Target::SweepBit { id } => u64::from(*id).rotate_left(5),
        Target::Pauli {
            pauli,
            id,
            inverted,
        } => {
            let pauli_bits = match pauli {
                stab_core::Pauli::X => 1,
                stab_core::Pauli::Y => 2,
                stab_core::Pauli::Z => 3,
            };
            pauli_bits ^ u64::from(id.get()).rotate_left(7) ^ u64::from(*inverted).rotate_left(9)
        }
        Target::Combiner => 17,
    }
}
