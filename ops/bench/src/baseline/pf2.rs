use std::hint::black_box;
use std::str::FromStr;

use stab_core::{Circuit, CircuitInstruction, CircuitItem, Flow, Target};

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
const TIME_REVERSE_FLOW_MEASUREMENT: &str = "MZZ 0 1\n";
const TIME_REVERSE_FLOW_MEASUREMENT_TEXTS: [&str; 4] = [
    "X0*X1 -> Y0*Y1 xor rec[-1]",
    "X0*X1 -> X0*X1",
    "Z0 -> Z1 xor rec[-1]",
    "Z0 -> Z0",
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
    let circuit = parse_circuit(&row.id, TIME_REVERSE_FLOW_MEASUREMENT)?;
    let flows = parse_flows(&row.id, TIME_REVERSE_FLOW_MEASUREMENT_TEXTS)?;
    Ok(vec![measure_stab_batched(
        "stab_circuit_time_reversed_for_flows_measurement",
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

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
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
        ("pf2-circuit-decompose-mpp-spp", "stab_circuit_decompose_mpp_spp") => {
            Some((8.0, "source-instructions/s"))
        }
        ("pf2-time-reverse-flow", "stab_circuit_time_reversed_for_flows_unitary") => {
            Some((TIME_REVERSE_FLOW_TEXTS.len() as f64, "flows/s"))
        }
        (
            "pf2-time-reverse-flow-measurement",
            "stab_circuit_time_reversed_for_flows_measurement",
        ) => Some((TIME_REVERSE_FLOW_MEASUREMENT_TEXTS.len() as f64, "flows/s")),
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
            "contract-only: Stab measures Rust Circuit::with_inlined_feedback on the scoped MPP feedback subset and selected bounded repeat-loop refolding case; pinned Stim has equivalent transform behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-circuit-decompose-mpp-spp" => Some(
            "contract-only: Stab measures Rust Circuit::decomposed over ISWAP, MPP, SPP, pair-measurement, noise, and annotation operations; pinned Stim has equivalent API behavior but no faithful Rust direct baseline in this harness",
        ),
        "pf2-time-reverse-flow" => Some(
            "contract-only: Stab measures the scoped Rust Circuit::time_reversed_for_flows unitary subset; broader measurement-rich QEC inverse rewrites remain active follow-up work and pinned Stim has no faithful Rust direct baseline in this harness",
        ),
        "pf2-time-reverse-flow-measurement" => Some(
            "contract-only: Stab measures the selected Rust Circuit::time_reversed_for_flows single measurement-rich instruction subset; broader QEC inverse rewrites remain active follow-up work and pinned Stim has no faithful Rust direct baseline in this harness",
        ),
        _ => None,
    }
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
