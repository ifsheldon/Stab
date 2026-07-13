use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{
    ExecutionEvidence, StatisticalAttemptContract, StatisticalRunPlan,
    attach_fixed_statistical_attempt, attempt_capture_violation, case_result, failure,
    finalize_sha256, parse_case_ids, parse_features, process_failure, record_statistical_attempts,
    success, update_seeded_digest, validate_qualification_host, validate_selection_policy,
};
use crate::RepoRoot;
use crate::qualification::artifact::QualificationOutputDir;
use crate::qualification::model::{
    BehavioralSurface, CaseId, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
    EvidenceState, EvidenceStatus, ExecutionContract, ExecutionTier, ExpectedSkip, FeatureId,
    QualificationManifest, ResourceContract, ResourceKind, SelectorKind,
};
use crate::qualification::report::{
    CaseOutcome, QualificationReport, ReportMetadata, SelectionSummary,
};
use crate::qualification::tier::CaseSelection;

fn fixed_statistical_contract() -> StatisticalAttemptContract {
    StatisticalAttemptContract {
        plan_id: "statistical-plan".to_string(),
        shots_per_batch: 100,
        comparisons_per_attempt: 1,
        batches_per_attempt: 1,
        batches_per_comparison: 1,
        shots_per_attempt: 100,
        exact_bound_per_attempt: 1e-7,
    }
}

#[test]
fn qualification_host_policy_fails_closed_outside_linux() {
    assert!(validate_qualification_host("linux").is_ok());
    for unsupported in ["windows", "macos", "freebsd"] {
        let error = validate_qualification_host(unsupported)
            .expect_err("unsupported qualification host must fail closed");
        assert!(error.to_string().contains("requires Linux"));
    }
}

fn case(id: &str, comparator: Comparator, status: EvidenceStatus) -> EvidenceCase {
    EvidenceCase {
        id: CaseId::try_new(id.to_string()).expect("case id"),
        feature_id: FeatureId::Cli,
        behavioral_surface: BehavioralSurface::Cli,
        provenance: EvidenceProvenance::QualificationPlan,
        source_id: id.to_string(),
        comparator,
        execution: ExecutionContract {
            tiers: vec![ExecutionTier::Pr, ExecutionTier::Full, ExecutionTier::Soak],
            timeout_ms: 1_000,
            stdout_limit_bytes: 1_024,
            stderr_limit_bytes: 1_024,
            artifact_limit_bytes: 3_072,
            expected_skip: ExpectedSkip::Never,
        },
        statistical_plan: None,
        property_plan: None,
        primary_selector: EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::CargoTest,
            value: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "stab-cli".to_string(),
                id.to_string(),
                "--quiet".to_string(),
                "--exact".to_string(),
            ],
        },
        supporting_selectors: Vec::new(),
        resource_contract: ResourceContract {
            kind: ResourceKind::NotApplicable,
            detail: "not applicable".to_string(),
        },
        negative_axes: Vec::new(),
        performance_groups: Vec::new(),
        deferred_product: None,
        status,
    }
}

fn report(selected_count: usize) -> QualificationReport {
    QualificationReport::new(
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
            selected_count,
            planned_count: 0,
            deferred_count: 0,
        },
    )
}

#[test]
fn feature_filter_rejects_unknown_values_and_deduplicates_known_values() {
    assert!(parse_features(&["not-a-feature".to_string()]).is_err());
    assert_eq!(
        parse_features(&[
            "CQ-CLI".to_string(),
            "CQ-CLI".to_string(),
            "CQ-RESOURCE".to_string(),
        ])
        .expect("known feature filters"),
        BTreeSet::from([FeatureId::Cli, FeatureId::Resource])
    );
}

#[test]
fn case_filter_rejects_missing_ids() {
    let manifest: QualificationManifest = serde_json::from_str(
        r#"{"schema_version":3,"stim_version":"v1.16.0","stim_commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","rust_toolchain":"nightly","python_ast_version":"3.13","semantic_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","features":[],"upstream_cases":[],"public_api_items":[],"evidence_cases":[]}"#,
    )
    .expect("empty manifest shape");

    assert!(parse_case_ids(&manifest, &["missing".to_string()]).is_err());
}

#[cfg(unix)]
#[test]
fn failed_subprocess_evidence_preserves_non_utf8_stream_bytes() {
    let output = crate::run_process(
        Path::new("/bin/sh"),
        ["-c", "printf '\\377\\000' >&2; exit 7"],
        &[],
        None,
    )
    .expect("capture failed process");
    let evidence = super::process_failure(&output, "expected failure");

    assert_eq!(evidence.stderr.as_deref(), Some(&[0xff, 0x00][..]));
    assert!(
        evidence
            .failure
            .as_deref()
            .is_some_and(|bytes| bytes.ends_with(&[0xff, 0x00]))
    );
}

#[test]
fn explicit_selection_rejects_planned_out_of_tier_and_unpermitted_deferred_cases() {
    let planned = case("planned", Comparator::Structural, EvidenceStatus::Planned);
    let out_of_tier = case(
        "out-of-tier",
        Comparator::Structural,
        EvidenceStatus::Implemented,
    );
    let deferred = case("deferred", Comparator::Structural, EvidenceStatus::Deferred);

    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: Vec::new(),
                planned: vec![&planned],
                deferred: Vec::new(),
                out_of_tier: Vec::new(),
            },
            true,
            false,
        ),
        Err(super::RunError::NonExecutableSelection(_))
    ));
    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: Vec::new(),
                planned: Vec::new(),
                deferred: Vec::new(),
                out_of_tier: vec![&out_of_tier],
            },
            true,
            false,
        ),
        Err(super::RunError::OutOfTierSelection(_))
    ));
    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: Vec::new(),
                planned: Vec::new(),
                deferred: vec![&deferred],
                out_of_tier: Vec::new(),
            },
            true,
            false,
        ),
        Err(super::RunError::DeferredSelection(_))
    ));
    assert!(
        validate_selection_policy(
            &CaseSelection {
                selected: Vec::new(),
                planned: Vec::new(),
                deferred: vec![&deferred],
                out_of_tier: Vec::new(),
            },
            true,
            true,
        )
        .is_ok()
    );
    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: Vec::new(),
                planned: Vec::new(),
                deferred: vec![&deferred],
                out_of_tier: Vec::new(),
            },
            false,
            true,
        ),
        Err(super::RunError::DeferredSelection(message))
            if message.contains("requires explicit --case filters")
    ));
    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: vec![&out_of_tier],
                planned: Vec::new(),
                deferred: Vec::new(),
                out_of_tier: Vec::new(),
            },
            false,
            true,
        ),
        Err(super::RunError::DeferredSelection(message))
            if message.contains("requires explicit --case filters")
    ));
    assert!(matches!(
        validate_selection_policy(
            &CaseSelection {
                selected: vec![&out_of_tier],
                planned: Vec::new(),
                deferred: Vec::new(),
                out_of_tier: Vec::new(),
            },
            true,
            true,
        ),
        Err(super::RunError::DeferredSelection(message))
            if message.contains("did not select any deferred case")
    ));
}

#[test]
fn fixed_statistical_failure_records_only_the_attempt_that_ran() {
    let case = case(
        "statistical-case",
        Comparator::Statistical,
        EvidenceStatus::Implemented,
    );
    let mut report = report(1);
    report.statistical_planned_shots = 300;
    report
        .statistical_planned_seeds
        .insert(case.id.to_string(), vec![7, 8, 9]);
    let statistical = StatisticalRunPlan {
        declared_bound: 1e-6,
        shots: 300,
        seeds: BTreeMap::from([(case.id.to_string(), vec![7, 8, 9])]),
        shots_per_attempt: BTreeMap::from([(case.id.to_string(), 100)]),
        exact_bound_per_attempt: BTreeMap::from([(case.id.to_string(), 1e-7)]),
        attempt_contracts: BTreeMap::from([(case.id.to_string(), fixed_statistical_contract())]),
    };
    let contract = fixed_statistical_contract();
    let execution =
        attach_fixed_statistical_attempt(failure("failed"), Some(&[7]), Some(&contract));

    record_statistical_attempts(&mut report, &case, &execution, &statistical)
        .expect("record failed attempt");

    assert_eq!(report.statistical_planned_shots, 300);
    assert_eq!(report.statistical_shots, 0);
    assert_eq!(report.statistical_consumed_bound.get(), 0.0);
    assert_eq!(
        report
            .statistical_planned_seeds
            .get(case.id.as_str())
            .expect("statistical case should retain its frozen seed panel"),
        &[7, 8, 9]
    );
    assert_eq!(
        report
            .statistical_seeds
            .get(case.id.as_str())
            .expect("statistical case should report the seeds it executed"),
        &[7]
    );
    assert_eq!(report.statistical_attempts.len(), 1);
    let attempt = report
        .statistical_attempts
        .first()
        .expect("failed statistical case should retain its failed attempt");
    assert_eq!(attempt.outcome, CaseOutcome::Failed);
    assert_eq!(attempt.completed_shots, 0);
    assert_eq!(attempt.completed_comparisons, 0);
    assert_eq!(attempt.completed_batches, 0);
}

#[test]
fn completed_statistical_rejection_charges_its_frozen_shots_and_bound() {
    let case = case(
        "statistical-completed-failure",
        Comparator::Statistical,
        EvidenceStatus::Implemented,
    );
    let mut report = report(1);
    report.statistical_planned_shots = 100;
    report
        .statistical_planned_seeds
        .insert(case.id.to_string(), vec![7]);
    let statistical = StatisticalRunPlan {
        declared_bound: 1e-6,
        shots: 100,
        seeds: BTreeMap::from([(case.id.to_string(), vec![7])]),
        shots_per_attempt: BTreeMap::from([(case.id.to_string(), 100)]),
        exact_bound_per_attempt: BTreeMap::from([(case.id.to_string(), 1e-7)]),
        attempt_contracts: BTreeMap::from([(case.id.to_string(), fixed_statistical_contract())]),
    };
    let output = crate::ProcessOutput {
        status: Some(1),
        stdout: crate::CapturedOutput {
            bytes: b"STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t100\n".to_vec(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: b"distribution rejected".to_vec(),
            truncated: false,
        },
    };
    let contract = fixed_statistical_contract();
    let execution = attach_fixed_statistical_attempt(
        process_failure(&output, "statistical assertion failed"),
        Some(&[7]),
        Some(&contract),
    );

    record_statistical_attempts(&mut report, &case, &execution, &statistical)
        .expect("record completed failed attempt");

    assert_eq!(report.statistical_shots, 100);
    assert_eq!(report.statistical_consumed_bound.get(), 1e-7);
    assert_eq!(
        report
            .statistical_attempts
            .first()
            .expect("completed failed attempt")
            .completed_shots,
        100
    );
}

fn statistical_execution(status: i32, stdout: &[u8]) -> ExecutionEvidence {
    let output = crate::ProcessOutput {
        status: Some(status),
        stdout: crate::CapturedOutput {
            bytes: stdout.to_vec(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    };
    if status == 0 {
        success(output, Some(1))
    } else {
        process_failure(&output, "statistical test rejected its distribution")
    }
}

#[test]
fn fixed_statistical_completion_requires_exact_frozen_work() {
    let contract = StatisticalAttemptContract {
        comparisons_per_attempt: 2,
        batches_per_attempt: 2,
        batches_per_comparison: 1,
        shots_per_attempt: 200,
        ..fixed_statistical_contract()
    };
    let complete = attach_fixed_statistical_attempt(
        statistical_execution(
            0,
            b"STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t100\nSTAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t1\t100\n",
        ),
        Some(&[7]),
        Some(&contract),
    );
    assert_eq!(complete.outcome, CaseOutcome::Passed);
    let attempt = complete
        .statistical_attempts
        .first()
        .expect("complete statistical attempt");
    assert_eq!(attempt.completed_shots, 200);
    assert_eq!(attempt.completed_comparisons, 2);
    assert_eq!(attempt.completed_batches, 2);

    let partial = attach_fixed_statistical_attempt(
        statistical_execution(0, b"STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t100\n"),
        Some(&[7]),
        Some(&contract),
    );
    assert_eq!(partial.outcome, CaseOutcome::Failed);
    let attempt = partial
        .statistical_attempts
        .first()
        .expect("partial statistical attempt");
    assert_eq!(attempt.completed_shots, 100);
    assert_eq!(attempt.completed_comparisons, 1);
    assert_eq!(attempt.completed_batches, 1);
}

#[test]
fn fixed_statistical_completion_rejects_the_wrong_batch_shape() {
    let contract = StatisticalAttemptContract {
        comparisons_per_attempt: 2,
        batches_per_attempt: 6,
        batches_per_comparison: 3,
        shots_per_attempt: 600,
        ..fixed_statistical_contract()
    };
    let execution = attach_fixed_statistical_attempt(
        statistical_execution(0, b"STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t600\n"),
        Some(&[7]),
        Some(&contract),
    );

    assert_eq!(execution.outcome, CaseOutcome::Failed);
    let attempt = execution
        .statistical_attempts
        .first()
        .expect("rejected statistical attempt");
    assert_eq!(attempt.completed_shots, 0);
    assert_eq!(attempt.completed_comparisons, 0);
    assert_eq!(attempt.completed_batches, 0);
    assert!(execution.failure.as_deref().is_some_and(|failure| {
        String::from_utf8_lossy(failure).contains("frozen 300 shots per comparison")
    }));
}

#[test]
fn fixed_statistical_completion_retains_a_valid_prefix_before_a_malformed_suffix() {
    let contract = StatisticalAttemptContract {
        comparisons_per_attempt: 2,
        batches_per_attempt: 2,
        batches_per_comparison: 1,
        shots_per_attempt: 200,
        ..fixed_statistical_contract()
    };
    let execution = attach_fixed_statistical_attempt(
        statistical_execution(
            0,
            b"STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t100\nSTAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t1\n",
        ),
        Some(&[7]),
        Some(&contract),
    );

    assert_eq!(execution.outcome, CaseOutcome::Failed);
    let attempt = execution
        .statistical_attempts
        .first()
        .expect("partially completed statistical attempt");
    assert_eq!(attempt.completed_shots, 100);
    assert_eq!(attempt.completed_comparisons, 1);
    assert_eq!(attempt.completed_batches, 1);
    assert!(execution.failure.as_deref().is_some_and(|failure| {
        String::from_utf8_lossy(failure).contains("malformed statistical completion marker")
    }));
}

#[test]
fn fixed_statistical_completion_rejects_missing_malformed_and_stale_markers() {
    let invalid_outputs = [
        "",
        "STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\n",
        "STAB_CQ1_STATISTICAL\t2\tstatistical-plan\t7\t0\t100\n",
        "STAB_CQ1_STATISTICAL\t1\tstale-plan\t7\t0\t100\n",
        "STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t8\t0\t100\n",
        "STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t1\t100\n",
        "STAB_CQ1_STATISTICAL\t1\tstatistical-plan\t7\t0\t50\n",
    ];
    for stdout in invalid_outputs {
        let execution = attach_fixed_statistical_attempt(
            statistical_execution(0, stdout.as_bytes()),
            Some(&[7]),
            Some(&fixed_statistical_contract()),
        );
        assert_eq!(execution.outcome, CaseOutcome::Failed, "stdout={stdout:?}");
        assert!(execution.failure.is_some(), "stdout={stdout:?}");
    }
}

#[test]
fn fixed_statistical_selector_rejects_unexecutable_seed_expansion() {
    let execution = attach_fixed_statistical_attempt(
        ExecutionEvidence {
            outcome: CaseOutcome::Passed,
            completed_execution: true,
            exit_status: Some(0),
            exact_test_count: Some(1),
            stdout: Some(Vec::new()),
            stderr: Some(Vec::new()),
            stdout_bytes: Some(0),
            stderr_bytes: Some(0),
            stdout_digest: None,
            stderr_digest: None,
            failure: None,
            property_regression: None,
            statistical_attempts: Vec::new(),
        },
        Some(&[1, 2]),
        Some(&fixed_statistical_contract()),
    );

    assert_eq!(execution.outcome, CaseOutcome::Failed);
    assert!(execution.statistical_attempts.is_empty());
    assert!(execution.failure.as_deref().is_some_and(|bytes| {
        bytes == b"fixed statistical selector requires exactly one frozen seed"
    }));
}

#[test]
#[ignore = "invoked only as the child of the registered-worker timeout regression"]
fn registered_property_timeout_worker() {
    let id = super::super::property::TIMEOUT_TARGET_ID;
    let digest = super::super::property::registered_execution_plan_digest(id)
        .expect("registered timeout plan");
    let output = super::super::property::run_registered_worker(id, &digest)
        .expect("registered timeout target");
    assert!(output.success());
}

#[cfg(unix)]
#[test]
fn registered_property_worker_is_subject_to_the_killable_timeout() {
    let executable = std::env::current_exe().expect("current test executable");
    let started = std::time::Instant::now();
    let error = crate::process::run_process_with_timeout(
        &executable,
        [
            "--ignored",
            "--exact",
            "qualification::runner::tests::registered_property_timeout_worker",
        ],
        &[],
        None,
        std::time::Duration::from_millis(200),
    )
    .expect_err("registered property worker should time out");

    assert!(matches!(error, crate::OracleError::TimedOut { .. }));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn multi_seed_output_is_hashed_incrementally_and_limited_per_attempt() {
    use sha2::{Digest as _, Sha256};

    let mut hasher = Sha256::new();
    update_seeded_digest(&mut hasher, 7, Some(b"first"));
    update_seeded_digest(&mut hasher, 8, Some(b"second"));
    let mut expected = Vec::new();
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.push(1);
    expected.extend_from_slice(&5_u64.to_le_bytes());
    expected.extend_from_slice(b"first");
    expected.extend_from_slice(&8_u64.to_le_bytes());
    expected.push(1);
    expected.extend_from_slice(&6_u64.to_le_bytes());
    expected.extend_from_slice(b"second");
    assert_eq!(finalize_sha256(hasher), super::sha256(&expected));

    let mut case = case(
        "bounded-statistical",
        Comparator::Statistical,
        EvidenceStatus::Implemented,
    );
    case.execution.stdout_limit_bytes = 3;
    let accepted = crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: vec![0; 3],
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    };
    assert!(attempt_capture_violation(&case, &accepted).is_none());

    let rejected = crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: vec![0; 4],
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    };
    assert!(
        attempt_capture_violation(&case, &rejected)
            .is_some_and(|reason| reason.contains("per-attempt stdout limit"))
    );
}

#[test]
fn failed_case_artifacts_preserve_raw_streams_and_bind_their_digests() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = RepoRoot {
        path: temporary.path().to_path_buf(),
    };
    let output = QualificationOutputDir::parse(
        &root,
        Path::new("target/qualification/correctness/raw-failure"),
    )
    .expect("output root");
    let case = case(
        "raw-failure",
        Comparator::ErrorClass,
        EvidenceStatus::Implemented,
    );
    let stdout = vec![0x00, 0xff, b'\n'];
    let stderr = vec![0x80, 0x00, b'e'];
    let execution = ExecutionEvidence {
        outcome: CaseOutcome::Failed,
        completed_execution: true,
        exit_status: Some(1),
        exact_test_count: None,
        stdout: Some(stdout.clone()),
        stderr: Some(stderr.clone()),
        stdout_bytes: Some(stdout.len()),
        stderr_bytes: Some(stderr.len()),
        stdout_digest: None,
        stderr_digest: None,
        failure: Some(b"expected failure".to_vec()),
        property_regression: None,
        statistical_attempts: Vec::new(),
    };

    let result = case_result(
        &case,
        execution,
        "d".repeat(64),
        &"e".repeat(64),
        &output,
        &[],
        &"f".repeat(64),
    )
    .expect("write failed-case artifacts");

    assert_eq!(result.stdout_sha256, Some(super::sha256(&stdout)));
    assert_eq!(result.stderr_sha256, Some(super::sha256(&stderr)));
    assert_eq!(
        output
            .read(Path::new("cases/raw-failure/stdout.bin"), stdout.len())
            .expect("read raw stdout"),
        stdout
    );
    assert_eq!(
        output
            .read(Path::new("cases/raw-failure/stderr.bin"), stderr.len())
            .expect("read raw stderr"),
        stderr
    );
}
