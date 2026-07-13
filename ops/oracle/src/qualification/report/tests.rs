use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{
    ArtifactRecord, CaseOutcome, CaseResult, DomainComparatorCount, ExpectedCase, ProbabilityBound,
    QualificationReport, ReportExpectation, ReportMetadata, SelectionSummary,
};
use crate::RepoRoot;
use crate::qualification::artifact::QualificationOutputDir;
use crate::qualification::model::{
    Comparator, EvidenceSelector, EvidenceState, ExecutionTier, FeatureId, SelectorKind,
};
use crate::qualification::receipt::{
    ExecutableIdentity, ExecutionReceiptInput, ExecutionVerdict, ReceiptArtifact, StreamReceipt,
    new_execution_receipt,
};

fn publish_execution_receipt(
    output: &QualificationOutputDir,
    report: &mut QualificationReport,
    verdict: ExecutionVerdict,
) {
    let exit_status = if verdict == ExecutionVerdict::InfrastructureFailure {
        None
    } else if verdict == ExecutionVerdict::Accepted {
        Some(0)
    } else {
        Some(1)
    };
    publish_execution_receipt_with_status(output, report, verdict, exit_status);
}

fn publish_execution_receipt_with_status(
    output: &QualificationOutputDir,
    report: &mut QualificationReport,
    verdict: ExecutionVerdict,
    exit_status: Option<i32>,
) {
    let result = report.results.first_mut().expect("case result");
    let complete = verdict != ExecutionVerdict::InfrastructureFailure;
    let receipt = new_execution_receipt(ExecutionReceiptInput {
        run_request_sha256: report.run_request_sha256.clone(),
        case_id: result.case_id.clone(),
        selector_sha256: result.selector_sha256.clone(),
        executables: Vec::new(),
        execution_environment_sha256: "e".repeat(64),
        verdict,
        exit_status,
        exact_test_count: result.exact_test_count,
        stdout: result.stdout_sha256.as_ref().map(|digest| StreamReceipt {
            bytes: 0,
            sha256: digest.clone(),
            complete,
        }),
        stderr: result.stderr_sha256.as_ref().map(|digest| StreamReceipt {
            bytes: 0,
            sha256: digest.clone(),
            complete,
        }),
        statistical_attempts: Vec::new(),
        auxiliary_outputs: result
            .artifacts
            .iter()
            .map(|artifact| ReceiptArtifact {
                path: artifact.path.clone(),
                bytes: artifact.bytes,
                sha256: artifact.sha256.clone(),
            })
            .collect(),
    });
    result.execution_receipt_sha256 = receipt.publish(output).expect("execution receipt");
}

#[test]
fn report_requires_the_source_owned_status_for_an_accepted_receipt() {
    let (_temporary, output, mut report) = report();
    let mut expectation = report_expectation(&report);
    expectation
        .selected_cases
        .get_mut("case-a")
        .expect("expected case")
        .expected_exit_status = 1;

    assert!(report.validate(&output, &expectation).is_err());

    publish_execution_receipt_with_status(
        &output,
        &mut report,
        ExecutionVerdict::Accepted,
        Some(1),
    );
    assert!(report.validate(&output, &expectation).is_ok());
}

#[test]
fn probability_bound_json_round_trip_preserves_exact_bits() {
    let canonical = "1.77437429772043210e-12";
    let value = ProbabilityBound::try_new(canonical.parse().expect("parse bound")).expect("bound");
    let bytes = serde_json::to_vec(&value).expect("serialize bound");
    let parsed: ProbabilityBound = serde_json::from_slice(&bytes).expect("parse bound");

    assert_eq!(parsed.get().to_bits(), value.get().to_bits());
    assert_eq!(bytes, format!("\"{canonical}\"").as_bytes());
    assert!(ProbabilityBound::try_new(-0.0).is_err());
}

#[test]
fn report_rejects_an_execution_receipt_from_another_executable_set() {
    let (_temporary, output, mut report) = report();
    let mut expectation = report_expectation(&report);
    expectation.executables = vec![ExecutableIdentity {
        role: "cargo".to_string(),
        bytes: 1,
        sha256: "a".repeat(64),
    }];
    publish_execution_receipt(&output, &mut report, ExecutionVerdict::Accepted);

    let error = report
        .validate(&output, &expectation)
        .expect_err("receipt executable mismatch must fail");

    assert!(error.to_string().contains("execution receipt is stale"));
}

#[test]
fn report_rejects_statistical_work_missing_from_the_execution_receipt() {
    let (_temporary, output, mut report) = report();
    let result = report.results.first_mut().expect("case result");
    result.comparator = Comparator::Statistical;
    let count = report.case_counts.first_mut().expect("case count");
    count.comparator = Comparator::Statistical;
    report.statistical_declared_budget = ProbabilityBound::try_new(1e-4).expect("declared bound");
    report.statistical_consumed_bound = ProbabilityBound::try_new(1e-6).expect("consumed bound");
    report.statistical_planned_shots = 100;
    report
        .statistical_planned_seeds
        .insert("case-a".to_string(), vec![7]);
    report.statistical_shots = 100;
    report
        .statistical_seeds
        .insert("case-a".to_string(), vec![7]);
    report.statistical_attempts = vec![super::StatisticalAttempt {
        case_id: "case-a".to_string(),
        seed: 7,
        completed_shots: 100,
        completed_comparisons: 1,
        completed_batches: 1,
        outcome: CaseOutcome::Passed,
    }];
    report.finish();
    let mut expectation = report_expectation(&report);
    expectation
        .statistical_shots_per_batch
        .insert("case-a".to_string(), 100);
    expectation
        .statistical_comparisons_per_attempt
        .insert("case-a".to_string(), 1);
    expectation
        .statistical_batches_per_attempt
        .insert("case-a".to_string(), 1);
    expectation
        .statistical_shots_per_attempt
        .insert("case-a".to_string(), 100);
    expectation
        .statistical_exact_bound_per_attempt
        .insert("case-a".to_string(), 1e-6);

    let error = report
        .validate(&output, &expectation)
        .expect_err("report-only statistical work must fail");

    assert!(
        error
            .to_string()
            .contains("statistical attempts disagree with its execution receipt")
    );
}

#[test]
fn report_rejects_statistical_batches_with_the_wrong_comparison_shape() {
    let (_temporary, output, mut report) = report();
    let result = report.results.first_mut().expect("case result");
    result.comparator = Comparator::Statistical;
    result.outcome = CaseOutcome::Failed;
    let count = report.case_counts.first_mut().expect("case count");
    count.comparator = Comparator::Statistical;
    count.passed = 0;
    count.failed = 1;
    report.statistical_declared_budget = ProbabilityBound::try_new(1e-4).expect("declared bound");
    report.statistical_consumed_bound = ProbabilityBound::try_new(5e-7).expect("consumed bound");
    report.statistical_planned_shots = 600;
    report
        .statistical_planned_seeds
        .insert("case-a".to_string(), vec![7]);
    report.statistical_shots = 600;
    report
        .statistical_seeds
        .insert("case-a".to_string(), vec![7]);
    report.statistical_attempts = vec![super::StatisticalAttempt {
        case_id: "case-a".to_string(),
        seed: 7,
        completed_shots: 600,
        completed_comparisons: 1,
        completed_batches: 6,
        outcome: CaseOutcome::Failed,
    }];
    report.finish();
    let mut expectation = report_expectation(&report);
    expectation
        .statistical_shots_per_batch
        .insert("case-a".to_string(), 100);
    expectation
        .statistical_comparisons_per_attempt
        .insert("case-a".to_string(), 2);
    expectation
        .statistical_batches_per_attempt
        .insert("case-a".to_string(), 6);
    expectation
        .statistical_shots_per_attempt
        .insert("case-a".to_string(), 600);
    expectation
        .statistical_exact_bound_per_attempt
        .insert("case-a".to_string(), 1e-6);

    let error = report
        .validate(&output, &expectation)
        .expect_err("wrong batches per comparison must fail");

    assert!(
        error
            .to_string()
            .contains("completion outside its frozen work contract")
    );
}

fn report() -> (
    tempfile::TempDir,
    QualificationOutputDir,
    QualificationReport,
) {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = RepoRoot {
        path: temporary.path().to_path_buf(),
    };
    let output =
        QualificationOutputDir::parse(&root, Path::new("target/qualification/correctness/test"))
            .expect("output");
    let mut report = QualificationReport::new(
        ReportMetadata {
            qualification_manifest_digest: "a".repeat(64),
            stab_commit: "b".repeat(40),
            local_modifications: true,
            stim_tag: "v1.16.0".to_string(),
            stim_commit: "c".repeat(40),
            rust_toolchain: "nightly-test".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            operating_system: "linux".to_string(),
            architecture: "x86_64".to_string(),
        },
        SelectionSummary {
            tier: ExecutionTier::Pr,
            feature_filters: Vec::new(),
            case_filters: Vec::new(),
            allow_deferred: false,
            selected_count: 1,
            planned_count: 0,
            deferred_count: 0,
        },
    );
    let selector = EvidenceSelector {
        state: EvidenceState::Existing,
        kind: SelectorKind::OracleFixture,
        value: vec!["fixture-a".to_string()],
    };
    report.run_request_sha256 = "f".repeat(64);
    report.results.push(CaseResult {
        case_id: "case-a".to_string(),
        feature_id: FeatureId::Cli,
        comparator: Comparator::ExactBytes,
        selector_sha256: super::selector_sha256(&selector).expect("selector digest"),
        execution_receipt_sha256: String::new(),
        selector,
        outcome: CaseOutcome::Passed,
        exact_test_count: None,
        stdout_sha256: Some("d".repeat(64)),
        stderr_sha256: Some("e".repeat(64)),
        artifacts: Vec::new(),
    });
    report.statistical_declared_budget = ProbabilityBound::try_new(1e-6).expect("budget");
    report.case_counts = vec![DomainComparatorCount {
        feature_id: FeatureId::Cli,
        comparator: Comparator::ExactBytes,
        passed: 1,
        failed: 0,
        planned: 0,
        deferred: 0,
    }];
    report.finish();
    publish_execution_receipt(&output, &mut report, ExecutionVerdict::Accepted);
    (temporary, output, report)
}

fn expected_selectors(report: &QualificationReport) -> BTreeMap<String, String> {
    BTreeMap::from([(
        "case-a".to_string(),
        report
            .results
            .first()
            .expect("case result")
            .selector_sha256
            .clone(),
    )])
}

fn report_expectation(report: &QualificationReport) -> ReportExpectation {
    let result = report.results.first().expect("case result");
    ReportExpectation {
        metadata: ReportMetadata {
            qualification_manifest_digest: report.qualification_manifest_digest.clone(),
            stab_commit: report.stab_commit.clone(),
            local_modifications: report.local_modifications,
            stim_tag: report.stim_tag.clone(),
            stim_commit: report.stim_commit.clone(),
            rust_toolchain: report.rust_toolchain.clone(),
            target_triple: report.target_triple.clone(),
            operating_system: report.operating_system.clone(),
            architecture: report.architecture.clone(),
        },
        run_request_sha256: report.run_request_sha256.clone(),
        executables: Vec::new(),
        execution_environment_sha256: "e".repeat(64),
        tier: report.tier,
        feature_filters: report.feature_filters.clone(),
        case_filters: report.case_filters.clone(),
        allow_deferred: report.allow_deferred,
        selected_cases: BTreeMap::from([(
            result.case_id.clone(),
            ExpectedCase {
                feature_id: result.feature_id,
                comparator: result.comparator,
                selector: result.selector.clone(),
                selector_sha256: result.selector_sha256.clone(),
                expected_exit_status: 0,
                artifact_limit_bytes: 4_096,
                stdout_receipt_limit_bytes: 4_096,
                stderr_receipt_limit_bytes: 4_096,
            },
        )]),
        planned_cases: Vec::new(),
        deferred_cases: Vec::new(),
        statistical_declared_budget: report.statistical_declared_budget.get(),
        statistical_planned_shots: report.statistical_planned_shots,
        statistical_planned_seeds: report.statistical_planned_seeds.clone(),
        statistical_shots_per_batch: BTreeMap::new(),
        statistical_comparisons_per_attempt: BTreeMap::new(),
        statistical_batches_per_attempt: BTreeMap::new(),
        statistical_shots_per_attempt: BTreeMap::new(),
        statistical_exact_bound_per_attempt: BTreeMap::new(),
        property_corpus_ids: report.property_corpus_ids.clone(),
        resource_contracts: report.resource_contracts.clone(),
        upstream_dispositions: report.upstream_dispositions.clone(),
        deferred_products: report.deferred_products.clone(),
    }
}

fn expectation(selectors: BTreeMap<String, String>) -> super::PreflightExpectation {
    super::PreflightExpectation {
        manifest_digest: "a".repeat(64),
        run_request_sha256: "f".repeat(64),
        stab_commit: "b".repeat(40),
        stim_commit: "c".repeat(40),
        selectors,
        current_worktree_dirty: false,
        allow_dirty: true,
        cases: vec!["case-a".to_string()],
    }
}

fn validate_current_preflight(
    output: &QualificationOutputDir,
    report_expectation: &ReportExpectation,
    expectation: &super::PreflightExpectation,
) -> Result<(), super::ReportError> {
    let (_, completion_sha256) = crate::qualification::receipt::RunCompletionReceipt::read(output)
        .expect("completion receipt");
    super::validate_preflight(output, report_expectation, expectation, &completion_sha256)
}

#[test]
fn report_round_trip_is_reproducible_and_preserves_dirty_metadata() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish");
    let first = output
        .read(Path::new("report.json"), 1 << 20)
        .expect("first report");
    super::regenerate(&output, &report_expectation).expect("regenerate");
    let second = output
        .read(Path::new("report.json"), 1 << 20)
        .expect("second report");

    assert_eq!(first, second);
    assert!(
        QualificationReport::read(&output)
            .unwrap()
            .local_modifications
    );
}

#[test]
fn report_rejects_partial_and_traversing_artifacts() {
    let (_temporary, output, mut report) = report();
    report.selection_complete = false;
    report.results.first_mut().expect("case result").artifacts = vec![ArtifactRecord {
        path: PathBuf::from("target/qualification/../escape"),
        bytes: 3,
        sha256: super::sha256(b"bad"),
    }];

    assert!(
        report
            .validate(&output, &report_expectation(&report))
            .is_err()
    );
}

#[test]
fn report_rejects_a_self_consistent_but_manifest_incomplete_selection() {
    let (_temporary, output, mut report) = report();
    let expectation = report_expectation(&report);
    report.results.clear();
    report.case_counts.clear();
    report.selected_count = 0;
    report.finish();

    let error = report
        .validate(&output, &expectation)
        .expect_err("manifest-owned selected case must remain required");

    assert!(
        error
            .to_string()
            .contains("selection counts disagree with reconstructed")
    );
    assert!(
        error
            .to_string()
            .contains("do not exactly cover the reconstructed selected case ids")
    );
}

#[test]
fn report_rejects_rewriting_a_rejected_case_as_passing() {
    let (_temporary, output, mut report) = report();
    publish_execution_receipt(&output, &mut report, ExecutionVerdict::Rejected);
    let expectation = report_expectation(&report);

    let error = report
        .validate(&output, &expectation)
        .expect_err("receipt-owned rejection must not become a report pass");

    assert!(
        error
            .to_string()
            .contains("outcome is not derivable from its execution receipt")
    );
}

#[test]
fn report_rejects_an_incomplete_stream_in_an_accepted_receipt() {
    let (_temporary, output, mut report) = report();
    let (mut receipt, _) = crate::qualification::receipt::ExecutionReceipt::read(&output, "case-a")
        .expect("execution receipt");
    receipt.stdout.as_mut().expect("stdout receipt").complete = false;
    report
        .results
        .first_mut()
        .expect("case result")
        .execution_receipt_sha256 = receipt.publish(&output).expect("replace execution receipt");

    let error = report
        .validate(&output, &report_expectation(&report))
        .expect_err("accepted receipt must prove complete output capture");

    assert!(error.to_string().contains("digest or completion disagrees"));
}

#[test]
fn report_rejects_stream_receipt_bytes_beyond_the_execution_contract() {
    let (_temporary, output, mut report) = report();
    let (mut receipt, _) = crate::qualification::receipt::ExecutionReceipt::read(&output, "case-a")
        .expect("execution receipt");
    receipt.stdout.as_mut().expect("stdout receipt").bytes = 4_097;
    report
        .results
        .first_mut()
        .expect("case result")
        .execution_receipt_sha256 = receipt.publish(&output).expect("replace execution receipt");

    let error = report
        .validate(&output, &report_expectation(&report))
        .expect_err("stream byte counts remain bounded by the execution contract");

    assert!(error.to_string().contains("receipt exceeds its 4096-byte"));
}

#[test]
fn report_rejects_an_accepted_cargo_receipt_without_one_exact_test() {
    let (_temporary, output, mut report) = report();
    let result = report.results.first_mut().expect("case result");
    result.selector.kind = SelectorKind::CargoTest;
    result.selector.value = vec![
        "-p".to_string(),
        "stab-core".to_string(),
        "--lib".to_string(),
        "case_a".to_string(),
        "--exact".to_string(),
    ];
    result.selector_sha256 = super::selector_sha256(&result.selector).expect("selector digest");
    result.exact_test_count = Some(0);
    publish_execution_receipt(&output, &mut report, ExecutionVerdict::Accepted);

    let error = report
        .validate(&output, &report_expectation(&report))
        .expect_err("accepted Cargo evidence must prove exactly one selected test");

    assert!(
        error
            .to_string()
            .contains("does not prove exactly one test")
    );
}

#[test]
fn report_rejects_artifact_content_changed_after_publication() {
    let (_temporary, output, mut report) = report();
    let result = report.results.first_mut().expect("case result");
    result.outcome = CaseOutcome::Failed;
    result.stdout_sha256 = Some(super::sha256(b"stdout"));
    result.stderr_sha256 = Some(super::sha256(b"stderr"));
    let path = output
        .write(Path::new("cases/case-a/failure.txt"), b"first")
        .expect("write failure artifact");
    result.artifacts.push(ArtifactRecord {
        path,
        bytes: 5,
        sha256: super::sha256(b"first"),
    });
    let count = report.case_counts.first_mut().expect("case count");
    count.passed = 0;
    count.failed = 1;
    report.finish();
    let expectation = report_expectation(&report);
    output
        .write(Path::new("cases/case-a/failure.txt"), b"other")
        .expect("replace failure artifact");

    let error = report
        .validate(&output, &expectation)
        .expect_err("changed artifact must invalidate report");

    assert!(
        error
            .to_string()
            .contains("disagrees with its size or digest")
    );
}

#[test]
fn report_rejects_artifact_size_claims_before_reading_files() {
    for claimed in [4_097, usize::MAX] {
        let (_temporary, output, mut report) = report();
        let expectation = report_expectation(&report);
        let result = report.results.first_mut().expect("case result");
        result.outcome = CaseOutcome::Failed;
        result.artifacts.push(ArtifactRecord {
            path: output
                .relative()
                .join("cases/case-a/untrusted-artifact.bin"),
            bytes: claimed,
            sha256: super::sha256(b"untrusted"),
        });
        let count = report.case_counts.first_mut().expect("case count");
        count.passed = 0;
        count.failed = 1;
        report.finish();

        let error = report
            .validate(&output, &expectation)
            .expect_err("oversized claim must fail before a file read");

        assert!(error.to_string().contains("with only 4096 bytes remaining"));
    }
}

#[test]
fn report_rejects_a_large_sparse_artifact_through_the_manifest_cap() {
    let (temporary, output, mut report) = report();
    let expectation = report_expectation(&report);
    let relative = PathBuf::from("cases/case-a/sparse.bin");
    let path = temporary.path().join(output.relative()).join(&relative);
    std::fs::create_dir_all(path.parent().expect("artifact parent")).expect("artifact parent");
    let file = std::fs::File::create(&path).expect("sparse artifact");
    file.set_len(1 << 30).expect("size sparse artifact");
    let result = report.results.first_mut().expect("case result");
    result.outcome = CaseOutcome::Failed;
    result.artifacts.push(ArtifactRecord {
        path: output.relative().join(relative),
        bytes: 4_096,
        sha256: super::sha256(b"not-the-sparse-file"),
    });
    let count = report.case_counts.first_mut().expect("case count");
    count.passed = 0;
    count.failed = 1;
    report.finish();

    let error = report
        .validate(&output, &expectation)
        .expect_err("sparse artifact must be rejected without reading its contents");

    assert!(error.to_string().contains("cannot be validated"));
    assert!(error.to_string().contains("4096-byte limit"));
}

#[test]
fn preflight_rejects_failed_and_missing_cases() {
    let (_temporary, output, mut report) = report();
    report.results.first_mut().expect("case result").outcome = CaseOutcome::Failed;
    let count = report.case_counts.first_mut().expect("case count");
    count.passed = 0;
    count.failed = 1;
    report.finish();
    publish_execution_receipt(&output, &mut report, ExecutionVerdict::Rejected);
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish failed report");
    let mut expectation = expectation(expected_selectors(&report));
    expectation.cases.push("case-b".to_string());

    assert!(validate_current_preflight(&output, &report_expectation, &expectation).is_err());
}

#[test]
fn preflight_rejects_any_report_recorded_with_allow_deferred() {
    let (_temporary, output, mut report) = report();
    report.allow_deferred = true;
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish diagnostic deferred report");
    let expectation = expectation(expected_selectors(&report));

    let error = validate_current_preflight(&output, &report_expectation, &expectation)
        .expect_err("allow-deferred reports must never be promotable");

    assert!(error.to_string().contains("used --allow-deferred"));
}

#[test]
fn preflight_rejects_dirty_and_stale_provenance() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let mut expectation = expectation(expected_selectors(&report));
    expectation.manifest_digest = "d".repeat(64);
    expectation.stab_commit = "e".repeat(40);
    expectation.stim_commit = "f".repeat(40);
    expectation.current_worktree_dirty = true;
    expectation.allow_dirty = false;

    let error = validate_current_preflight(&output, &report_expectation, &expectation)
        .expect_err("reject dirty stale report");
    let message = error.to_string();
    assert!(message.contains("manifest digest is stale"));
    assert!(message.contains("Stab commit is stale"));
    assert!(message.contains("Stim commit is stale"));
    assert!(message.contains("local modifications"));
}

#[test]
fn preflight_rejects_a_current_dirty_worktree_independently() {
    let (_temporary, output, mut report) = report();
    report.local_modifications = false;
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish clean report");
    let mut expectation = expectation(expected_selectors(&report));
    expectation.current_worktree_dirty = true;
    expectation.allow_dirty = false;

    let error = validate_current_preflight(&output, &report_expectation, &expectation)
        .expect_err("reject current dirty worktree");

    assert!(
        error
            .to_string()
            .contains("current worktree has local modifications")
    );
}

#[test]
fn preflight_rejects_a_stale_report_digest() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let expectation = expectation(expected_selectors(&report));
    let bytes = output
        .read(Path::new("preflight.json"), 1 << 20)
        .expect("read preflight");
    let mut preflight: super::CorrectnessPreflight =
        serde_json::from_slice(&bytes).expect("parse preflight");
    preflight.report_sha256 = "f".repeat(64);
    output
        .write(
            Path::new("preflight.json"),
            &super::canonical_json(&preflight).expect("canonical preflight"),
        )
        .expect("rewrite preflight");

    let error = validate_current_preflight(&output, &report_expectation, &expectation)
        .expect_err("reject stale report digest");

    assert!(
        error
            .to_string()
            .contains("preflight report digest is stale")
    );
}

#[test]
fn preflight_rejects_a_stale_controller_completion_digest() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let expectation = expectation(expected_selectors(&report));

    let error =
        super::validate_preflight(&output, &report_expectation, &expectation, &"0".repeat(64))
            .expect_err("controller completion digest must bind the executed outcomes");

    assert!(error.to_string().contains("controller-approved digest"));
}

#[test]
fn report_regeneration_rejects_a_completion_receipt_bound_to_another_report() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let (mut completion, _) = crate::qualification::receipt::RunCompletionReceipt::read(&output)
        .expect("completion receipt");
    completion.report_sha256 = "0".repeat(64);
    completion
        .publish(&output)
        .expect("replace completion receipt");

    let error = super::regenerate(&output, &report_expectation)
        .expect_err("completion receipt must bind the canonical report bytes");

    assert!(error.to_string().contains("completion receipt is stale"));
}

#[test]
fn preflight_rejects_metadata_that_disagrees_with_its_bound_report() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let expectation = expectation(expected_selectors(&report));
    let bytes = output
        .read(Path::new("preflight.json"), 1 << 20)
        .expect("read preflight");
    let mut preflight: super::CorrectnessPreflight =
        serde_json::from_slice(&bytes).expect("parse preflight");
    preflight.stab_commit = "f".repeat(40);
    output
        .write(
            Path::new("preflight.json"),
            &super::canonical_json(&preflight).expect("canonical preflight"),
        )
        .expect("rewrite preflight");

    let error = validate_current_preflight(&output, &report_expectation, &expectation)
        .expect_err("reject mismatched report metadata");

    assert!(
        error
            .to_string()
            .contains("preflight case or run metadata disagrees with report.json")
    );
}

#[test]
fn report_rejects_stale_resolved_selector_before_preflight() {
    let (_temporary, output, mut report) = report();
    let report_expectation = report_expectation(&report);
    report
        .results
        .first_mut()
        .expect("case result")
        .selector_sha256 = "f".repeat(64);

    let error = report
        .publish(&output, &report_expectation)
        .expect_err("reject stale selector");
    let message = error.to_string();
    assert!(message.contains("metadata or selector disagrees"));
}

#[test]
fn preflight_case_validation_rejects_missing_output_digest() {
    let (_temporary, output, report) = report();
    let report_expectation = report_expectation(&report);
    report
        .publish(&output, &report_expectation)
        .expect("publish report");
    let expectation = expectation(expected_selectors(&report));
    let report_bytes = output
        .read(Path::new("report.json"), 1 << 20)
        .expect("read report");
    let bytes = output
        .read(Path::new("preflight.json"), 1 << 20)
        .expect("read preflight");
    let mut preflight: super::CorrectnessPreflight =
        serde_json::from_slice(&bytes).expect("parse preflight");
    preflight
        .cases
        .get_mut("case-a")
        .expect("case preflight")
        .stdout_sha256 = None;

    let error = preflight
        .validate_cases(&expectation, &super::sha256(&report_bytes))
        .expect_err("reject missing output digest");
    let message = error.to_string();
    assert!(message.contains("lacks valid output digests"));
}

#[test]
fn report_rejects_statistical_attempts_after_terminal_failure() {
    let (_temporary, output, mut report) = report();
    let result = report.results.first_mut().expect("case result");
    result.comparator = Comparator::Statistical;
    result.outcome = CaseOutcome::Failed;
    let count = report.case_counts.first_mut().expect("case count");
    count.comparator = Comparator::Statistical;
    count.passed = 0;
    count.failed = 1;
    report.statistical_declared_budget = ProbabilityBound::try_new(1e-4).expect("declared bound");
    report.statistical_consumed_bound = ProbabilityBound::try_new(2e-6).expect("consumed bound");
    report.statistical_planned_shots = 200;
    report
        .statistical_planned_seeds
        .insert("case-a".to_string(), vec![1, 2]);
    report.statistical_shots = 200;
    report
        .statistical_seeds
        .insert("case-a".to_string(), vec![1, 2]);
    report.statistical_attempts = vec![
        super::StatisticalAttempt {
            case_id: "case-a".to_string(),
            seed: 1,
            completed_shots: 100,
            completed_comparisons: 1,
            completed_batches: 1,
            outcome: CaseOutcome::Failed,
        },
        super::StatisticalAttempt {
            case_id: "case-a".to_string(),
            seed: 2,
            completed_shots: 100,
            completed_comparisons: 1,
            completed_batches: 1,
            outcome: CaseOutcome::Passed,
        },
    ];
    report.finish();
    let mut report_expectation = report_expectation(&report);
    report_expectation
        .statistical_shots_per_batch
        .insert("case-a".to_string(), 100);
    report_expectation
        .statistical_comparisons_per_attempt
        .insert("case-a".to_string(), 1);
    report_expectation
        .statistical_batches_per_attempt
        .insert("case-a".to_string(), 1);
    report_expectation
        .statistical_shots_per_attempt
        .insert("case-a".to_string(), 100);
    report_expectation
        .statistical_exact_bound_per_attempt
        .insert("case-a".to_string(), 1e-6);

    let error = report
        .validate(&output, &report_expectation)
        .expect_err("attempts after failure must be rejected");

    assert!(
        error
            .to_string()
            .contains("attempt after a terminal failure")
    );
}
