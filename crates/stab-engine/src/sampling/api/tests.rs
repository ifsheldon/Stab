use super::*;
use stab_records::{MeasurementCodecSink, RecordFormat};

#[derive(Default)]
struct RecordSink {
    records: Vec<Vec<bool>>,
}

impl MeasurementSink for RecordSink {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            let record = (0..batch.width().get())
                .map(|bit_index| {
                    batch
                        .get(shot_index, bit_index)
                        .expect("validated batch dimensions")
                })
                .collect();
            self.records.push(record);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Default)]
struct DigestSink {
    digest: u64,
}

impl MeasurementSink for DigestSink {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            for bit_index in 0..batch.width().get() {
                self.digest = self.digest.rotate_left(1)
                    ^ u64::from(
                        batch
                            .get(shot_index, bit_index)
                            .expect("validated batch dimensions"),
                    );
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn executable_contract_identity_changes_plan_fingerprint() {
    let circuit = Circuit::from_stim_str("M 0\n").expect("parse circuit");
    let request = CompilationRequestFingerprint::for_sampling(&circuit);
    let first = PlanFingerprint::for_sampling(
        request,
        SamplingBackend::Scalar,
        1,
        ReferenceSampleLoopPolicy::Fold,
        1,
    );
    let variant = PlanFingerprint::for_sampling(
        request,
        SamplingBackend::Scalar,
        2,
        ReferenceSampleLoopPolicy::Fold,
        1,
    );
    let policy = PlanFingerprint::for_sampling(
        request,
        SamplingBackend::Scalar,
        1,
        ReferenceSampleLoopPolicy::Iterate,
        1,
    );
    let schema = PlanFingerprint::for_sampling(
        request,
        SamplingBackend::Scalar,
        1,
        ReferenceSampleLoopPolicy::Fold,
        2,
    );

    assert_ne!(first.digest(), variant.digest());
    assert_ne!(first.digest(), policy.digest());
    assert_ne!(first.digest(), schema.digest());
    assert_ne!(
        first.executable_contract_digest(),
        variant.executable_contract_digest()
    );
}

#[test]
fn counter_overflow_rejects_before_work_without_poisoning() {
    let circuit = Circuit::from_stim_str("M 0\n").expect("parse circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile plan");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(1)))
        .expect("construct session");
    session.total_committed_shots = u64::MAX;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::ZeroOne, MeasurementWidth::new(1))
        .expect("construct sink");

    assert!(matches!(
        session.run(ShotCount::new(1), &mut sink),
        Err(RunError::Engine {
            source: SamplingExecutionError::ShotCounterOverflow,
            ..
        })
    ));
    assert!(!session.is_poisoned());
}

#[test]
fn internal_batch_invariant_failure_poisons_with_exact_progress() {
    let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse circuit");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile plan");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(2)))
        .expect("construct session");
    // Frame state sized for a two-measurement program cannot record this plan's one measurement.
    let other = crate::detection::frame::CircuitProgram::compile(
        &Circuit::from_stim_str("M 0 1\n").expect("parse other circuit"),
        crate::detection::frame::ProgramOptions::SAMPLING,
    )
    .expect("compile other program");
    session.frame = SessionFrame::Pauli(
        crate::detection::frame::LaneFrameBatch::for_measurements(&other, LaneWidth::default())
            .expect("other frame state"),
    );
    let mut sink = RecordSink::default();

    assert!(matches!(
        session.run(ShotCount::new(1), &mut sink),
        Err(RunError::Engine {
            source: SamplingExecutionError::InternalInvariant { .. },
            progress,
        }) if progress == SamplingRunProgress::new(0, 1)
    ));
    assert!(session.is_poisoned());
    assert!(matches!(
        session.run(ShotCount::new(1), &mut sink),
        Err(RunError::Engine {
            source: SamplingExecutionError::SessionPoisoned,
            progress,
        }) if progress == SamplingRunProgress::new(0, 0)
    ));
}

#[test]
fn pauli_frame_preserves_reference_modes_and_seeded_partitions() {
    let circuit = Circuit::from_stim_str("X 9\nM 9\nR 9\nH 9\nX_ERROR(0.25) 9\nM 9\n")
        .expect("parse fixture");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile fixture");

    let mut with_reference = plan
        .session_with_reference_mode(
            RandomPolicy::Seeded(Seed::new(43)),
            ReferenceSampleMode::UseReferenceSample,
        )
        .expect("construct reference session");
    let mut without_reference = plan
        .session_with_reference_mode(
            RandomPolicy::Seeded(Seed::new(43)),
            ReferenceSampleMode::SkipReferenceSample,
        )
        .expect("construct skip-reference session");
    let mut physical = RecordSink::default();
    let mut deviations = RecordSink::default();
    with_reference
        .run(ShotCount::new(130), &mut physical)
        .expect("sample physical records");
    without_reference
        .run(ShotCount::new(130), &mut deviations)
        .expect("sample deviation records");
    assert!(
        physical
            .records
            .iter()
            .all(|record| record.first().copied() == Some(true))
    );
    assert!(
        deviations
            .records
            .iter()
            .all(|record| record.first().copied() == Some(false))
    );
    assert!(physical.records.iter().zip(&deviations.records).all(
        |(physical, deviation)| matches!(
            (physical.get(1), deviation.get(1)),
            (Some(physical), Some(deviation)) if physical == deviation
        )
    ));

    let mut streams = Vec::new();
    for &lane_width in LaneWidth::ALL {
        let session = || {
            plan.session_with_lane_width(
                RandomPolicy::Seeded(Seed::new(47)),
                ReferenceSampleMode::UseReferenceSample,
                lane_width,
            )
            .expect("construct session")
        };
        let mut combined_session = session();
        let mut partitioned_session = session();
        assert_eq!(partitioned_session.lane_width(), lane_width);
        let mut combined = RecordSink::default();
        combined_session
            .run(ShotCount::new(330), &mut combined)
            .expect("sample combined stream");
        let mut partitioned = Vec::new();
        for shots in [17, 47, 1, 63, 113, 89] {
            let mut part = RecordSink::default();
            partitioned_session
                .run(ShotCount::new(shots), &mut part)
                .expect("sample partition");
            partitioned.extend(part.records);
        }
        assert_eq!(partitioned_session.total_committed_shots().get(), 330);
        assert_eq!(combined.records, partitioned, "{lane_width:?}");
        streams.push(combined.records);
    }
    // The seeded stream is a function of the lane width as well as the seed.
    assert_eq!(streams.len(), LaneWidth::ALL.len());
    assert_ne!(streams.first(), streams.last());
}

#[test]
fn deterministic_sampling_is_identical_at_every_lane_width_and_shot_count() {
    // Resets, measurements, and H randomize the frame lanes, but every measurement result is
    // fixed by the reference sample, so each width must produce the same records.
    let circuit =
        Circuit::from_stim_str("X 0\nM 0\nH 5\nMR 0\nRX 2\nMX 2\nX 3\nCX 3 4\nM 3 4\nMPP X2*Z3\n")
            .expect("parse fixture");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile fixture");
    let expected = [true, true, false, true, true, true];
    for shots in [1_u64, 63, 64, 65, 255, 256, 257, 1000] {
        for &lane_width in LaneWidth::ALL {
            let mut session = plan
                .session_with_lane_width(
                    RandomPolicy::Seeded(Seed::new(shots)),
                    ReferenceSampleMode::UseReferenceSample,
                    lane_width,
                )
                .expect("construct session");
            let mut sink = RecordSink::default();
            session
                .run(ShotCount::new(shots), &mut sink)
                .expect("sample deterministic records");
            assert_eq!(sink.records.len() as u64, shots);
            assert!(
                sink.records.iter().all(|record| *record == expected),
                "{shots} shots at {lane_width:?}"
            );
        }
    }
}

#[test]
fn warmed_session_allocation_does_not_scale_with_shot_count() {
    for source in [
        "X_ERROR(0.25) 0\nM(0.125) 0\n",
        "H 0\nCX 0 1\nM 0 1\n",
        "SWAP 0 1\nH 0\nM 0 1\n",
        "SWAP 0 1\nMZZ 0 1\n",
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse fixture");
        let plan = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile fixture");
        let mut session = plan
            .session(RandomPolicy::Seeded(Seed::new(47)))
            .expect("construct session");
        let mut sink = DigestSink::default();
        session
            .run(ShotCount::new(64), &mut sink)
            .expect("warm session");

        let one = allocation_counter::measure(|| {
            session
                .run(ShotCount::new(64), &mut sink)
                .expect("one batch");
        });
        let many = allocation_counter::measure(|| {
            session
                .run(ShotCount::new(512), &mut sink)
                .expect("many batches");
        });
        assert_eq!(
            many.count_total, one.count_total,
            "allocation count scaled for {source:?}: one={one:?}, many={many:?}"
        );
        assert_eq!(
            many.bytes_total, one.bytes_total,
            "allocation bytes scaled for {source:?}: one={one:?}, many={many:?}"
        );
        assert_eq!(
            many.bytes_max, one.bytes_max,
            "peak allocation scaled for {source:?}: one={one:?}, many={many:?}"
        );
        std::hint::black_box(sink.digest);
    }
}

mod storage_policy;

/// Circuits whose only work is one Z measurement, optionally after one single-qubit Pauli
/// channel on the measured qubit, ran on a dedicated direct-Z plan before E3 and now run on the
/// bit-plane Pauli frame. Their seeded streams changed, so this pins their distribution against
/// the analytic outcome probability with a falsifiable bound.
#[test]
#[allow(
    clippy::cast_precision_loss,
    reason = "shot counts stay far below f64's exact integer range"
)]
fn former_direct_z_circuits_keep_their_analytic_outcome_distribution() {
    const SHOTS: u64 = 200_000;
    // (circuit, probability that the recorded bit is one, skip the reference sample)
    let flip =
        |pauli: f64, measurement: f64| pauli * (1.0 - measurement) + measurement * (1.0 - pauli);
    let cases = [
        ("M 0\n", 0.0, false),
        ("M !0\n", 1.0, false),
        ("M !0\n", 0.0, true),
        (
            "X_ERROR(0.25) 0\nM(0.125) !0\n",
            1.0 - flip(0.25, 0.125),
            false,
        ),
        ("X_ERROR(0.25) 0\nM(0.125) !0\n", flip(0.25, 0.125), true),
        (
            "R 0\nDEPOLARIZE1(0.3) 0\nM(0.05) 0\nDETECTOR rec[-1]\n",
            flip(0.2, 0.05),
            false,
        ),
        ("PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\nM 0\n", 0.3, false),
        ("Y_ERROR(0.4) 0\nM 0\n", 0.4, false),
        ("Z_ERROR(0.4) 0\nM(0.1) 0\n", 0.1, false),
    ];
    for (index, (source, probability, skip)) in cases.into_iter().enumerate() {
        let circuit = Circuit::from_stim_str(source).expect("parse single-measurement fixture");
        let plan = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile single-measurement fixture");
        let mode = if skip {
            ReferenceSampleMode::SkipReferenceSample
        } else {
            ReferenceSampleMode::UseReferenceSample
        };
        let mut session = plan
            .session_with_reference_mode(RandomPolicy::Seeded(Seed::new(index as u64 + 11)), mode)
            .expect("construct session");
        let mut sink = RecordSink::default();
        session
            .run(ShotCount::new(SHOTS), &mut sink)
            .expect("run single-measurement fixture");
        let ones = sink
            .records
            .iter()
            .filter(|record| record == &&vec![true])
            .count() as f64;
        let expected = SHOTS as f64 * probability;
        let tolerance = 6.0 * (SHOTS as f64 * probability * (1.0 - probability)).sqrt();
        assert!(
            (ones - expected).abs() <= tolerance,
            "{source} skip={skip}: observed {ones} ones, expected {expected:.1} +/- {tolerance:.1}"
        );
    }
}
