//! Oracle fixture manifest loading and execution.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use stab_core::{Circuit, DetectorErrorModel};
use thiserror::Error;

use crate::{OracleError, RepoRoot, StderrClass, compare_exact, compare_help_health};

mod hex_payload;
mod id;
mod outputs;
mod paths;
mod reverse_flow;
mod statistical;
mod two_sample;

pub(crate) use id::FixtureId;
use paths::{fixture_file, validate_fixture_path};
use reverse_flow::core_time_reverse_flows_output;

const FIXTURE_ROOT: &str = "oracle/fixtures";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunFilter {
    Exact,
    Statistical,
    Structural,
}

impl RunFilter {
    pub(crate) fn from_flags(
        exact: bool,
        statistical: bool,
        structural: bool,
    ) -> Result<Option<Self>, String> {
        match (exact, statistical, structural) {
            (false, false, false) => Ok(None),
            (true, false, false) => Ok(Some(Self::Exact)),
            (false, true, false) => Ok(Some(Self::Statistical)),
            (false, false, true) => Ok(Some(Self::Structural)),
            _ => Err("choose at most one of --exact, --statistical, or --structural".to_string()),
        }
    }

    /// `Statistical` selects both one-sample analytic rows and Stim-versus-Stab two-sample rows.
    fn matches(self, row: &FixtureRow) -> bool {
        match self {
            Self::Exact => row.comparator == FixtureComparator::ExactOutput,
            Self::Statistical => matches!(
                row.comparator,
                FixtureComparator::Statistical | FixtureComparator::TwoSample
            ),
            Self::Structural => row.comparator == FixtureComparator::Structural,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct FixtureRow {
    id: FixtureId,
    upstream_source: String,
    comparator: FixtureComparator,
    command_shape: String,
    argv: String,
    stdin_path: String,
    expected_stdout_path: String,
    expected_status: i32,
    expected_stderr_class: ExpectedStderrClass,
    statistical_plan: String,
    source_license_note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum FixtureComparator {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "help-health")]
    HelpHealth,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
    #[serde(rename = "two-sample")]
    TwoSample,
}

impl FixtureComparator {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactOutput => "exact-output",
            Self::HelpHealth => "help-health",
            Self::Statistical => "statistical",
            Self::Structural => "structural",
            Self::TwoSample => "two-sample",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum ExpectedStderrClass {
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "non-empty")]
    NonEmpty,
}

impl ExpectedStderrClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Empty => "empty",
            Self::NonEmpty => "non-empty",
        }
    }

    fn matches(self, actual: StderrClass) -> bool {
        match self {
            Self::Any => true,
            Self::Empty => actual == StderrClass::Empty,
            Self::NonEmpty => actual == StderrClass::NonEmpty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixturePathRequirement {
    MustExistFile,
    ExistingFileIfPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedStdoutPolicy {
    RequireExisting,
    AllowMissing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureManifest {
    rows: Vec<FixtureRow>,
}

#[derive(Debug, Error)]
pub(crate) enum FixtureError {
    #[error("failed to read fixture manifest {path}: {source}")]
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse fixture manifest: {0}")]
    Parse(#[from] csv::Error),

    #[error("fixture manifest validation failed:\n{0}")]
    Validation(Box<str>),

    #[error("failed to read fixture file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("fixture file {path} exceeds the {limit}-byte limit")]
    FixtureFileTooLarge { path: PathBuf, limit: usize },

    #[error("fixture hex payload {path} is invalid: {reason}")]
    InvalidHexPayload { path: PathBuf, reason: Box<str> },

    #[error("failed to prepare private fixture output directory at {path}: {source}")]
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write fixture output {path}: {source}")]
    WriteOutput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{id} expected status {expected}, got {actual:?}")]
    StatusMismatch {
        id: FixtureId,
        expected: i32,
        actual: Option<i32>,
    },

    #[error("{id} expected stderr class {expected}, got {actual:?}")]
    StderrClassMismatch {
        id: FixtureId,
        expected: &'static str,
        actual: StderrClass,
    },

    #[error("{id} expected stdout differs from {path}")]
    ExpectedStdoutMismatch { id: FixtureId, path: PathBuf },

    #[error("{id} expected fixture output differs from {path}")]
    ExpectedFixtureOutputMismatch { id: FixtureId, path: PathBuf },

    #[error("{id} fixture output {path} exceeds {limit} bytes")]
    AuxiliaryOutputTooLarge {
        id: FixtureId,
        path: PathBuf,
        limit: u64,
    },

    #[error("{id} failed core fixture execution: {reason}")]
    CoreFixtureFailed { id: FixtureId, reason: String },

    #[error("{id} failed comparator {comparator}: {reason}")]
    ComparatorMismatch {
        id: FixtureId,
        comparator: &'static str,
        reason: String,
    },
}

impl FixtureManifest {
    fn read(root: &RepoRoot) -> Result<Self, FixtureError> {
        let path = root.fixture_manifest();
        let bytes = paths::read_fixture_file(root, "manifest.csv")?;
        let content = String::from_utf8(bytes).map_err(|source| FixtureError::ReadManifest {
            path,
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;
        Self::from_csv(&content)
    }

    fn from_csv(content: &str) -> Result<Self, FixtureError> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        let rows = reader.deserialize().collect::<Result<Vec<_>, _>>()?;
        Ok(Self { rows })
    }

    fn check(&self, root: &RepoRoot) -> Result<(), FixtureError> {
        self.check_with_expected_stdout_policy(root, ExpectedStdoutPolicy::RequireExisting)
    }

    fn check_with_expected_stdout_policy(
        &self,
        root: &RepoRoot,
        expected_stdout_policy: ExpectedStdoutPolicy,
    ) -> Result<(), FixtureError> {
        let mut violations = Vec::new();
        let fixture_root = root.path.join(FIXTURE_ROOT);
        if let Err(violation) = paths::validate_fixture_root(root) {
            return Err(FixtureError::Validation(violation.into_boxed_str()));
        }
        for row in &self.rows {
            for (field, value) in [
                ("upstream_source", &row.upstream_source),
                ("command_shape", &row.command_shape),
                ("argv", &row.argv),
                ("source_license_note", &row.source_license_note),
            ] {
                if value.is_empty() {
                    violations.push(format!("{} has empty {field}", row.id));
                }
            }
            if row.argv_tokens().is_empty() {
                violations.push(format!("{} has no argv tokens", row.id));
            }
            if row.argv.split('|').any(str::is_empty) {
                violations.push(format!("{} has an empty argv token", row.id));
            }
            outputs::validate_row_tokens(
                row,
                &fixture_root,
                expected_stdout_policy,
                &mut violations,
            );
            if row.comparator == FixtureComparator::ExactOutput
                && row.expected_status == 0
                && row.expected_stdout_path.is_empty()
            {
                violations.push(format!("{} exact fixture has no expected stdout", row.id));
            }
            if matches!(
                row.comparator,
                FixtureComparator::Statistical
                    | FixtureComparator::Structural
                    | FixtureComparator::TwoSample
            ) && row.statistical_plan.is_empty()
            {
                violations.push(format!(
                    "{} comparator needs structural or statistical plan text",
                    row.id
                ));
            }
            if row.comparator == FixtureComparator::Statistical {
                let argv_tokens = row.argv_tokens();
                if let Some(reason) =
                    statistical::validate_statistical_plan(&row.statistical_plan, &argv_tokens)
                {
                    violations.push(format!("{} invalid statistical plan: {reason}", row.id));
                }
            }
            if row.comparator == FixtureComparator::TwoSample {
                if is_core_fixture(row) {
                    violations.push(format!(
                        "{} two-sample rows must run the Stim and Stab binaries",
                        row.id
                    ));
                }
                let argv_tokens = row.argv_tokens();
                if let Some(reason) =
                    two_sample::validate_two_sample_plan(&row.statistical_plan, &argv_tokens)
                {
                    violations.push(format!("{} invalid two-sample plan: {reason}", row.id));
                }
            }
            validate_vendor_source(root, row, &mut violations);
            for (field, relative, must_exist) in [
                (
                    "stdin_path",
                    row.stdin_path.as_str(),
                    FixturePathRequirement::MustExistFile,
                ),
                (
                    "expected_stdout_path",
                    row.expected_stdout_path.as_str(),
                    match expected_stdout_policy {
                        ExpectedStdoutPolicy::RequireExisting => {
                            FixturePathRequirement::MustExistFile
                        }
                        ExpectedStdoutPolicy::AllowMissing => {
                            FixturePathRequirement::ExistingFileIfPresent
                        }
                    },
                ),
            ] {
                if !relative.is_empty() {
                    if let Err(violation) = validate_fixture_path(
                        &fixture_root,
                        row.id.as_str(),
                        field,
                        relative,
                        must_exist,
                    ) {
                        violations.push(violation);
                        continue;
                    }
                    if relative.ends_with(".hex")
                        && fixture_root.join(relative).is_file()
                        && let Err(error) = hex_payload::read(root, relative)
                    {
                        violations.push(format!(
                            "{} has invalid hex-encoded {field}: {error}",
                            row.id
                        ));
                    }
                }
            }
        }
        self.check_unique_ids(&mut violations);
        violations
            .is_empty()
            .ok_or_else(|| FixtureError::Validation(violations.join("\n").into_boxed_str()))
    }

    fn check_unique_ids(&self, violations: &mut Vec<String>) {
        let mut seen = std::collections::BTreeSet::new();
        for row in &self.rows {
            if !seen.insert(&row.id) {
                violations.push(format!("{} is declared more than once", row.id));
            }
        }
    }

    fn list(&self) {
        let mut groups: BTreeMap<FixtureComparator, Vec<&FixtureRow>> = BTreeMap::new();
        for row in &self.rows {
            groups.entry(row.comparator).or_default().push(row);
        }
        for (comparator, rows) in groups {
            println!("{}:", comparator.as_str());
            for row in rows {
                println!(
                    "- {} {} -> {}",
                    row.id, row.command_shape, row.upstream_source
                );
            }
        }
    }
}

impl FixtureRow {
    fn argv_tokens(&self) -> Vec<String> {
        self.argv
            .split('|')
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn stdin(&self, root: &RepoRoot) -> Result<Vec<u8>, FixtureError> {
        if self.stdin_path.is_empty() {
            return Ok(Vec::new());
        }
        hex_payload::read(root, &self.stdin_path)
    }

    fn expected_stdout_file(&self, root: &RepoRoot) -> Result<PathBuf, FixtureError> {
        fixture_file(root, &self.expected_stdout_path)
    }
}

pub(crate) fn list_fixtures(root: &RepoRoot) -> Result<(), OracleError> {
    load_manifest(root)?.list();
    Ok(())
}

pub(crate) fn record_fixtures(
    root: &RepoRoot,
    check_clean: bool,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let manifest = load_manifest_with_expected_stdout_policy(
        root,
        if check_clean {
            ExpectedStdoutPolicy::RequireExisting
        } else {
            ExpectedStdoutPolicy::AllowMissing
        },
    )?;
    let stim_binary = crate::ensure_stim_binary(root, rebuild_stim)?;
    let mut reverse_flow_helper = None;
    for row in manifest.rows.iter().filter(|row| is_recordable(row)) {
        let stdin = row.stdin(root)?;
        let command = outputs::prepare_command(root, row, "stim-record")?;
        let output = if reverse_flow::is_reverse_flow_fixture(row) {
            let helper = match &reverse_flow_helper {
                Some(path) => path,
                None => reverse_flow_helper
                    .insert(crate::ensure_stim_reverse_flow_helper(root, rebuild_stim)?),
            };
            reverse_flow::run_pinned_stim_reverse_flow(root, row, &stdin, helper)?
        } else {
            run_prepared_fixture_process(root, &stim_binary, &command, &stdin)?
        };
        check_expected_process_shape(row, &output)?;
        let expected_path = row.expected_stdout_file(root)?;
        if check_clean {
            let expected = hex_payload::read(root, &row.expected_stdout_path)?;
            if expected != output.stdout.bytes {
                return Err(FixtureError::ExpectedStdoutMismatch {
                    id: row.id.clone(),
                    path: expected_path,
                }
                .into());
            }
            outputs::compare_expected_outputs(row, root, &command.outputs)?;
            println!("[stab-oracle] CLEAN {}", row.id);
        } else {
            hex_payload::write(root, &row.expected_stdout_path, &output.stdout.bytes)?;
            outputs::record_outputs(row, root, &command.outputs)?;
            println!("[stab-oracle] RECORDED {}", row.id);
        }
    }
    Ok(())
}

fn is_recordable(row: &FixtureRow) -> bool {
    row.comparator == FixtureComparator::ExactOutput
        && (!is_core_fixture(row) || reverse_flow::is_reverse_flow_fixture(row))
        && !row.expected_stdout_path.is_empty()
}

/// Runs every manifest fixture, optionally restricted to one comparator.
pub(crate) fn run_fixtures(
    root: &RepoRoot,
    filter: Option<RunFilter>,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    let mut stim_binary = None;
    let mut stab_binary = None;
    for row in &manifest.rows {
        if filter.is_some_and(|filter| !filter.matches(row)) {
            continue;
        }
        let stab = if is_core_fixture(row) {
            run_core_fixture(root, row)?
        } else {
            let stdin = row.stdin(root)?;
            let stim_binary_path = cached_stim_binary(root, rebuild_stim, &mut stim_binary)?;
            let stab_binary_path = cached_stab_binary(root, &mut stab_binary)?;
            let stim_command = outputs::prepare_command(root, row, "stim")?;
            let stab_command = outputs::prepare_command(root, row, "stab")?;
            let stim =
                run_prepared_fixture_process(root, &stim_binary_path, &stim_command, &stdin)?;
            let stab =
                run_prepared_fixture_process(root, &stab_binary_path, &stab_command, &stdin)?;
            compare_fixture(row, &stim, &stab)?;
            outputs::compare_outputs(row, &stim_command.outputs, &stab_command.outputs)?;
            stab
        };
        println!(
            "[stab-oracle] PASS {} status={:?} stderr_class={:?}",
            row.id,
            stab.status,
            stab.stderr_class()
        );
    }
    Ok(())
}

fn run_prepared_fixture_process(
    root: &RepoRoot,
    program: &Path,
    command: &outputs::PreparedFixtureCommand,
    stdin: &[u8],
) -> Result<crate::ProcessOutput, OracleError> {
    let monitored_files = command
        .outputs
        .iter()
        .map(outputs::FixtureOutput::monitored_file)
        .collect::<Vec<_>>();
    crate::process::run_process_monitoring_files(
        program,
        &command.argv,
        stdin,
        Some(&root.path),
        &monitored_files,
        outputs::AUXILIARY_OUTPUT_LIMIT_BYTES,
    )
}

fn cached_stim_binary(
    root: &RepoRoot,
    rebuild_stim: bool,
    cache: &mut Option<PathBuf>,
) -> Result<PathBuf, OracleError> {
    if let Some(path) = cache {
        return Ok(path.clone());
    }
    let path = crate::ensure_stim_binary(root, rebuild_stim)?;
    *cache = Some(path.clone());
    Ok(path)
}

fn cached_stab_binary(
    root: &RepoRoot,
    cache: &mut Option<PathBuf>,
) -> Result<PathBuf, OracleError> {
    if let Some(path) = cache {
        return Ok(path.clone());
    }
    let path = crate::ensure_stab_cli_binary(root)?;
    *cache = Some(path.clone());
    Ok(path)
}

fn is_core_fixture(row: &FixtureRow) -> bool {
    row.argv_tokens().first().is_some_and(|token| {
        matches!(
            token.as_str(),
            "core-parse-print"
                | "core-circuit-parse-print"
                | "core-dem-parse-print"
                | "core-time-reverse-flows"
        )
    })
}

fn run_core_fixture(
    root: &RepoRoot,
    row: &FixtureRow,
) -> Result<crate::ProcessOutput, FixtureError> {
    let stdin = row.stdin(root)?;
    let output = core_parse_print_output(row, &stdin)?;
    check_expected_process_shape(row, &output)?;
    match row.comparator {
        FixtureComparator::ExactOutput => compare_expected_stdout(row, root, &output)?,
        FixtureComparator::Structural => {
            compare_core_parse_print_structure(row, &stdin, &output.stdout.bytes)?;
        }
        FixtureComparator::HelpHealth
        | FixtureComparator::Statistical
        | FixtureComparator::TwoSample => {
            return Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason: "core fixtures only support exact-output and structural comparators"
                    .to_string(),
            });
        }
    }
    Ok(output)
}

fn core_parse_print_output(
    row: &FixtureRow,
    stdin: &[u8],
) -> Result<crate::ProcessOutput, FixtureError> {
    if !is_core_fixture(row) {
        return Err(FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!("unsupported core fixture argv {}", row.argv),
        });
    }
    let tokens = row.argv_tokens();
    let Some(kind) = tokens.first().map(String::as_str) else {
        return Err(FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: "core fixture has no command token".to_string(),
        });
    };
    let input = fixture_utf8(row, "stdin", stdin)?;
    if kind == "core-time-reverse-flows" {
        return core_time_reverse_flows_output(row, input, &tokens);
    }
    if kind == "core-dem-parse-print" {
        let dem = parse_core_dem(row, "stdin", input)?;
        return Ok(crate::ProcessOutput {
            status: Some(0),
            stdout: crate::CapturedOutput {
                bytes: dem.to_dem_string().into_bytes(),
                truncated: false,
            },
            stderr: crate::CapturedOutput {
                bytes: Vec::new(),
                truncated: false,
            },
        });
    }
    let circuit = parse_core_circuit(row, "stdin", input)?;
    Ok(crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: circuit.to_stim_string().into_bytes(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    })
}

fn compare_expected_stdout(
    row: &FixtureRow,
    root: &RepoRoot,
    output: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    let expected_path = row.expected_stdout_file(root)?;
    let expected = hex_payload::read(root, &row.expected_stdout_path)?;
    if expected != output.stdout.bytes {
        return Err(FixtureError::ExpectedStdoutMismatch {
            id: row.id.clone(),
            path: expected_path,
        });
    }
    Ok(())
}

fn compare_core_parse_print_structure(
    row: &FixtureRow,
    stdin: &[u8],
    stdout: &[u8],
) -> Result<(), FixtureError> {
    if row.argv == "core-dem-parse-print" {
        let original = parse_core_dem(row, "stdin", fixture_utf8(row, "stdin", stdin)?)?;
        let reparsed = parse_core_dem(row, "printed stdout", fixture_utf8(row, "stdout", stdout)?)?;
        if original != reparsed {
            return Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason: "parse-print-parse changed DEM semantics".to_string(),
            });
        }
        return Ok(());
    }
    let original = parse_core_circuit(row, "stdin", fixture_utf8(row, "stdin", stdin)?)?;
    let reparsed = parse_core_circuit(row, "printed stdout", fixture_utf8(row, "stdout", stdout)?)?;
    if original != reparsed {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: "parse-print-parse changed circuit semantics".to_string(),
        });
    }
    Ok(())
}

fn parse_core_circuit(row: &FixtureRow, label: &str, input: &str) -> Result<Circuit, FixtureError> {
    Circuit::from_stim_str(input).map_err(|source| FixtureError::CoreFixtureFailed {
        id: row.id.clone(),
        reason: format!("{label} parse failed: {source}"),
    })
}

fn parse_core_dem(
    row: &FixtureRow,
    label: &str,
    input: &str,
) -> Result<DetectorErrorModel, FixtureError> {
    DetectorErrorModel::from_dem_str(input).map_err(|source| FixtureError::CoreFixtureFailed {
        id: row.id.clone(),
        reason: format!("{label} DEM parse failed: {source}"),
    })
}

fn fixture_utf8<'a>(
    row: &FixtureRow,
    label: &str,
    bytes: &'a [u8],
) -> Result<&'a str, FixtureError> {
    std::str::from_utf8(bytes).map_err(|source| FixtureError::CoreFixtureFailed {
        id: row.id.clone(),
        reason: format!("{label} is not UTF-8: {source}"),
    })
}

fn load_manifest(root: &RepoRoot) -> Result<FixtureManifest, OracleError> {
    let manifest = FixtureManifest::read(root)?;
    manifest.check(root)?;
    Ok(manifest)
}

fn load_manifest_with_expected_stdout_policy(
    root: &RepoRoot,
    expected_stdout_policy: ExpectedStdoutPolicy,
) -> Result<FixtureManifest, OracleError> {
    let manifest = FixtureManifest::read(root)?;
    manifest.check_with_expected_stdout_policy(root, expected_stdout_policy)?;
    Ok(manifest)
}

fn check_expected_process_shape(
    row: &FixtureRow,
    output: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    if output.status != Some(row.expected_status) {
        return Err(FixtureError::StatusMismatch {
            id: row.id.clone(),
            expected: row.expected_status,
            actual: output.status,
        });
    }
    let actual_stderr_class = output.stderr_class();
    if !row.expected_stderr_class.matches(actual_stderr_class) {
        return Err(FixtureError::StderrClassMismatch {
            id: row.id.clone(),
            expected: row.expected_stderr_class.as_str(),
            actual: actual_stderr_class,
        });
    }
    Ok(())
}

fn compare_fixture(
    row: &FixtureRow,
    stim: &crate::ProcessOutput,
    stab: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    check_expected_process_shape(row, stab)?;
    if stim.status != Some(row.expected_status) {
        return Err(FixtureError::StatusMismatch {
            id: row.id.clone(),
            expected: row.expected_status,
            actual: stim.status,
        });
    }

    let reason = match row.comparator {
        FixtureComparator::ExactOutput => compare_exact(stim, stab),
        FixtureComparator::HelpHealth => compare_help_health(stim, stab),
        FixtureComparator::Statistical => match statistical::source_for_plan(&row.statistical_plan)
        {
            Ok(statistical::StatisticalSource::Stdout) => {
                statistical::compare_statistical_plan(&row.statistical_plan, &stim.stdout.bytes)
                    .map(|reason| format!("Stim stdout: {reason}"))
                    .or_else(|| {
                        statistical::compare_statistical_plan(
                            &row.statistical_plan,
                            &stab.stdout.bytes,
                        )
                        .map(|reason| format!("Stab stdout: {reason}"))
                    })
            }
            Ok(statistical::StatisticalSource::FixtureOutput) => compare_exact(stim, stab),
            Err(reason) => Some(reason),
        },
        FixtureComparator::TwoSample => two_sample::compare_two_sample_plan(
            &row.statistical_plan,
            &stim.stdout.bytes,
            &stab.stdout.bytes,
        ),
        FixtureComparator::Structural => Some(format!(
            "{} comparator runs only for in-process core fixtures",
            row.comparator.as_str()
        )),
    };
    if let Some(reason) = reason {
        return Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason,
        });
    }
    Ok(())
}

fn validate_vendor_source(root: &RepoRoot, row: &FixtureRow, violations: &mut Vec<String>) {
    let source = Path::new(&row.upstream_source);
    if source.components().any(paths::unsafe_component) {
        violations.push(format!(
            "{} has unsafe upstream source {}",
            row.id, row.upstream_source
        ));
        return;
    }
    let path = root.stim_source().join(source);
    if !path.is_file() {
        violations.push(format!(
            "{} upstream source does not exist: {}",
            row.id, row.upstream_source
        ));
    }
}

#[cfg(test)]
mod tests;
