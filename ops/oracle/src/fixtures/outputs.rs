use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd as _;
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::Arc;

use super::paths::{fixture_file, validate_fixture_path};
use super::statistical::{self, StatisticalSource};
use super::{
    ExpectedStdoutPolicy, FixtureComparator, FixtureError, FixturePathRequirement, FixtureRow,
    RepoRoot, is_core_fixture, is_direct_rust_fixture,
};

const FIXTURE_INPUT_TOKEN_PREFIX: &str = "{fixture_input:";
const FIXTURE_OUTPUT_TOKEN_PREFIX: &str = "{fixture_output:";
const FIXTURE_TOKEN_SUFFIX: &str = "}";
#[cfg(target_os = "linux")]
const FIXTURE_OUTPUT_PARENT: &str = "/tmp";
pub(super) const AUXILIARY_OUTPUT_LIMIT_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
pub(super) struct PreparedFixtureCommand {
    pub(super) argv: Vec<OsString>,
    pub(super) outputs: Vec<FixtureOutput>,
    _scratch: Option<ScratchRunDirectory>,
}

#[derive(Debug)]
struct ScratchRunDirectory {
    #[cfg(target_os = "linux")]
    parent: std::fs::File,
    #[cfg(target_os = "linux")]
    directory: Arc<std::fs::File>,
    #[cfg(target_os = "linux")]
    name: OsString,
    #[cfg(not(target_os = "linux"))]
    temporary: tempfile::TempDir,
}

impl Drop for ScratchRunDirectory {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            drop(crate::qualification::artifact::cleanup_owned_directory(
                &self.parent,
                &self.name,
                &self.directory,
            ));
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct FixtureOutput {
    pub(super) expected_relative: String,
    actual: crate::safe_file::SafeFileLocation,
}

impl FixtureOutput {
    pub(super) fn actual_path(&self) -> &Path {
        self.actual.display_path()
    }

    pub(super) fn monitored_file(&self) -> crate::safe_file::SafeFileLocation {
        self.actual.clone()
    }

    #[cfg(test)]
    pub(super) fn from_path(expected_relative: impl Into<String>, actual_path: PathBuf) -> Self {
        Self {
            expected_relative: expected_relative.into(),
            actual: crate::safe_file::SafeFileLocation::path(actual_path),
        }
    }
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
    let statistical_source = if row.comparator == FixtureComparator::Statistical {
        statistical::source_for_plan(&row.statistical_plan).ok()
    } else {
        None
    };
    let uses_statistical_fixture_output =
        statistical_source == Some(StatisticalSource::FixtureOutput);
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
                if row.comparator != FixtureComparator::ExactOutput
                    && !uses_statistical_fixture_output
                {
                    violations.push(format!(
                        "{} fixture output placeholders require exact-output comparator or statistical source=fixture_output",
                        row.id
                    ));
                }
                if !fixture_outputs.insert(relative.to_string()) {
                    violations.push(format!(
                        "{} declares duplicate fixture output {relative}",
                        row.id
                    ));
                }
                let requirement = if uses_statistical_fixture_output {
                    FixturePathRequirement::ExistingFileIfPresent
                } else {
                    match expected_stdout_policy {
                        ExpectedStdoutPolicy::RequireExisting => {
                            FixturePathRequirement::MustExistFile
                        }
                        ExpectedStdoutPolicy::AllowMissing => {
                            FixturePathRequirement::ExistingFileIfPresent
                        }
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
    if uses_statistical_fixture_output && fixture_outputs.len() != 1 {
        violations.push(format!(
            "{} statistical source=fixture_output requires exactly one fixture output placeholder",
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
    let mut scratch_dir = None;
    for token in row.argv_tokens() {
        match parse_fixture_arg_token(&row.id, &token)
            .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?
        {
            Some(FixtureArgToken::Input(relative)) => {
                argv.push(fixture_file(root, relative)?.into_os_string());
            }
            Some(FixtureArgToken::Output(relative)) => {
                let run_dir = match &mut scratch_dir {
                    Some(existing) => existing,
                    None => scratch_dir.insert(ScratchRunDirectory::create(process_label)?),
                };
                let file_name = OsString::from(format!(
                    "{:02}-{}",
                    outputs.len(),
                    safe_fixture_output_component(relative)
                ));
                let actual = run_dir.output_location(file_name);
                argv.push(actual.display_path().as_os_str().to_owned());
                outputs.push(FixtureOutput {
                    expected_relative: relative.to_string(),
                    actual,
                });
            }
            None => argv.push(OsString::from(token)),
        }
    }
    Ok(PreparedFixtureCommand {
        argv,
        outputs,
        _scratch: scratch_dir,
    })
}

impl ScratchRunDirectory {
    fn create(process_label: &str) -> Result<Self, FixtureError> {
        let prefix = format!(
            ".stab-oracle-{}-",
            safe_fixture_output_component(process_label)
        );
        #[cfg(target_os = "linux")]
        {
            let temporary = tempfile::Builder::new()
                .prefix(&prefix)
                .tempdir_in(FIXTURE_OUTPUT_PARENT)
                .map_err(|source| FixtureError::CreateOutputDir {
                    path: PathBuf::from(FIXTURE_OUTPUT_PARENT),
                    source,
                })?;
            let temporary_path = temporary.path().to_path_buf();
            let name = temporary_path
                .file_name()
                .ok_or_else(|| FixtureError::CreateOutputDir {
                    path: temporary_path.clone(),
                    source: std::io::Error::other(
                        "temporary fixture output directory has no final component",
                    ),
                })?
                .to_owned();
            let parent = crate::safe_file::open_directory(Path::new(FIXTURE_OUTPUT_PARENT))
                .map_err(|source| FixtureError::CreateOutputDir {
                    path: PathBuf::from(FIXTURE_OUTPUT_PARENT),
                    source: std::io::Error::other(source),
                })?;
            let directory = crate::qualification::artifact::open_directory_at(&parent, &name)
                .map_err(|source| FixtureError::CreateOutputDir {
                    path: temporary_path.clone(),
                    source: source.into(),
                })?;
            rustix::io::fcntl_setfd(&directory, rustix::io::FdFlags::empty()).map_err(
                |source| FixtureError::CreateOutputDir {
                    path: temporary_path.clone(),
                    source: source.into(),
                },
            )?;
            drop(temporary.keep());
            let scratch = Self {
                parent,
                directory: Arc::new(directory),
                name,
            };
            rustix::fs::fsync(&scratch.parent).map_err(|source| FixtureError::CreateOutputDir {
                path: temporary_path,
                source: source.into(),
            })?;
            Ok(scratch)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let temporary =
                tempfile::Builder::new()
                    .prefix(&prefix)
                    .tempdir()
                    .map_err(|source| FixtureError::CreateOutputDir {
                        path: std::env::temp_dir(),
                        source,
                    })?;
            Ok(Self { temporary })
        }
    }

    fn output_location(&self, name: OsString) -> crate::safe_file::SafeFileLocation {
        #[cfg(target_os = "linux")]
        {
            let child_path = PathBuf::from(format!(
                "/proc/self/fd/{}/{}",
                self.directory.as_raw_fd(),
                name.to_string_lossy()
            ));
            crate::safe_file::SafeFileLocation::directory_entry(
                Arc::clone(&self.directory),
                name,
                child_path,
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            let path = self.temporary.path().join(name);
            crate::safe_file::SafeFileLocation::path(path)
        }
    }
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
        super::paths::write_fixture_file(root, &output.expected_relative, &actual)?;
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
        let expected = super::paths::read_fixture_file(root, &output.expected_relative)?;
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
    if row.comparator == FixtureComparator::Statistical {
        return match statistical::source_for_plan(&row.statistical_plan) {
            Ok(StatisticalSource::Stdout) => Ok(()),
            Ok(StatisticalSource::FixtureOutput) => {
                compare_statistical_fixture_output(row, stim_outputs, stab_outputs)
            }
            Err(reason) => Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason,
            }),
        };
    }
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

fn compare_statistical_fixture_output(
    row: &FixtureRow,
    stim_outputs: &[FixtureOutput],
    stab_outputs: &[FixtureOutput],
) -> Result<(), FixtureError> {
    let ([stim], [stab]) = (stim_outputs, stab_outputs) else {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: format!(
                "statistical source=fixture_output requires one fixture output, got Stim {} and Stab {}",
                stim_outputs.len(),
                stab_outputs.len()
            ),
        });
    };
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
    if let Some(reason) = statistical::compare_statistical_plan(&row.statistical_plan, &stim_bytes)
    {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: format!("Stim fixture output {}: {reason}", stim.expected_relative),
        });
    }
    if let Some(reason) = statistical::compare_statistical_plan(&row.statistical_plan, &stab_bytes)
    {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: format!("Stab fixture output {}: {reason}", stab.expected_relative),
        });
    }
    Ok(())
}

pub(super) fn completed_statistical_shots_for_output(
    row: &FixtureRow,
    outputs: &[FixtureOutput],
) -> Option<u64> {
    let [output] = outputs else {
        return None;
    };
    let bytes = read_actual_fixture_output(row, output).ok()?;
    statistical::completed_shots(&row.statistical_plan, &bytes)
}

pub(super) fn read_actual_fixture_output(
    row: &FixtureRow,
    output: &FixtureOutput,
) -> Result<Vec<u8>, FixtureError> {
    let mut file = output
        .actual
        .open_regular_file()
        .map_err(|source| FixtureError::ReadFile {
            path: output.actual_path().to_path_buf(),
            source: std::io::Error::other(source),
        })?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(AUXILIARY_OUTPUT_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| FixtureError::ReadFile {
            path: output.actual_path().to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > AUXILIARY_OUTPUT_LIMIT_BYTES {
        return Err(FixtureError::AuxiliaryOutputTooLarge {
            id: row.id.clone(),
            path: output.actual_path().to_path_buf(),
            limit: AUXILIARY_OUTPUT_LIMIT_BYTES,
        });
    }
    Ok(bytes)
}
