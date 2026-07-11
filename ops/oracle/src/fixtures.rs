//! Oracle fixture manifest loading and execution.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use stab_core::{Circuit, DetectorErrorModel};
use thiserror::Error;

use crate::{OracleError, RepoRoot, StderrClass, compare_exact, compare_help_health};

mod direct_rust;
mod milestone;
mod outputs;
mod paths;
mod reverse_flow;
mod statistical;

#[cfg(test)]
use direct_rust::{cargo_test_passed_test_count, check_direct_rust_fixture_executed_tests};
use direct_rust::{is_direct_rust_fixture, run_direct_rust_fixture};
pub(crate) use milestone::Milestone;
use paths::{fixture_file, validate_fixture_path};
use reverse_flow::core_time_reverse_flows_output;

const FIXTURE_ROOT: &str = "oracle/fixtures";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunMode {
    ImplementedOnly,
    All,
    Milestone(String),
}

impl RunMode {
    fn milestone_filter(&self) -> Result<Option<Milestone>, FixtureError> {
        match self {
            Self::Milestone(milestone) => parse_milestone(milestone).map(Some),
            Self::ImplementedOnly | Self::All => Ok(None),
        }
    }

    fn reports_pending(&self) -> bool {
        matches!(self, Self::All | Self::Milestone(_))
    }
}

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

    fn matches(self, row: &FixtureRow) -> bool {
        match self {
            Self::Exact => {
                matches!(
                    row.parity_mode,
                    ParityMode::ExactOutput | ParityMode::ExactOutputAndStatistical
                ) && row.comparator != FixtureComparator::Statistical
            }
            Self::Statistical => row.comparator == FixtureComparator::Statistical,
            Self::Structural => {
                row.parity_mode == ParityMode::Structural
                    || row.comparator == FixtureComparator::Structural
            }
        }
    }
}

fn parse_milestone(value: &str) -> Result<Milestone, FixtureError> {
    Milestone::parse(value).map_err(|milestone| FixtureError::UnknownMilestone { milestone })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct FixtureRow {
    id: String,
    milestone: Milestone,
    upstream_source: String,
    parity_mode: ParityMode,
    comparator: FixtureComparator,
    command_shape: String,
    argv: String,
    stdin_path: String,
    expected_stdout_path: String,
    expected_status: i32,
    expected_stderr_class: ExpectedStderrClass,
    status: FixtureStatus,
    statistical_plan: String,
    source_license_note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum ParityMode {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "exact-output-and-statistical")]
    ExactOutputAndStatistical,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
}

impl ParityMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactOutput => "exact-output",
            Self::ExactOutputAndStatistical => "exact-output-and-statistical",
            Self::Property => "property",
            Self::Statistical => "statistical",
            Self::Structural => "structural",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum FixtureComparator {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "help-health")]
    HelpHealth,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
}

impl FixtureComparator {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactOutput => "exact-output",
            Self::HelpHealth => "help-health",
            Self::Property => "property",
            Self::Statistical => "statistical",
            Self::Structural => "structural",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum FixtureStatus {
    #[serde(rename = "implemented")]
    Implemented,
    #[serde(rename = "ignored")]
    Ignored,
    #[serde(rename = "manifest-only")]
    ManifestOnly,
    #[serde(rename = "red")]
    Red,
}

impl FixtureStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::Ignored => "ignored",
            Self::ManifestOnly => "manifest-only",
            Self::Red => "red",
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct CompatibilityRow {
    upstream_path: String,
    source_kind: CompatibilitySourceKind,
    milestone: CompatibilityMilestone,
    priority: CompatibilityPriority,
    parity_mode: CompatibilityParityMode,
    status: CompatibilityStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilitySourceKind {
    #[serde(rename = "cxx-test")]
    CxxTest,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityMilestone {
    #[serde(rename = "M4")]
    M4,
    #[serde(rename = "M5")]
    M5,
    #[serde(rename = "M6")]
    M6,
    #[serde(rename = "M7")]
    M7,
    #[serde(rename = "M8")]
    M8,
    #[serde(rename = "M9")]
    M9,
    #[serde(rename = "M10")]
    M10,
    #[serde(rename = "M11")]
    M11,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityPriority {
    #[serde(rename = "P0")]
    P0,
    #[serde(rename = "P1")]
    P1,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityParityMode {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "exact-output-and-statistical")]
    ExactOutputAndStatistical,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CompatibilityStatus {
    #[serde(rename = "planned")]
    Planned,
    #[serde(other)]
    Other,
}

impl CompatibilityRow {
    fn requires_fixture(&self) -> bool {
        self.source_kind == CompatibilitySourceKind::CxxTest
            && matches!(
                self.milestone,
                CompatibilityMilestone::M4
                    | CompatibilityMilestone::M5
                    | CompatibilityMilestone::M6
                    | CompatibilityMilestone::M7
                    | CompatibilityMilestone::M8
                    | CompatibilityMilestone::M9
                    | CompatibilityMilestone::M10
                    | CompatibilityMilestone::M11
            )
            && matches!(
                self.priority,
                CompatibilityPriority::P0 | CompatibilityPriority::P1
            )
            && self.status == CompatibilityStatus::Planned
    }

    fn fixture_milestone(&self) -> Option<Milestone> {
        match self.milestone {
            CompatibilityMilestone::M4 => Some(Milestone::M4),
            CompatibilityMilestone::M5 => Some(Milestone::M5),
            CompatibilityMilestone::M6 => Some(Milestone::M6),
            CompatibilityMilestone::M7 => Some(Milestone::M7),
            CompatibilityMilestone::M8 => Some(Milestone::M8),
            CompatibilityMilestone::M9 => Some(Milestone::M9),
            CompatibilityMilestone::M10 => Some(Milestone::M10),
            CompatibilityMilestone::M11 => Some(Milestone::M11),
            CompatibilityMilestone::Other => None,
        }
    }

    fn fixture_parity_mode(&self) -> Option<ParityMode> {
        match self.parity_mode {
            CompatibilityParityMode::ExactOutput => Some(ParityMode::ExactOutput),
            CompatibilityParityMode::ExactOutputAndStatistical => {
                Some(ParityMode::ExactOutputAndStatistical)
            }
            CompatibilityParityMode::Property => Some(ParityMode::Property),
            CompatibilityParityMode::Statistical => Some(ParityMode::Statistical),
            CompatibilityParityMode::Structural => Some(ParityMode::Structural),
            CompatibilityParityMode::Other => None,
        }
    }
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

    #[error(
        "unknown fixture milestone {milestone}; expected one of M0, M4, M5, M6, M7, M8, M9, M10, M11, M12, PF1, PF2, PF3, PF4, PF5, PF6, or PF7"
    )]
    UnknownMilestone { milestone: String },

    #[error("failed to read fixture file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("fixture file {path} exceeds the {limit}-byte limit")]
    FixtureFileTooLarge { path: PathBuf, limit: usize },

    #[error("failed to inspect fixture output {path}: {source}")]
    InspectOutput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to create fixture output directory {path}: {source}")]
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write fixture output {path}: {source}")]
    WriteOutput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("unsafe fixture scratch path {path}: {reason}")]
    UnsafeScratchPath { path: PathBuf, reason: String },

    #[error("{id} expected status {expected}, got {actual:?}")]
    StatusMismatch {
        id: String,
        expected: i32,
        actual: Option<i32>,
    },

    #[error("{id} expected stderr class {expected}, got {actual:?}")]
    StderrClassMismatch {
        id: String,
        expected: &'static str,
        actual: StderrClass,
    },

    #[error("{id} expected stdout differs from {path}")]
    ExpectedStdoutMismatch { id: String, path: PathBuf },

    #[error("{id} expected fixture output differs from {path}")]
    ExpectedFixtureOutputMismatch { id: String, path: PathBuf },

    #[error("{id} fixture output {path} exceeds {limit} bytes")]
    AuxiliaryOutputTooLarge {
        id: String,
        path: PathBuf,
        limit: u64,
    },

    #[error("{id} failed core fixture execution: {reason}")]
    CoreFixtureFailed { id: String, reason: String },

    #[error("{id} failed comparator {comparator}: {reason}")]
    ComparatorMismatch {
        id: String,
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
        let mut ids = BTreeSet::new();
        let fixture_root = root.path.join(FIXTURE_ROOT);
        if let Err(violation) = paths::validate_fixture_root(root) {
            return Err(FixtureError::Validation(violation.into_boxed_str()));
        }
        for row in &self.rows {
            if row.id.is_empty() {
                violations.push("row with empty id".to_string());
            } else if !ids.insert(row.id.clone()) {
                violations.push(format!("duplicate fixture id {}", row.id));
            }
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
            if is_direct_rust_fixture(row) && row.argv_tokens().len() < 2 {
                violations.push(format!("{} cargo-test row has no cargo arguments", row.id));
            }
            if row.comparator == FixtureComparator::ExactOutput
                && row.status != FixtureStatus::ManifestOnly
                && row.expected_status == 0
                && row.expected_stdout_path.is_empty()
            {
                violations.push(format!("{} exact fixture has no expected stdout", row.id));
            }
            if matches!(
                row.comparator,
                FixtureComparator::Property
                    | FixtureComparator::Statistical
                    | FixtureComparator::Structural
            ) && row.statistical_plan.is_empty()
            {
                violations.push(format!(
                    "{} comparator needs structural or statistical plan text",
                    row.id
                ));
            }
            if row.comparator == FixtureComparator::Statistical
                && row.status != FixtureStatus::ManifestOnly
            {
                let argv_tokens = row.argv_tokens();
                if let Some(reason) =
                    statistical::validate_statistical_plan(&row.statistical_plan, &argv_tokens)
                {
                    violations.push(format!("{} invalid statistical plan: {reason}", row.id));
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
                    validate_fixture_path(&fixture_root, &row.id, field, relative, must_exist)
                        .unwrap_or_else(|violation| violations.push(violation));
                }
            }
        }
        self.check_compatibility_coverage(root, &mut violations);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(FixtureError::Validation(
                violations.join("\n").into_boxed_str(),
            ))
        }
    }

    fn check_compatibility_coverage(&self, root: &RepoRoot, violations: &mut Vec<String>) {
        let fixture_keys = self
            .rows
            .iter()
            .map(|row| (row.upstream_source.as_str(), row.milestone, row.parity_mode))
            .collect::<BTreeSet<_>>();
        let matrix_path = root.compatibility_matrix();
        let bytes = match crate::safe_file::read_regular_file_bounded(
            &matrix_path,
            crate::matrix::MAX_COMPATIBILITY_MATRIX_BYTES,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                violations.push(format!("failed to read compatibility matrix: {error}"));
                return;
            }
        };
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(error) => {
                violations.push(format!("compatibility matrix is not UTF-8: {error}"));
                return;
            }
        };
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        for row in reader.deserialize::<CompatibilityRow>() {
            match row {
                Ok(row) => {
                    if row.requires_fixture() {
                        let Some(milestone) = row.fixture_milestone() else {
                            violations.push(format!(
                                "missing M2 fixture milestone mapping for {}",
                                row.upstream_path
                            ));
                            continue;
                        };
                        let Some(parity_mode) = row.fixture_parity_mode() else {
                            violations.push(format!(
                                "missing M2 fixture parity mapping for {}",
                                row.upstream_path
                            ));
                            continue;
                        };
                        if !fixture_keys.contains(&(
                            row.upstream_path.as_str(),
                            milestone,
                            parity_mode,
                        )) {
                            violations.push(format!(
                                "missing M2 fixture row for {} ({}/{})",
                                row.upstream_path,
                                milestone.as_str(),
                                parity_mode.as_str()
                            ));
                        }
                    }
                }
                Err(error) => {
                    violations.push(format!("failed to parse compatibility matrix row: {error}"));
                }
            }
        }
    }

    fn list(&self, milestone_filter: Option<Milestone>) {
        let mut groups: BTreeMap<(Milestone, ParityMode, FixtureStatus), Vec<&FixtureRow>> =
            BTreeMap::new();
        for row in self.rows.iter().filter(|row| {
            milestone_filter
                .map(|milestone| row.milestone == milestone)
                .unwrap_or(true)
        }) {
            groups
                .entry((row.milestone, row.parity_mode, row.status))
                .or_default()
                .push(row);
        }
        if groups.is_empty() {
            if let Some(milestone) = milestone_filter {
                println!(
                    "[stab-oracle] no fixtures are declared for {}",
                    milestone.as_str()
                );
            }
            return;
        }
        for ((milestone, parity_mode, status), rows) in groups {
            println!(
                "{} / {} / {}:",
                milestone.as_str(),
                parity_mode.as_str(),
                status.as_str()
            );
            for row in rows {
                println!(
                    "- {} [{}] {} -> {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape,
                    row.upstream_source
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
        paths::read_fixture_file(root, &self.stdin_path)
    }

    fn expected_stdout_file(&self, root: &RepoRoot) -> Result<PathBuf, FixtureError> {
        fixture_file(root, &self.expected_stdout_path)
    }
}

pub(crate) fn list_fixtures(root: &RepoRoot, milestone: Option<&str>) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    let milestone_filter = milestone.map(parse_milestone).transpose()?;
    manifest.list(milestone_filter);
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
            let expected = paths::read_fixture_file(root, &row.expected_stdout_path)?;
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
            paths::write_fixture_file(root, &row.expected_stdout_path, &output.stdout.bytes)?;
            outputs::record_outputs(row, root, &command.outputs)?;
            println!("[stab-oracle] RECORDED {}", row.id);
        }
    }
    Ok(())
}

fn is_recordable(row: &FixtureRow) -> bool {
    row.comparator == FixtureComparator::ExactOutput
        && row.status != FixtureStatus::ManifestOnly
        && (!is_core_fixture(row) || reverse_flow::is_reverse_flow_fixture(row))
        && !row.expected_stdout_path.is_empty()
}

pub(crate) fn run_fixtures(
    root: &RepoRoot,
    mode: RunMode,
    filter: Option<RunFilter>,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    let milestone_filter = mode.milestone_filter()?;
    let reports_pending = mode.reports_pending();
    let mut stim_binary = None;
    let mut stab_binary = None;
    for row in &manifest.rows {
        if !matches_milestone_filter(row, milestone_filter) {
            continue;
        }
        if filter.is_some_and(|filter| !filter.matches(row)) {
            continue;
        }
        match row.status {
            FixtureStatus::Implemented => {
                let stab = if is_core_fixture(row) {
                    run_core_fixture(root, row)?
                } else if is_direct_rust_fixture(row) {
                    run_direct_rust_fixture(root, row)?
                } else {
                    let stdin = row.stdin(root)?;
                    let stim_binary_path =
                        cached_stim_binary(root, rebuild_stim, &mut stim_binary)?;
                    let stab_binary_path = cached_stab_binary(root, &mut stab_binary)?;
                    let stim_command = outputs::prepare_command(root, row, "stim")?;
                    let stab_command = outputs::prepare_command(root, row, "stab")?;
                    let stim = run_prepared_fixture_process(
                        root,
                        &stim_binary_path,
                        &stim_command,
                        &stdin,
                    )?;
                    let stab = run_prepared_fixture_process(
                        root,
                        &stab_binary_path,
                        &stab_command,
                        &stdin,
                    )?;
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
            FixtureStatus::Red if reports_pending => {
                println!(
                    "[stab-oracle] RED {} [{}] {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape
                );
            }
            FixtureStatus::Ignored if reports_pending => {
                println!(
                    "[stab-oracle] IGNORED {} [{}] {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape
                );
            }
            FixtureStatus::ManifestOnly if reports_pending => {
                println!(
                    "[stab-oracle] MANIFEST-ONLY {} [{}] {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn run_prepared_fixture_process(
    root: &RepoRoot,
    program: &Path,
    command: &outputs::PreparedFixtureCommand,
    stdin: &[u8],
) -> Result<crate::ProcessOutput, OracleError> {
    let monitored_paths = command
        .outputs
        .iter()
        .map(|output| output.actual_path.as_path())
        .collect::<Vec<_>>();
    crate::process::run_process_monitoring_files(
        program,
        &command.argv,
        stdin,
        Some(&root.path),
        &monitored_paths,
        outputs::AUXILIARY_OUTPUT_LIMIT_BYTES,
    )
}

fn matches_milestone_filter(row: &FixtureRow, milestone_filter: Option<Milestone>) -> bool {
    milestone_filter
        .map(|milestone| row.milestone == milestone)
        .unwrap_or(true)
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
        | FixtureComparator::Property
        | FixtureComparator::Statistical => {
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
    let expected = paths::read_fixture_file(root, &row.expected_stdout_path)?;
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
                statistical::compare_statistical_plan(&row.statistical_plan, &stab.stdout.bytes)
            }
            Ok(statistical::StatisticalSource::FixtureOutput) => compare_exact(stim, stab),
            Err(reason) => Some(reason),
        },
        FixtureComparator::Property | FixtureComparator::Structural => Some(format!(
            "{} comparator is not runnable until the milestone implementation defines it",
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
