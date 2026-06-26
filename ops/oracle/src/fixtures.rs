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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct CompatibilityRow {
    upstream_path: String,
    source_kind: CompatibilitySourceKind,
    milestone: CompatibilityMilestone,
    priority: CompatibilityPriority,
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
                    !row.stdin_path.is_empty(),
                ),
                (
                    "expected_stdout_path",
                    row.expected_stdout_path.as_str(),
                    false,
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
        let fixture_sources = self
            .rows
            .iter()
            .map(|row| row.upstream_source.as_str())
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
                    if row.requires_fixture()
                        && !fixture_sources.contains(row.upstream_path.as_str())
                    {
                        violations
                            .push(format!("missing M2 fixture row for {}", row.upstream_path));
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

    fn stdin(&self, root: &RepoRoot) -> Result<String, FixtureError> {
        if self.stdin_path.is_empty() {
            return Ok(String::new());
        }
        let path = fixture_file(root, &self.stdin_path)?;
        std::fs::read_to_string(&path).map_err(|source| FixtureError::ReadFile { path, source })
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
    let manifest = load_manifest(root)?;
    let stim_binary = crate::ensure_stim_binary(root, rebuild_stim)?;
    for row in manifest
        .rows
        .iter()
        .filter(|row| row.comparator == FixtureComparator::ExactOutput)
        .filter(|row| !row.expected_stdout_path.is_empty())
    {
        let output = crate::run_process(
            &stim_binary,
            row.argv_tokens(),
            &row.stdin(root)?,
            Some(&root.path),
        )?;
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
            if let Some(parent) = expected_path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| {
                    FixtureError::CreateOutputDir {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
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
    must_exist: bool,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    if relative_path.components().any(unsafe_component) {
        return Err(format!("{id} has unsafe {field} {relative}"));
    }
    let full_path = fixture_root.join(relative_path);
    if must_exist && !full_path.is_file() {
        return Err(format!("{id} {field} does not exist: {relative}"));
    }
    Ok(())
}

fn fixture_file(root: &RepoRoot, relative: &str) -> Result<PathBuf, FixtureError> {
    let fixture_root = root.path.join(FIXTURE_ROOT);
    validate_fixture_path(&fixture_root, "fixture", "path", relative, false)
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
    use super::{FixtureComparator, FixtureManifest};

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
}
