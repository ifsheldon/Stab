use std::path::Path;

use self::model::{PerformanceDisposition, QualificationSuite, RowClassification, RowDecision};
use crate::config::PREFIX;
use crate::error::BenchError;
use crate::manifest::BenchmarkManifest;
use crate::root::RepoRoot;

mod checklist;
mod discovery;
mod io;
mod migration;
mod model;
mod runtime;
mod status;
mod validation;

pub(crate) use runtime::exact_median;
pub(crate) use runtime::identity::{GitCommit, Sha256Digest};
pub(crate) use runtime::{
    BaselineCandidateArgs, CompletionArgs, CompletionCheckpointArgs, CompletionReportArgs,
    DiagnosticArgs, ParityArgs, ProbeArgs, ReportArgs, RollupArgs, RollupReportArgs, RunArgs,
    SelfRegressionArgs, SimdCompareArgs, SimdReportArgs, WorkerArgs,
};
pub(crate) use status::StatusArgs;

const EXPECTED_FROZEN_DIGEST: &str =
    "61e219da4df930ff3dc595099794b518a933aad94a04ac8388a1b76361c35032";
const MAX_SUITE_BYTES: usize = 1 << 20;

pub(crate) fn run_worker(args: WorkerArgs) -> Result<(), BenchError> {
    runtime::run_worker(args).map_err(BenchError::Qualification)
}

pub(crate) fn probe(root: &RepoRoot, args: ProbeArgs) -> Result<(), BenchError> {
    with_formal_session(root, |session| {
        runtime::run_probe(session, args).map_err(BenchError::Qualification)
    })
}

pub(crate) fn regenerate_clifford_vectors(root: &RepoRoot, check: bool) -> Result<(), BenchError> {
    runtime::regenerate_clifford_vectors(root, check).map_err(BenchError::Qualification)?;
    println!(
        "[{PREFIX}] {} Clifford qualification vectors",
        if check { "validated" } else { "regenerated" }
    );
    Ok(())
}

pub(crate) fn worker_reproducibility(root: &RepoRoot) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let (stim_binary_sha256, stab_binary_sha256) =
            runtime::verify_worker_reproducibility(session, EXPECTED_FROZEN_DIGEST)
                .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] private qualification workers are reproducible: stim={} stab={}",
            stim_binary_sha256, stab_binary_sha256
        );
        Ok(())
    })
}

pub(crate) fn run_qualification(root: &RepoRoot, args: RunArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_qualification(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published performance qualification evidence at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn run_diagnostic(root: &RepoRoot, args: DiagnosticArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_diagnostic(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published Stab-only product diagnostic at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn run_simd_compare(root: &RepoRoot, args: SimdCompareArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_simd_compare(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published scalar-versus-portable-SIMD diagnostic at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn simd_report(root: &RepoRoot, args: SimdReportArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let input = runtime::run_simd_report(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] replayed scalar-versus-portable-SIMD diagnostic at {}",
            input.display()
        );
        Ok(())
    })
}

pub(crate) fn report(root: &RepoRoot, args: ReportArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_report(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] validated performance qualification evidence at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn completion(root: &RepoRoot, args: CompletionArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_completion(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published performance qualification completion manifest at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn completion_report(
    root: &RepoRoot,
    args: CompletionReportArgs,
) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let validation = runtime::run_completion_report(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        match validation {
            runtime::CompletionReportValidation::Replayed(output) => println!(
                "[{PREFIX}] reconstructed performance qualification completion manifest at {}",
                output.path().display()
            ),
            runtime::CompletionReportValidation::HistoricalReadable {
                path,
                schema_version,
            } => println!(
                "[{PREFIX}] historical completion schema {schema_version} is readable at {}; source artifacts were not replayed",
                path.display()
            ),
        }
        Ok(())
    })
}

pub(crate) fn completion_checkpoint(
    root: &RepoRoot,
    args: CompletionCheckpointArgs,
) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let replayed = runtime::completion_checkpoint_manifest(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        runtime::require_completion_checkpoint_current(session, &replayed)
            .map_err(BenchError::Qualification)?;
        status::publish_completion_manifest(root, replayed.report_json())
    })?;
    println!(
        "[{PREFIX}] published authenticated completion checkpoint at benchmarks/qualification-completion-checkpoint.json"
    );
    Ok(())
}

pub(crate) fn parity(root: &RepoRoot, args: ParityArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let summary = runtime::run_parity(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] qualification Stim parity group={} checked={} report_only={}",
            summary.group_id, summary.checked_measurements, summary.report_only
        );
        Ok(())
    })
}

pub(crate) fn self_regression(root: &RepoRoot, args: SelfRegressionArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let summary = runtime::run_self_regression(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] Stab self-regression group={} checked={} unseeded={} outcome={:?}",
            summary.group_id,
            summary.checked_measurements,
            summary.unseeded_measurements,
            summary.outcome
        );
        Ok(())
    })
}

pub(crate) fn regression_baseline_candidate(
    root: &RepoRoot,
    args: BaselineCandidateArgs,
) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::generate_regression_baseline_candidate(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published Stab self-regression baseline candidate at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn rollup(root: &RepoRoot, args: RollupArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_rollup(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] published performance qualification scale-family rollup at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn rollup_report(root: &RepoRoot, args: RollupReportArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let output = runtime::run_rollup_report(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] replayed performance qualification scale-family rollup at {}",
            output.display()
        );
        Ok(())
    })
}

fn with_checked_formal_session<T>(
    root: &RepoRoot,
    action: impl FnOnce(&runtime::QualificationSession) -> Result<T, BenchError>,
) -> Result<T, BenchError> {
    with_formal_session(root, |session| {
        let source_root = session.source_root();
        let manifest = BenchmarkManifest::read(source_root)?;
        manifest.check(source_root)?;
        check(source_root, &manifest)?;
        action(session)
    })
}

fn with_formal_session<T>(
    root: &RepoRoot,
    action: impl FnOnce(&runtime::QualificationSession) -> Result<T, BenchError>,
) -> Result<T, BenchError> {
    let session = runtime::QualificationSession::open(root).map_err(BenchError::Qualification)?;
    let action_result = action(&session);
    let session_result = session.require_current().map_err(BenchError::Qualification);
    match (action_result, session_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(action), Ok(())) => Err(action),
        (Ok(_), Err(session)) => Err(session),
        (Err(action), Err(session)) => Err(BenchError::QualificationSession {
            action: Box::new(action),
            session: Box::new(session),
        }),
    }
}

pub(crate) fn check(root: &RepoRoot, manifest: &BenchmarkManifest) -> Result<(), BenchError> {
    ensure_frozen()?;
    let references = discovery::load_source_references(root)?;
    let checked_bytes = read_bytes(root, &root.performance_qualification())?;
    let checked: QualificationSuite = serde_json::from_slice(&checked_bytes)?;
    validation::validate(&checked, manifest, &references, EXPECTED_FROZEN_DIGEST)?;
    let generated = discovery::generate(root, manifest)?;
    validation::validate(&generated, manifest, &references, "UNFROZEN")?;
    validation::validate(&generated, manifest, &references, EXPECTED_FROZEN_DIGEST)?;
    if checked_bytes != render(&generated)? {
        return Err(BenchError::QualificationDrift);
    }
    migration::check(root, &checked)?;
    runtime::check_contracts(root, EXPECTED_FROZEN_DIGEST, &checked, &references)
        .map_err(BenchError::Qualification)?;
    print_summary(&checked, None);
    Ok(())
}

pub(crate) fn list(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    feature: Option<&str>,
) -> Result<(), BenchError> {
    ensure_frozen()?;
    let references = discovery::load_source_references(root)?;
    let checked = read(root)?;
    validation::validate(&checked, manifest, &references, EXPECTED_FROZEN_DIGEST)?;
    if let Some(value) = feature
        && !discovery::PERFORMANCE_FEATURE_IDS.contains(&value)
    {
        return Err(BenchError::Qualification(format!(
            "unknown performance feature {value:?}"
        )));
    }
    print_summary(&checked, feature);
    Ok(())
}

pub(crate) fn regenerate(root: &RepoRoot, manifest: &BenchmarkManifest) -> Result<(), BenchError> {
    let generated = discovery::generate(root, manifest)?;
    let references = discovery::load_source_references(root)?;
    validation::validate(&generated, manifest, &references, "UNFROZEN")?;
    let checked_path = root.performance_qualification();
    let bytes = render(&generated)?;
    atomic_write(root, &checked_path, &bytes)?;
    println!(
        "[{PREFIX}] wrote {} performance features and {} inherited manifest dispositions",
        generated.performance_features.len(),
        generated.manifest_rows.len()
    );
    println!(
        "[{PREFIX}] performance qualification digest {}",
        generated.semantic_digest
    );
    Ok(())
}

pub(crate) fn status(root: &RepoRoot, args: StatusArgs) -> Result<(), BenchError> {
    status::run(root, args)
}

fn ensure_frozen() -> Result<(), BenchError> {
    if EXPECTED_FROZEN_DIGEST == "UNFROZEN" {
        Err(BenchError::QualificationUnfrozen)
    } else {
        Ok(())
    }
}

fn read(root: &RepoRoot) -> Result<QualificationSuite, BenchError> {
    let path = root.performance_qualification();
    let bytes = read_bytes(root, &path)?;
    serde_json::from_slice(&bytes).map_err(BenchError::Json)
}

fn read_bytes(root: &RepoRoot, path: &Path) -> Result<Vec<u8>, BenchError> {
    let bytes = crate::source_file::read_repo_regular_file_bounded(root, path, MAX_SUITE_BYTES)?;
    io::preflight_json_shape(&bytes)?;
    Ok(bytes)
}

fn render(suite: &QualificationSuite) -> Result<Vec<u8>, BenchError> {
    let mut bytes = serde_json::to_vec_pretty(suite)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn atomic_write(root: &RepoRoot, path: &Path, bytes: &[u8]) -> Result<(), BenchError> {
    crate::source_file::atomic_write_repo_regular_file(root, path, bytes)
}

fn print_summary(suite: &QualificationSuite, feature: Option<&str>) {
    let rows = suite
        .manifest_rows
        .iter()
        .filter(|row| feature.is_none_or(|value| row.performance_feature == value))
        .collect::<Vec<_>>();
    println!(
        "[{PREFIX}] performance qualification schema={} stim={} commit={} digest={}",
        suite.schema_version, suite.stim_version, suite.stim_commit, suite.semantic_digest
    );
    println!(
        "[{PREFIX}] selection={} features={} manifest-rows={} runtime-linked={} perf-sources={} perf-symbols={}",
        feature.unwrap_or("all"),
        suite
            .performance_features
            .iter()
            .filter(|item| feature.is_none_or(|value| item.id == value))
            .count(),
        rows.len(),
        rows.iter()
            .filter(|row| row.runtime_group_id.is_some())
            .count(),
        suite.upstream_perf_sources.len(),
        suite
            .upstream_perf_sources
            .iter()
            .map(|source| source.symbols.len())
            .sum::<usize>()
    );
    println!(
        "[{PREFIX}] decisions retained={} reworked={} diagnostic={} superseded={} removed={}",
        count_decision(&rows, RowDecision::Retained),
        count_decision(&rows, RowDecision::Reworked),
        count_decision(&rows, RowDecision::Diagnostic),
        count_decision(&rows, RowDecision::Superseded),
        count_decision(&rows, RowDecision::Removed)
    );
    println!(
        "[{PREFIX}] unresolved proxy={} stale={} duplicate={} missing-scale={} missing-preflight={} missing-output-digest={} missing-comparator={} asymmetric-cli={} heterogeneous={} unmatched-submeasurement={}",
        count_classification(&rows, RowClassification::Proxy),
        count_classification(&rows, RowClassification::Stale),
        count_classification(&rows, RowClassification::Duplicate),
        count_classification(&rows, RowClassification::MissingScale),
        count_classification(&rows, RowClassification::MissingCorrectnessPreflight),
        count_classification(&rows, RowClassification::MissingOutputDigest),
        count_classification(&rows, RowClassification::MissingComparator),
        count_classification(&rows, RowClassification::InProcessProcessMismatch),
        count_classification(&rows, RowClassification::HeterogeneousMeasurements),
        count_classification(&rows, RowClassification::UnmatchedSubmeasurement)
    );
    println!(
        "[{PREFIX}] dispositions covered-by-parent={} future-candidate={} diagnostic={} not-performance-relevant={} exact-threshold-pairs={}",
        count_disposition(&rows, PerformanceDisposition::CoveredByParent),
        count_disposition(&rows, PerformanceDisposition::FutureCandidate),
        count_disposition(&rows, PerformanceDisposition::Diagnostic),
        count_disposition(&rows, PerformanceDisposition::NotPerformanceRelevant),
        rows.iter()
            .map(|row| row.threshold_measurement_pairs.len())
            .sum::<usize>(),
    );
}

fn count_disposition(
    rows: &[&model::ManifestRowDisposition],
    value: PerformanceDisposition,
) -> usize {
    rows.iter().filter(|row| row.disposition == value).count()
}

fn count_decision(rows: &[&model::ManifestRowDisposition], value: RowDecision) -> usize {
    rows.iter().filter(|row| row.decision == value).count()
}

fn count_classification(
    rows: &[&model::ManifestRowDisposition],
    value: RowClassification,
) -> usize {
    rows.iter()
        .filter(|row| row.classifications.contains(&value))
        .count()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn checked_inventory_reader_rejects_symlinks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        std::fs::write(&target, b"{}").expect("write target");
        std::os::unix::fs::symlink(&target, &link).expect("create symlink");

        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        let error = read_bytes(&root, &link).expect_err("symlink must be rejected");

        assert!(error.to_string().contains("nonsymlink file"));
    }

    #[test]
    fn atomic_inventory_write_rejects_nonregular_destination() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        let destination = directory.path().join("inventory-dir");
        std::fs::create_dir(&destination).expect("create destination directory");
        let error = atomic_write(&root, &destination, b"{}")
            .expect_err("directory destination must be rejected");
        assert!(error.to_string().contains("replace only a regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_inventory_write_uses_source_owned_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        let path = directory.path().join("inventory.json");

        atomic_write(&root, &path, b"{}\n").expect("write inventory");

        let mode = std::fs::metadata(&path)
            .expect("inventory metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_inventory_write_rejects_symlink_ancestor() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let outside = tempfile::tempdir().expect("outside directory");
        let root = RepoRoot::resolve(directory.path()).expect("resolve root");
        let link = directory.path().join("benchmarks");
        std::os::unix::fs::symlink(outside.path(), &link).expect("create ancestor symlink");

        let error = atomic_write(&root, &link.join("inventory.json"), b"{}\n")
            .expect_err("ancestor symlink must be rejected");

        assert!(error.to_string().contains("source input"));
        assert!(!outside.path().join("inventory.json").exists());
    }

    #[test]
    fn formal_session_preserves_action_and_final_identity_failures() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let repository = parent.path().join("repository");
        std::fs::create_dir(&repository).expect("create repository");
        let root = RepoRoot::resolve(&repository).expect("resolve repository");
        let detached = parent.path().join("detached");

        let error = with_formal_session(&root, |_| {
            std::fs::rename(&repository, &detached).expect("detach repository");
            std::fs::create_dir(&repository).expect("replace repository");
            Err::<(), _>(BenchError::Qualification(
                "injected action failure".to_string(),
            ))
        })
        .expect_err("both failures must be preserved");

        assert!(matches!(
            error,
            BenchError::QualificationSession { action, session }
                if matches!(*action, BenchError::Qualification(ref message) if message == "injected action failure")
                    && matches!(*session, BenchError::Qualification(ref message) if message.contains("repository root"))
        ));
    }
}
