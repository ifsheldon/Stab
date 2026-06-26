//! Oracle fixture manifest loading and execution.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::{OracleError, RepoRoot, StderrClass, compare_exact, compare_help_health};

const FIXTURE_ROOT: &str = "oracle/fixtures";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunMode {
    ImplementedOnly,
    All,
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
enum Milestone {
    #[serde(rename = "M0")]
    M0,
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
}

impl Milestone {
    fn as_str(self) -> &'static str {
        match self {
            Self::M0 => "M0",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
        }
    }
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

    #[error("failed to read fixture file {path}: {source}")]
    ReadFile {
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

    #[error("{id} failed comparator {comparator}: {reason}")]
    ComparatorMismatch {
        id: String,
        comparator: &'static str,
        reason: String,
    },
}

impl FixtureManifest {
    fn read_from_path(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let path = path.as_ref();
        let content =
            std::fs::read_to_string(path).map_err(|source| FixtureError::ReadManifest {
                path: path.to_path_buf(),
                source,
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
            if row.comparator == FixtureComparator::ExactOutput
                && row.status != FixtureStatus::ManifestOnly
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
        let content = match std::fs::read_to_string(root.compatibility_matrix()) {
            Ok(content) => content,
            Err(error) => {
                violations.push(format!("failed to read compatibility matrix: {error}"));
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

    fn list(&self) {
        let mut groups: BTreeMap<(Milestone, ParityMode, FixtureStatus), Vec<&FixtureRow>> =
            BTreeMap::new();
        for row in &self.rows {
            groups
                .entry((row.milestone, row.parity_mode, row.status))
                .or_default()
                .push(row);
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
        let path = fixture_file(root, &self.stdin_path)?;
        std::fs::read(&path).map_err(|source| FixtureError::ReadFile { path, source })
    }

    fn expected_stdout_file(&self, root: &RepoRoot) -> Result<PathBuf, FixtureError> {
        fixture_file(root, &self.expected_stdout_path)
    }
}

pub(crate) fn list_fixtures(root: &RepoRoot) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    manifest.list();
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
    for row in manifest
        .rows
        .iter()
        .filter(|row| row.comparator == FixtureComparator::ExactOutput)
        .filter(|row| row.status != FixtureStatus::ManifestOnly)
        .filter(|row| !row.expected_stdout_path.is_empty())
    {
        let stdin = row.stdin(root)?;
        let output = crate::run_process(&stim_binary, row.argv_tokens(), &stdin, Some(&root.path))?;
        check_expected_process_shape(row, &output)?;
        let expected_path = row.expected_stdout_file(root)?;
        if check_clean {
            let expected =
                std::fs::read(&expected_path).map_err(|source| FixtureError::ReadFile {
                    path: expected_path.clone(),
                    source,
                })?;
            if expected != output.stdout.bytes {
                return Err(FixtureError::ExpectedStdoutMismatch {
                    id: row.id.clone(),
                    path: expected_path,
                }
                .into());
            }
            println!("[stab-oracle] CLEAN {}", row.id);
        } else {
            prepare_fixture_output_file(root, &row.expected_stdout_path)?;
            std::fs::write(&expected_path, &output.stdout.bytes).map_err(|source| {
                FixtureError::WriteOutput {
                    path: expected_path.clone(),
                    source,
                }
            })?;
            println!("[stab-oracle] RECORDED {}", row.id);
        }
    }
    Ok(())
}

pub(crate) fn run_fixtures(
    root: &RepoRoot,
    mode: RunMode,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let manifest = load_manifest(root)?;
    let stim_binary = crate::ensure_stim_binary(root, rebuild_stim)?;
    let stab_binary = crate::ensure_stab_cli_binary(root)?;
    for row in &manifest.rows {
        match row.status {
            FixtureStatus::Implemented => {
                let stdin = row.stdin(root)?;
                let argv = row.argv_tokens();
                let stim = crate::run_process(&stim_binary, &argv, &stdin, Some(&root.path))?;
                let stab = crate::run_process(&stab_binary, &argv, &stdin, Some(&root.path))?;
                compare_fixture(row, &stim, &stab)?;
                println!(
                    "[stab-oracle] PASS {} status={:?} stderr_class={:?}",
                    row.id,
                    stab.status,
                    stab.stderr_class()
                );
            }
            FixtureStatus::Red if mode == RunMode::All => {
                println!(
                    "[stab-oracle] RED {} [{}] {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape
                );
            }
            FixtureStatus::Ignored if mode == RunMode::All => {
                println!(
                    "[stab-oracle] IGNORED {} [{}] {}",
                    row.id,
                    row.comparator.as_str(),
                    row.command_shape
                );
            }
            FixtureStatus::ManifestOnly if mode == RunMode::All => {
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

fn load_manifest(root: &RepoRoot) -> Result<FixtureManifest, OracleError> {
    let manifest = FixtureManifest::read_from_path(root.fixture_manifest())?;
    manifest.check(root)?;
    Ok(manifest)
}

fn load_manifest_with_expected_stdout_policy(
    root: &RepoRoot,
    expected_stdout_policy: ExpectedStdoutPolicy,
) -> Result<FixtureManifest, OracleError> {
    let manifest = FixtureManifest::read_from_path(root.fixture_manifest())?;
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
    let reason = match row.comparator {
        FixtureComparator::ExactOutput => compare_exact(stim, stab),
        FixtureComparator::HelpHealth => compare_help_health(stim, stab),
        FixtureComparator::Property
        | FixtureComparator::Statistical
        | FixtureComparator::Structural => Some(format!(
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
    if source.components().any(unsafe_component) {
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

fn validate_fixture_path(
    fixture_root: &Path,
    id: &str,
    field: &str,
    relative: &str,
    requirement: FixturePathRequirement,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    validate_relative_path_components(id, field, relative_path, relative)?;
    let canonical_root = fixture_root
        .canonicalize()
        .map_err(|source| format!("failed to resolve fixture root {fixture_root:?}: {source}"))?;
    validate_existing_fixture_components(&canonical_root, id, field, relative_path, requirement)?;
    let full_path = canonical_root.join(relative_path);
    if let Ok(canonical_path) = full_path.canonicalize()
        && !canonical_path.starts_with(&canonical_root)
    {
        return Err(format!("{id} {field} escapes fixture root: {relative}"));
    }
    if let Some(parent) = full_path.parent()
        && let Ok(canonical_parent) = parent.canonicalize()
        && !canonical_parent.starts_with(&canonical_root)
    {
        return Err(format!(
            "{id} {field} parent escapes fixture root: {relative}"
        ));
    }
    Ok(())
}

fn validate_relative_path_components(
    id: &str,
    field: &str,
    relative_path: &Path,
    relative: &str,
) -> Result<(), String> {
    let mut has_component = false;
    for component in relative_path.components() {
        has_component = true;
        if unsafe_component(component) {
            return Err(format!("{id} has unsafe {field} {relative}"));
        }
    }
    if has_component {
        Ok(())
    } else {
        Err(format!("{id} has empty {field}"))
    }
}

fn validate_existing_fixture_components(
    fixture_root: &Path,
    id: &str,
    field: &str,
    relative_path: &Path,
    requirement: FixturePathRequirement,
) -> Result<(), String> {
    let mut current = fixture_root.to_path_buf();
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(format!(
                "{id} has unsafe {field} {}",
                relative_path.display()
            ));
        };
        current.push(name);
        let is_final_component = components.peek().is_none();
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "{id} {field} contains symlink: {}",
                        relative_path.display()
                    ));
                }
                if !is_final_component && !metadata.is_dir() {
                    return Err(format!(
                        "{id} {field} has non-directory parent: {}",
                        relative_path.display()
                    ));
                }
                if is_final_component && !metadata.is_file() {
                    return Err(format!(
                        "{id} {field} is not a file: {}",
                        relative_path.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if requirement == FixturePathRequirement::MustExistFile {
                    return Err(format!(
                        "{id} {field} does not exist: {}",
                        relative_path.display()
                    ));
                }
                break;
            }
            Err(error) => {
                return Err(format!(
                    "{id} failed to inspect {field} {}: {error}",
                    relative_path.display()
                ));
            }
        }
    }
    Ok(())
}

fn prepare_fixture_output_file(root: &RepoRoot, relative: &str) -> Result<(), FixtureError> {
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    let path = fixture_root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| FixtureError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))
}

fn fixture_file(root: &RepoRoot, relative: &str) -> Result<PathBuf, FixtureError> {
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(
        &fixture_root,
        "fixture",
        "path",
        relative,
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .map_err(|violation| FixtureError::Validation(violation.into_boxed_str()))?;
    Ok(fixture_root.join(relative))
}

fn unsafe_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir | Component::CurDir
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ExpectedStdoutPolicy, FixtureComparator, FixtureManifest, FixturePathRequirement,
        validate_fixture_path,
    };

    const MANIFEST_CSV: &str = include_str!("../../../oracle/fixtures/manifest.csv");
    const HEADER: &str = "id,milestone,upstream_source,parity_mode,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,status,statistical_plan,source_license_note\n";

    #[test]
    fn repository_fixture_manifest_passes_validation() {
        let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

        manifest.check(&root).expect("manifest validation");
    }

    #[test]
    fn fixture_manifest_has_implemented_smoke_cases() {
        let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
        let implemented = manifest
            .rows
            .iter()
            .filter(|row| row.status.as_str() == "implemented")
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(implemented, vec!["smoke-help", "smoke-tiny-circuit"]);
    }

    #[test]
    fn exact_output_rows_have_expected_stdout_paths() {
        let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");

        for row in manifest
            .rows
            .iter()
            .filter(|row| row.comparator == FixtureComparator::ExactOutput)
            .filter(|row| row.status != super::FixtureStatus::ManifestOnly)
        {
            assert!(!row.expected_stdout_path.is_empty(), "{}", row.id);
        }
    }

    #[test]
    fn repository_exact_output_files_exist() {
        let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

        for row in manifest
            .rows
            .iter()
            .filter(|row| row.comparator == FixtureComparator::ExactOutput)
            .filter(|row| !row.expected_stdout_path.is_empty())
        {
            assert!(
                row.expected_stdout_file(&root).unwrap().is_file(),
                "{}",
                row.id
            );
        }
    }

    #[test]
    fn validation_rejects_statistical_row_without_plan() {
        let csv = format!(
            "{HEADER}bad,M8,src/stim/cmd/command_sample.test.cc,statistical,statistical,stim sample,sample|--shots|10,inputs/sample_noisy.stim,,0,empty,red,,hand-authored\n"
        );
        let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
        let error = manifest.check(&root).expect_err("missing plan should fail");

        assert!(
            error
                .to_string()
                .contains("comparator needs structural or statistical plan text")
        );
    }

    #[test]
    fn validation_rejects_empty_argv_tokens() {
        let csv = format!(
            "{HEADER}bad,M8,src/stim/cmd/command_sample.test.cc,statistical,statistical,stim sample,sample||--shots|10,inputs/sample_noisy.stim,,0,empty,red,sample_count=10; fixed_seed=1; false_positive_rate<=0.001,hand-authored\n"
        );
        let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
        let error = manifest
            .check(&root)
            .expect_err("empty argv token should fail");

        assert!(error.to_string().contains("has an empty argv token"));
    }

    #[test]
    fn validation_requires_fixture_milestone_and_parity_to_match_matrix() {
        let csv = format!(
            "{HEADER}m7-convert-01-to-dets,M7,src/stim/cmd/command_convert.test.cc,exact-output,exact-output,stim convert 01 to dets,convert|--in_format=01|--out_format=dets,inputs/convert_measurements.01,expected/m7_convert_01_to_dets.stdout,0,empty,red,,hand-authored\n"
        );
        let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
        let error = manifest
            .check(&root)
            .expect_err("M7 convert row must not satisfy M4 coverage");

        assert!(error.to_string().contains(
            "missing M2 fixture row for src/stim/cmd/command_convert.test.cc (M4/exact-output)"
        ));
    }

    #[test]
    fn validation_requires_declared_expected_stdout_by_default() {
        let csv = format!(
            "{HEADER}bad,M4,src/stim/cmd/command_convert.test.cc,exact-output,exact-output,stab-core circuit parse print,core-circuit-parse-print,inputs/parser_basic.stim,expected/missing-golden.stdout,0,any,manifest-only,,hand-authored\n"
        );
        let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

        let error = manifest
            .check(&root)
            .expect_err("declared expected stdout should exist during validation");
        assert!(
            error.to_string().contains(
                "bad expected_stdout_path does not exist: expected/missing-golden.stdout"
            )
        );

        let allow_missing_error = manifest
            .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
            .expect_err("synthetic manifest should still fail compatibility coverage");
        assert!(
            !allow_missing_error.to_string().contains(
                "bad expected_stdout_path does not exist: expected/missing-golden.stdout"
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn fixture_path_validation_rejects_symlink_components() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture_root = temp.path().join("fixtures");
        std::fs::create_dir(&fixture_root).expect("create fixture root");
        let outside = temp.path().join("outside.stdout");
        std::fs::write(&outside, b"outside").expect("write outside file");
        std::os::unix::fs::symlink(&outside, fixture_root.join("link.stdout"))
            .expect("create symlink");

        let error = validate_fixture_path(
            &fixture_root,
            "bad",
            "expected_stdout_path",
            "link.stdout",
            FixturePathRequirement::ExistingFileIfPresent,
        )
        .expect_err("symlink fixture path should fail");

        assert!(error.contains("contains symlink"));
    }
}
