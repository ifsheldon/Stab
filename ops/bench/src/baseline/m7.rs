use std::hint::black_box;

use stab_core::{
    CodeDistance, ColorCodeParams, ColorCodeTask, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, SurfaceCodeParams, SurfaceCodeTask, generate_color_code_circuit,
    generate_repetition_code_circuit, generate_surface_code_circuit,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::measure_stab;

const CLI_DISPATCH_ARGS: &[&str] = &[
    "stab",
    "gen",
    "--code",
    "repetition_code",
    "--task",
    "memory",
    "--distance",
    "3",
    "--rounds",
    "3",
];
const CONVERT_STIM_ARGS: &[&str] = &["stab", "convert", "--in_format=stim", "--out_format=stim"];
const CONVERT_STIM_FIXTURE: &str =
    include_str!("../../../../oracle/fixtures/inputs/parser_basic.stim");

pub(super) fn run_cli_dispatch_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![measure_stab("stab_cli_dispatch_gen_d3_r3", || {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = stab_cli::run_from(
            CLI_DISPATCH_ARGS,
            std::io::empty(),
            &mut stdout,
            &mut stderr,
        );
        if status != 0 {
            return Err(BenchError::StabRunner {
                row_id: row.id.clone(),
                message: format!(
                    "stab-cli dispatch failed with status {status}: {}",
                    String::from_utf8_lossy(&stderr)
                ),
            });
        }
        black_box(stdout.len());
        Ok(())
    })?])
}

pub(super) fn run_convert_stim_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![measure_stab("stab_convert_stim_canonical", || {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = stab_cli::run_from(
            CONVERT_STIM_ARGS,
            CONVERT_STIM_FIXTURE.as_bytes(),
            &mut stdout,
            &mut stderr,
        );
        if status != 0 {
            return Err(BenchError::StabRunner {
                row_id: row.id.clone(),
                message: format!(
                    "stab-cli convert failed with status {status}: {}",
                    String::from_utf8_lossy(&stderr)
                ),
            });
        }
        black_box(stdout.len());
        Ok(())
    })?])
}

pub(super) fn run_generator_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    let Some(generator) = GeneratorWorkload::from_row_id(&row.id)? else {
        return Ok(None);
    };
    Ok(Some(vec![measure_stab(
        generator.measurement_name(),
        || {
            let generated = generator
                .generate()
                .map_err(|error| BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: error.to_string(),
                })?;
            black_box(generated.circuit().items().len());
            black_box(generated.layout_text().len());
            Ok(())
        },
    )?]))
}

#[derive(Clone, Copy, Debug)]
enum GeneratorFamily {
    Repetition,
    RotatedSurface,
    UnrotatedSurface,
    Color,
}

#[derive(Clone, Copy, Debug)]
struct GeneratorWorkload {
    family: GeneratorFamily,
    distance: u32,
    rounds: u64,
}

impl GeneratorWorkload {
    fn from_row_id(row_id: &str) -> Result<Option<Self>, BenchError> {
        let Some((family, suffix)) = parse_generator_family(row_id) else {
            return Ok(None);
        };
        let (distance, rounds) = parse_distance_round_suffix(row_id, suffix)?;
        Ok(Some(Self {
            family,
            distance,
            rounds,
        }))
    }

    fn measurement_name(self) -> &'static str {
        match self.family {
            GeneratorFamily::Repetition => "stab_gen_repetition",
            GeneratorFamily::RotatedSurface => "stab_gen_rotated_surface",
            GeneratorFamily::UnrotatedSurface => "stab_gen_unrotated_surface",
            GeneratorFamily::Color => "stab_gen_color",
        }
    }

    fn generate(self) -> Result<stab_core::GeneratedCircuit, stab_core::CircuitError> {
        let rounds = RoundCount::try_new(self.rounds)?;
        let distance = CodeDistance::try_new(self.distance)?;
        match self.family {
            GeneratorFamily::Repetition => {
                let params =
                    RepetitionCodeParams::new(rounds, distance, RepetitionCodeTask::Memory)?;
                generate_repetition_code_circuit(&params)
            }
            GeneratorFamily::RotatedSurface => {
                let params =
                    SurfaceCodeParams::new(rounds, distance, SurfaceCodeTask::RotatedMemoryZ)?;
                generate_surface_code_circuit(&params)
            }
            GeneratorFamily::UnrotatedSurface => {
                let params =
                    SurfaceCodeParams::new(rounds, distance, SurfaceCodeTask::UnrotatedMemoryZ)?;
                generate_surface_code_circuit(&params)
            }
            GeneratorFamily::Color => {
                let params = ColorCodeParams::new(rounds, distance, ColorCodeTask::MemoryXyz)?;
                generate_color_code_circuit(&params)
            }
        }
    }
}

fn parse_generator_family(row_id: &str) -> Option<(GeneratorFamily, &str)> {
    row_id
        .strip_prefix("m7-gen-repetition-")
        .map(|suffix| (GeneratorFamily::Repetition, suffix))
        .or_else(|| {
            row_id
                .strip_prefix("m7-gen-rotated-surface-")
                .map(|suffix| (GeneratorFamily::RotatedSurface, suffix))
        })
        .or_else(|| {
            row_id
                .strip_prefix("m7-gen-unrotated-surface-")
                .map(|suffix| (GeneratorFamily::UnrotatedSurface, suffix))
        })
        .or_else(|| {
            row_id
                .strip_prefix("m7-gen-color-")
                .map(|suffix| (GeneratorFamily::Color, suffix))
        })
}

fn parse_distance_round_suffix(row_id: &str, suffix: &str) -> Result<(u32, u64), BenchError> {
    let Some(without_d) = suffix.strip_prefix('d') else {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("invalid M7 generator suffix {suffix:?}"),
        });
    };
    let Some((distance, rounds)) = without_d.split_once("-r") else {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("invalid M7 generator suffix {suffix:?}"),
        });
    };
    let distance = distance
        .parse::<u32>()
        .map_err(|error| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("invalid M7 generator distance in {suffix:?}: {error}"),
        })?;
    let rounds = rounds
        .parse::<u64>()
        .map_err(|error| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("invalid M7 generator rounds in {suffix:?}: {error}"),
        })?;
    Ok((distance, rounds))
}
