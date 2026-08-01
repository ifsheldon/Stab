#![allow(
    clippy::expect_used,
    reason = "the external experiment test uses one bounded deterministic workflow"
)]

use stab_analysis::{
    CodeDistance, ErrorAnalyzerOptions, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    circuit_to_detector_error_model, generate_repetition_code_circuit,
};
use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeCancellation, DecoderInputBatchView, decode_batch,
};
use stab_engine::{
    MeasurementToDetectionCompiler, RandomPolicy, SamplingCompiler, Seed, ShotCount,
};
use stab_model::Probability;
use stab_records::{CorrectionWidth, DetectionBatchView, DetectionSink, ObservablePredictionBatch};
use stab_reference_decoder::{ExactMlDecodeError, ExactMlDecoderSession};
use thiserror::Error;

const EXPERIMENT_SEED: u64 = 0xA7D3_C0DE;
const EXPERIMENT_SHOTS: u64 = 1_024;
const DIAGNOSTIC_REPORTS: [(u64, u64); 3] = [(1_024, 37), (16_384, 586), (262_144, 9_294)];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExperimentReport {
    shots: u64,
    logical_failures: u64,
}

#[test]
fn public_sample_detect_decode_experiment_is_seeded_and_partition_invariant() {
    for (shots, expected_failures) in DIAGNOSTIC_REPORTS {
        let report = run_experiment(&[shots]);
        assert_eq!(report.shots, shots);
        assert_eq!(report.logical_failures, expected_failures);
        assert!(report.logical_failures > 0);
        assert!(report.logical_failures < report.shots);
    }

    let whole = run_experiment(&[EXPERIMENT_SHOTS]);
    let repeated = run_experiment(&[EXPERIMENT_SHOTS]);
    let partitioned = run_experiment(&[17, 63, 64, 113, 767]);

    assert_eq!(whole, repeated);
    assert_eq!(whole, partitioned);
    assert_eq!(whole.shots, EXPERIMENT_SHOTS);
    assert_eq!(whole.logical_failures, 37);
    assert!(whole.logical_failures > 0);
    assert!(whole.logical_failures < whole.shots);
}

fn run_experiment(partitions: &[u64]) -> ExperimentReport {
    assert!(partitions.iter().sum::<u64>() > 0);
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(3).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("repetition parameters")
    .with_before_round_data_depolarization(probability(0.05))
    .with_before_measure_flip_probability(probability(0.025))
    .with_after_reset_flip_probability(probability(0.0125))
    .with_after_clifford_depolarization(probability(0.00625));
    let generated = generate_repetition_code_circuit(&params).expect("generate repetition circuit");
    let circuit = generated.circuit();
    let model = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
        .expect("lower detector-error model");

    let mut decoder = ExactMlDecoderSession::try_compile_model(&model).expect("compile decoder");
    let sampling_plan = SamplingCompiler::new()
        .compile(circuit)
        .expect("compile sampling");
    let detection_plan = MeasurementToDetectionCompiler::new()
        .compile(circuit)
        .expect("compile measurement conversion");
    assert_eq!(
        detection_plan.detector_width(),
        decoder.layout().detector_width()
    );
    assert_eq!(
        detection_plan.observable_width(),
        decoder.layout().observable_width()
    );

    let mut sampling = sampling_plan
        .session(RandomPolicy::Seeded(Seed::new(EXPERIMENT_SEED)))
        .expect("sampling session");
    let mut conversion = detection_plan.session().expect("conversion session");
    let mut report = ExperimentReport::default();

    for &shots in partitions {
        let mut sink = LogicalFailureSink::new(&mut decoder, &mut report);
        {
            let mut adapter = conversion
                .start_delivery(&mut sink)
                .expect("start typed conversion delivery");
            let summary = sampling
                .run(ShotCount::new(shots), &mut adapter)
                .expect("sample through typed conversion delivery");
            assert_eq!(summary.committed_shots().get(), shots);
        }
        assert!(sink.finished);
    }
    report
}

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("probability")
}

struct LogicalFailureSink<'a> {
    decoder: &'a mut ExactMlDecoderSession,
    report: &'a mut ExperimentReport,
    predictions: ObservablePredictionBatch,
    cancellation: DecodeCancellation,
    finished: bool,
}

impl<'a> LogicalFailureSink<'a> {
    fn new(decoder: &'a mut ExactMlDecoderSession, report: &'a mut ExperimentReport) -> Self {
        let correction_width = CorrectionWidth::new(decoder.layout().observable_width().get());
        Self {
            decoder,
            report,
            predictions: ObservablePredictionBatch::zeros(64, correction_width)
                .expect("bounded prediction batch"),
            cancellation: DecodeCancellation::new(),
            finished: false,
        }
    }
}

impl DetectionSink for LogicalFailureSink<'_> {
    type Error = ExperimentSinkError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let summary = decode_batch(
            self.decoder,
            DecoderInputBatchView::from_detection(batch),
            &mut self.predictions,
            &self.cancellation,
        )?;
        if summary.status() != DecodeBatchStatus::Completed
            || summary.completed_shots() != batch.shot_count()
        {
            return Err(ExperimentSinkError::UnexpectedProgress {
                requested: batch.shot_count(),
                completed: summary.completed_shots(),
            });
        }
        for shot_index in 0..batch.shot_count() {
            let predicted = self.predictions.records().get(shot_index, 0).ok_or(
                ExperimentSinkError::MissingBit {
                    kind: "prediction",
                    shot_index,
                },
            )?;
            let actual =
                batch
                    .observables()
                    .get(shot_index, 0)
                    .ok_or(ExperimentSinkError::MissingBit {
                        kind: "observable truth",
                        shot_index,
                    })?;
            self.report.logical_failures += u64::from(predicted != actual);
        }
        self.report.shots += u64::try_from(batch.shot_count()).map_err(|_| {
            ExperimentSinkError::ShotCountOverflow {
                shot_count: batch.shot_count(),
            }
        })?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finished = true;
        Ok(())
    }
}

#[derive(Debug, Error)]
enum ExperimentSinkError {
    #[error(transparent)]
    Decode(#[from] DecodeBatchError<ExactMlDecodeError>),

    #[error("decoder completed {completed} of {requested} requested records")]
    UnexpectedProgress { requested: usize, completed: usize },

    #[error("{kind} bit is missing for shot {shot_index}")]
    MissingBit {
        kind: &'static str,
        shot_index: usize,
    },

    #[error("batch shot count {shot_count} does not fit the experiment counter")]
    ShotCountOverflow { shot_count: usize },
}
