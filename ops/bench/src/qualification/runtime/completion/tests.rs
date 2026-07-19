use super::*;
use std::num::NonZeroU64;

use crate::qualification::runtime::correctness::{
    CorrectnessPreflightEvidence, CorrectnessPreflightStatus,
};
use crate::qualification::runtime::group::ScaleContract;
use crate::qualification::runtime::protocol::{InputDigest, ProtocolId};
use crate::qualification::runtime::rollup::{RollupReplayEvidence, RollupSourceEvidence};

fn digest(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn workers() -> WorkerIdentityEvidence {
    WorkerIdentityEvidence {
        stim_source_sha256: digest('1'),
        stim_build_fingerprint: digest('2'),
        stim_binary_sha256: digest('3'),
        stab_source_sha256: digest('4'),
        stab_build_fingerprint: digest('5'),
        stab_binary_sha256: digest('6'),
        contract_preflight_sha256: digest('7'),
    }
}

fn artifacts(path: &str, byte: char) -> Vec<ArtifactReceipt> {
    ["report.json", "preflight.json", "report.md"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| ArtifactReceipt {
            path: path.to_string(),
            name: name.to_string(),
            bytes: 100 + u64::try_from(index).expect("small fixture index"),
            sha256: digest(byte),
        })
        .collect()
}

fn publish_directory(
    root: &RepoRoot,
    path: &Path,
    report: &[u8],
    preflight: &[u8],
    markdown: &[u8],
) {
    let output = super::super::artifact::QualificationOutput::begin(root, path)
        .expect("begin test publication");
    output
        .write("report.json", report)
        .expect("write test report");
    output
        .write("preflight.json", preflight)
        .expect("write test preflight");
    output
        .write("report.md", markdown)
        .expect("write test markdown");
    output.commit().expect("publish test directory");
}

fn replace_directory(
    repository: &tempfile::TempDir,
    path: &Path,
    report: &[u8],
    preflight: &[u8],
    markdown: &[u8],
) {
    let target = repository.path().join(path);
    let moved = target.with_extension("detached");
    std::fs::rename(&target, &moved).expect("move bound directory");
    std::fs::create_dir(&target).expect("replace bound directory");
    std::fs::write(target.join("report.json"), report).expect("write replacement report");
    std::fs::write(target.join("preflight.json"), preflight).expect("write replacement preflight");
    std::fs::write(target.join("report.md"), markdown).expect("write replacement markdown");
}

fn probe(workers: &WorkerIdentityEvidence) -> AdapterProbeReceipt {
    AdapterProbeReceipt {
        probe_id: "pq2-test-adapter-smoke".to_string(),
        runtime_group_id: "PERFQ-TEST".to_string(),
        evidence_mode: "timing".to_string(),
        iteration_count: 4,
        work_items: 4_096,
        work_count: 16_384,
        input_bytes: 512,
        input_digest: digest('8'),
        output_digest: digest('9'),
        stim_source_sha256: workers.stim_source_sha256.clone(),
        stim_build_fingerprint: workers.stim_build_fingerprint.clone(),
        stim_binary_sha256: workers.stim_binary_sha256.clone(),
        stab_source_sha256: workers.stab_source_sha256.clone(),
        stab_build_fingerprint: digest('a'),
    }
}

fn directory(
    tier: QualificationTier,
    scale: Option<&str>,
    name: &str,
    byte: char,
) -> EvidenceDirectoryReceipt {
    let path = format!("target/benchmarks/qualification/{name}");
    EvidenceDirectoryReceipt {
        tier,
        scale_id: scale.map(ToOwned::to_owned),
        artifacts: artifacts(&path, byte),
        path,
    }
}

fn receipt() -> CompletionReceipt {
    let workers = workers();
    let probe = probe(&workers);
    let source_reports = vec![
        directory(QualificationTier::Full, Some("small"), "full-small", 'b'),
        directory(QualificationTier::Soak, Some("small"), "soak-small", 'c'),
    ];
    let rollups = vec![
        directory(QualificationTier::Full, None, "full-rollup", 'd'),
        directory(QualificationTier::Soak, None, "soak-rollup", 'e'),
    ];
    let mut steps = Vec::new();
    push_step(
        &mut steps,
        CompletionStepKind::WorkerReproducibility,
        &"f".repeat(40),
        vec!["qualification-worker-reproducibility".to_string()],
        Vec::new(),
        Vec::new(),
        CompletionStepResult::WorkerReproducibility {
            workers: workers.clone(),
        },
    );
    push_step(
        &mut steps,
        CompletionStepKind::AdapterProbe,
        &"f".repeat(40),
        probe_arguments(&probe),
        Vec::new(),
        Vec::new(),
        CompletionStepResult::AdapterProbe { probe },
    );
    for source in &source_reports {
        let scale_id = source.scale_id.clone().expect("source scale");
        push_step(
            &mut steps,
            CompletionStepKind::ReportReplay,
            &"f".repeat(40),
            vec![
                "qualification-report".to_string(),
                "--input".to_string(),
                source.path.clone(),
            ],
            source.artifacts.clone(),
            source.artifacts.clone(),
            CompletionStepResult::ReportReplay {
                tier: source.tier,
                scale_id,
            },
        );
        push_step(
            &mut steps,
            CompletionStepKind::Regression,
            &"f".repeat(40),
            vec![
                "qualification-regression".to_string(),
                "--input".to_string(),
                source.path.clone(),
                "--baseline".to_string(),
                super::super::regression::DEFAULT_BASELINE.to_string(),
            ],
            source.artifacts.clone(),
            Vec::new(),
            CompletionStepResult::Regression {
                group_id: "PERFQ-TEST".to_string(),
                checked_measurements: 1,
                report_only: false,
            },
        );
    }
    for rollup in &rollups {
        push_step(
            &mut steps,
            CompletionStepKind::RollupReplay,
            &"f".repeat(40),
            vec![
                "qualification-rollup-report".to_string(),
                "--input".to_string(),
                rollup.path.clone(),
            ],
            rollup.artifacts.clone(),
            rollup.artifacts.clone(),
            CompletionStepResult::RollupReplay {
                tier: rollup.tier,
                scale_count: 1,
                overall_outcome: GateOutcome::Passed,
            },
        );
    }
    CompletionReceipt {
        schema_version: COMPLETION_SCHEMA_VERSION,
        output: "target/benchmarks/qualification/completion".to_string(),
        generated_unix_epoch_seconds: 1_234,
        group_id: "PERFQ-TEST".to_string(),
        group_contract_sha256: digest('0'),
        performance_inventory_sha256: digest('1'),
        correctness_inventory_sha256: digest('2'),
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        repository: RepositoryEvidence {
            commit_before: "f".repeat(40),
            commit_after: "f".repeat(40),
            local_modifications_before: false,
            local_modifications_after: false,
        },
        environment: CompletionEnvironmentEvidence {
            host_policy_sha256: digest('3'),
            host_profile_id: "controlled".to_string(),
            architecture: "aarch64".to_string(),
            cpu_identity: "test CPU".to_string(),
            target_triple: "aarch64-unknown-linux-gnu".to_string(),
            toolchain_sha256: digest('4'),
        },
        workers,
        correctness_preflight: CorrectnessPreflightEvidence {
            status: CorrectnessPreflightStatus::Passed,
            case_ids: vec!["cq-test".to_string()],
            reason: "exact prerequisite passed".to_string(),
            source_directory: Some("target/qualification/correctness/test".to_string()),
            qualification_manifest_sha256: Some(digest('5')),
            request_sha256: Some(digest('6')),
            completion_sha256: Some(digest('7')),
            report_sha256: Some(digest('8')),
            preflight_sha256: Some(digest('9')),
        },
        source_reports,
        rollups,
        steps,
    }
}

fn group_contract() -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new("PERFQ-TEST").expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new("test-workload").expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            work_items: NonZeroU64::new(4_096).expect("nonzero work"),
            input_bytes: 512,
            input_digest: InputDigest::try_new(digest('8')).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-test".to_string()],
        owner: ProtocolId::try_new("bench").expect("owner id"),
        profiler_note: None,
        comparator_sources: Vec::new(),
    }
}

fn rollup_evidence(receipt: &CompletionReceipt, tier: QualificationTier) -> RollupReplayEvidence {
    let rollup = receipt
        .rollups
        .iter()
        .find(|rollup| rollup.tier == tier)
        .expect("tier rollup");
    let sources = receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == tier)
        .map(|source| RollupSourceEvidence {
            scale_id: source.scale_id.clone().expect("source scale"),
            path: PathBuf::from(&source.path),
            report_sha256: artifact_digest(&source.artifacts, "report.json")
                .expect("source report digest")
                .to_string(),
            preflight_sha256: artifact_digest(&source.artifacts, "preflight.json")
                .expect("source preflight digest")
                .to_string(),
        })
        .collect();
    RollupReplayEvidence {
        output: PathBuf::from(&rollup.path),
        report_sha256: artifact_digest(&rollup.artifacts, "report.json")
            .expect("rollup report digest")
            .to_string(),
        preflight_sha256: artifact_digest(&rollup.artifacts, "preflight.json")
            .expect("rollup preflight digest")
            .to_string(),
        markdown_sha256: artifact_digest(&rollup.artifacts, "report.md")
            .expect("rollup markdown digest")
            .to_string(),
        group_id: receipt.group_id.clone(),
        tier,
        stab_commit: receipt.repository.commit_after.clone(),
        host_policy_sha256: receipt.environment.host_policy_sha256.clone(),
        host_profile_id: receipt.environment.host_profile_id.clone(),
        architecture: receipt.environment.architecture.clone(),
        cpu_identity: receipt.environment.cpu_identity.clone(),
        target_triple: receipt.environment.target_triple.clone(),
        toolchain_sha256: receipt.environment.toolchain_sha256.clone(),
        workers: receipt.workers.clone(),
        correctness_preflight: receipt.correctness_preflight.clone(),
        overall_outcome: GateOutcome::Passed,
        sources,
    }
}

fn step_mut(receipt: &mut CompletionReceipt, index: usize) -> &mut CompletionStep {
    receipt.steps.get_mut(index).expect("fixture step")
}

fn selected_report(
    directory: &EvidenceDirectoryReceipt,
    workers: &WorkerIdentityEvidence,
) -> SelectedReport {
    SelectedReport {
        path: DirectQualificationArtifactPath::try_new(Path::new(&directory.path))
            .expect("direct source path"),
        tier: directory.tier,
        scale_id: directory.scale_id.clone().expect("source scale"),
        workers: workers.clone(),
        artifacts: directory.artifacts.clone(),
    }
}

struct FakeActions {
    receipt: CompletionReceipt,
    calls: Vec<String>,
    fail_at: Option<String>,
    change_report_artifact: bool,
}

impl FakeActions {
    fn new(receipt: CompletionReceipt) -> Self {
        Self {
            receipt,
            calls: Vec::new(),
            fail_at: None,
            change_report_artifact: false,
        }
    }

    fn record(&mut self, action: String) -> Result<(), CompletionError> {
        self.calls.push(action.clone());
        if self.fail_at.as_deref() == Some(&action) {
            Err(CompletionError::Action {
                name: "injected workflow action",
                detail: action,
            })
        } else {
            Ok(())
        }
    }
}

impl workflow::Actions for FakeActions {
    fn workers(
        &mut self,
        _context: &ReplayContext<'_>,
    ) -> Result<WorkerIdentityEvidence, CompletionError> {
        self.record("workers".to_string())?;
        Ok(self.receipt.workers.clone())
    }

    fn probe(
        &mut self,
        _context: &ReplayContext<'_>,
        _group_id: &str,
    ) -> Result<AdapterProbeReceipt, CompletionError> {
        self.record("probe".to_string())?;
        Ok(probe(&self.receipt.workers))
    }

    fn report(
        &mut self,
        _context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<Vec<ArtifactReceipt>, CompletionError> {
        self.record(format!("report-{:?}-{}", selected.tier, selected.scale_id))?;
        let mut artifacts = selected.artifacts.clone();
        if self.change_report_artifact {
            artifacts.first_mut().expect("report artifact").sha256 = digest('a');
        }
        Ok(artifacts)
    }

    fn regression(
        &mut self,
        context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<super::super::regression::RegressionSummary, CompletionError> {
        self.record(format!(
            "regression-{:?}-{}",
            selected.tier, selected.scale_id
        ))?;
        Ok(super::super::regression::RegressionSummary {
            group_id: context.contract.id.to_string(),
            checked_measurements: context.contract.measurement_ids.len(),
            report_only: false,
        })
    }

    fn rollup(
        &mut self,
        _context: &ReplayContext<'_>,
        path: &DirectQualificationArtifactPath,
    ) -> Result<
        (
            Vec<ArtifactReceipt>,
            Vec<ArtifactReceipt>,
            RollupReplayEvidence,
        ),
        CompletionError,
    > {
        let directory = self
            .receipt
            .rollups
            .iter()
            .find(|rollup| Path::new(&rollup.path) == path.as_path())
            .cloned()
            .ok_or(CompletionError::RollupIdentity)?;
        self.record(format!("rollup-{:?}", directory.tier))?;
        let evidence = rollup_evidence(&self.receipt, directory.tier);
        Ok((directory.artifacts.clone(), directory.artifacts, evidence))
    }
}

fn run_fake_workflow(
    actions: &mut FakeActions,
) -> Result<workflow::WorkflowEvidence, CompletionError> {
    let temporary = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(temporary.path()).expect("repository root");
    let contract = group_contract();
    let commit = actions.receipt.repository.commit_after.clone();
    let performance_inventory = actions.receipt.performance_inventory_sha256.clone();
    let correctness_inventory = actions.receipt.correctness_inventory_sha256.clone();
    let context = ReplayContext {
        root: &root,
        performance_inventory_sha256: &performance_inventory,
        correctness_inventory_sha256: &correctness_inventory,
        contract: &contract,
        repository_commit: &commit,
    };
    let full = actions
        .receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == QualificationTier::Full)
        .map(|source| selected_report(source, &actions.receipt.workers))
        .collect();
    let soak = actions
        .receipt
        .source_reports
        .iter()
        .filter(|source| source.tier == QualificationTier::Soak)
        .map(|source| selected_report(source, &actions.receipt.workers))
        .collect();
    let full_rollup = DirectQualificationArtifactPath::try_new(Path::new(
        &actions.receipt.rollups.first().expect("full rollup").path,
    ))
    .expect("full rollup path");
    let soak_rollup = DirectQualificationArtifactPath::try_new(Path::new(
        &actions.receipt.rollups.get(1).expect("soak rollup").path,
    ))
    .expect("soak rollup path");
    workflow::run(
        &context,
        "PERFQ-TEST",
        full,
        soak,
        full_rollup,
        soak_rollup,
        actions,
    )
}

#[test]
fn completion_structure_binds_exact_step_sequence_and_artifacts() {
    let valid = receipt();
    validation::validate(&valid).expect("valid completion receipt");

    let mut wrong_exit = valid.clone();
    step_mut(&mut wrong_exit, 2).exit_status = 1;
    assert!(validation::validate(&wrong_exit).is_err());

    let mut repaired_input = valid.clone();
    step_mut(&mut repaired_input, 2)
        .inputs
        .get_mut(2)
        .expect("fixture report artifact")
        .sha256 = digest('a');
    assert!(validation::validate(&repaired_input).is_err());

    let mut report_only = valid;
    let result = &mut step_mut(&mut report_only, 3).result;
    assert!(matches!(result, CompletionStepResult::Regression { .. }));
    if let CompletionStepResult::Regression {
        report_only: report_only_flag,
        ..
    } = result
    {
        *report_only_flag = true;
    }
    assert!(validation::validate(&report_only).is_err());
}

#[test]
fn completion_workflow_runs_exact_handlers_in_order() {
    let mut actions = FakeActions::new(receipt());
    let evidence = run_fake_workflow(&mut actions).expect("successful workflow");
    assert_eq!(
        actions.calls,
        [
            "workers",
            "probe",
            "report-Full-small",
            "regression-Full-small",
            "report-Soak-small",
            "regression-Soak-small",
            "rollup-Full",
            "rollup-Soak",
        ]
    );
    assert_eq!(evidence.source_reports.len(), 2);
    assert_eq!(evidence.rollups.len(), 2);
    assert_eq!(evidence.steps.len(), 8);
}

#[test]
fn completion_workflow_stops_at_first_handler_failure() {
    let mut actions = FakeActions::new(receipt());
    actions.fail_at = Some("regression-Full-small".to_string());
    assert!(run_fake_workflow(&mut actions).is_err());
    assert_eq!(
        actions.calls,
        [
            "workers",
            "probe",
            "report-Full-small",
            "regression-Full-small",
        ]
    );
}

#[test]
fn completion_workflow_rejects_non_idempotent_live_report_replay() {
    let mut actions = FakeActions::new(receipt());
    actions.change_report_artifact = true;
    assert!(matches!(
        run_fake_workflow(&mut actions),
        Err(CompletionError::NonIdempotentReplay(_))
    ));
    assert_eq!(actions.calls, ["workers", "probe", "report-Full-small"]);
}

#[test]
fn completion_structure_rejects_path_tier_and_result_substitution() {
    let valid = receipt();

    let mut duplicate_path = valid.clone();
    let full_path = duplicate_path
        .source_reports
        .first()
        .expect("full source")
        .path
        .clone();
    let duplicate_source = duplicate_path
        .source_reports
        .get_mut(1)
        .expect("soak source");
    duplicate_source.path.clone_from(&full_path);
    for artifact in &mut duplicate_source.artifacts {
        artifact.path.clone_from(&full_path);
    }
    assert!(validation::validate(&duplicate_path).is_err());

    let mut wrong_scale_family = valid.clone();
    wrong_scale_family
        .source_reports
        .get_mut(1)
        .expect("soak source")
        .scale_id = Some("different".to_string());
    assert!(validation::validate(&wrong_scale_family).is_err());

    let mut reversed_tiers = valid.clone();
    reversed_tiers.source_reports.swap(0, 1);
    assert!(validation::validate(&reversed_tiers).is_err());

    let mut unsafe_path = valid.clone();
    let unsafe_source = unsafe_path.source_reports.first_mut().expect("full source");
    unsafe_source.path = "target/benchmarks/qualification/../escape".to_string();
    let unsafe_source_path = unsafe_source.path.clone();
    for artifact in &mut unsafe_source.artifacts {
        artifact.path.clone_from(&unsafe_source_path);
    }
    assert!(validation::validate(&unsafe_path).is_err());

    let mut failed_rollup = valid.clone();
    let last = failed_rollup.steps.len() - 1;
    if let CompletionStepResult::RollupReplay {
        overall_outcome, ..
    } = &mut step_mut(&mut failed_rollup, last).result
    {
        *overall_outcome = GateOutcome::Failed;
    }
    assert!(matches!(
        &step_mut(&mut failed_rollup, last).result,
        CompletionStepResult::RollupReplay {
            overall_outcome: GateOutcome::Failed,
            ..
        }
    ));
    assert!(validation::validate(&failed_rollup).is_err());

    let mut wrong_arguments = valid;
    step_mut(&mut wrong_arguments, 3).canonical_arguments.pop();
    assert!(validation::validate(&wrong_arguments).is_err());
}

#[test]
fn completion_preflight_binds_steps_and_every_evidence_directory() {
    let original = receipt();
    let original_json = canonical_json(&original).expect("canonical receipt");
    let original_preflight =
        completion_preflight(&original, &original_json).expect("completion preflight");

    let mut changed_step = original.clone();
    step_mut(&mut changed_step, 1)
        .canonical_arguments
        .push("--changed".to_string());
    let changed_json = canonical_json(&changed_step).expect("changed receipt");
    let changed_preflight =
        completion_preflight(&changed_step, &changed_json).expect("changed preflight");
    assert_ne!(original_preflight, changed_preflight);

    let mut changed_artifact = original;
    changed_artifact
        .rollups
        .first_mut()
        .expect("full rollup")
        .artifacts
        .first_mut()
        .expect("rollup report")
        .sha256 = digest('a');
    let changed_json = canonical_json(&changed_artifact).expect("changed artifact receipt");
    let changed_preflight =
        completion_preflight(&changed_artifact, &changed_json).expect("changed artifact preflight");
    assert_ne!(original_preflight, changed_preflight);
}

#[test]
fn completion_final_publication_rejects_source_replacement() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let source_path = Path::new("target/benchmarks/qualification/completion-source");
    publish_directory(
        &root,
        source_path,
        b"source report\n",
        b"source preflight\n",
        b"source markdown\n",
    );
    let source_path = DirectQualificationArtifactPath::try_new(source_path).expect("source path");
    let source_receipt = EvidenceDirectoryReceipt {
        tier: QualificationTier::Full,
        scale_id: Some("small".to_string()),
        path: path_text(source_path.as_path()).expect("source path text"),
        artifacts: read_artifact_receipts(&root, &source_path).expect("source artifacts"),
    };
    let mut completion = receipt();
    completion.source_reports = vec![source_receipt];
    completion.rollups.clear();
    let output_path = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/completion-output",
    ))
    .expect("completion output path");
    let publication = CompletionPublication {
        root: &root,
        output_path: &output_path,
        receipt: &completion,
        report_json: b"completion report\n",
        preflight_json: b"completion preflight\n",
        markdown: "completion markdown\n",
        existing_report_json: None,
        existing_preflight_json: None,
    };
    assert!(matches!(
        publication.publish_with(
            || {
                replace_directory(
                    &repository,
                    source_path.as_path(),
                    b"source report\n",
                    b"source preflight\n",
                    b"source markdown\n",
                );
                Ok(())
            },
            |output| output.commit().map_err(CompletionError::Artifact),
        ),
        Err(CompletionError::Artifact(
            super::super::artifact::ArtifactError::DirectoryIdentity(_)
        ))
    ));
    assert!(!repository.path().join(output_path.as_path()).exists());
}

#[test]
fn completion_final_publication_rejects_replay_target_replacement() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let output_path = Path::new("target/benchmarks/qualification/completion-replay");
    publish_directory(
        &root,
        output_path,
        b"old report\n",
        b"old preflight\n",
        b"old markdown\n",
    );
    let output_path =
        DirectQualificationArtifactPath::try_new(output_path).expect("completion output path");
    let mut completion = receipt();
    completion.source_reports.clear();
    completion.rollups.clear();
    let publication = CompletionPublication {
        root: &root,
        output_path: &output_path,
        receipt: &completion,
        report_json: b"new report\n",
        preflight_json: b"new preflight\n",
        markdown: "new markdown\n",
        existing_report_json: Some(b"old report\n"),
        existing_preflight_json: Some(b"old preflight\n"),
    };
    assert!(matches!(
        publication.publish_with(
            || {
                replace_directory(
                    &repository,
                    output_path.as_path(),
                    b"old report\n",
                    b"old preflight\n",
                    b"old markdown\n",
                );
                Ok(())
            },
            |output| output.commit().map_err(CompletionError::Artifact),
        ),
        Err(CompletionError::Artifact(
            super::super::artifact::ArtifactError::DirectoryIdentity(_)
        ))
    ));
    assert_eq!(
        std::fs::read(
            repository
                .path()
                .join(output_path.as_path())
                .join("report.json")
        )
        .expect("read replacement report"),
        b"old report\n"
    );
}

#[test]
fn completion_replay_publication_survives_old_tree_cleanup_failure() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let output_path = Path::new("target/benchmarks/qualification/completion-cleanup");
    publish_directory(
        &root,
        output_path,
        b"old report\n",
        b"old preflight\n",
        b"old markdown\n",
    );
    let output_path =
        DirectQualificationArtifactPath::try_new(output_path).expect("completion output path");
    let mut completion = receipt();
    completion.source_reports.clear();
    completion.rollups.clear();
    CompletionPublication {
        root: &root,
        output_path: &output_path,
        receipt: &completion,
        report_json: b"new report\n",
        preflight_json: b"new preflight\n",
        markdown: "new markdown\n",
        existing_report_json: Some(b"old report\n"),
        existing_preflight_json: Some(b"old preflight\n"),
    }
    .publish_with(
        || Ok(()),
        |output| {
            output
                .commit_with_cleanup(|_, _, _, _| {
                    Err(super::super::artifact::ArtifactError::DirectoryIdentity(
                        "injected cleanup failure",
                    ))
                })
                .map_err(CompletionError::Artifact)
        },
    )
    .expect("cleanup failure must not invalidate completion publication");
    assert_eq!(
        std::fs::read(
            repository
                .path()
                .join(output_path.as_path())
                .join("report.json")
        )
        .expect("read published completion"),
        b"new report\n"
    );
}

#[test]
fn completion_boundary_rejects_stale_preflight_and_noncanonical_json() {
    let receipt = receipt();
    let report_json = canonical_json(&receipt).expect("canonical receipt");
    let mut preflight = completion_preflight(&receipt, &report_json).expect("preflight");
    preflight.step_count += 1;
    assert!(
        validate_existing_boundary(
            &receipt,
            &preflight,
            &report_json,
            Path::new(&receipt.output),
            &receipt.performance_inventory_sha256,
            &receipt.correctness_inventory_sha256,
        )
        .is_err()
    );

    let mut noncanonical = report_json;
    noncanonical.extend_from_slice(b" \n");
    assert!(parse_canonical::<CompletionReceipt>(&noncanonical).is_err());
}

#[test]
fn replay_arguments_preserve_full_and_soak_evidence_roles() {
    let receipt = receipt();
    let args = arguments_from_receipt(&receipt).expect("replay arguments");
    let full_source = receipt.source_reports.first().expect("full source");
    let soak_source = receipt.source_reports.get(1).expect("soak source");
    let full_rollup = receipt.rollups.first().expect("full rollup");
    let soak_rollup = receipt.rollups.get(1).expect("soak rollup");
    assert_eq!(args.group, receipt.group_id);
    assert_eq!(args.full_inputs, [PathBuf::from(&full_source.path)]);
    assert_eq!(args.soak_inputs, [PathBuf::from(&soak_source.path)]);
    assert_eq!(args.full_rollup, PathBuf::from(&full_rollup.path));
    assert_eq!(args.soak_rollup, PathBuf::from(&soak_rollup.path));
    assert_eq!(args.out, PathBuf::from(&receipt.output));
}

#[test]
fn adapter_probe_must_match_the_reproducible_report_workers() {
    let workers = workers();
    let valid = probe(&workers);
    validate_probe(&valid, &workers, "PERFQ-TEST").expect("matching probe");

    let mut stale = valid;
    stale.stim_binary_sha256 = digest('a');
    assert!(validate_probe(&stale, &workers, "PERFQ-TEST").is_err());

    let valid = probe(&workers);
    let mut wrong_group = valid.clone();
    wrong_group.runtime_group_id = "PERFQ-OTHER".to_string();
    assert!(validate_probe(&wrong_group, &workers, "PERFQ-TEST").is_err());

    let mut zero_work = valid.clone();
    zero_work.work_count = 0;
    assert!(validate_probe(&zero_work, &workers, "PERFQ-TEST").is_err());

    let mut wrong_mode = valid.clone();
    wrong_mode.evidence_mode = "memory".to_string();
    assert!(validate_probe(&wrong_mode, &workers, "PERFQ-TEST").is_err());

    let mut stale_stab_source = valid;
    stale_stab_source.stab_source_sha256 = digest('b');
    assert!(validate_probe(&stale_stab_source, &workers, "PERFQ-TEST").is_err());
}

#[test]
fn completion_requires_clean_unchanged_repository_identity() {
    let clean = super::super::git::RepositoryState {
        commit: "f".repeat(40),
        local_modifications: false,
    };
    require_clean_repository(&clean).expect("clean repository");
    require_expected_repository(&clean, &clean.commit).expect("expected repository");
    require_same_clean_repository(&clean, &clean).expect("unchanged repository");

    let dirty = super::super::git::RepositoryState {
        local_modifications: true,
        ..clean.clone()
    };
    assert!(matches!(
        require_clean_repository(&dirty),
        Err(CompletionError::DirtyRepository)
    ));

    let changed = super::super::git::RepositoryState {
        commit: "e".repeat(40),
        local_modifications: false,
    };
    assert!(matches!(
        require_same_clean_repository(&clean, &changed),
        Err(CompletionError::RepositoryChanged { .. })
    ));
}

#[test]
fn completion_group_must_be_nonempty_promotable_and_thresholded() {
    let valid = group_contract();
    require_completion_group(&valid).expect("completion-eligible group");

    let mut diagnostic = group_contract();
    diagnostic.claim_class = ClaimClass::DiagnosticInfrastructure;
    assert!(require_completion_group(&diagnostic).is_err());

    let mut report_only = group_contract();
    report_only.baseline_eligibility = BaselineEligibility::ReportOnly;
    assert!(require_completion_group(&report_only).is_err());

    let mut no_scales = group_contract();
    no_scales.scales.clear();
    assert!(require_completion_group(&no_scales).is_err());

    let mut no_measurements = group_contract();
    no_measurements.measurement_ids.clear();
    assert!(require_completion_group(&no_measurements).is_err());
}

#[test]
fn completion_rollups_bind_sources_workers_outcome_and_environment() {
    let receipt = receipt();
    let contract = group_contract();
    let full = rollup_evidence(&receipt, QualificationTier::Full);
    let soak = rollup_evidence(&receipt, QualificationTier::Soak);
    validate_rollup(
        &full,
        &contract,
        QualificationTier::Full,
        &receipt.repository.commit_after,
        &receipt.source_reports,
        &receipt.workers,
    )
    .expect("valid full rollup");
    require_matching_rollup_identity(&full, &soak).expect("matching rollup identity");

    let mut failed = full.clone();
    failed.overall_outcome = GateOutcome::Failed;
    assert!(
        validate_rollup(
            &failed,
            &contract,
            QualificationTier::Full,
            &receipt.repository.commit_after,
            &receipt.source_reports,
            &receipt.workers,
        )
        .is_err()
    );

    let mut wrong_source = full.clone();
    wrong_source
        .sources
        .first_mut()
        .expect("rollup source")
        .report_sha256 = digest('a');
    assert!(
        validate_rollup(
            &wrong_source,
            &contract,
            QualificationTier::Full,
            &receipt.repository.commit_after,
            &receipt.source_reports,
            &receipt.workers,
        )
        .is_err()
    );

    let mut mixed_architecture = soak.clone();
    mixed_architecture.architecture = "x86_64".to_string();
    assert!(require_matching_rollup_identity(&full, &mixed_architecture).is_err());

    let mut mixed_cpu = soak.clone();
    mixed_cpu.cpu_identity = "different CPU".to_string();
    assert!(require_matching_rollup_identity(&full, &mixed_cpu).is_err());

    let mut mixed_correctness = soak;
    mixed_correctness.correctness_preflight.report_sha256 = Some(digest('a'));
    assert!(require_matching_rollup_identity(&full, &mixed_correctness).is_err());
}
