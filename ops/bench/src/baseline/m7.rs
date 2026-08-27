use std::ffi::OsString;
use std::hint::black_box;
use std::path::Path;

use stab_analysis::{
    AnalysisResult, CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit,
    RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::process::{check_success, run_process};
use crate::report::Measurement;
use crate::root::RepoRoot;

use super::{batch_sinks::OutputWitness, measure_stab};

#[cfg(test)]
mod tests;

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
const LEGACY_DISPATCH_ARGS: &[&str] = &[
    "stab",
    "--gen=repetition_code",
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
const CONVERT_STIM_CANONICAL_EXPECTED: &[u8] =
    b"H 0\nCX 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
// Frozen from pinned Stim v1.16.0 for the exact LEGACY_DISPATCH_ARGS command.
const LEGACY_DISPATCH_EXPECTED: OutputWitness = OutputWitness::new(757, 0xbd1f_30bb_8b3b_aa5d);

pub(super) fn run_cli_dispatch_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let preflight = run_dispatch_once(row, CLI_DISPATCH_ARGS, "stab-cli dispatch")?;
    ensure_dispatch_witness(
        &row.id,
        "CLI dispatch",
        OutputWitness::from_bytes(&preflight),
    )?;
    black_box(preflight);
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

pub(super) fn run_legacy_dispatch_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let preflight = run_legacy_dispatch_once(row)?;
    ensure_legacy_dispatch_witness(&row.id, OutputWitness::from_bytes(&preflight))?;
    black_box(preflight);
    Ok(vec![measure_stab("stab_pf7_cli_legacy_gen_d3_r3", || {
        let stdout = run_legacy_dispatch_once(row)?;
        black_box(stdout.len());
        Ok(())
    })?])
}

fn run_legacy_dispatch_once(row: &BenchmarkRow) -> Result<Vec<u8>, BenchError> {
    run_dispatch_once(row, LEGACY_DISPATCH_ARGS, "stab-cli legacy dispatch")
}

fn run_dispatch_once(
    row: &BenchmarkRow,
    args: &[&str],
    label: &'static str,
) -> Result<Vec<u8>, BenchError> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args, std::io::empty(), &mut stdout, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "{label} failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    Ok(stdout)
}

fn ensure_legacy_dispatch_witness(row_id: &str, actual: OutputWitness) -> Result<(), BenchError> {
    ensure_dispatch_witness(row_id, "legacy dispatch", actual)
}

fn ensure_dispatch_witness(
    row_id: &str,
    label: &str,
    actual: OutputWitness,
) -> Result<(), BenchError> {
    if actual == LEGACY_DISPATCH_EXPECTED {
        return Ok(());
    }
    Err(BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: format!(
            "{label} output changed from pinned Stim v1.16.0: expected {LEGACY_DISPATCH_EXPECTED:?}, got {actual:?}"
        ),
    })
}

pub(super) fn run_convert_stim_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let preflight = run_convert_stim_once(row)?;
    ensure_exact_bytes(
        &row.id,
        "canonical .stim conversion",
        CONVERT_STIM_CANONICAL_EXPECTED,
        &preflight,
    )?;
    black_box(preflight);
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

fn run_convert_stim_once(row: &BenchmarkRow) -> Result<Vec<u8>, BenchError> {
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
                "stab-cli canonical convert witness failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    Ok(stdout)
}

pub(super) fn run_generator_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    let Some(generator) = GeneratorWorkload::from_row_id(&row.id)? else {
        return Ok(None);
    };
    generator.ensure_pinned_cli_equivalence(row)?;
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

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if row_id.starts_with("m7-gen-") && name.starts_with("stab_gen_") {
        return Some((1.0, "circuits/s"));
    }
    match (row_id, name) {
        ("m7-cli-dispatch", "stab_cli_dispatch_gen_d3_r3") => Some((1.0, "dispatches/s")),
        ("pf7-cli-legacy-dispatch-startup", "stab_pf7_cli_legacy_gen_d3_r3") => {
            Some((1.0, "dispatches/s"))
        }
        ("m7-convert-stim-canonical", "stab_convert_stim_canonical") => {
            Some((CONVERT_STIM_FIXTURE.len() as f64, "bytes/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m7-perf-harness" => Some(
            "contract-only: verifies baseline metadata coverage; no Stab runtime workload is expected",
        ),
        "m7-cli-dispatch" => Some(
            "report-only: Stab measures in-process gen dispatch; upstream baseline is sample-heavy main dispatch",
        ),
        "pf7-cli-legacy-dispatch-startup" => Some(
            "report-only: Stab measures accepted legacy --gen dispatch through the public CLI parser for PF7 visible CLI parity",
        ),
        "m7-convert-stim-canonical" => Some(
            "contract-only: Stab measures in-process canonical .stim conversion; pinned Stim has no matching circuit-convert CLI",
        ),
        id if id.starts_with("m7-gen-") => Some(
            "report-only: Stab measures direct Rust generator construction and formatting-independent circuit access",
        ),
        _ => None,
    }
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

    fn cli_args(self) -> Vec<OsString> {
        let mut args = vec![OsString::from("stab"), OsString::from("gen")];
        match self.family {
            GeneratorFamily::Repetition => {
                args.extend([
                    OsString::from("--code"),
                    OsString::from("repetition_code"),
                    OsString::from("--task"),
                    OsString::from("memory"),
                ]);
            }
            GeneratorFamily::RotatedSurface => {
                args.extend([
                    OsString::from("--code"),
                    OsString::from("surface_code"),
                    OsString::from("--task"),
                    OsString::from("rotated_memory_z"),
                ]);
            }
            GeneratorFamily::UnrotatedSurface => {
                args.extend([
                    OsString::from("--code"),
                    OsString::from("surface_code"),
                    OsString::from("--task"),
                    OsString::from("unrotated_memory_z"),
                ]);
            }
            GeneratorFamily::Color => {
                args.extend([
                    OsString::from("--code"),
                    OsString::from("color_code"),
                    OsString::from("--task"),
                    OsString::from("memory_xyz"),
                ]);
            }
        }
        args.extend([
            OsString::from("--distance"),
            OsString::from(self.distance.to_string()),
            OsString::from("--rounds"),
            OsString::from(self.rounds.to_string()),
        ]);
        args
    }

    fn ensure_pinned_cli_equivalence(self, row: &BenchmarkRow) -> Result<(), BenchError> {
        let args = self.cli_args();
        let root = RepoRoot::resolve(Path::new(".")).map_err(|error| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "could not resolve repository root for pinned Stim gen witness: {error}"
            ),
        })?;
        let pinned = run_pinned_stim_cli(row, &root, &args)?;
        let stab = run_stab_cli_bytes(row, args)?;
        ensure_exact_bytes(&row.id, "generator CLI output", &pinned, &stab)
    }

    fn generate(self) -> AnalysisResult<GeneratedCircuit> {
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

fn run_stab_cli_bytes(row: &BenchmarkRow, args: Vec<OsString>) -> Result<Vec<u8>, BenchError> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args, std::io::empty(), &mut stdout, &mut stderr);
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli generator witness failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    Ok(stdout)
}

fn run_pinned_stim_cli(
    row: &BenchmarkRow,
    root: &RepoRoot,
    args: &[OsString],
) -> Result<Vec<u8>, BenchError> {
    let stim_args = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let output = run_process(&root.stim_binary(), &stim_args, b"", &root.path, true)?;
    check_success(&root.stim_binary(), &output).map_err(|error| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!("pinned Stim generator witness failed: {error}"),
    })?;
    Ok(output.stdout)
}

fn ensure_exact_bytes(
    row_id: &str,
    label: &str,
    expected: &[u8],
    actual: &[u8],
) -> Result<(), BenchError> {
    if actual == expected {
        return Ok(());
    }
    Err(BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: format!(
            "{label} changed from the pinned Stim v1.16.0 byte output: expected {} bytes with {:?}, got {} bytes with {:?}",
            expected.len(),
            OutputWitness::from_bytes(expected),
            actual.len(),
            OutputWitness::from_bytes(actual)
        ),
    })
}
