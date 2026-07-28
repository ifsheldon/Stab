#[cfg(not(test))]
use std::path::{Component, Path};
use std::{ffi::OsString, hint::black_box, path::PathBuf};

use crate::{error::BenchError, manifest::BenchmarkRow, report::Measurement, root::RepoRoot};

#[cfg(not(test))]
use crate::process::{check_success, run_checked_status, run_process};

use super::batch_sinks::ByteDigestWriter;
use super::{
    batch_sinks::OutputWitness, measure_stab_iterations_with_memory_operation, stab_runner_error,
};

const CLI_PROCESS_LAUNCHES_PER_MEASUREMENT: usize = 1;

pub(super) fn run_stab_cli_process_row(
    root: &RepoRoot,
    profile: &str,
    row: &BenchmarkRow,
    measurement_name: &'static str,
    expected: OutputWitness,
) -> Result<Vec<Measurement>, BenchError> {
    let stdin = row.stdin(root)?;

    #[cfg(not(test))]
    {
        let args = row
            .argv_tokens()
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let program = build_stab_cli_binary(root, profile, &row.id)?;
        let preflight = run_process(&program, &args, &stdin, &root.path, true)?;
        check_success(&program, &preflight)?;
        let actual = OutputWitness::from_bytes(&preflight.stdout);
        ensure_witness(row, expected, actual)?;

        let memory_args = in_process_args(root, row);
        Ok(vec![measure_stab_iterations_with_memory_operation(
            measurement_name,
            CLI_PROCESS_LAUNCHES_PER_MEASUREMENT,
            || {
                let output = run_process(&program, &args, &stdin, &root.path, false)?;
                check_success(&program, &output)?;
                black_box((output.status, output.parent_observed_peak_rss_bytes));
                Ok(())
            },
            || {
                let actual = run_in_process(row, memory_args.clone(), &stdin)?;
                ensure_witness(row, expected, actual)?;
                black_box(actual);
                Ok(())
            },
        )?])
    }

    #[cfg(test)]
    {
        let _ = profile;
        let args = in_process_args(root, row);
        let preflight = run_in_process(row, args.clone(), &stdin)?;
        ensure_witness(row, expected, preflight)?;
        Ok(vec![measure_stab_iterations_with_memory_operation(
            measurement_name,
            CLI_PROCESS_LAUNCHES_PER_MEASUREMENT,
            || {
                let actual = run_in_process(row, args.clone(), &stdin)?;
                ensure_witness(row, expected, actual)?;
                black_box(actual);
                Ok(())
            },
            || {
                let actual = run_in_process(row, args.clone(), &stdin)?;
                ensure_witness(row, expected, actual)?;
                black_box(actual);
                Ok(())
            },
        )?])
    }
}

fn ensure_witness(
    row: &BenchmarkRow,
    expected: OutputWitness,
    actual: OutputWitness,
) -> Result<(), BenchError> {
    if actual == expected {
        return Ok(());
    }
    Err(stab_runner_error(
        &row.id,
        format!("process-equivalent CLI output changed: expected {expected:?}, got {actual:?}"),
    ))
}

fn run_in_process(
    row: &BenchmarkRow,
    args: Vec<OsString>,
    stdin: &[u8],
) -> Result<OutputWitness, BenchError> {
    let mut stdout = ByteDigestWriter::default();
    let mut stderr = Vec::new();
    let status = stab_cli::run_from(args, stdin, &mut stdout, &mut stderr);
    if status != 0 {
        return Err(stab_runner_error(
            &row.id,
            format!(
                "in-process CLI preflight failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        ));
    }
    Ok(stdout.witness())
}

fn in_process_args(root: &RepoRoot, row: &BenchmarkRow) -> Vec<OsString> {
    let mut args = vec![OsString::from("stab")];
    let mut path_value_follows = false;
    for token in row.argv_tokens() {
        if path_value_follows {
            args.push(resolve_repo_path(root, &token));
            path_value_follows = false;
            continue;
        }
        if is_path_flag(&token) {
            args.push(OsString::from(token));
            path_value_follows = true;
            continue;
        }
        if let Some((flag, value)) = token.split_once('=')
            && is_path_flag(flag)
        {
            let mut resolved = OsString::from(flag);
            resolved.push("=");
            resolved.push(resolve_repo_path(root, value));
            args.push(resolved);
            continue;
        }
        args.push(OsString::from(token));
    }
    args
}

fn is_path_flag(token: &str) -> bool {
    matches!(
        token,
        "--circuit"
            | "--dem"
            | "--sweep"
            | "--replay_err_in"
            | "--obs_out"
            | "--err_out"
            | "--in"
            | "--out"
    )
}

fn resolve_repo_path(root: &RepoRoot, value: &str) -> OsString {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path.into_os_string()
    } else {
        root.path.join(path).into_os_string()
    }
}

#[cfg(not(test))]
fn build_stab_cli_binary(
    root: &RepoRoot,
    profile: &str,
    row_id: &str,
) -> Result<PathBuf, BenchError> {
    let profile_path = Path::new(profile);
    if profile.is_empty()
        || profile_path.components().count() != 1
        || !matches!(profile_path.components().next(), Some(Component::Normal(_)))
    {
        return Err(stab_runner_error(
            row_id,
            format!("unsafe Cargo profile name {profile:?}"),
        ));
    }
    run_checked_status(
        "cargo",
        [
            "build",
            "--quiet",
            "--profile",
            profile,
            "--package",
            "stab-cli",
            "--bin",
            "stab",
        ],
        &root.path,
    )?;
    let profile_dir = if profile == "dev" { "debug" } else { profile };
    let binary = root
        .path
        .join("target")
        .join(profile_dir)
        .join(format!("stab{}", std::env::consts::EXE_SUFFIX));
    if !binary.is_file() {
        return Err(stab_runner_error(
            row_id,
            format!(
                "Cargo completed without producing the expected Stab CLI binary {}",
                binary.display()
            ),
        ));
    }
    Ok(binary)
}
