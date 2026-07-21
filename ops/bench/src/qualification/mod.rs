use std::collections::BTreeMap;
use std::path::Path;

use self::model::{
    CorrectnessBinding, PerformanceDisposition, QualificationSuite, RowClassification, RowDecision,
    RowOrigin,
};
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
mod validation;

pub(crate) use runtime::{
    CompletionArgs, CompletionReportArgs, ProbeArgs, RegressionArgs, ReportArgs, RollupArgs,
    RollupReportArgs, RunArgs, WorkerArgs,
};

const EXPECTED_FROZEN_DIGEST: &str =
    "a47866ba5eab70392dd2754391d3d7d8588567a7cbfc1f81a569be813804ce51";
const MAX_SUITE_BYTES: usize = 32 << 20;

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
            runtime::verify_worker_reproducibility(session).map_err(BenchError::Qualification)?;
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
            "[{PREFIX}] published performance qualification completion receipt at {}",
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
        let output = runtime::run_completion_report(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] replayed performance qualification completion receipt at {}",
            output.display()
        );
        Ok(())
    })
}

pub(crate) fn regression(root: &RepoRoot, args: RegressionArgs) -> Result<(), BenchError> {
    with_checked_formal_session(root, |session| {
        let checked = read(session.source_root())?;
        let summary = runtime::run_regression(
            session,
            EXPECTED_FROZEN_DIGEST,
            &checked.correctness_digest,
            args,
        )
        .map_err(BenchError::Qualification)?;
        println!(
            "[{PREFIX}] qualification regression group={} checked={} report_only={}",
            summary.group_id, summary.checked_measurements, summary.report_only
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
    let result = action(&session);
    session
        .require_current()
        .map_err(BenchError::Qualification)?;
    result
}

pub(crate) fn check(root: &RepoRoot, manifest: &BenchmarkManifest) -> Result<(), BenchError> {
    ensure_frozen()?;
    let references = discovery::load_source_references(root)?;
    let checked_bytes = read_bytes(root, &root.performance_qualification())?;
    let checked: QualificationSuite = serde_json::from_slice(&checked_bytes)?;
    validation::validate(&checked, manifest, &references, EXPECTED_FROZEN_DIGEST)?;
    let generated = discovery::generate(root, manifest)?;
    validation::validate(&generated, manifest, &references, EXPECTED_FROZEN_DIGEST)?;
    if checked_bytes != render(&generated)? {
        return Err(BenchError::QualificationDrift);
    }
    migration::check(root, &checked)?;
    runtime::check_contracts(root, EXPECTED_FROZEN_DIGEST, &checked)
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

pub(crate) fn regenerate(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    check: bool,
) -> Result<(), BenchError> {
    let generated = discovery::generate(root, manifest)?;
    let references = discovery::load_source_references(root)?;
    validation::validate(
        &generated,
        manifest,
        &references,
        if check {
            EXPECTED_FROZEN_DIGEST
        } else {
            "UNFROZEN"
        },
    )?;
    migration::check(root, &generated)?;
    let bytes = render(&generated)?;
    if check {
        ensure_frozen()?;
        if read_bytes(root, &root.performance_qualification())? != bytes {
            return Err(BenchError::QualificationDrift);
        }
        println!("[{PREFIX}] performance qualification regeneration is clean");
    } else {
        atomic_write(root, &root.performance_qualification(), &bytes)?;
        println!(
            "[{PREFIX}] wrote {} checklist rows, {} public API items, {} groups, and {} manifest decisions",
            generated.checklist_items.len(),
            generated.public_api_items.len(),
            generated.qualification_groups.len(),
            generated.manifest_rows.len()
        );
        println!(
            "[{PREFIX}] performance qualification digest {}",
            generated.semantic_digest
        );
    }
    Ok(())
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
    let groups = suite
        .qualification_groups
        .iter()
        .filter(|group| feature.is_none_or(|value| group.performance_feature == value))
        .collect::<Vec<_>>();
    let rows = suite
        .manifest_rows
        .iter()
        .filter(|row| {
            feature.is_none_or(|value| {
                groups.iter().any(|group| {
                    group.id == row.primary_group_id && group.performance_feature == value
                })
            })
        })
        .collect::<Vec<_>>();
    let mut dispositions = BTreeMap::<String, usize>::new();
    for group in &groups {
        *dispositions
            .entry(format!("{:?}", group.disposition))
            .or_default() += 1;
    }
    println!(
        "[{PREFIX}] performance qualification schema={} stim={} commit={} digest={}",
        suite.schema_version, suite.stim_version, suite.stim_commit, suite.semantic_digest
    );
    println!(
        "[{PREFIX}] selection={} checklist={} public-api={} groups={} manifest-rows={} perf-sources={} perf-symbols={}",
        feature.unwrap_or("all"),
        suite
            .checklist_items
            .iter()
            .filter(|item| feature.is_none_or(|value| item
                .performance_features
                .iter()
                .any(|candidate| candidate == value)))
            .count(),
        suite
            .public_api_items
            .iter()
            .filter(|item| feature.is_none_or(|value| item.performance_feature == value))
            .count(),
        groups.len(),
        rows.len(),
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
        "[{PREFIX}] group-dispositions measured={} covered-by-parent={} not-performance-relevant={} no-faithful-comparator={}",
        dispositions
            .get(&format!("{:?}", PerformanceDisposition::Measured))
            .copied()
            .unwrap_or(0),
        dispositions
            .get(&format!("{:?}", PerformanceDisposition::CoveredByParent))
            .copied()
            .unwrap_or(0),
        dispositions
            .get(&format!(
                "{:?}",
                PerformanceDisposition::NotPerformanceRelevant
            ))
            .copied()
            .unwrap_or(0),
        dispositions
            .get(&format!(
                "{:?}",
                PerformanceDisposition::NoFaithfulStimComparator
            ))
            .copied()
            .unwrap_or(0)
    );
    println!(
        "[{PREFIX}] primary-rows inherited={} planned={} correctness exact-api-owners={} exact-cases={} planned-preflight={} exact-threshold-pairs={}",
        groups
            .iter()
            .filter(|group| group.row_origin == RowOrigin::Inherited)
            .count(),
        groups
            .iter()
            .filter(|group| group.row_origin == RowOrigin::Planned)
            .count(),
        groups
            .iter()
            .filter(|group| group.correctness_binding == CorrectnessBinding::ExactApiOwners)
            .count(),
        groups
            .iter()
            .filter(|group| group.correctness_binding == CorrectnessBinding::ExactCases)
            .count(),
        groups
            .iter()
            .filter(|group| group.correctness_binding == CorrectnessBinding::Unresolved)
            .count(),
        rows.iter()
            .map(|row| row.threshold_measurement_pairs.len())
            .sum::<usize>()
    );
    println!(
        "[{PREFIX}] item-dispositions checklist-covered={} checklist-not-performance={} api-covered={} api-not-performance={}",
        suite
            .checklist_items
            .iter()
            .filter(|item| {
                feature.is_none_or(|value| {
                    item.performance_features
                        .iter()
                        .any(|candidate| candidate == value)
                }) && item.disposition == PerformanceDisposition::CoveredByParent
            })
            .count(),
        suite
            .checklist_items
            .iter()
            .filter(|item| {
                feature.is_none_or(|value| {
                    item.performance_features
                        .iter()
                        .any(|candidate| candidate == value)
                }) && item.disposition == PerformanceDisposition::NotPerformanceRelevant
            })
            .count(),
        suite
            .public_api_items
            .iter()
            .filter(|item| {
                feature.is_none_or(|value| {
                    item.performance_feature == value
                        || item
                            .supporting_performance_features
                            .iter()
                            .any(|candidate| candidate == value)
                }) && item.disposition == PerformanceDisposition::CoveredByParent
            })
            .count(),
        suite
            .public_api_items
            .iter()
            .filter(|item| {
                feature.is_none_or(|value| {
                    item.performance_feature == value
                        || item
                            .supporting_performance_features
                            .iter()
                            .any(|candidate| candidate == value)
                }) && item.disposition == PerformanceDisposition::NotPerformanceRelevant
            })
            .count()
    );
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
}
