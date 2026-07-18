use super::*;

const ORACLE_V7_CASE_ID: &str = "cq-evidence-blocker-083f1e2d245c4b57";
const ORACLE_V7_MANIFEST_SHA256: &str =
    "4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87";
const ORACLE_V7_STAB_COMMIT: &str = "3f2f382627c8421de0a668819d467a9f252de20f";
const ORACLE_V7_REQUEST_SHA256: &str =
    "40e7d167e4b5e43dfcf9b44ae6ae2b8bbe84cc30c322ca5562877e6292352a7b";
const ORACLE_V7_COMPLETION_SHA256: &str =
    "3332a64e1bc92474004d10dbbd63efd206df3ed699c8d988435c9c3ba41a1abb";
const ORACLE_V7_STATISTICAL_CASE_ID: &str = "cq-evidence-blocker-0e6bd405bb42f3d5";
const ORACLE_V7_STATISTICAL_REQUEST_SHA256: &str =
    "f97f854356b6715939e3093bd6253dcc3f6637b766b649d2ecded97d94bd74d8";
const ORACLE_V7_STATISTICAL_COMPLETION_SHA256: &str =
    "7b917cf6015c0c947ec1940aafa8d69d7a2e0242e9eaab1f7fe6d29ae5e88d7a";
const ORACLE_V7_FIXTURE_FILES: [(&str, &[u8]); 5] = [
    (
        "request.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7/request.json"),
    ),
    (
        "report.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7/report.json"),
    ),
    (
        "completion.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7/completion.json"),
    ),
    (
        "preflight.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7/preflight.json"),
    ),
    (
        "cases/cq-evidence-blocker-083f1e2d245c4b57/execution-receipt.json",
        include_bytes!(
            "../../../../fixtures/correctness-schema-v7/cases/cq-evidence-blocker-083f1e2d245c4b57/execution-receipt.json"
        ),
    ),
];
const ORACLE_V7_STATISTICAL_FIXTURE_FILES: [(&str, &[u8]); 5] = [
    (
        "request.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7-statistical/request.json"),
    ),
    (
        "report.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7-statistical/report.json"),
    ),
    (
        "completion.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7-statistical/completion.json"),
    ),
    (
        "preflight.json",
        include_bytes!("../../../../fixtures/correctness-schema-v7-statistical/preflight.json"),
    ),
    (
        "cases/cq-evidence-blocker-0e6bd405bb42f3d5/execution-receipt.json",
        include_bytes!(
            "../../../../fixtures/correctness-schema-v7-statistical/cases/cq-evidence-blocker-0e6bd405bb42f3d5/execution-receipt.json"
        ),
    ),
];

#[derive(Clone, Copy)]
enum FixtureMutation {
    None,
    LegacySchema,
    MismatchedReceiptSchema,
    MismatchedPreflightSchema,
    ResolvedSelectorDigest,
    FabricatedPreflightPass,
    FailedReport,
    MismatchedCompletion,
}

#[derive(Clone, Copy)]
enum OraclePassingMutation {
    MissingStdout,
    MissingStderr,
    IncompleteStdout,
    RetainedArtifact,
    MissingExitStatus,
    MultipleCargoTests,
    UnreportedStatisticalAttempt,
    MissingReceiptStatisticalAttempt,
}

#[derive(Clone, Copy)]
enum OracleStatisticalMutation {
    OrphanAttempt,
    DuplicateAttempt,
    NonStatisticalOwnership,
    ShotAggregateDrift,
    SeedAggregateDrift,
    IncompleteSeedPanel,
    ZeroCompletedWork,
}

#[test]
fn diagnostic_evidence_is_explicitly_nonapplicable() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let evidence = validate(
        &root,
        CorrectnessRequirement::NotApplicable {
            reason: "infrastructure-only workload",
        },
    )
    .expect("diagnostic preflight");
    assert_eq!(evidence.status, CorrectnessPreflightStatus::NotApplicable);
    assert!(evidence.case_ids.is_empty());
    assert!(evidence.request_sha256.is_none());
}

#[test]
fn correctness_output_path_matches_the_producer_boundary() {
    for accepted in [
        "target/qualification/cq2-full",
        "target/qualification/correctness/full",
    ] {
        validate_output_path(Path::new(accepted)).expect("producer-compatible output path");
    }
    for rejected in [
        "target/qualification",
        "target/qualification/../escape",
        "/target/qualification/cq2-full",
    ] {
        assert!(validate_output_path(Path::new(rejected)).is_err());
    }
}

#[test]
fn required_preflight_reconstructs_canonical_cq_artifacts() {
    let fixture = fixture(FixtureMutation::None);
    let evidence = validate(
        &fixture.root,
        CorrectnessRequirement::Required {
            output: &fixture.relative,
            case_ids: &["cq-case".to_string()],
            expected_manifest_sha256: &fixture.manifest,
            expected_stab_commit: &fixture.commit,
            expected_request_sha256: &fixture.request_sha256,
            expected_completion_sha256: &fixture.completion_sha256,
        },
    )
    .expect("bound correctness preflight");
    assert_eq!(evidence.status, CorrectnessPreflightStatus::Passed);
}

#[test]
fn oracle_produced_schema_v7_artifacts_reconstruct() {
    let fixture = oracle_v7_fixture();
    let evidence = validate(
        &fixture.root,
        CorrectnessRequirement::Required {
            output: &fixture.relative,
            case_ids: &[ORACLE_V7_CASE_ID.to_string()],
            expected_manifest_sha256: ORACLE_V7_MANIFEST_SHA256,
            expected_stab_commit: ORACLE_V7_STAB_COMMIT,
            expected_request_sha256: ORACLE_V7_REQUEST_SHA256,
            expected_completion_sha256: ORACLE_V7_COMPLETION_SHA256,
        },
    )
    .expect("current Oracle artifacts should satisfy performance preflight");

    assert_eq!(evidence.status, CorrectnessPreflightStatus::Passed);
}

#[test]
fn oracle_produced_schema_v7_statistical_artifacts_reconstruct() {
    let fixture = oracle_v7_statistical_fixture();
    let evidence = validate(
        &fixture.root,
        CorrectnessRequirement::Required {
            output: &fixture.relative,
            case_ids: &[ORACLE_V7_STATISTICAL_CASE_ID.to_string()],
            expected_manifest_sha256: ORACLE_V7_MANIFEST_SHA256,
            expected_stab_commit: ORACLE_V7_STAB_COMMIT,
            expected_request_sha256: ORACLE_V7_STATISTICAL_REQUEST_SHA256,
            expected_completion_sha256: ORACLE_V7_STATISTICAL_COMPLETION_SHA256,
        },
    )
    .expect("current Oracle statistical artifacts should satisfy performance preflight");

    assert_eq!(evidence.status, CorrectnessPreflightStatus::Passed);
}

#[test]
fn rehashed_non_oracle_passing_families_fail_closed() {
    for mutation in [
        OraclePassingMutation::MissingStdout,
        OraclePassingMutation::MissingStderr,
        OraclePassingMutation::IncompleteStdout,
        OraclePassingMutation::RetainedArtifact,
        OraclePassingMutation::MissingExitStatus,
        OraclePassingMutation::MultipleCargoTests,
        OraclePassingMutation::UnreportedStatisticalAttempt,
        OraclePassingMutation::MissingReceiptStatisticalAttempt,
    ] {
        let fixture = oracle_v7_fixture();
        let completion_sha256 = mutate_oracle_fixture(&fixture, mutation);
        let result = validate(
            &fixture.root,
            CorrectnessRequirement::Required {
                output: &fixture.relative,
                case_ids: &[ORACLE_V7_CASE_ID.to_string()],
                expected_manifest_sha256: ORACLE_V7_MANIFEST_SHA256,
                expected_stab_commit: ORACLE_V7_STAB_COMMIT,
                expected_request_sha256: ORACLE_V7_REQUEST_SHA256,
                expected_completion_sha256: &completion_sha256,
            },
        );
        assert!(result.is_err());
    }
}

#[test]
fn rehashed_invalid_statistical_ledgers_fail_closed() {
    for mutation in [
        OracleStatisticalMutation::OrphanAttempt,
        OracleStatisticalMutation::DuplicateAttempt,
        OracleStatisticalMutation::NonStatisticalOwnership,
        OracleStatisticalMutation::ShotAggregateDrift,
        OracleStatisticalMutation::SeedAggregateDrift,
        OracleStatisticalMutation::IncompleteSeedPanel,
        OracleStatisticalMutation::ZeroCompletedWork,
    ] {
        let fixture = oracle_v7_statistical_fixture();
        let completion_sha256 = mutate_oracle_statistical_fixture(&fixture, mutation);
        let result = validate(
            &fixture.root,
            CorrectnessRequirement::Required {
                output: &fixture.relative,
                case_ids: &[ORACLE_V7_STATISTICAL_CASE_ID.to_string()],
                expected_manifest_sha256: ORACLE_V7_MANIFEST_SHA256,
                expected_stab_commit: ORACLE_V7_STAB_COMMIT,
                expected_request_sha256: ORACLE_V7_STATISTICAL_REQUEST_SHA256,
                expected_completion_sha256: &completion_sha256,
            },
        );
        assert!(result.is_err());
    }
}

#[test]
fn historical_schema_family_remains_replayable() {
    let fixture = fixture(FixtureMutation::LegacySchema);
    assert_fixture_accepted(&fixture);
}

#[test]
fn mixed_schema_families_fail_closed() {
    for mutation in [
        FixtureMutation::MismatchedReceiptSchema,
        FixtureMutation::MismatchedPreflightSchema,
    ] {
        let fixture = fixture(mutation);
        assert_fixture_rejected(&fixture);
    }
}

#[test]
fn resolved_fixture_selector_digest_stays_bound_to_the_approved_request() {
    let fixture = fixture(FixtureMutation::ResolvedSelectorDigest);
    let evidence = validate(
        &fixture.root,
        CorrectnessRequirement::Required {
            output: &fixture.relative,
            case_ids: &["cq-case".to_string()],
            expected_manifest_sha256: &fixture.manifest,
            expected_stab_commit: &fixture.commit,
            expected_request_sha256: &fixture.request_sha256,
            expected_completion_sha256: &fixture.completion_sha256,
        },
    )
    .expect("resolved fixture selector digest remains request-bound");
    assert_eq!(evidence.status, CorrectnessPreflightStatus::Passed);
}

#[test]
fn edited_preflight_cannot_invent_a_passing_case() {
    let fixture = fixture(FixtureMutation::FabricatedPreflightPass);
    assert_fixture_rejected(&fixture);
}

#[test]
fn failed_report_is_rejected_even_when_dependent_hashes_are_refreshed() {
    let fixture = fixture(FixtureMutation::FailedReport);
    assert_fixture_rejected(&fixture);
}

#[test]
fn completion_must_exactly_reconstruct_report_results() {
    let fixture = fixture(FixtureMutation::MismatchedCompletion);
    assert_fixture_rejected(&fixture);
}

struct OracleFixture {
    _repository: tempfile::TempDir,
    root: RepoRoot,
    relative: PathBuf,
}

fn oracle_v7_fixture() -> OracleFixture {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = PathBuf::from("target/qualification/oracle-schema-v7");
    let output = repository.path().join(&relative);
    for (path, bytes) in ORACLE_V7_FIXTURE_FILES {
        let destination = output.join(path);
        std::fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        std::fs::write(destination, bytes).expect("write Oracle fixture");
    }
    OracleFixture {
        _repository: repository,
        root,
        relative,
    }
}

fn oracle_v7_statistical_fixture() -> OracleFixture {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = PathBuf::from("target/qualification/oracle-schema-v7-statistical");
    let output = repository.path().join(&relative);
    for (path, bytes) in ORACLE_V7_STATISTICAL_FIXTURE_FILES {
        let destination = output.join(path);
        std::fs::create_dir_all(destination.parent().expect("fixture parent"))
            .expect("create fixture parent");
        std::fs::write(destination, bytes).expect("write Oracle statistical fixture");
    }
    OracleFixture {
        _repository: repository,
        root,
        relative,
    }
}

fn mutate_oracle_fixture(fixture: &OracleFixture, mutation: OraclePassingMutation) -> String {
    let output = fixture.root.path.join(&fixture.relative);
    let receipt_path = output
        .join("cases")
        .join(ORACLE_V7_CASE_ID)
        .join("execution-receipt.json");
    let mut receipt: ExecutionReceipt = read_fixture(&receipt_path);
    let mut report: CorrectnessReport = read_fixture(&output.join("report.json"));
    let mut completion: RunCompletion = read_fixture(&output.join("completion.json"));
    let mut preflight: CorrectnessPreflight = read_fixture(&output.join("preflight.json"));

    match mutation {
        OraclePassingMutation::MissingStdout => {
            receipt.stdout = None;
            only_result_mut(&mut report).stdout_sha256 = None;
            preflight
                .cases
                .get_mut(ORACLE_V7_CASE_ID)
                .expect("Oracle fixture preflight case")
                .stdout_sha256 = None;
        }
        OraclePassingMutation::MissingStderr => {
            receipt.stderr = None;
            only_result_mut(&mut report).stderr_sha256 = None;
            preflight
                .cases
                .get_mut(ORACLE_V7_CASE_ID)
                .expect("Oracle fixture preflight case")
                .stderr_sha256 = None;
        }
        OraclePassingMutation::IncompleteStdout => {
            receipt
                .stdout
                .as_mut()
                .expect("Oracle fixture stdout receipt")
                .complete = false;
        }
        OraclePassingMutation::RetainedArtifact => {
            let bytes = b"retained failure artifact";
            let path = PathBuf::from(format!("cases/{ORACLE_V7_CASE_ID}/failure.txt"));
            std::fs::write(output.join(&path), bytes).expect("write retained artifact");
            let artifact = ReportArtifact {
                path,
                bytes: bytes.len(),
                sha256: super::super::run::sha256_hex(bytes),
            };
            receipt.auxiliary_outputs.push(artifact.clone());
            only_result_mut(&mut report).artifacts.push(artifact);
        }
        OraclePassingMutation::MissingExitStatus => receipt.exit_status = None,
        OraclePassingMutation::MultipleCargoTests => {
            receipt.exact_test_count = Some(2);
            only_result_mut(&mut report).exact_test_count = Some(2);
        }
        OraclePassingMutation::UnreportedStatisticalAttempt => {
            receipt
                .statistical_attempts
                .push(StatisticalAttemptReceipt {
                    seed: 7,
                    verdict: "passed".to_string(),
                    completed_shots: 1,
                    completed_comparisons: 1,
                    completed_batches: 1,
                });
        }
        OraclePassingMutation::MissingReceiptStatisticalAttempt => {
            report.statistical_attempts.push(StatisticalAttempt {
                case_id: ORACLE_V7_CASE_ID.to_string(),
                seed: 11,
                completed_shots: 1,
                completed_comparisons: 1,
                completed_batches: 1,
                outcome: "passed".to_string(),
            });
        }
    }

    let receipt_bytes = canonical(&receipt);
    let receipt_sha256 = super::super::run::sha256_hex(&receipt_bytes);
    std::fs::write(&receipt_path, receipt_bytes).expect("rewrite execution receipt");

    only_result_mut(&mut report).execution_receipt_sha256 = receipt_sha256.clone();
    let report_bytes = canonical(&report);
    let report_sha256 = super::super::run::sha256_hex(&report_bytes);
    std::fs::write(output.join("report.json"), report_bytes).expect("rewrite report");

    completion.report_sha256 = report_sha256.clone();
    completion
        .cases
        .first_mut()
        .expect("Oracle fixture completion case")
        .execution_receipt_sha256 = receipt_sha256.clone();
    let completion_bytes = canonical(&completion);
    let completion_sha256 = super::super::run::sha256_hex(&completion_bytes);
    std::fs::write(output.join("completion.json"), completion_bytes).expect("rewrite completion");

    preflight.report_sha256 = report_sha256;
    preflight.completion_sha256 = completion_sha256.clone();
    let stdout_sha256 = only_result(&report).stdout_sha256.clone();
    let stderr_sha256 = only_result(&report).stderr_sha256.clone();
    let preflight_case = preflight
        .cases
        .get_mut(ORACLE_V7_CASE_ID)
        .expect("Oracle fixture preflight case");
    preflight_case.execution_receipt_sha256 = receipt_sha256;
    preflight_case.stdout_sha256 = stdout_sha256;
    preflight_case.stderr_sha256 = stderr_sha256;
    std::fs::write(output.join("preflight.json"), canonical(&preflight))
        .expect("rewrite preflight");

    completion_sha256
}

fn mutate_oracle_statistical_fixture(
    fixture: &OracleFixture,
    mutation: OracleStatisticalMutation,
) -> String {
    let output = fixture.root.path.join(&fixture.relative);
    let receipt_path = output
        .join("cases")
        .join(ORACLE_V7_STATISTICAL_CASE_ID)
        .join("execution-receipt.json");
    let mut receipt: ExecutionReceipt = read_fixture(&receipt_path);
    let mut report: CorrectnessReport = read_fixture(&output.join("report.json"));
    let mut completion: RunCompletion = read_fixture(&output.join("completion.json"));
    let mut preflight: CorrectnessPreflight = read_fixture(&output.join("preflight.json"));

    match mutation {
        OracleStatisticalMutation::OrphanAttempt => {
            let mut attempt = report
                .statistical_attempts
                .first()
                .expect("Oracle statistical attempt")
                .clone();
            attempt.case_id = "cq-orphan".to_string();
            report.statistical_attempts.push(attempt);
        }
        OracleStatisticalMutation::DuplicateAttempt => {
            report.statistical_attempts.push(
                report
                    .statistical_attempts
                    .first()
                    .expect("Oracle statistical attempt")
                    .clone(),
            );
            receipt.statistical_attempts.push(
                receipt
                    .statistical_attempts
                    .first()
                    .expect("Oracle statistical receipt attempt")
                    .clone(),
            );
        }
        OracleStatisticalMutation::NonStatisticalOwnership => {
            only_result_mut(&mut report).comparator = "exact-value".to_string();
        }
        OracleStatisticalMutation::ShotAggregateDrift => {
            report.statistical_planned_shots += 1;
            report.statistical_shots += 1;
        }
        OracleStatisticalMutation::SeedAggregateDrift => {
            let seeds = vec![12648439];
            report
                .statistical_planned_seeds
                .insert(ORACLE_V7_STATISTICAL_CASE_ID.to_string(), seeds.clone());
            report
                .statistical_seeds
                .insert(ORACLE_V7_STATISTICAL_CASE_ID.to_string(), seeds);
        }
        OracleStatisticalMutation::IncompleteSeedPanel => {
            report
                .statistical_planned_seeds
                .get_mut(ORACLE_V7_STATISTICAL_CASE_ID)
                .expect("Oracle planned statistical seeds")
                .push(12648439);
        }
        OracleStatisticalMutation::ZeroCompletedWork => {
            let attempt = report
                .statistical_attempts
                .first_mut()
                .expect("Oracle statistical attempt");
            attempt.completed_shots = 0;
            attempt.completed_comparisons = 0;
            attempt.completed_batches = 0;
            let receipt_attempt = receipt
                .statistical_attempts
                .first_mut()
                .expect("Oracle statistical receipt attempt");
            receipt_attempt.completed_shots = 0;
            receipt_attempt.completed_comparisons = 0;
            receipt_attempt.completed_batches = 0;
            report.statistical_planned_shots = 0;
            report.statistical_shots = 0;
        }
    }

    rewrite_oracle_fixture(
        &output,
        &receipt_path,
        ORACLE_V7_STATISTICAL_CASE_ID,
        receipt,
        report,
        &mut completion,
        &mut preflight,
    )
}

fn rewrite_oracle_fixture(
    output: &Path,
    receipt_path: &Path,
    case_id: &str,
    receipt: ExecutionReceipt,
    mut report: CorrectnessReport,
    completion: &mut RunCompletion,
    preflight: &mut CorrectnessPreflight,
) -> String {
    let receipt_bytes = canonical(&receipt);
    let receipt_sha256 = super::super::run::sha256_hex(&receipt_bytes);
    std::fs::write(receipt_path, receipt_bytes).expect("rewrite execution receipt");

    only_result_mut(&mut report).execution_receipt_sha256 = receipt_sha256.clone();
    let report_bytes = canonical(&report);
    let report_sha256 = super::super::run::sha256_hex(&report_bytes);
    std::fs::write(output.join("report.json"), report_bytes).expect("rewrite report");

    completion.report_sha256 = report_sha256.clone();
    completion
        .cases
        .iter_mut()
        .find(|case| case.case_id == case_id)
        .expect("Oracle fixture completion case")
        .execution_receipt_sha256 = receipt_sha256.clone();
    let completion_bytes = canonical(completion);
    let completion_sha256 = super::super::run::sha256_hex(&completion_bytes);
    std::fs::write(output.join("completion.json"), completion_bytes).expect("rewrite completion");

    preflight.report_sha256 = report_sha256;
    preflight.completion_sha256 = completion_sha256.clone();
    let result = only_result(&report);
    let preflight_case = preflight
        .cases
        .get_mut(case_id)
        .expect("Oracle fixture preflight case");
    preflight_case.execution_receipt_sha256 = receipt_sha256;
    preflight_case
        .stdout_sha256
        .clone_from(&result.stdout_sha256);
    preflight_case
        .stderr_sha256
        .clone_from(&result.stderr_sha256);
    std::fs::write(output.join("preflight.json"), canonical(preflight)).expect("rewrite preflight");

    completion_sha256
}

fn read_fixture<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&std::fs::read(path).expect("read fixture")).expect("parse fixture")
}

fn only_result(report: &CorrectnessReport) -> &CaseResult {
    assert_eq!(report.results.len(), 1, "Oracle fixture result count");
    report.results.first().expect("Oracle fixture result")
}

fn only_result_mut(report: &mut CorrectnessReport) -> &mut CaseResult {
    assert_eq!(report.results.len(), 1, "Oracle fixture result count");
    report.results.first_mut().expect("Oracle fixture result")
}

struct Fixture {
    _repository: tempfile::TempDir,
    root: RepoRoot,
    relative: PathBuf,
    manifest: String,
    commit: String,
    request_sha256: String,
    completion_sha256: String,
}

fn assert_fixture_rejected(fixture: &Fixture) {
    assert!(
        validate(
            &fixture.root,
            CorrectnessRequirement::Required {
                output: &fixture.relative,
                case_ids: &["cq-case".to_string()],
                expected_manifest_sha256: &fixture.manifest,
                expected_stab_commit: &fixture.commit,
                expected_request_sha256: &fixture.request_sha256,
                expected_completion_sha256: &fixture.completion_sha256,
            },
        )
        .is_err()
    );
}

fn assert_fixture_accepted(fixture: &Fixture) {
    validate(
        &fixture.root,
        CorrectnessRequirement::Required {
            output: &fixture.relative,
            case_ids: &["cq-case".to_string()],
            expected_manifest_sha256: &fixture.manifest,
            expected_stab_commit: &fixture.commit,
            expected_request_sha256: &fixture.request_sha256,
            expected_completion_sha256: &fixture.completion_sha256,
        },
    )
    .expect("correctness fixture should pass");
}

fn fixture(mutation: FixtureMutation) -> Fixture {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let relative = PathBuf::from("target/qualification/correctness/full");
    let output = repository.path().join(&relative);
    std::fs::create_dir_all(output.join("cases/cq-case")).expect("create correctness output");
    let manifest = "a".repeat(64);
    let commit = "b".repeat(40);
    let schema_family = if matches!(mutation, FixtureMutation::LegacySchema) {
        CorrectnessSchemaFamily::V6
    } else {
        CorrectnessSchemaFamily::V7
    };
    let resolved_selector = matches!(mutation, FixtureMutation::ResolvedSelectorDigest);
    let selector = EvidenceSelector {
        state: "existing".to_string(),
        kind: if resolved_selector {
            "oracle-fixture".to_string()
        } else {
            "cargo-test".to_string()
        },
        value: if resolved_selector {
            vec!["fixture-id".to_string()]
        } else {
            vec!["cargo".to_string(), "test".to_string()]
        },
    };
    let selector_sha256 = if resolved_selector {
        "9".repeat(64)
    } else {
        super::super::run::sha256_hex(&serde_json::to_vec(&selector).expect("serialize selector"))
    };
    let executable = ExecutableIdentity {
        role: "cargo".to_string(),
        bytes: 1,
        sha256: "c".repeat(64),
    };
    let request = RunRequest {
        schema_version: RUN_REQUEST_SCHEMA_VERSION,
        qualification_manifest_digest: manifest.clone(),
        stab_commit: commit.clone(),
        worktree_was_clean: true,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        tier: "full".to_string(),
        feature_filters: Vec::new(),
        case_filters: Vec::new(),
        allow_deferred: false,
        executables: vec![executable.clone()],
        execution_environment_sha256: "d".repeat(64),
        selected_cases: vec![RequestedCase {
            case_id: "cq-case".to_string(),
            selector_sha256: selector_sha256.clone(),
            case_contract_sha256: "e".repeat(64),
        }],
        planned_case_ids: Vec::new(),
        deferred_case_ids: Vec::new(),
    };
    let request_bytes = canonical(&request);
    let request_sha256 = super::super::run::sha256_hex(&request_bytes);
    std::fs::write(output.join("request.json"), &request_bytes).expect("write request");

    let receipt = ExecutionReceipt {
        schema_version: if matches!(mutation, FixtureMutation::MismatchedReceiptSchema) {
            CorrectnessSchemaFamily::V6.execution_receipt_version()
        } else {
            schema_family.execution_receipt_version()
        },
        run_request_sha256: request_sha256.clone(),
        case_id: "cq-case".to_string(),
        selector_sha256: selector_sha256.clone(),
        executables: vec![executable],
        execution_environment_sha256: "d".repeat(64),
        verdict: "accepted".to_string(),
        exit_status: Some(0),
        exact_test_count: Some(1),
        stdout: Some(StreamReceipt {
            bytes: 0,
            sha256: "f".repeat(64),
            complete: true,
        }),
        stderr: Some(StreamReceipt {
            bytes: 0,
            sha256: "1".repeat(64),
            complete: true,
        }),
        statistical_attempts: Vec::new(),
        auxiliary_outputs: Vec::new(),
    };
    let receipt_bytes = canonical(&receipt);
    let receipt_sha256 = super::super::run::sha256_hex(&receipt_bytes);
    std::fs::write(
        output.join("cases/cq-case/execution-receipt.json"),
        receipt_bytes,
    )
    .expect("write execution receipt");

    let outcome = if matches!(mutation, FixtureMutation::FailedReport) {
        "failed"
    } else {
        "passed"
    };
    let report = CorrectnessReport {
        schema_version: schema_family.report_and_preflight_version(),
        qualification_manifest_digest: manifest.clone(),
        run_request_sha256: request_sha256.clone(),
        stab_commit: commit.clone(),
        local_modifications: false,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        rust_toolchain: "nightly".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        operating_system: "linux".to_string(),
        architecture: "x86_64".to_string(),
        tier: "full".to_string(),
        feature_filters: Vec::new(),
        case_filters: Vec::new(),
        allow_deferred: false,
        selected_count: 1,
        planned_count: 0,
        deferred_count: 0,
        passed_count: usize::from(outcome == "passed"),
        failed_count: usize::from(outcome == "failed"),
        selection_complete: true,
        statistical_declared_budget: "0.00000000000000000e0".to_string(),
        statistical_consumed_bound: "0.00000000000000000e0".to_string(),
        statistical_planned_shots: 0,
        statistical_planned_seeds: BTreeMap::new(),
        statistical_shots: 0,
        statistical_seeds: BTreeMap::new(),
        statistical_attempts: Vec::new(),
        property_corpus_ids: Vec::new(),
        resource_case_count: 0,
        upstream_dispositions: Vec::new(),
        deferred_products: BTreeMap::new(),
        case_counts: Vec::new(),
        resource_contracts: Vec::new(),
        results: vec![CaseResult {
            case_id: "cq-case".to_string(),
            feature_id: "CQ-CASE".to_string(),
            comparator: "exact-value".to_string(),
            selector,
            selector_sha256,
            execution_receipt_sha256: receipt_sha256,
            outcome: outcome.to_string(),
            exact_test_count: Some(1),
            stdout_sha256: Some("f".repeat(64)),
            stderr_sha256: Some("1".repeat(64)),
            artifacts: Vec::new(),
        }],
    };
    let report_bytes = canonical(&report);
    let report_sha256 = super::super::run::sha256_hex(&report_bytes);
    std::fs::write(output.join("report.json"), &report_bytes).expect("write report");

    let result = report.results.first().expect("single report result");
    let completion_digest = if matches!(mutation, FixtureMutation::MismatchedCompletion) {
        "0".repeat(64)
    } else {
        result.execution_receipt_sha256.clone()
    };
    let completion = RunCompletion {
        schema_version: RUN_COMPLETION_SCHEMA_VERSION,
        run_request_sha256: request_sha256.clone(),
        report_sha256: report_sha256.clone(),
        cases: vec![CompletedCase {
            case_id: "cq-case".to_string(),
            execution_receipt_sha256: completion_digest,
        }],
    };
    let completion_bytes = canonical(&completion);
    let completion_sha256 = super::super::run::sha256_hex(&completion_bytes);
    std::fs::write(output.join("completion.json"), &completion_bytes).expect("write completion");

    let preflight_outcome = if matches!(mutation, FixtureMutation::FabricatedPreflightPass) {
        "failed"
    } else {
        outcome
    };
    let preflight = CorrectnessPreflight {
        schema_version: if matches!(mutation, FixtureMutation::MismatchedPreflightSchema) {
            CorrectnessSchemaFamily::V6.report_and_preflight_version()
        } else {
            schema_family.report_and_preflight_version()
        },
        report_sha256,
        completion_sha256: completion_sha256.clone(),
        qualification_manifest_digest: manifest.clone(),
        run_request_sha256: request_sha256.clone(),
        stab_commit: commit.clone(),
        local_modifications: false,
        stim_commit: STIM_COMMIT.to_string(),
        tier: "full".to_string(),
        allow_deferred: false,
        selection_complete: true,
        deferred_count: 0,
        cases: BTreeMap::from([(
            "cq-case".to_string(),
            CorrectnessCaseReceipt {
                outcome: preflight_outcome.to_string(),
                selector_sha256: result.selector_sha256.clone(),
                execution_receipt_sha256: result.execution_receipt_sha256.clone(),
                stdout_sha256: result.stdout_sha256.clone(),
                stderr_sha256: result.stderr_sha256.clone(),
            },
        )]),
    };
    std::fs::write(output.join("preflight.json"), canonical(&preflight)).expect("write preflight");

    Fixture {
        _repository: repository,
        root,
        relative,
        manifest,
        commit,
        request_sha256,
        completion_sha256,
    }
}

fn canonical<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize canonical fixture");
    bytes.push(b'\n');
    bytes
}
