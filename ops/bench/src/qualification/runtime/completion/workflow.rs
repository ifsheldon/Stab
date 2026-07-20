use super::{
    AdapterProbeReceipt, ArtifactReceipt, CompletionError, CompletionStep, CompletionStepKind,
    CompletionStepResult, DirectQualificationArtifactPath, EvidenceDirectoryReceipt,
    QualificationTier, ReplayContext, SelectedReport, WorkerIdentityEvidence, artifact_digest,
    checked_action, path_text, probe_arguments, push_step, read_artifact_receipts,
    require_matching_rollup_identity, shared_workers, validate_probe, validate_rollup,
};

pub(super) struct WorkflowEvidence {
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) source_reports: Vec<EvidenceDirectoryReceipt>,
    pub(super) rollups: Vec<EvidenceDirectoryReceipt>,
    pub(super) full_rollup: super::super::rollup::RollupReplayEvidence,
    pub(super) steps: Vec<CompletionStep>,
}

pub(super) trait Actions {
    fn workers(
        &mut self,
        context: &ReplayContext<'_>,
    ) -> Result<WorkerIdentityEvidence, CompletionError>;

    fn probe(
        &mut self,
        context: &ReplayContext<'_>,
        group_id: &str,
    ) -> Result<AdapterProbeReceipt, CompletionError>;

    fn report(
        &mut self,
        context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<Vec<ArtifactReceipt>, CompletionError>;

    fn regression(
        &mut self,
        context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<super::super::regression::RegressionSummary, CompletionError>;

    fn rollup(
        &mut self,
        context: &ReplayContext<'_>,
        path: &DirectQualificationArtifactPath,
    ) -> Result<
        (
            Vec<ArtifactReceipt>,
            Vec<ArtifactReceipt>,
            super::super::rollup::RollupReplayEvidence,
        ),
        CompletionError,
    >;
}

pub(super) struct ProductionActions;

impl Actions for ProductionActions {
    fn workers(
        &mut self,
        context: &ReplayContext<'_>,
    ) -> Result<WorkerIdentityEvidence, CompletionError> {
        checked_action(
            context.root,
            context.repository,
            context.repository_commit,
            "worker reproducibility",
            super::super::invocation::verify_private_worker_reproducibility,
        )
    }

    fn probe(
        &mut self,
        context: &ReplayContext<'_>,
        group_id: &str,
    ) -> Result<AdapterProbeReceipt, CompletionError> {
        checked_action(
            context.root,
            context.repository,
            context.repository_commit,
            "adapter probe",
            |source_root| {
                super::super::probe::run_source_owned_adapter_probe(source_root, group_id)
            },
        )
    }

    fn report(
        &mut self,
        context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<Vec<ArtifactReceipt>, CompletionError> {
        checked_action(
            context.root,
            context.repository,
            context.repository_commit,
            "report replay",
            |source_root| {
                super::super::report::run_with_repository(
                    context.root,
                    source_root,
                    context.repository,
                    &selected.path,
                    context.performance_inventory_sha256,
                    context.correctness_inventory_sha256,
                )
            },
        )?;
        read_artifact_receipts(context.root, context.repository, &selected.path)
    }

    fn regression(
        &mut self,
        context: &ReplayContext<'_>,
        selected: &SelectedReport,
    ) -> Result<super::super::regression::RegressionSummary, CompletionError> {
        checked_action(
            context.root,
            context.repository,
            context.repository_commit,
            "regression replay",
            |source_root| {
                super::super::regression::run_with_repository(
                    context.root,
                    source_root,
                    context.repository,
                    context.performance_inventory_sha256,
                    context.correctness_inventory_sha256,
                    &selected.path,
                )
            },
        )
    }

    fn rollup(
        &mut self,
        context: &ReplayContext<'_>,
        path: &DirectQualificationArtifactPath,
    ) -> Result<
        (
            Vec<ArtifactReceipt>,
            Vec<ArtifactReceipt>,
            super::super::rollup::RollupReplayEvidence,
        ),
        CompletionError,
    > {
        let before = read_artifact_receipts(context.root, context.repository, path)?;
        let evidence = checked_action(
            context.root,
            context.repository,
            context.repository_commit,
            "rollup replay",
            |source_root| {
                super::super::rollup::replay_with_repository(
                    context.root,
                    source_root,
                    context.repository,
                    context.performance_inventory_sha256,
                    context.correctness_inventory_sha256,
                    path.clone(),
                )
            },
        )?;
        let after = read_artifact_receipts(context.root, context.repository, path)?;
        Ok((before, after, evidence))
    }
}

pub(super) fn run(
    context: &ReplayContext<'_>,
    group_id: &str,
    full: Vec<SelectedReport>,
    soak: Vec<SelectedReport>,
    full_rollup: DirectQualificationArtifactPath,
    soak_rollup: DirectQualificationArtifactPath,
    actions: &mut impl Actions,
) -> Result<WorkflowEvidence, CompletionError> {
    let expected_workers = shared_workers(&full, &soak)?;
    let workers = actions.workers(context)?;
    if workers != expected_workers {
        return Err(CompletionError::WorkerIdentity);
    }
    let mut steps = Vec::new();
    push_step(
        &mut steps,
        CompletionStepKind::WorkerReproducibility,
        context.repository_commit,
        vec!["qualification-worker-reproducibility".to_string()],
        Vec::new(),
        Vec::new(),
        CompletionStepResult::WorkerReproducibility {
            workers: workers.clone(),
        },
    );

    let probe = actions.probe(context, group_id)?;
    validate_probe(&probe, &workers, group_id)?;
    push_step(
        &mut steps,
        CompletionStepKind::AdapterProbe,
        context.repository_commit,
        probe_arguments(&probe),
        Vec::new(),
        Vec::new(),
        CompletionStepResult::AdapterProbe { probe },
    );

    let mut source_reports = Vec::with_capacity(full.len() + soak.len());
    for selected in full.into_iter().chain(soak) {
        replay_report(context, selected, actions, &mut steps, &mut source_reports)?;
    }
    let (full_receipt, full_evidence) = replay_rollup(
        context,
        QualificationTier::Full,
        &full_rollup,
        &source_reports,
        &workers,
        actions,
        &mut steps,
    )?;
    let (soak_receipt, soak_evidence) = replay_rollup(
        context,
        QualificationTier::Soak,
        &soak_rollup,
        &source_reports,
        &workers,
        actions,
        &mut steps,
    )?;
    require_matching_rollup_identity(&full_evidence, &soak_evidence)?;
    Ok(WorkflowEvidence {
        workers,
        source_reports,
        rollups: vec![full_receipt, soak_receipt],
        full_rollup: full_evidence,
        steps,
    })
}

fn replay_report(
    context: &ReplayContext<'_>,
    selected: SelectedReport,
    actions: &mut impl Actions,
    steps: &mut Vec<CompletionStep>,
    receipts: &mut Vec<EvidenceDirectoryReceipt>,
) -> Result<(), CompletionError> {
    let after = actions.report(context, &selected)?;
    let path = selected.path.clone().into_path_buf();
    if after != selected.artifacts {
        return Err(CompletionError::NonIdempotentReplay(path));
    }
    push_step(
        steps,
        CompletionStepKind::ReportReplay,
        context.repository_commit,
        vec![
            "qualification-report".to_string(),
            "--input".to_string(),
            path_text(selected.path.as_path())?,
        ],
        selected.artifacts.clone(),
        after.clone(),
        CompletionStepResult::ReportReplay {
            tier: selected.tier,
            scale_id: selected.scale_id.clone(),
        },
    );
    let summary = actions.regression(context, &selected)?;
    if summary.group_id != context.contract.id.to_string()
        || summary.report_only
        || summary.checked_measurements != context.contract.measurement_ids.len()
    {
        return Err(CompletionError::RegressionDisposition);
    }
    push_step(
        steps,
        CompletionStepKind::Regression,
        context.repository_commit,
        vec![
            "qualification-regression".to_string(),
            "--input".to_string(),
            path_text(selected.path.as_path())?,
            "--baseline".to_string(),
            super::super::regression::DEFAULT_BASELINE.to_string(),
        ],
        after.clone(),
        Vec::new(),
        CompletionStepResult::Regression {
            group_id: summary.group_id,
            checked_measurements: summary.checked_measurements,
            report_only: summary.report_only,
        },
    );
    receipts.push(EvidenceDirectoryReceipt {
        tier: selected.tier,
        scale_id: Some(selected.scale_id),
        path: path_text(selected.path.as_path())?,
        artifacts: after,
    });
    Ok(())
}

fn replay_rollup(
    context: &ReplayContext<'_>,
    tier: QualificationTier,
    path: &DirectQualificationArtifactPath,
    source_reports: &[EvidenceDirectoryReceipt],
    workers: &WorkerIdentityEvidence,
    actions: &mut impl Actions,
    steps: &mut Vec<CompletionStep>,
) -> Result<
    (
        EvidenceDirectoryReceipt,
        super::super::rollup::RollupReplayEvidence,
    ),
    CompletionError,
> {
    let (before, after, evidence) = actions.rollup(context, path)?;
    if before != after {
        return Err(CompletionError::NonIdempotentReplay(
            path.clone().into_path_buf(),
        ));
    }
    if evidence.output != path.as_path()
        || artifact_digest(&after, "report.json")? != evidence.report_sha256
        || artifact_digest(&after, "preflight.json")? != evidence.preflight_sha256
        || artifact_digest(&after, "report.md")? != evidence.markdown_sha256
    {
        return Err(CompletionError::ArtifactBinding(path_text(path.as_path())?));
    }
    validate_rollup(
        &evidence,
        context.contract,
        tier,
        context.repository_commit,
        source_reports,
        workers,
    )?;
    push_step(
        steps,
        CompletionStepKind::RollupReplay,
        context.repository_commit,
        vec![
            "qualification-rollup-report".to_string(),
            "--input".to_string(),
            path_text(path.as_path())?,
        ],
        before,
        after.clone(),
        CompletionStepResult::RollupReplay {
            tier,
            scale_count: evidence.sources.len(),
            overall_outcome: evidence.overall_outcome,
        },
    );
    Ok((
        EvidenceDirectoryReceipt {
            tier,
            scale_id: None,
            path: path_text(path.as_path())?,
            artifacts: after,
        },
        evidence,
    ))
}
