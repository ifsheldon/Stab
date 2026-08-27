use std::hint::black_box;

use stab_engine::{DemSamplingCompiler, DemSamplingPlan, RandomPolicy, Seed, ShotCount};
use stab_model::DetectorErrorModel;

use crate::baseline::batch_sinks::DemDigestSink;
use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{TRANSFORM_REPETITIONS, measure_stab_batched, stab_runner_error};

#[cfg(not(test))]
const SAMPLER_REPEAT_COUNT: u64 = 4096;
#[cfg(test)]
const SAMPLER_REPEAT_COUNT: u64 = 2;
#[cfg(not(test))]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 1_000_000;
#[cfg(test)]
const SAMPLER_NO_OP_REPEAT_COUNT: u64 = 64;
#[cfg(not(test))]
const SAMPLER_DETERMINISTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_DETERMINISTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT: u64 = 64_000_001;
#[cfg(test)]
const SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT: u64 = 65;
#[cfg(not(test))]
const SAMPLER_SHOTS: usize = 64;
#[cfg(test)]
const SAMPLER_SHOTS: usize = 2;

const SAMPLER_NESTED_STOCHASTIC_ERRORS_PER_REPETITION: u64 = 7;

pub(super) fn run_dem_sampler_repeat_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let fixture = sampler_repeat_fixture();
    let model = DetectorErrorModel::from_dem_str(&fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let plan = compile_plan(row, &model)?;
    let no_op_fixture = sampler_no_op_repeat_fixture();
    let no_op_model = DetectorErrorModel::from_dem_str(&no_op_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let no_op_plan = compile_plan(row, &no_op_model)?;
    let deterministic_fixture = sampler_deterministic_repeat_fixture();
    let deterministic_model = DetectorErrorModel::from_dem_str(&deterministic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let deterministic_plan = compile_plan(row, &deterministic_model)?;
    let single_stochastic_fixture = sampler_single_stochastic_repeat_fixture();
    let single_stochastic_model = DetectorErrorModel::from_dem_str(&single_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let single_stochastic_plan = compile_plan(row, &single_stochastic_model)?;
    let flat_stochastic_fixture = sampler_flat_stochastic_repeat_fixture();
    let flat_stochastic_model = DetectorErrorModel::from_dem_str(&flat_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let flat_stochastic_plan = compile_plan(row, &flat_stochastic_model)?;
    let nested_stochastic_fixture = sampler_nested_stochastic_repeat_fixture();
    let nested_stochastic_model = DetectorErrorModel::from_dem_str(&nested_stochastic_fixture)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let nested_stochastic_plan = compile_plan(row, &nested_stochastic_model)?;

    Ok(vec![
        measure_stab_batched(
            "stab_pf4_dem_sampler_compile_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                let compiled = DemSamplingCompiler::new()
                    .compile(&model)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(compiled.error_count());
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &plan)?);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_zero_probability_folded_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &no_op_plan)?);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_deterministic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &deterministic_plan)?);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &single_stochastic_plan)?);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &flat_stochastic_plan)?);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat",
            TRANSFORM_REPETITIONS,
            || {
                black_box(sample_witness(row, &nested_stochastic_plan)?);
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pf4-dem-sampler-folded-repeat", "stab_pf4_dem_sampler_compile_folded_repeat") => {
            Some((SAMPLER_REPEAT_COUNT as f64, "logical-error-occurrences/s"))
        }
        ("pf4-dem-sampler-folded-repeat", "stab_pf4_dem_sampler_sample_folded_repeat") => Some((
            (SAMPLER_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "error-applications/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_zero_probability_folded_repeat",
        ) => Some((
            (SAMPLER_NO_OP_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "skipped-detector-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_deterministic_parity_repeat",
        ) => Some((
            (SAMPLER_DETERMINISTIC_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "folded-deterministic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT as f64) * (SAMPLER_SHOTS as f64),
            "folded-stochastic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT as f64) * 3.0 * (SAMPLER_SHOTS as f64),
            "folded-flat-stochastic-error-occurrences/s",
        )),
        (
            "pf4-dem-sampler-folded-repeat",
            "stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat",
        ) => Some((
            (SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT as f64)
                * (SAMPLER_NESTED_STOCHASTIC_ERRORS_PER_REPETITION as f64)
                * (SAMPLER_SHOTS as f64),
            "folded-nested-stochastic-error-occurrences/s",
        )),
        _ => None,
    }
}

fn sampler_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_REPEAT_COUNT} {{
    error(0.25) D0 L0
    shift_detectors 1
}}
"
    )
}

fn sampler_no_op_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_NO_OP_REPEAT_COUNT} {{
    error(0) D0
}}
"
    )
}

fn sampler_deterministic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_DETERMINISTIC_REPEAT_COUNT} {{
    error(1) D0 L0
}}
"
    )
}

fn sampler_single_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_SINGLE_STOCHASTIC_REPEAT_COUNT} {{
    error(0.25) D0 L0
}}
"
    )
}

fn sampler_flat_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_FLAT_STOCHASTIC_REPEAT_COUNT} {{
    error(0.25) D0 L0
    error(0.125) D0
    error(1) L1
}}
"
    )
}

fn sampler_nested_stochastic_repeat_fixture() -> String {
    format!(
        "\
repeat {SAMPLER_NESTED_STOCHASTIC_REPEAT_COUNT} {{
    repeat 3 {{
        error(0.25) D0 L0
        error(0.125) D1
    }}
    error(1) L1
}}
"
    )
}

fn compile_plan(
    row: &BenchmarkRow,
    model: &DetectorErrorModel,
) -> Result<DemSamplingPlan, BenchError> {
    DemSamplingCompiler::new()
        .compile(model)
        .map_err(|error| stab_runner_error(&row.id, error))
}

fn sample_witness(
    row: &BenchmarkRow,
    plan: &DemSamplingPlan,
) -> Result<(u64, u64, u64), BenchError> {
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut sink = DemDigestSink::default();
    session
        .run(ShotCount::new(SAMPLER_SHOTS as u64), &mut sink)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok(sink.witness())
}
