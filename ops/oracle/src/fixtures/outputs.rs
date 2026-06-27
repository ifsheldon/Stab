use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::paths::{fixture_file, prepare_fixture_output_file, validate_fixture_path};
use super::{
    ExpectedStdoutPolicy, FixtureComparator, FixtureError, FixturePathRequirement, FixtureRow,
    RepoRoot, is_core_fixture, is_direct_rust_fixture,
};

const FIXTURE_INPUT_TOKEN_PREFIX: &str = "{fixture_input:";
const FIXTURE_OUTPUT_TOKEN_PREFIX: &str = "{fixture_output:";
const FIXTURE_TOKEN_SUFFIX: &str = "}";
const FIXTURE_OUTPUT_ROOT: &str = "target/oracle/fixture-outputs";
const AUXILIARY_OUTPUT_LIMIT_BYTES: u64 = 1024 * 1024;

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedFixtureCommand {
    pub(super) argv: Vec<OsString>,
    pub(super) outputs: Vec<FixtureOutput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FixtureOutput {
    pub(super) expected_relative: String,
    pub(super) actual_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FixtureArgToken<'a> {
    Input(&'a str),
    Output(&'a str),
}

pub(super) fn validate_row_tokens(
    row: &FixtureRow,
    fixture_root: &Path,
    expected_stdout_policy: ExpectedStdoutPolicy,
    violations: &mut Vec<String>,
) {
    let mut has_placeholder = false;
    let mut fixture_outputs = BTreeSet::new();
    for token in row.argv_tokens() {
        let parsed = match parse_fixture_arg_token(&row.id, &token) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => continue,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        has_placeholder = true;
        match parsed {
            FixtureArgToken::Input(relative) => {
                validate_fixture_path(
                    fixture_root,
                    &row.id,
                    "fixture_input",
                    relative,
                    FixturePathRequirement::MustExistFile,
                )
                .unwrap_or_else(|violation| violations.push(violation));
            }
            FixtureArgToken::Output(relative) => {
                if row.comparator != FixtureComparator::ExactOutput {
                    violations.push(format!(
                        "{} fixture output placeholders require exact-output comparator",
                        row.id
                    ));
                }
                if !fixture_outputs.insert(relative.to_string()) {
                    violations.push(format!(
                        "{} declares duplicate fixture output {relative}",
                        row.id
                    ));
                }
                let requirement = match expected_stdout_policy {
                    ExpectedStdoutPolicy::RequireExisting => FixturePathRequirement::MustExistFile,
                    ExpectedStdoutPolicy::AllowMissing => {
                        FixturePathRequirement::ExistingFileIfPresent
                    }
                };
                validate_fixture_path(
                    fixture_root,
                    &row.id,
                    "fixture_output",
                    relative,
                    requirement,
                )
                .unwrap_or_else(|violation| violations.push(violation));
            }
        }
    }
    if has_placeholder && (is_core_fixture(row) || is_direct_rust_fixture(row)) {
        violations.push(format!(
            "{} fixture placeholders are only supported for CLI fixture rows",
            row.id
        ));
    }
}

pub(super) fn parse_fixture_arg_token<'a>(
    id: &str,
    token: &'a str,
) -> Result<Option<FixtureArgToken<'a>>, String> {
    if let Some(relative) = parse_prefixed_token(id, token, FIXTURE_INPUT_TOKEN_PREFIX, "input")? {
        return Ok(Some(FixtureArgToken::Input(relative)));
    }
    if let Some(relative) = parse_prefixed_token(id, token, FIXTURE_OUTPUT_TOKEN_PREFIX, "output")?
    {
        return Ok(Some(FixtureArgToken::Output(relative)));
    }
    Ok(None)
}

fn parse_prefixed_token<'a>(
    id: &str,
    token: &'a str,
    prefix: &str,
    label: &str,
) -> Result<Option<&'a str>, String> {
    if let Some(without_prefix) = token.strip_prefix(prefix) {
        if let Some(relative) = without_prefix.strip_suffix(FIXTURE_TOKEN_SUFFIX) {
            if relative.is_empty() {
                return Err(format!("{id} has empty fixture {label} token"));
            }
            return Ok(Some(relative));
        }
        return Err(format!("{id} has malformed fixture {label} token {token}"));
    }
    if token.contains(prefix) {
        return Err(format!("{id} has malformed fixture {label} token {token}"));
    }
    Ok(None)
}

pub(super) fn prepare_command(
    root: &RepoRoot,
    row: &FixtureRow,
    process_label: &str,
) -> Result<PreparedFixtureCommand, FixtureError> {
    let mut argv = Vec::new();
    let mut outputs = Vec::new();
    let mut scratch_dir: Option<PathBuf> = None;
    for token in row.argv_tokens() {
        match parse_fixture_arg_token(&row.id, &token)
            .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?
        {
            Some(FixtureArgToken::Input(relative)) => {
                argv.push(fixture_file(root, relative)?.into_os_string());
            }
            Some(FixtureArgToken::Output(relative)) => {
                let run_dir = if let Some(existing) = &scratch_dir {
                    existing.clone()
                } else {
                    let created = create_scratch_run_dir(root, row, process_label)?;
                    scratch_dir = Some(created.clone());
                    created
                };
                let actual_path = run_dir.join(format!(
                    "{:02}-{}",
                    outputs.len(),
                    safe_fixture_output_component(relative)
                ));
                argv.push(actual_path.clone().into_os_string());
                outputs.push(FixtureOutput {
                    expected_relative: relative.to_string(),
                    actual_path,
                });
            }
            None => argv.push(OsString::from(token)),
        }
    }
    Ok(PreparedFixtureCommand { argv, outputs })
}

fn create_scratch_run_dir(
    root: &RepoRoot,
    row: &FixtureRow,
    process_label: &str,
) -> Result<PathBuf, FixtureError> {
    let scratch_root = root.path.join(FIXTURE_OUTPUT_ROOT);
    ensure_directory_without_symlink_components(&scratch_root)?;
    let pid = std::process::id();
    for _ in 0..1024 {
        let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = scratch_root.join(format!(
            "run-{pid}-{counter:016x}-{}-{}",
            safe_fixture_output_component(&row.id),
            safe_fixture_output_component(process_label)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(FixtureError::CreateOutputDir { path, source });
            }
        }
    }
    Err(FixtureError::CoreFixtureFailed {
        id: row.id.clone(),
        reason: "failed to allocate unique fixture output scratch directory".to_string(),
    })
}

fn ensure_directory_without_symlink_components(path: &Path) -> Result<(), FixtureError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => current.push(component.as_os_str()),
            Component::Normal(name) => {
                current.push(name);
                ensure_directory_component(&current)?;
            }
            Component::CurDir | Component::ParentDir => {
                return Err(FixtureError::UnsafeScratchPath {
                    path: path.to_path_buf(),
                    reason: "scratch path contains relative components".to_string(),
                });
            }
        }
    }
    Ok(())
}

fn ensure_directory_component(path: &Path) -> Result<(), FixtureError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => validate_directory_metadata(path, &metadata),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(|source| FixtureError::CreateOutputDir {
                path: path.to_path_buf(),
                source,
            })?;
            let metadata =
                std::fs::symlink_metadata(path).map_err(|source| FixtureError::InspectOutput {
                    path: path.to_path_buf(),
                    source,
                })?;
            validate_directory_metadata(path, &metadata)
        }
        Err(source) => Err(FixtureError::InspectOutput {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn validate_directory_metadata(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), FixtureError> {
    if metadata.file_type().is_symlink() {
        return Err(FixtureError::UnsafeScratchPath {
            path: path.to_path_buf(),
            reason: "scratch path contains symlink component".to_string(),
        });
    }
    if !metadata.is_dir() {
        return Err(FixtureError::UnsafeScratchPath {
            path: path.to_path_buf(),
            reason: "scratch path component is not a directory".to_string(),
        });
    }
    Ok(())
}

fn safe_fixture_output_component(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "_".to_string()
    } else {
        safe
    }
}

pub(super) fn record_outputs(
    row: &FixtureRow,
    root: &RepoRoot,
    outputs: &[FixtureOutput],
) -> Result<(), FixtureError> {
    for output in outputs {
        let actual = read_actual_fixture_output(row, output)?;
        prepare_fixture_output_file(root, &output.expected_relative)?;
        let expected_path = fixture_file(root, &output.expected_relative)?;
        std::fs::write(&expected_path, actual).map_err(|source| FixtureError::WriteOutput {
            path: expected_path,
            source,
        })?;
    }
    Ok(())
}

pub(super) fn compare_expected_outputs(
    row: &FixtureRow,
    root: &RepoRoot,
    outputs: &[FixtureOutput],
) -> Result<(), FixtureError> {
    for output in outputs {
        let actual = read_actual_fixture_output(row, output)?;
        let expected_path = fixture_file(root, &output.expected_relative)?;
        let expected = std::fs::read(&expected_path).map_err(|source| FixtureError::ReadFile {
            path: expected_path.clone(),
            source,
        })?;
        if expected != actual {
            return Err(FixtureError::ExpectedFixtureOutputMismatch {
                id: row.id.clone(),
                path: expected_path,
            });
        }
    }
    Ok(())
}

pub(super) fn compare_outputs(
    row: &FixtureRow,
    stim_outputs: &[FixtureOutput],
    stab_outputs: &[FixtureOutput],
) -> Result<(), FixtureError> {
    if stim_outputs.len() != stab_outputs.len() {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: format!(
                "fixture output count mismatch: Stim used {}, Stab used {}",
                stim_outputs.len(),
                stab_outputs.len()
            ),
        });
    }
    for (stim, stab) in stim_outputs.iter().zip(stab_outputs) {
        if stim.expected_relative != stab.expected_relative {
            return Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason: format!(
                    "fixture output path mismatch: Stim expected {}, Stab expected {}",
                    stim.expected_relative, stab.expected_relative
                ),
            });
        }
        let stim_bytes = read_actual_fixture_output(row, stim)?;
        let stab_bytes = read_actual_fixture_output(row, stab)?;
        if stim_bytes != stab_bytes {
            return Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason: format!("fixture output {} differs", stim.expected_relative),
            });
        }
    }
    Ok(())
}

fn read_actual_fixture_output(
    row: &FixtureRow,
    output: &FixtureOutput,
) -> Result<Vec<u8>, FixtureError> {
    let metadata = std::fs::symlink_metadata(&output.actual_path).map_err(|source| {
        FixtureError::ReadFile {
            path: output.actual_path.clone(),
            source,
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!(
                "fixture output {} is not a regular file",
                output.actual_path.display()
            ),
        });
    }
    if metadata.len() > AUXILIARY_OUTPUT_LIMIT_BYTES {
        return Err(FixtureError::AuxiliaryOutputTooLarge {
            id: row.id.clone(),
            path: output.actual_path.clone(),
            limit: AUXILIARY_OUTPUT_LIMIT_BYTES,
        });
    }
    std::fs::read(&output.actual_path).map_err(|source| FixtureError::ReadFile {
        path: output.actual_path.clone(),
        source,
    })
}
