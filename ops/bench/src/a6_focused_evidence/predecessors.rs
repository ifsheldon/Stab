use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::focused_error;
use crate::error::BenchError;
use crate::manifest::is_safe_benchmark_id;
use crate::process::{
    OutputPolicy, ProcessEnvironment, ProcessLimits, ProcessRequest, ProcessResult,
    run_bounded_process,
};
use crate::qualification::{GitCommit, Sha256Digest};
use crate::root::RepoRoot;
use crate::source_file::read_repo_regular_file_bounded;

pub(super) const REGISTRY_PATH: &str = "benchmarks/a6-predecessors.json";

const REGISTRY_SCHEMA_VERSION: u32 = 1;
const PATCH_DIGEST_CONTRACT: &str = "git-raw-tree-delta-sha256-v1";
const PRESERVATION_TAG_PREFIX: &str = "refs/tags/a6-predecessors/";
const GIT: &str = "/usr/bin/git";
const MAX_REGISTRY_BYTES: usize = 1 << 20;
const MAX_PATCH_BYTES: usize = 64 << 20;
const MAX_GIT_DIAGNOSTIC_BYTES: usize = 64 << 10;
const MAX_BACKPORTS: usize = 256;
const MAX_PHASES: usize = 256;
const MAX_MEASUREMENT_BYTES: usize = 256;
const GIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PredecessorRegistry {
    schema_version: u32,
    patch_digest_contract: String,
    backports: Vec<BackportRecord>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackportRecord {
    historical_product_commit: String,
    instrumentation_backport_commit: String,
    patch_sha256: String,
    phases: Vec<PhaseKey>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub(super) struct PhaseKey {
    pub(super) row_id: String,
    pub(super) measurement: String,
}

impl PhaseKey {
    pub(super) fn new(row_id: impl Into<String>, measurement: impl Into<String>) -> Self {
        Self {
            row_id: row_id.into(),
            measurement: measurement.into(),
        }
    }

    fn display(&self) -> String {
        format!("{}/{}", self.row_id, self.measurement)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PredecessorIdentity {
    pub(super) historical_product_commit: String,
    pub(super) instrumentation_backport_commit: String,
    pub(super) patch_sha256: String,
}

impl From<&BackportRecord> for PredecessorIdentity {
    fn from(record: &BackportRecord) -> Self {
        Self {
            historical_product_commit: record.historical_product_commit.clone(),
            instrumentation_backport_commit: record.instrumentation_backport_commit.clone(),
            patch_sha256: record.patch_sha256.clone(),
        }
    }
}

#[derive(Debug)]
pub(super) struct ValidatedPredecessorRegistry {
    source_sha256: String,
    by_phase: BTreeMap<PhaseKey, PredecessorIdentity>,
}

impl ValidatedPredecessorRegistry {
    pub(super) fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub(super) fn phases(&self) -> impl Iterator<Item = &PhaseKey> {
        self.by_phase.keys()
    }

    pub(super) fn identity_for(
        &self,
        phase: &PhaseKey,
    ) -> Result<&PredecessorIdentity, BenchError> {
        self.by_phase.get(phase).ok_or_else(|| {
            focused_error(format!(
                "predecessor registry omits phase {}",
                phase.display()
            ))
        })
    }

    pub(super) fn require_report_commit(
        &self,
        phase: &PhaseKey,
        report_commit: &str,
    ) -> Result<&PredecessorIdentity, BenchError> {
        let identity = self.identity_for(phase)?;
        if report_commit != identity.instrumentation_backport_commit {
            return Err(focused_error(format!(
                "predecessor report for {} records commit {report_commit}, expected registered instrumentation backport {}",
                phase.display(),
                identity.instrumentation_backport_commit
            )));
        }
        Ok(identity)
    }
}

impl PredecessorRegistry {
    pub(super) fn from_bytes(bytes: &[u8]) -> Result<Self, BenchError> {
        serde_json::from_slice(bytes)
            .map_err(|error| focused_error(format!("failed to parse {REGISTRY_PATH}: {error}")))
    }

    pub(super) fn validate_structure(&self) -> Result<(), BenchError> {
        let mut issues = Vec::new();
        if self.schema_version != REGISTRY_SCHEMA_VERSION {
            issues.push(format!(
                "schema_version={} expected {REGISTRY_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.patch_digest_contract != PATCH_DIGEST_CONTRACT {
            issues.push(format!(
                "patch_digest_contract={:?} expected {PATCH_DIGEST_CONTRACT:?}",
                self.patch_digest_contract
            ));
        }
        if self.backports.len() > MAX_BACKPORTS {
            issues.push(format!(
                "backports contains {} entries, expected at most {MAX_BACKPORTS}",
                self.backports.len()
            ));
        }

        let mut backport_commits = BTreeSet::new();
        let mut phases = BTreeSet::new();
        let mut phase_count = 0usize;
        for record in &self.backports {
            validate_revision_field(
                "historical_product_commit",
                &record.historical_product_commit,
                &mut issues,
            );
            validate_revision_field(
                "instrumentation_backport_commit",
                &record.instrumentation_backport_commit,
                &mut issues,
            );
            if record.historical_product_commit == record.instrumentation_backport_commit {
                issues.push(format!(
                    "backport {} uses its instrumentation commit as the historical product commit",
                    record.instrumentation_backport_commit
                ));
            }
            if !backport_commits.insert(record.instrumentation_backport_commit.clone()) {
                issues.push(format!(
                    "duplicate instrumentation_backport_commit {}",
                    record.instrumentation_backport_commit
                ));
            }
            if !Sha256Digest::is_valid_str(&record.patch_sha256) {
                issues.push(format!(
                    "backport {} has invalid patch_sha256 {:?}",
                    record.instrumentation_backport_commit, record.patch_sha256
                ));
            }
            if record.phases.is_empty() {
                issues.push(format!(
                    "backport {} has no phases",
                    record.instrumentation_backport_commit
                ));
            }
            phase_count = phase_count.saturating_add(record.phases.len());
            for phase in &record.phases {
                validate_phase(phase, &mut issues);
                if !phases.insert(phase.clone()) {
                    issues.push(format!(
                        "phase {} is assigned to more than one backport",
                        phase.display()
                    ));
                }
            }
        }
        if phase_count > MAX_PHASES {
            issues.push(format!(
                "registry contains {phase_count} phases, expected at most {MAX_PHASES}"
            ));
        }

        finish_validation(issues)
    }

    pub(super) fn validate_exact_phase_coverage(
        &self,
        expected: &BTreeSet<PhaseKey>,
    ) -> Result<(), BenchError> {
        let actual = self
            .backports
            .iter()
            .flat_map(|record| record.phases.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut issues = Vec::new();
        for phase in expected.difference(&actual) {
            issues.push(format!(
                "predecessor registry omits required phase {}",
                phase.display()
            ));
        }
        for phase in actual.difference(expected) {
            issues.push(format!(
                "predecessor registry invents phase {}",
                phase.display()
            ));
        }
        finish_validation(issues)
    }
}

pub(super) fn read_and_validate(
    root: &RepoRoot,
    source_revision: &str,
    expected_phases: &BTreeSet<PhaseKey>,
) -> Result<ValidatedPredecessorRegistry, BenchError> {
    if !GitCommit::is_canonical_str(source_revision) {
        return Err(focused_error(format!(
            "predecessor registry source revision {source_revision:?} is not a lowercase 40-byte Git object id"
        )));
    }
    let bytes = read_tracked_registry(root, source_revision)?;
    let registry = PredecessorRegistry::from_bytes(&bytes)?;
    registry.validate_structure()?;
    registry.validate_exact_phase_coverage(expected_phases)?;

    let mut by_phase = BTreeMap::new();
    for record in &registry.backports {
        validate_backport_graph(root, source_revision, record)?;
        let identity = PredecessorIdentity::from(record);
        for phase in &record.phases {
            if by_phase.insert(phase.clone(), identity.clone()).is_some() {
                return Err(focused_error(format!(
                    "phase {} is assigned to more than one validated backport",
                    phase.display()
                )));
            }
        }
    }

    Ok(ValidatedPredecessorRegistry {
        source_sha256: hex::encode(Sha256::digest(&bytes)),
        by_phase,
    })
}

fn read_tracked_registry(root: &RepoRoot, source_revision: &str) -> Result<Vec<u8>, BenchError> {
    let object = format!("{source_revision}:{REGISTRY_PATH}");
    let tracked = run_git(
        root,
        &["show".to_string(), object.clone()],
        MAX_REGISTRY_BYTES.saturating_add(1),
        false,
    )?;
    if tracked.stdout.len() > MAX_REGISTRY_BYTES {
        return Err(focused_error(format!(
            "tracked predecessor registry {object} exceeds {MAX_REGISTRY_BYTES} bytes"
        )));
    }

    let path = root.resolve_relative(Path::new(REGISTRY_PATH));
    let working = read_repo_regular_file_bounded(root, &path, MAX_REGISTRY_BYTES)?;
    if working != tracked.stdout {
        return Err(focused_error(format!(
            "working {REGISTRY_PATH} differs from tracked blob {object}"
        )));
    }
    Ok(working)
}

fn validate_backport_graph(
    root: &RepoRoot,
    source_revision: &str,
    record: &BackportRecord,
) -> Result<(), BenchError> {
    require_commit(root, source_revision, "current A6 source revision")?;
    require_commit(
        root,
        &record.historical_product_commit,
        "historical product commit",
    )?;
    require_commit(
        root,
        &record.instrumentation_backport_commit,
        "instrumentation backport commit",
    )?;
    if record.historical_product_commit == source_revision {
        return Err(focused_error(format!(
            "backport {} is based on the current A6 source revision instead of a historical product commit",
            record.instrumentation_backport_commit
        )));
    }

    let preservation_ref = preservation_ref(&record.instrumentation_backport_commit);
    let peeled = rev_parse_commit(root, &preservation_ref)?;
    if peeled != record.instrumentation_backport_commit {
        return Err(focused_error(format!(
            "preservation tag {preservation_ref} resolves to {peeled}, expected {}",
            record.instrumentation_backport_commit
        )));
    }

    let parents = commit_with_parents(root, &record.instrumentation_backport_commit)?;
    let [parent] = parents.as_slice() else {
        return Err(focused_error(format!(
            "instrumentation backport {} must be a non-merge commit with exactly one parent, found {}",
            record.instrumentation_backport_commit,
            parents.len()
        )));
    };
    if parent != &record.historical_product_commit {
        return Err(focused_error(format!(
            "instrumentation backport {} has parent {}, expected historical product commit {}",
            record.instrumentation_backport_commit, parent, record.historical_product_commit
        )));
    }

    let ancestry = run_git(
        root,
        &[
            "merge-base".to_string(),
            "--is-ancestor".to_string(),
            record.instrumentation_backport_commit.clone(),
            source_revision.to_string(),
        ],
        4096,
        true,
    )?;
    match ancestry.status {
        Some(0) => {
            return Err(focused_error(format!(
                "instrumentation backport {} is in current source ancestry {source_revision}",
                record.instrumentation_backport_commit
            )));
        }
        Some(1) => {}
        status => {
            return Err(focused_error(format!(
                "Git ancestry check for backport {} returned unexpected status {status:?}",
                record.instrumentation_backport_commit
            )));
        }
    }

    let actual_patch = canonical_patch_sha256(
        root,
        &record.historical_product_commit,
        &record.instrumentation_backport_commit,
    )?;
    if actual_patch != record.patch_sha256 {
        return Err(focused_error(format!(
            "instrumentation backport {} patch SHA-256 is {actual_patch}, expected {}",
            record.instrumentation_backport_commit, record.patch_sha256
        )));
    }
    Ok(())
}

fn require_commit(root: &RepoRoot, revision: &str, label: &str) -> Result<(), BenchError> {
    let resolved = rev_parse_commit(root, revision)?;
    if resolved != revision {
        return Err(focused_error(format!(
            "{label} {revision} resolves to unexpected commit {resolved}"
        )));
    }
    Ok(())
}

fn rev_parse_commit(root: &RepoRoot, revision: &str) -> Result<String, BenchError> {
    let output = run_git(
        root,
        &[
            "rev-parse".to_string(),
            "--verify".to_string(),
            format!("{revision}^{{commit}}"),
        ],
        4096,
        false,
    )?;
    one_revision(&output.stdout, "Git rev-parse")
}

fn commit_with_parents(root: &RepoRoot, revision: &str) -> Result<Vec<String>, BenchError> {
    let output = run_git(
        root,
        &[
            "rev-list".to_string(),
            "--parents".to_string(),
            "-n".to_string(),
            "1".to_string(),
            revision.to_string(),
        ],
        4096,
        false,
    )?;
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|error| focused_error(format!("Git rev-list output is not UTF-8: {error}")))?;
    let mut words = text.split_ascii_whitespace();
    let commit = words
        .next()
        .ok_or_else(|| focused_error("Git rev-list omitted the requested commit"))?;
    if commit != revision {
        return Err(focused_error(format!(
            "Git rev-list returned commit {commit}, expected {revision}"
        )));
    }
    let parents = words.map(str::to_string).collect::<Vec<_>>();
    if parents
        .iter()
        .any(|parent| !GitCommit::is_canonical_str(parent))
    {
        return Err(focused_error(format!(
            "Git rev-list returned an invalid parent for {revision}"
        )));
    }
    Ok(parents)
}

fn canonical_patch_sha256(
    root: &RepoRoot,
    product_commit: &str,
    backport_commit: &str,
) -> Result<String, BenchError> {
    let output = run_git(
        root,
        &[
            "diff-tree".to_string(),
            "--no-commit-id".to_string(),
            "--raw".to_string(),
            "-r".to_string(),
            "-z".to_string(),
            "--no-renames".to_string(),
            "--abbrev=40".to_string(),
            "--no-ext-diff".to_string(),
            "--no-textconv".to_string(),
            product_commit.to_string(),
            backport_commit.to_string(),
            "--".to_string(),
        ],
        MAX_PATCH_BYTES,
        false,
    )?;
    if output.stdout.is_empty() {
        return Err(focused_error(format!(
            "instrumentation backport {backport_commit} has an empty tree delta from {product_commit}"
        )));
    }
    Ok(hex::encode(Sha256::digest(&output.stdout)))
}

fn run_git(
    root: &RepoRoot,
    arguments: &[String],
    stdout_limit: usize,
    allow_status_one: bool,
) -> Result<ProcessResult, BenchError> {
    let mut args = vec![
        OsString::from("--no-pager"),
        OsString::from("--no-replace-objects"),
        OsString::from("--no-optional-locks"),
        OsString::from("-c"),
        OsString::from("core.fsmonitor=false"),
        OsString::from("-c"),
        OsString::from("core.untrackedCache=false"),
        OsString::from("-c"),
        OsString::from("diff.external="),
        OsString::from("-c"),
        OsString::from("submodule.recurse=false"),
    ];
    args.extend(arguments.iter().map(OsString::from));
    let output = run_bounded_process(&ProcessRequest {
        program: PathBuf::from(GIT),
        args,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: controlled_git_environment(),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout: OutputPolicy::Capture {
                maximum_bytes: stdout_limit,
            },
            stderr: OutputPolicy::Capture {
                maximum_bytes: MAX_GIT_DIAGNOSTIC_BYTES,
            },
            regular_file_bytes: None,
            timeout: GIT_TIMEOUT,
        },
    })?;
    let accepted = output.status == Some(0) || allow_status_one && output.status == Some(1);
    if !accepted {
        return Err(focused_error(format!(
            "controlled Git command {:?} failed with status {:?}: {}",
            arguments,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if output.status == Some(0) && !output.stderr.is_empty() {
        return Err(focused_error(format!(
            "controlled Git command {:?} emitted unexpected stderr: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn controlled_git_environment() -> ProcessEnvironment {
    vec![
        (OsString::from("HOME"), OsString::from("/nonexistent")),
        (
            OsString::from("XDG_CONFIG_HOME"),
            OsString::from("/nonexistent"),
        ),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
        (
            OsString::from("GIT_NO_REPLACE_OBJECTS"),
            OsString::from("1"),
        ),
        (OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0")),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
        (OsString::from("GIT_LITERAL_PATHSPECS"), OsString::from("1")),
    ]
    .into()
}

fn preservation_ref(backport_commit: &str) -> String {
    format!("{PRESERVATION_TAG_PREFIX}{backport_commit}")
}

fn one_revision(bytes: &[u8], label: &str) -> Result<String, BenchError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| focused_error(format!("{label} output is not UTF-8: {error}")))?;
    let mut lines = text.lines();
    let revision = lines
        .next()
        .ok_or_else(|| focused_error(format!("{label} omitted its revision")))?;
    if lines.next().is_some() || !GitCommit::is_canonical_str(revision) {
        return Err(focused_error(format!(
            "{label} returned invalid revision output {text:?}"
        )));
    }
    Ok(revision.to_string())
}

fn validate_revision_field(label: &str, value: &str, issues: &mut Vec<String>) {
    if !GitCommit::is_canonical_str(value) {
        issues.push(format!(
            "{label}={value:?} is not a lowercase 40-byte Git object id"
        ));
    }
}

fn validate_phase(phase: &PhaseKey, issues: &mut Vec<String>) {
    if !is_safe_benchmark_id(&phase.row_id) {
        issues.push(format!(
            "phase row_id {:?} is not a safe benchmark id",
            phase.row_id
        ));
    }
    if phase.measurement.trim().is_empty()
        || phase.measurement.len() > MAX_MEASUREMENT_BYTES
        || phase
            .measurement
            .bytes()
            .any(|byte| byte.is_ascii_control())
    {
        issues.push(format!(
            "phase {} has an invalid measurement",
            phase.display()
        ));
    }
}

fn finish_validation(issues: Vec<String>) -> Result<(), BenchError> {
    if issues.is_empty() {
        Ok(())
    } else {
        Err(focused_error(issues.join("\n")))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::fs;
    use std::process::{Command, Output};

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    const ROW: &str = "m8-test";
    const MEASUREMENT: &str = "stab_test";

    struct TestGraph {
        _temporary: TempDir,
        root: RepoRoot,
        product: String,
        backport: String,
        source: String,
        patch_sha256: String,
    }

    #[test]
    fn checked_registry_is_pending_and_structurally_valid() {
        let registry = PredecessorRegistry::from_bytes(include_bytes!(
            "../../../../benchmarks/a6-predecessors.json"
        ))
        .expect("parse checked pending registry");
        registry
            .validate_structure()
            .expect("checked pending registry structure");
        registry
            .validate_exact_phase_coverage(&BTreeSet::new())
            .expect("pending registry covers no phases");
        assert!(registry.backports.is_empty());
    }

    #[test]
    fn accepts_tracked_tagged_direct_parent_and_looks_up_report_commit() {
        let graph = standard_graph(true);
        let expected = BTreeSet::from([PhaseKey::new(ROW, MEASUREMENT)]);
        let registry =
            read_and_validate(&graph.root, &graph.source, &expected).expect("valid registry");

        assert_eq!(
            registry.source_sha256(),
            hex::encode(Sha256::digest(
                fs::read(graph.root.resolve_relative(Path::new(REGISTRY_PATH)))
                    .expect("read registry")
            ))
        );
        assert_eq!(registry.phases().count(), 1);
        let identity = registry
            .require_report_commit(&PhaseKey::new(ROW, MEASUREMENT), &graph.backport)
            .expect("registered report commit");
        assert_eq!(identity.historical_product_commit, graph.product);
        assert_eq!(identity.patch_sha256, graph.patch_sha256);

        let error = registry
            .require_report_commit(&PhaseKey::new(ROW, MEASUREMENT), &graph.source)
            .expect_err("unregistered report commit");
        assert!(error.to_string().contains("expected registered"));
    }

    #[test]
    fn rejects_missing_or_moved_preservation_tag() {
        let graph = standard_graph(true);
        test_git(
            &graph.root.path,
            &["tag", "--delete", &preservation_ref(&graph.backport)],
        );
        let record = record(&graph);
        let error = validate_backport_graph(&graph.root, &graph.source, &record)
            .expect_err("missing preservation tag");
        assert!(error.to_string().contains("rev-parse"));

        test_git(
            &graph.root.path,
            &["tag", &preservation_ref(&graph.backport), &graph.product],
        );
        let error = validate_backport_graph(&graph.root, &graph.source, &record)
            .expect_err("moved preservation tag");
        assert!(error.to_string().contains("resolves to"));
    }

    #[test]
    fn rejects_wrong_parent_and_merge_backports() {
        let graph = standard_graph(true);
        let mut wrong_parent = record(&graph);
        wrong_parent.historical_product_commit = graph.source.clone();
        let error = validate_backport_graph(&graph.root, &graph.source, &wrong_parent)
            .expect_err("wrong direct parent");
        assert!(
            error.to_string().contains("based on the current")
                || error.to_string().contains("expected historical")
        );

        let merge = merge_graph();
        let record = record(&merge);
        let error = validate_backport_graph(&merge.root, &merge.source, &record)
            .expect_err("merge backport");
        assert!(error.to_string().contains("non-merge"));
    }

    #[test]
    fn rejects_patch_digest_mutation_and_changed_same_size_content() {
        let graph = standard_graph(true);
        let mut wrong_digest = record(&graph);
        wrong_digest.patch_sha256 = "0".repeat(64);
        let error = validate_backport_graph(&graph.root, &graph.source, &wrong_digest)
            .expect_err("wrong patch digest");
        assert!(error.to_string().contains("patch SHA-256"));

        let mutation_digest = alternate_backport_patch(&graph);
        assert_ne!(
            graph.patch_sha256, mutation_digest,
            "same-size content mutation must change the canonical tree delta"
        );
    }

    #[test]
    fn rejects_empty_backport_patch() {
        let graph = empty_backport_graph();
        let record = BackportRecord {
            historical_product_commit: graph.product.clone(),
            instrumentation_backport_commit: graph.backport.clone(),
            patch_sha256: "0".repeat(64),
            phases: vec![PhaseKey::new(ROW, MEASUREMENT)],
        };
        let error =
            validate_backport_graph(&graph.root, &graph.source, &record).expect_err("empty patch");
        assert!(error.to_string().contains("empty tree delta"));
    }

    #[test]
    fn rejects_backport_in_current_source_ancestry() {
        let graph = ancestral_backport_graph();
        let record = record(&graph);
        let error = validate_backport_graph(&graph.root, &graph.source, &record)
            .expect_err("backport in current ancestry");
        assert!(error.to_string().contains("is in current source ancestry"));
    }

    #[test]
    fn ignores_git_replacement_objects() {
        let graph = standard_graph(true);
        test_git(
            &graph.root.path,
            &["replace", &graph.backport, &graph.source],
        );
        let record = record(&graph);
        validate_backport_graph(&graph.root, &graph.source, &record)
            .expect("replacement refs cannot affect controlled Git");
    }

    #[test]
    fn rejects_duplicate_missing_and_extra_phases() {
        let graph = standard_graph(true);
        let duplicate = PredecessorRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION,
            patch_digest_contract: PATCH_DIGEST_CONTRACT.to_string(),
            backports: vec![
                record(&graph),
                BackportRecord {
                    historical_product_commit: graph.product.clone(),
                    instrumentation_backport_commit: "f".repeat(40),
                    patch_sha256: "a".repeat(64),
                    phases: vec![PhaseKey::new(ROW, MEASUREMENT)],
                },
            ],
        };
        let error = duplicate.validate_structure().expect_err("duplicate phase");
        assert!(error.to_string().contains("assigned to more than one"));

        let registry = PredecessorRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION,
            patch_digest_contract: PATCH_DIGEST_CONTRACT.to_string(),
            backports: vec![record(&graph)],
        };
        let missing = BTreeSet::from([
            PhaseKey::new(ROW, MEASUREMENT),
            PhaseKey::new("m8-missing", "stab_missing"),
        ]);
        let error = registry
            .validate_exact_phase_coverage(&missing)
            .expect_err("missing phase");
        assert!(error.to_string().contains("omits required phase"));

        let empty = BTreeSet::new();
        let error = registry
            .validate_exact_phase_coverage(&empty)
            .expect_err("extra phase");
        assert!(error.to_string().contains("invents phase"));
    }

    #[test]
    fn tracked_registry_is_bounded_and_must_match_working_bytes() {
        let graph = standard_graph(true);
        fs::write(
            graph.root.resolve_relative(Path::new(REGISTRY_PATH)),
            b"{}\n",
        )
        .expect("mutate working registry");
        let expected = BTreeSet::from([PhaseKey::new(ROW, MEASUREMENT)]);
        let error = read_and_validate(&graph.root, &graph.source, &expected)
            .expect_err("working registry mutation");
        assert!(error.to_string().contains("differs from tracked blob"));
    }

    fn record(graph: &TestGraph) -> BackportRecord {
        BackportRecord {
            historical_product_commit: graph.product.clone(),
            instrumentation_backport_commit: graph.backport.clone(),
            patch_sha256: graph.patch_sha256.clone(),
            phases: vec![PhaseKey::new(ROW, MEASUREMENT)],
        }
    }

    fn standard_graph(tag_backport: bool) -> TestGraph {
        let temporary = initialized_repository();
        let repository = temporary.path();
        fs::write(repository.join("product.txt"), b"product\n").expect("write product");
        commit_all(repository, "product");
        let product = rev_parse(repository, "HEAD");

        test_git(repository, &["switch", "--quiet", "-c", "instrumentation"]);
        fs::write(repository.join("instrumentation.txt"), b"evidence-a\n")
            .expect("write instrumentation");
        commit_all(repository, "instrumentation");
        let backport = rev_parse(repository, "HEAD");
        if tag_backport {
            test_git(repository, &["tag", &preservation_ref(&backport)]);
        }

        test_git(repository, &["switch", "--quiet", "main"]);
        fs::write(repository.join("current.txt"), b"current\n").expect("write current");
        commit_all(repository, "current");
        let root = RepoRoot::resolve(repository).expect("resolve test repository");
        let patch_sha256 =
            canonical_patch_sha256(&root, &product, &backport).expect("canonical patch");
        write_registry(repository, &product, &backport, &patch_sha256);
        let source = rev_parse(repository, "HEAD");

        TestGraph {
            _temporary: temporary,
            root,
            product,
            backport,
            source,
            patch_sha256,
        }
    }

    fn alternate_backport_patch(graph: &TestGraph) -> String {
        let repository = &graph.root.path;
        test_git(
            repository,
            &["switch", "--quiet", "--detach", &graph.product],
        );
        fs::write(repository.join("instrumentation.txt"), b"evidence-b\n")
            .expect("write alternate instrumentation");
        commit_all(repository, "alternate instrumentation");
        let backport = rev_parse(repository, "HEAD");
        test_git(repository, &["tag", &preservation_ref(&backport)]);
        canonical_patch_sha256(&graph.root, &graph.product, &backport)
            .expect("alternate canonical patch")
    }

    fn empty_backport_graph() -> TestGraph {
        let temporary = initialized_repository();
        let repository = temporary.path();
        fs::write(repository.join("product.txt"), b"product\n").expect("write product");
        commit_all(repository, "product");
        let product = rev_parse(repository, "HEAD");

        test_git(repository, &["switch", "--quiet", "-c", "empty-backport"]);
        test_git(
            repository,
            &["commit", "--quiet", "--allow-empty", "-m", "empty"],
        );
        let backport = rev_parse(repository, "HEAD");
        test_git(repository, &["tag", &preservation_ref(&backport)]);

        test_git(repository, &["switch", "--quiet", "main"]);
        fs::write(repository.join("current.txt"), b"current\n").expect("write current");
        commit_all(repository, "current");
        let source = rev_parse(repository, "HEAD");
        let root = RepoRoot::resolve(repository).expect("resolve test repository");
        TestGraph {
            _temporary: temporary,
            root,
            product,
            backport,
            source,
            patch_sha256: "0".repeat(64),
        }
    }

    fn ancestral_backport_graph() -> TestGraph {
        let temporary = initialized_repository();
        let repository = temporary.path();
        fs::write(repository.join("product.txt"), b"product\n").expect("write product");
        commit_all(repository, "product");
        let product = rev_parse(repository, "HEAD");

        fs::write(repository.join("instrumentation.txt"), b"evidence\n")
            .expect("write instrumentation");
        commit_all(repository, "instrumentation");
        let backport = rev_parse(repository, "HEAD");
        test_git(repository, &["tag", &preservation_ref(&backport)]);
        let root = RepoRoot::resolve(repository).expect("resolve test repository");
        let patch_sha256 =
            canonical_patch_sha256(&root, &product, &backport).expect("canonical patch");

        fs::write(repository.join("current.txt"), b"current\n").expect("write current");
        commit_all(repository, "current");
        let source = rev_parse(repository, "HEAD");
        TestGraph {
            _temporary: temporary,
            root,
            product,
            backport,
            source,
            patch_sha256,
        }
    }

    fn merge_graph() -> TestGraph {
        let temporary = initialized_repository();
        let repository = temporary.path();
        fs::write(repository.join("product.txt"), b"product\n").expect("write product");
        commit_all(repository, "product");
        let product = rev_parse(repository, "HEAD");

        test_git(repository, &["switch", "--quiet", "-c", "side"]);
        fs::write(repository.join("side.txt"), b"side\n").expect("write side");
        commit_all(repository, "side");

        test_git(repository, &["switch", "--quiet", "main"]);
        fs::write(repository.join("instrumentation.txt"), b"evidence\n")
            .expect("write instrumentation");
        commit_all(repository, "instrumentation parent");
        let direct_parent = rev_parse(repository, "HEAD");
        test_git(
            repository,
            &[
                "merge",
                "--quiet",
                "--no-ff",
                "side",
                "-m",
                "merge backport",
            ],
        );
        let backport = rev_parse(repository, "HEAD");
        test_git(repository, &["tag", &preservation_ref(&backport)]);

        test_git(repository, &["switch", "--quiet", "--detach", &product]);
        fs::write(repository.join("current.txt"), b"current\n").expect("write current");
        commit_all(repository, "current");
        let source = rev_parse(repository, "HEAD");
        let root = RepoRoot::resolve(repository).expect("resolve test repository");
        let patch_sha256 =
            canonical_patch_sha256(&root, &direct_parent, &backport).expect("merge patch digest");
        TestGraph {
            _temporary: temporary,
            root,
            product: direct_parent,
            backport,
            source,
            patch_sha256,
        }
    }

    fn initialized_repository() -> TempDir {
        let temporary = tempfile::tempdir().expect("temporary repository");
        test_git(
            temporary.path(),
            &["init", "--quiet", "--initial-branch=main"],
        );
        test_git(temporary.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            temporary.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        temporary
    }

    fn write_registry(repository: &Path, product: &str, backport: &str, patch_sha256: &str) {
        fs::create_dir_all(repository.join("benchmarks")).expect("create benchmarks");
        let value = json!({
            "schema_version": REGISTRY_SCHEMA_VERSION,
            "patch_digest_contract": PATCH_DIGEST_CONTRACT,
            "backports": [{
                "historical_product_commit": product,
                "instrumentation_backport_commit": backport,
                "patch_sha256": patch_sha256,
                "phases": [{
                    "row_id": ROW,
                    "measurement": MEASUREMENT
                }]
            }]
        });
        let mut bytes = serde_json::to_vec_pretty(&value).expect("serialize registry");
        bytes.push(b'\n');
        fs::write(repository.join(REGISTRY_PATH), bytes).expect("write registry");
        commit_all(repository, "registry");
    }

    fn commit_all(repository: &Path, message: &str) {
        test_git(repository, &["add", "--all"]);
        test_git(repository, &["commit", "--quiet", "-m", message]);
    }

    fn rev_parse(repository: &Path, revision: &str) -> String {
        String::from_utf8(test_git(repository, &["rev-parse", revision]).stdout)
            .expect("Git revision UTF-8")
            .trim()
            .to_string()
    }

    fn test_git(repository: &Path, arguments: &[&str]) -> Output {
        let output = Command::new(GIT)
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env_clear()
            .env("HOME", "/nonexistent")
            .env("XDG_CONFIG_HOME", "/nonexistent")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("run test Git command");
        assert!(
            output.status.success(),
            "Git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}
