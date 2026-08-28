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
    session.batch = SessionBatch::DirectZ([0]);
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
fn direct_z_variant_matches_the_general_frame_stream() {
    let fixtures = [("X_ERROR(0.25) 0\nM(0.125) 0\n", 1)];

    for (source, expected_discriminator) in fixtures {
        let circuit = Circuit::from_stim_str(source).expect("parse fixture");
        let plan = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile fixture");
        assert_eq!(
            plan.inner.kind.executable_discriminator(),
            expected_discriminator
        );
        for reference_mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            let mut session = plan
                .session_with_reference_mode(RandomPolicy::Seeded(Seed::new(43)), reference_mode)
                .expect("construct session");
            let mut sink = RecordSink::default();
            session
                .run(ShotCount::new(129), &mut sink)
                .expect("run plan variant");

            let reference = match reference_mode {
                ReferenceSampleMode::UseReferenceSample => None,
                ReferenceSampleMode::SkipReferenceSample => {
                    Some(compute_reference_sample(&plan.inner).expect("reference sample"))
                }
            };
            let mut rng = sampler_rng(Some(43));
            let mut frame = StabilizerFrame::new(plan.inner.qubit_count);
            let mut record = Vec::with_capacity(plan.inner.measurement_count);
            let mut output = Vec::with_capacity(plan.inner.measurement_count);
            let mut expected = Vec::new();
            for _ in 0..129 {
                sample_general_into(
                    &plan.inner.operations,
                    &mut frame,
                    &mut record,
                    &mut output,
                    reference.as_deref(),
                    &mut rng,
                )
                .expect("execute general-frame comparison");
                expected.push(output.clone());
            }
            assert_eq!(sink.records, expected, "{source:?} {reference_mode:?}");
        }
    }
}

#[test]
fn pauli_frame_preserves_reference_modes_and_seeded_partitions() {
    let circuit = Circuit::from_stim_str("X 9\nM 9\nR 9\nH 9\nX_ERROR(0.25) 9\nM 9\n")
        .expect("parse fixture");
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .expect("compile fixture");
    assert_eq!(plan.inner.kind.executable_discriminator(), 4);

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

    let mut combined_session = plan
        .session(RandomPolicy::Seeded(Seed::new(47)))
        .expect("construct combined session");
    let mut partitioned_session = plan
        .session(RandomPolicy::Seeded(Seed::new(47)))
        .expect("construct partitioned session");
    let mut combined = RecordSink::default();
    combined_session
        .run(ShotCount::new(130), &mut combined)
        .expect("sample combined stream");
    let mut first = RecordSink::default();
    let mut second = RecordSink::default();
    partitioned_session
        .run(ShotCount::new(17), &mut first)
        .expect("sample first partition");
    partitioned_session
        .run(ShotCount::new(113), &mut second)
        .expect("sample second partition");
    first.records.extend(second.records);
    assert_eq!(partitioned_session.total_committed_shots().get(), 130);
    assert_eq!(combined.records, first.records);
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
