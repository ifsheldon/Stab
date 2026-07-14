use super::super::protocol::{GitCommit, PROTOCOL_SCHEMA_VERSION, Sha256Digest, WorkerMeasurement};
use super::super::run::{CalibrationProbeEvidence, ImplementationCalibration, MemoryEvidence};
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
        workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
        measurement_id: ProtocolId::try_new("main").expect("measurement id"),
        iteration_count: iterations,
        elapsed_seconds,
        work_count: iterations.checked_mul(WORK_ITEMS).expect("work count"),
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
fn invocation_receipt_binds_phase_and_worker_identity() {
    let source = repeated(b'a');
    let build = repeated(b'b');
    let identity = ReceiptIdentity {
        work_items: WORK_ITEMS,
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
