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
mod row;
mod statistical;
mod two_sample;

pub(crate) use id::FixtureId;
use paths::{fixture_file, validate_fixture_path};
use reverse_flow::core_time_reverse_flows_output;
use row::{CoreFixture, FixtureRow, RawFixtureRow, RowPlan};

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

#[derive(Clone, Debug, PartialEq)]
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

    #[error("{id} expected {side} status {expected}, got {actual:?}")]
    StatusMismatch {
        id: FixtureId,
        side: ProcessSide,
        expected: i32,
        actual: Option<i32>,
    },

    #[error("{id} expected {side} stderr class {expected}, got {actual:?}")]
    StderrClassMismatch {
        id: FixtureId,
        side: ProcessSide,
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

/// Which implementation produced an observed process shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessSide {
    /// Pinned Stim, through its CLI or a libstim helper.
    Stim,
    /// The Stab CLI or an in-process Stab core fixture.
    Stab,
}

impl std::fmt::Display for ProcessSide {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Stim => "Stim",
            Self::Stab => "Stab",
        })
    }
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

    /// Parses every row's `argv` and `statistical_plan` into typed values, reporting every
    /// malformed row at once.
    fn from_csv(content: &str) -> Result<Self, FixtureError> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        let raw_rows = reader
            .deserialize::<RawFixtureRow>()
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = Vec::with_capacity(raw_rows.len());
        let mut violations = Vec::new();
        for raw in raw_rows {
            match FixtureRow::try_from(raw) {
                Ok(row) => rows.push(row),
                Err(row_violations) => violations.extend(row_violations),
            }
        }
        if !violations.is_empty() {
            return Err(FixtureError::Validation(
                violations.join("\n").into_boxed_str(),
            ));
        }
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
    let mut stim = PinnedStim::new(root, rebuild_stim);
    for row in manifest.rows.iter().filter(|row| is_recordable(row)) {
        crate::check_cancelled()?;
        let stdin = row.stdin(root)?;
        let (output, command) = match row.core() {
            Some(core) => (stim.run_core(row, core, &stdin)?, None),
            None => {
                let command = outputs::prepare_command(root, row, "stim-record")?;
                let output = run_prepared_fixture_process(root, stim.binary()?, &command, &stdin)?;
                (output, Some(command))
            }
        };
        check_expected_process_shape(row, ProcessSide::Stim, &output)?;
        let fixture_outputs = command.as_ref().map_or(&[][..], |command| &command.outputs);
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
            outputs::compare_expected_outputs(row, root, fixture_outputs)?;
            println!("[stab-oracle] CLEAN {}", row.id);
        } else {
            hex_payload::write(root, &row.expected_stdout_path, &output.stdout.bytes)?;
            outputs::record_outputs(row, root, fixture_outputs)?;
            println!("[stab-oracle] RECORDED {}", row.id);
        }
    }
    Ok(())
}

/// Every exact-output row with a stored golden is recorded from, and checked against, pinned
/// Stim: CLI rows through the `stim` binary and core rows through a libstim helper.
fn is_recordable(row: &FixtureRow) -> bool {
    row.comparator == FixtureComparator::ExactOutput && !row.expected_stdout_path.is_empty()
}

/// Lazily built pinned Stim programs for one record or run pass.
struct PinnedStim<'a> {
    root: &'a RepoRoot,
    rebuild: bool,
    binary: Option<PathBuf>,
    reverse_flow: Option<PathBuf>,
    parse_print: Option<PathBuf>,
}

impl<'a> PinnedStim<'a> {
    const fn new(root: &'a RepoRoot, rebuild: bool) -> Self {
        Self {
            root,
            rebuild,
            binary: None,
            reverse_flow: None,
            parse_print: None,
        }
    }

    fn binary(&mut self) -> Result<&Path, OracleError> {
        if self.binary.is_none() {
            self.binary = Some(crate::ensure_stim_binary(self.root, self.rebuild)?);
        }
        self.binary
            .as_deref()
            .ok_or_else(|| missing_program("pinned Stim"))
    }

    /// Runs the pinned libstim equivalent of a core fixture.
    fn run_core(
        &mut self,
        row: &FixtureRow,
        core: &CoreFixture,
        stdin: &[u8],
    ) -> Result<crate::ProcessOutput, OracleError> {
        let (root, rebuild) = (self.root, self.rebuild);
        match core {
            CoreFixture::TimeReverseFlows { case_id } => {
                if self.reverse_flow.is_none() {
                    self.reverse_flow =
                        Some(crate::ensure_stim_reverse_flow_helper(root, rebuild)?);
                }
                let helper = self
                    .reverse_flow
                    .as_deref()
                    .ok_or_else(|| missing_program("reverse-flow helper"))?;
                reverse_flow::run_pinned_stim_reverse_flow(root, row, case_id, stdin, helper)
            }
            CoreFixture::CircuitParsePrint | CoreFixture::DemParsePrint => {
                if self.parse_print.is_none() {
                    self.parse_print = Some(crate::ensure_stim_parse_print_helper(root, rebuild)?);
                }
                let helper = self
                    .parse_print
                    .as_deref()
                    .ok_or_else(|| missing_program("parse-print helper"))?;
                let mode = if *core == CoreFixture::DemParsePrint {
                    "dem"
                } else {
                    "circuit"
                };
                crate::run_process(helper, [mode], stdin, &root.path)
            }
        }
    }
}

fn missing_program(label: &str) -> OracleError {
    OracleError::InvalidRunSelection(format!("internal error: {label} was not prepared"))
}

/// Runs every manifest fixture, optionally restricted to one comparator.
pub(crate) fn run_fixtures(
    root: &RepoRoot,
    filter: Option<RunFilter>,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    let mut stim = PinnedStim::new(root, rebuild_stim);
    let mut stab_binary = None;
    for row in &manifest.rows {
        if filter.is_some_and(|filter| !filter.matches(row)) {
            continue;
        }
        crate::check_cancelled()?;
        let stab = if let Some(core) = row.core() {
            run_core_fixture(root, row, core)?
        } else {
            let stdin = row.stdin(root)?;
            let stim_binary_path = stim.binary()?.to_path_buf();
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
    crate::process::run_process_with_file_limit(
        program,
        &command.argv,
        stdin,
        &root.path,
        outputs::AUXILIARY_OUTPUT_LIMIT_BYTES,
    )
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

fn run_core_fixture(
    root: &RepoRoot,
    row: &FixtureRow,
    core: &CoreFixture,
) -> Result<crate::ProcessOutput, FixtureError> {
    let stdin = row.stdin(root)?;
    let output = core_parse_print_output(row, core, &stdin)?;
    check_expected_process_shape(row, ProcessSide::Stab, &output)?;
    match row.comparator {
        FixtureComparator::ExactOutput => compare_expected_stdout(row, root, &output)?,
        FixtureComparator::Structural => {
            compare_core_parse_print_structure(row, core, &stdin, &output.stdout.bytes)?;
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
    core: &CoreFixture,
    stdin: &[u8],
) -> Result<crate::ProcessOutput, FixtureError> {
    let input = fixture_utf8(row, "stdin", stdin)?;
    let stdout = match core {
        CoreFixture::TimeReverseFlows { case_id } => {
            return core_time_reverse_flows_output(row, input, case_id);
        }
        CoreFixture::DemParsePrint => parse_core_dem(row, "stdin", input)?.to_dem_string(),
        CoreFixture::CircuitParsePrint => parse_core_circuit(row, "stdin", input)?.to_stim_string(),
    };
    Ok(crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: stdout.into_bytes(),
        },
        stderr: crate::CapturedOutput { bytes: Vec::new() },
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
    core: &CoreFixture,
    stdin: &[u8],
    stdout: &[u8],
) -> Result<(), FixtureError> {
    let input = fixture_utf8(row, "stdin", stdin)?;
    let printed = fixture_utf8(row, "stdout", stdout)?;
    let changed = match core {
        CoreFixture::DemParsePrint => (parse_core_dem(row, "stdin", input)?
            != parse_core_dem(row, "printed stdout", printed)?)
        .then_some("parse-print-parse changed DEM semantics"),
        CoreFixture::CircuitParsePrint => (parse_core_circuit(row, "stdin", input)?
            != parse_core_circuit(row, "printed stdout", printed)?)
        .then_some("parse-print-parse changed circuit semantics"),
        CoreFixture::TimeReverseFlows { .. } => {
            Some("reverse-flow fixtures have no structural comparison")
        }
    };
    match changed {
        Some(reason) => Err(FixtureError::ComparatorMismatch {
            id: row.id.clone(),
            comparator: row.comparator.as_str(),
            reason: reason.to_string(),
        }),
        None => Ok(()),
    }
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
    side: ProcessSide,
    output: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    if output.status != Some(row.expected_status) {
        return Err(FixtureError::StatusMismatch {
            id: row.id.clone(),
            side,
            expected: row.expected_status,
            actual: output.status,
        });
    }
    let actual_stderr_class = output.stderr_class();
    if !row.expected_stderr_class.matches(actual_stderr_class) {
        return Err(FixtureError::StderrClassMismatch {
            id: row.id.clone(),
            side,
            expected: row.expected_stderr_class.as_str(),
            actual: actual_stderr_class,
        });
    }
    Ok(())
}

/// Checks both processes against the row's declared status and stderr class, then applies the
/// row's comparator.
fn compare_fixture(
    row: &FixtureRow,
    stim: &crate::ProcessOutput,
    stab: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    check_expected_process_shape(row, ProcessSide::Stab, stab)?;
    check_expected_process_shape(row, ProcessSide::Stim, stim)?;

    let reason = match (row.comparator, &row.plan) {
        (FixtureComparator::ExactOutput, _) => compare_exact(stim, stab),
        (FixtureComparator::HelpHealth, _) => compare_help_health(stim, stab),
        (FixtureComparator::Statistical, RowPlan::Statistical(plan)) => match plan.source() {
            statistical::StatisticalSource::Stdout => plan
                .compare(&stim.stdout.bytes)
                .map(|reason| format!("Stim stdout: {reason}"))
                .or_else(|| {
                    plan.compare(&stab.stdout.bytes)
                        .map(|reason| format!("Stab stdout: {reason}"))
                }),
            statistical::StatisticalSource::FixtureOutput => compare_exact(stim, stab),
        },
        (FixtureComparator::TwoSample, RowPlan::TwoSample(plan)) => {
            two_sample::compare_two_sample_plan(plan, &stim.stdout.bytes, &stab.stdout.bytes)
        }
        (FixtureComparator::Structural, _) => Some(format!(
            "{} comparator runs only for in-process core fixtures",
            row.comparator.as_str()
        )),
        (FixtureComparator::Statistical | FixtureComparator::TwoSample, _) => Some(format!(
            "internal error: {} row has no parsed plan",
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
