use super::super::protocol::{
    GitCommit, PROTOCOL_SCHEMA_VERSION, RAW_WORK_TIMING_BOUNDARY, Sha256Digest, WorkerMeasurement,
};
use super::super::run::{
    CalibrationEvidence, CalibrationProbeEvidence, CommonBatchMode, ImplementationCalibration,
    MemoryEvidence,
};
use super::super::statistics::StatisticsSummary;
use super::*;

const WORK_ITEMS: u64 = 4;
const CPU: u32 = 7;

fn repeated(byte: u8) -> String {
    std::iter::repeat_n(char::from(byte), 64).collect()
}

fn measurement(
    implementation: Implementation,
    evidence_mode: EvidenceMode,
    iterations: u64,
    elapsed_seconds: f64,
) -> WorkerMeasurement {
    WorkerMeasurement {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        implementation,
        evidence_mode,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
        measurement_id: ProtocolId::try_new("main").expect("measurement id"),
        iteration_count: iterations,
        elapsed_seconds,
        work_count: iterations.checked_mul(WORK_ITEMS).expect("work count"),
        input_bytes: 0,
        input_digest: InputDigest::try_new(
            "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
        )
        .expect("empty input digest"),
        output_digest: SemanticDigest::try_new(repeated(b'd')).expect("semantic digest"),
        setup_rss_bytes: Some(100),
        peak_rss_bytes: Some(120),
        affinity_cpu: Some(CPU),
        stim_commit: GitCommit::try_new(STIM_COMMIT).expect("Stim commit"),
        source_digest: Sha256Digest::try_new(repeated(b'a')).expect("source digest"),
        build_fingerprint: Sha256Digest::try_new(repeated(b'b')).expect("build fingerprint"),
    }
}

fn invocation(
    implementation: Implementation,
    evidence_mode: EvidenceMode,
    iterations: u64,
    elapsed_seconds: f64,
) -> super::super::invocation::InvocationRecord {
    super::super::invocation::InvocationRecord {
        implementation,
        evidence_mode,
        process_wall_seconds: elapsed_seconds + 0.01,
        parent_observed_peak_rss_bytes: Some(200),
        rows: vec![measurement(
            implementation,
            evidence_mode,
            iterations,
            elapsed_seconds,
        )],
    }
}

fn calibration(implementation: Implementation) -> ImplementationCalibration {
    ImplementationCalibration {
        implementation,
        selected_iterations: 350,
        selected_measured_seconds: 0.35,
        probes: vec![
            CalibrationProbeEvidence {
                iterations: 1,
                invocation: invocation(implementation, EvidenceMode::Timing, 1, 0.001),
            },
            CalibrationProbeEvidence {
                iterations: 350,
                invocation: invocation(implementation, EvidenceMode::Timing, 350, 0.35),
            },
        ],
    }
}

fn independent_calibration_evidence() -> CalibrationEvidence {
    let mut stab = calibration(Implementation::Stab);
    stab.probes
        .first_mut()
        .expect("initial Stab probe")
        .invocation = invocation(Implementation::Stab, EvidenceMode::Timing, 1, 0.000_01);
    stab.selected_iterations = 35_000;
    stab.selected_measured_seconds = 0.35;
    let selected = stab.probes.last_mut().expect("selected Stab probe");
    selected.iterations = 35_000;
    selected.invocation = invocation(Implementation::Stab, EvidenceMode::Timing, 35_000, 0.35);
    selected
        .invocation
        .rows
        .first_mut()
        .expect("selected Stab row")
        .output_digest = SemanticDigest::try_new(repeated(b'e')).expect("Stab selected digest");
    CalibrationEvidence {
        acceptance_minimum_seconds: 0.25,
        target_minimum_seconds: 0.35,
        maximum_seconds: 2.0,
        wide_ratio_maximum_seconds: 20.0,
        batch_policy: TimingBatchPolicy::IndependentThroughput,
        common_batch_mode: CommonBatchMode::IndependentThroughput,
        stim: calibration(Implementation::Stim),
        stab,
        common_iterations: 350,
        common_validation: PairExecution {
            pair_index: 0,
            order: PairOrder::StimThenStab,
            stim: invocation(Implementation::Stim, EvidenceMode::Timing, 350, 0.35),
            stab: invocation(Implementation::Stab, EvidenceMode::Timing, 350, 0.005),
        },
    }
}

fn independent_pair(pair_index: usize) -> PairExecution {
    let mut stab = invocation(Implementation::Stab, EvidenceMode::Timing, 35_000, 0.35);
    stab.rows
        .first_mut()
        .expect("Stab timing row")
        .output_digest = SemanticDigest::try_new(repeated(b'e')).expect("Stab selected digest");
    PairExecution {
        pair_index,
        order: PairOrder::for_pair(pair_index),
        stim: invocation(Implementation::Stim, EvidenceMode::Timing, 350, 0.35),
        stab,
    }
}

fn independent_timing_attempt() -> TimingAttempt {
    let warmups = (0..3).map(independent_pair).collect::<Vec<_>>();
    let samples = (0..9).map(independent_pair).collect::<Vec<_>>();
    let paired_samples = samples
        .iter()
        .flat_map(|sample| {
            validate_pair_execution(
                sample,
                EvidenceMode::Timing,
                TimingBatchPolicy::IndependentThroughput,
            )
            .expect("valid independent sample")
        })
        .collect::<Vec<_>>();
    let measurement_id = ProtocolId::try_new("main").expect("measurement id");
    let summary = super::super::statistics::summarize(measurement_id, &paired_samples, 1.25)
        .expect("independent statistics");
    TimingAttempt {
        attempt_index: 0,
        kind: TimingAttemptKind::Initial,
        warmups,
        samples,
        paired_samples,
        worst_confidence_interval_upper: summary.confidence_interval_upper,
        statistics: vec![summary],
    }
}

fn timing_attempt(
    attempt_index: usize,
    kind: TimingAttemptKind,
    outcome: GateOutcome,
) -> TimingAttempt {
    TimingAttempt {
        attempt_index,
        kind,
        warmups: Vec::new(),
        samples: Vec::new(),
        paired_samples: Vec::new(),
        statistics: vec![StatisticsSummary {
            measurement_id: ProtocolId::try_new("main").expect("measurement id"),
            pair_count: 9,
            median_ratio: 1.0,
            confidence_interval_lower: 1.0,
            confidence_interval_upper: 1.0,
            stim_relative_mad: 0.0,
            stab_relative_mad: 0.0,
            ratio_relative_mad: if outcome == GateOutcome::Noisy {
                0.11
            } else {
                0.0
            },
            threshold: 1.25,
            outcome,
        }],
        worst_confidence_interval_upper: 1.0,
    }
}

#[test]
fn noisy_attempt_gets_exactly_one_complete_rerun_slot() {
    let passed = timing_attempt(0, TimingAttemptKind::Initial, GateOutcome::Passed);
    validate_timing_attempt_policy(std::slice::from_ref(&passed))
        .expect("non-noisy initial attempt is final");
    let untriggered = vec![
        passed,
        timing_attempt(
            1,
            TimingAttemptKind::PairedRatioNoiseRerun,
            GateOutcome::Passed,
        ),
    ];
    assert!(matches!(
        validate_timing_attempt_policy(&untriggered),
        Err(ReportError::TimingAttemptCount(2))
    ));

    let noisy = timing_attempt(0, TimingAttemptKind::Initial, GateOutcome::Noisy);
    assert!(matches!(
        validate_timing_attempt_policy(std::slice::from_ref(&noisy)),
        Err(ReportError::TimingAttemptCount(1))
    ));
    let retained = vec![
        noisy,
        timing_attempt(
            1,
            TimingAttemptKind::PairedRatioNoiseRerun,
            GateOutcome::Failed,
        ),
    ];
    validate_timing_attempt_policy(&retained)
        .expect("one complete rerun is retained regardless of its result");

    let mut wrong_reason = retained;
    wrong_reason.last_mut().expect("rerun").kind = TimingAttemptKind::Initial;
    assert!(matches!(
        validate_timing_attempt_policy(&wrong_reason),
        Err(ReportError::TimingAttemptIdentity)
    ));
}

#[test]
fn failed_or_noisy_product_evidence_requires_a_profiler_note() {
    let passed = vec![timing_attempt(
        0,
        TimingAttemptKind::Initial,
        GateOutcome::Passed,
    )];
    let failed = vec![timing_attempt(
        0,
        TimingAttemptKind::Initial,
        GateOutcome::Failed,
    )];
    assert!(require_failure_evidence(ClaimClass::PromotablePerformance, &passed, false).is_ok());
    assert!(matches!(
        require_failure_evidence(ClaimClass::PromotablePerformance, &failed, false),
        Err(ReportError::FailureEvidence)
    ));
    assert!(require_failure_evidence(ClaimClass::PromotablePerformance, &failed, true).is_ok());
    assert!(require_failure_evidence(ClaimClass::DiagnosticInfrastructure, &failed, false).is_ok());
}

#[test]
fn calibration_evidence_must_replay_the_controller_decision() {
    let valid = calibration(Implementation::Stim);
    replay_calibration(&valid).expect("valid calibration replay");

    let mut wrapper_iterations = valid.clone();
    wrapper_iterations
        .probes
        .first_mut()
        .expect("first probe")
        .iterations = 2;

    let mut row_iterations = valid.clone();
    row_iterations
        .probes
        .first_mut()
        .and_then(|probe| probe.invocation.rows.first_mut())
        .expect("first row")
        .iteration_count = 2;

    let mut selected_iterations = valid.clone();
    selected_iterations.selected_iterations = 349;

    let mut extra_probe = valid.clone();
    extra_probe
        .probes
        .push(extra_probe.probes.last().expect("last probe").clone());

    let mut early_acceptance = valid.clone();
    early_acceptance
        .probes
        .first_mut()
        .and_then(|probe| probe.invocation.rows.first_mut())
        .expect("first row")
        .elapsed_seconds = 0.4;

    for mutation in [
        wrapper_iterations,
        row_iterations,
        selected_iterations,
        extra_probe,
        early_acceptance,
    ] {
        assert!(matches!(
            replay_calibration(&mutation),
            Err(ReportError::Calibration)
        ));
    }
}

#[test]
fn report_rederives_the_common_batch_mode() {
    let common_validation = PairExecution {
        pair_index: 0,
        order: PairOrder::StimThenStab,
        stim: invocation(Implementation::Stim, EvidenceMode::Timing, 350, 0.35),
        stab: invocation(Implementation::Stab, EvidenceMode::Timing, 350, 0.35),
    };
    let mut evidence = CalibrationEvidence {
        acceptance_minimum_seconds: 0.25,
        target_minimum_seconds: 0.35,
        maximum_seconds: 2.0,
        wide_ratio_maximum_seconds: 20.0,
        batch_policy: TimingBatchPolicy::CommonIterations,
        common_batch_mode: CommonBatchMode::Standard,
        stim: calibration(Implementation::Stim),
        stab: calibration(Implementation::Stab),
        common_iterations: 350,
        common_validation,
    };
    validate_common_batch_mode(&evidence).expect("standard mode is reconstructed");

    evidence.common_batch_mode = CommonBatchMode::WideRatio;
    assert!(matches!(
        validate_common_batch_mode(&evidence),
        Err(ReportError::Calibration)
    ));

    evidence.common_batch_mode = CommonBatchMode::Standard;
    evidence
        .common_validation
        .stim
        .rows
        .first_mut()
        .expect("Stim row")
        .elapsed_seconds = 3.0;
    assert!(matches!(
        validate_common_batch_mode(&evidence),
        Err(ReportError::Calibration)
    ));

    evidence.common_batch_mode = CommonBatchMode::WideRatio;
    evidence.stim.selected_iterations = 100;
    evidence
        .common_validation
        .stim
        .rows
        .first_mut()
        .expect("Stim row")
        .elapsed_seconds = 5.0;
    validate_common_batch_mode(&evidence).expect("wide-ratio mode is reconstructed");
}

#[test]
fn independent_throughput_mode_and_selected_receipts_are_rederived() {
    let mut evidence = independent_calibration_evidence();
    validate_common_batch_mode(&evidence).expect("independent mode is reconstructed");

    let workload_id = ProtocolId::try_new("protocol-smoke").expect("workload id");
    let measurement_id = ProtocolId::try_new("main").expect("measurement id");
    let common_output = SemanticDigest::try_new(repeated(b'd')).expect("semantic digest");
    let common_phase = PhaseExpectation {
        evidence_mode: EvidenceMode::Timing,
        iterations: evidence.common_iterations,
        workload_id: &workload_id,
        measurement_id: &measurement_id,
        output_digest: Some(&common_output),
    };
    let selected = timing_phase(
        TimingBatchPolicy::IndependentThroughput,
        &common_phase,
        &evidence.stab,
    )
    .expect("selected Stab phase");
    assert_eq!(selected.iterations, 35_000);
    assert_eq!(
        selected.output_digest,
        Some(
            &evidence
                .stab
                .probes
                .last()
                .expect("selected probe")
                .invocation
                .rows
                .first()
                .expect("selected row")
                .output_digest
        )
    );

    let common_owner_digest = &mut evidence
        .stim
        .probes
        .last_mut()
        .expect("selected Stim probe")
        .invocation
        .rows
        .first_mut()
        .expect("selected Stim row")
        .output_digest;
    *common_owner_digest = SemanticDigest::try_new(repeated(b'e')).expect("different digest");
    assert!(matches!(
        timing_phase(
            TimingBatchPolicy::IndependentThroughput,
            &common_phase,
            &evidence.stim,
        ),
        Err(ReportError::Calibration)
    ));
    evidence
        .stim
        .probes
        .last_mut()
        .expect("selected Stim probe")
        .invocation
        .rows
        .first_mut()
        .expect("selected Stim row")
        .output_digest = common_output.clone();

    let mut stab_common_owner = calibration(Implementation::Stab);
    stab_common_owner
        .probes
        .last_mut()
        .expect("selected Stab probe")
        .invocation
        .rows
        .first_mut()
        .expect("selected Stab row")
        .output_digest = SemanticDigest::try_new(repeated(b'e')).expect("different digest");
    assert!(matches!(
        timing_phase(
            TimingBatchPolicy::IndependentThroughput,
            &common_phase,
            &stab_common_owner,
        ),
        Err(ReportError::Calibration)
    ));

    let mut equal_count_stab = calibration(Implementation::Stab);
    equal_count_stab
        .probes
        .last_mut()
        .expect("selected Stab probe")
        .invocation
        .rows
        .first_mut()
        .expect("selected Stab row")
        .output_digest = SemanticDigest::try_new(repeated(b'e')).expect("different digest");
    assert!(matches!(
        timing_phase(
            TimingBatchPolicy::IndependentThroughput,
            &common_phase,
            &equal_count_stab,
        ),
        Err(ReportError::Calibration)
    ));

    evidence.common_batch_mode = CommonBatchMode::Standard;
    assert!(matches!(
        validate_common_batch_mode(&evidence),
        Err(ReportError::Calibration)
    ));
    evidence.common_batch_mode = CommonBatchMode::IndependentThroughput;
    evidence
        .common_validation
        .stab
        .rows
        .first_mut()
        .expect("Stab row")
        .elapsed_seconds = 2.001;
    assert!(matches!(
        validate_common_batch_mode(&evidence),
        Err(ReportError::Calibration)
    ));

    evidence.stab.selected_iterations = 34_999;
    assert!(matches!(
        timing_phase(
            TimingBatchPolicy::IndependentThroughput,
            &common_phase,
            &evidence.stab,
        ),
        Err(ReportError::Calibration)
    ));
}

#[test]
fn common_timing_phase_replays_the_live_selected_output_guard() {
    let workload_id = ProtocolId::try_new("protocol-smoke").expect("workload id");
    let measurement_id = ProtocolId::try_new("main").expect("measurement id");
    let common_output = SemanticDigest::try_new(repeated(b'd')).expect("common output");
    let common_phase = PhaseExpectation {
        evidence_mode: EvidenceMode::Timing,
        iterations: 350,
        workload_id: &workload_id,
        measurement_id: &measurement_id,
        output_digest: Some(&common_output),
    };
    for implementation in [Implementation::Stim, Implementation::Stab] {
        let valid = calibration(implementation);
        assert!(timing_phase(TimingBatchPolicy::CommonIterations, &common_phase, &valid).is_ok());

        let mut changed_owner = valid;
        changed_owner
            .probes
            .last_mut()
            .expect("selected probe")
            .invocation
            .rows
            .first_mut()
            .expect("selected row")
            .output_digest = SemanticDigest::try_new(repeated(b'e')).expect("changed digest");
        assert!(matches!(
            timing_phase(
                TimingBatchPolicy::CommonIterations,
                &common_phase,
                &changed_owner,
            ),
            Err(ReportError::Calibration)
        ));
    }

    let mut smaller_non_owner = calibration(Implementation::Stab);
    smaller_non_owner.selected_iterations = 35;
    let selected = smaller_non_owner.probes.last_mut().expect("selected probe");
    selected.iterations = 35;
    let selected_row = selected.invocation.rows.first_mut().expect("selected row");
    selected_row.iteration_count = 35;
    selected_row.work_count = 35 * WORK_ITEMS;
    selected_row.output_digest =
        SemanticDigest::try_new(repeated(b'e')).expect("count-specific digest");
    assert!(
        timing_phase(
            TimingBatchPolicy::CommonIterations,
            &common_phase,
            &smaller_non_owner,
        )
        .is_ok()
    );
}

#[test]
fn complete_independent_timing_contract_rejects_adversarial_mutations() {
    let calibration = independent_calibration_evidence();
    let attempt = independent_timing_attempt();
    validate_calibration(&calibration).expect("valid independent calibration");
    timing::validate_evidence(
        QualificationTier::Full,
        calibration.batch_policy,
        std::slice::from_ref(&attempt),
    )
    .expect("valid independent timing evidence");

    let workload_id = ProtocolId::try_new("protocol-smoke").expect("workload id");
    let measurement_id = ProtocolId::try_new("main").expect("measurement id");
    let common_output = SemanticDigest::try_new(repeated(b'd')).expect("common output");
    let input_digest =
        InputDigest::try_new("6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1")
            .expect("empty input digest");
    let source = repeated(b'a');
    let build = repeated(b'b');
    let identity = ReceiptIdentity {
        work_items: WORK_ITEMS,
        input_bytes: 0,
        input_digest: &input_digest,
        invocation_timeout_seconds: 600.0,
        expected_cpu: CPU,
        stim_commit: STIM_COMMIT,
        stim_source: &source,
        stim_build: &build,
        stab_source: &source,
        stab_build: &build,
    };
    let common_phase = PhaseExpectation {
        evidence_mode: EvidenceMode::Timing,
        iterations: calibration.common_iterations,
        workload_id: &workload_id,
        measurement_id: &measurement_id,
        output_digest: Some(&common_output),
    };
    let stim_phase = timing_phase(calibration.batch_policy, &common_phase, &calibration.stim)
        .expect("Stim timing phase");
    let stab_phase = timing_phase(calibration.batch_policy, &common_phase, &calibration.stab)
        .expect("Stab timing phase");
    validate_pair_receipts(
        &identity,
        &calibration.common_validation,
        &common_phase,
        &common_phase,
    )
    .expect("common semantic receipts");
    for pair in attempt.warmups.iter().chain(&attempt.samples) {
        validate_pair_receipts(&identity, pair, &stim_phase, &stab_phase)
            .expect("selected timing receipts");
    }

    let mut changed_policy = calibration.clone();
    changed_policy.batch_policy = TimingBatchPolicy::CommonIterations;
    assert!(validate_calibration(&changed_policy).is_err());
    assert!(
        timing::validate_evidence(
            QualificationTier::Full,
            TimingBatchPolicy::CommonIterations,
            std::slice::from_ref(&attempt),
        )
        .is_err()
    );

    let mut changed_count = attempt.clone();
    changed_count
        .samples
        .first_mut()
        .expect("timing sample")
        .stab
        .rows
        .first_mut()
        .expect("Stab row")
        .iteration_count -= 1;
    assert!(
        timing::validate_evidence(
            QualificationTier::Full,
            calibration.batch_policy,
            &[changed_count],
        )
        .is_err()
    );

    let mut changed_work = attempt.clone();
    changed_work
        .samples
        .first_mut()
        .expect("timing sample")
        .stab
        .rows
        .first_mut()
        .expect("Stab row")
        .work_count += 1;
    assert!(
        timing::validate_evidence(
            QualificationTier::Full,
            calibration.batch_policy,
            &[changed_work],
        )
        .is_err()
    );

    let mut changed_output = attempt.samples.first().expect("timing sample").clone();
    changed_output
        .stab
        .rows
        .first_mut()
        .expect("Stab row")
        .output_digest = SemanticDigest::try_new(repeated(b'f')).expect("changed output");
    assert!(validate_pair_receipts(&identity, &changed_output, &stim_phase, &stab_phase,).is_err());

    let mut changed_common = calibration.common_validation.clone();
    let changed_common_output =
        SemanticDigest::try_new(repeated(b'f')).expect("changed common output");
    changed_common
        .stim
        .rows
        .first_mut()
        .expect("Stim row")
        .output_digest = changed_common_output.clone();
    changed_common
        .stab
        .rows
        .first_mut()
        .expect("Stab row")
        .output_digest = changed_common_output;
    assert!(
        validate_pair_receipts(&identity, &changed_common, &common_phase, &common_phase,).is_err()
    );

    let mut changed_selected = calibration.stab.clone();
    changed_selected.selected_iterations -= 1;
    assert!(timing_phase(calibration.batch_policy, &common_phase, &changed_selected,).is_err());

    let mut changed_ratio = attempt;
    changed_ratio
        .paired_samples
        .first_mut()
        .expect("paired sample")
        .ratio *= 2.0;
    assert!(
        timing::validate_evidence(
            QualificationTier::Full,
            calibration.batch_policy,
            &[changed_ratio],
        )
        .is_err()
    );
}

#[test]
fn retained_independent_samples_may_jitter_outside_calibration_range() {
    let mut execution = PairExecution {
        pair_index: 0,
        order: PairOrder::StimThenStab,
        stim: invocation(Implementation::Stim, EvidenceMode::Timing, 350, 0.249),
        stab: invocation(Implementation::Stab, EvidenceMode::Timing, 35_000, 2.001),
    };
    validate_pair_execution(
        &execution,
        EvidenceMode::Timing,
        TimingBatchPolicy::IndependentThroughput,
    )
    .expect(
        "retained samples use receipt, noise, and timeout checks instead of calibration bounds",
    );

    execution
        .stim
        .rows
        .first_mut()
        .expect("Stim row")
        .elapsed_seconds = 0.0;
    assert!(
        validate_pair_execution(
            &execution,
            EvidenceMode::Timing,
            TimingBatchPolicy::IndependentThroughput,
        )
        .is_err()
    );
}

#[test]
fn invocation_receipt_binds_phase_and_worker_identity() {
    let source = repeated(b'a');
    let build = repeated(b'b');
    let identity = ReceiptIdentity {
        work_items: WORK_ITEMS,
        input_bytes: 0,
        input_digest: &InputDigest::try_new(
            "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
        )
        .expect("empty input digest"),
        invocation_timeout_seconds: 30.0,
        expected_cpu: CPU,
        stim_commit: STIM_COMMIT,
        stim_source: &source,
        stim_build: &build,
        stab_source: &source,
        stab_build: &build,
    };
    let workload_id = ProtocolId::try_new("protocol-smoke").expect("workload id");
    let measurement_id = ProtocolId::try_new("main").expect("measurement id");
    let output_digest = SemanticDigest::try_new(repeated(b'd')).expect("output digest");
    let phase = PhaseExpectation {
        evidence_mode: EvidenceMode::Timing,
        iterations: 2,
        workload_id: &workload_id,
        measurement_id: &measurement_id,
        output_digest: Some(&output_digest),
    };
    let valid = invocation(Implementation::Stim, EvidenceMode::Timing, 2, 0.02);
    validate_invocation_receipt(&identity, &valid, Implementation::Stim, &phase)
        .expect("valid invocation receipt");

    let mut wrong_workload = valid.clone();
    wrong_workload.rows.first_mut().expect("row").workload_id =
        ProtocolId::try_new("other").expect("other workload");
    let mut wrong_measurement = valid.clone();
    wrong_measurement
        .rows
        .first_mut()
        .expect("row")
        .measurement_id = ProtocolId::try_new("other").expect("other measurement");
    let mut wrong_iterations = valid.clone();
    wrong_iterations
        .rows
        .first_mut()
        .expect("row")
        .iteration_count = 3;
    let mut wrong_work = valid.clone();
    wrong_work.rows.first_mut().expect("row").work_count = 9;
    let mut wrong_input_bytes = valid.clone();
    wrong_input_bytes.rows.first_mut().expect("row").input_bytes = 1;
    let mut wrong_input = valid.clone();
    wrong_input.rows.first_mut().expect("row").input_digest =
        InputDigest::try_new(repeated(b'e')).expect("other input digest");
    let mut wrong_cpu = valid.clone();
    wrong_cpu.rows.first_mut().expect("row").affinity_cpu = Some(CPU + 1);
    let mut wrong_digest = valid.clone();
    wrong_digest.rows.first_mut().expect("row").output_digest =
        SemanticDigest::try_new(repeated(b'e')).expect("other digest");
    let mut wrong_source = valid.clone();
    wrong_source.rows.first_mut().expect("row").source_digest =
        Sha256Digest::try_new(repeated(b'c')).expect("other source");
    let mut impossible_wall = valid;
    impossible_wall.process_wall_seconds = 0.001;

    for mutation in [
        wrong_workload,
        wrong_measurement,
        wrong_iterations,
        wrong_work,
        wrong_input_bytes,
        wrong_input,
        wrong_cpu,
        wrong_digest,
        wrong_source,
        impossible_wall,
    ] {
        assert!(matches!(
            validate_invocation_receipt(&identity, &mutation, Implementation::Stim, &phase),
            Err(ReportError::WorkerReceipt)
        ));
    }
}

#[test]
fn memory_summary_repeats_parent_rss_exactly() {
    let stim = invocation(Implementation::Stim, EvidenceMode::Memory, 2, 0.02);
    let stab = invocation(Implementation::Stab, EvidenceMode::Memory, 2, 0.02);
    let execution = PairExecution {
        pair_index: 0,
        order: PairOrder::StimThenStab,
        stim,
        stab,
    };
    let mut memory = MemoryEvidence {
        evidence_mode: EvidenceMode::Memory,
        iterations: 2,
        work_count: 8,
        stim_setup_rss_bytes: 100,
        stim_peak_rss_bytes: 120,
        stim_parent_observed_peak_rss_bytes: Some(200),
        stab_setup_rss_bytes: 100,
        stab_peak_rss_bytes: 120,
        stab_parent_observed_peak_rss_bytes: Some(200),
        execution,
    };
    let stim_row = memory.execution.stim.rows.first().expect("Stim row");
    let stab_row = memory.execution.stab.rows.first().expect("Stab row");
    assert!(memory_receipts_match(&memory, stim_row, stab_row));

    memory.stim_parent_observed_peak_rss_bytes = Some(201);
    let stim_row = memory.execution.stim.rows.first().expect("Stim row");
    let stab_row = memory.execution.stab.rows.first().expect("Stab row");
    assert!(!memory_receipts_match(&memory, stim_row, stab_row));
}
