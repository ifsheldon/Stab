use std::io::{Read as _, Write as _};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use stab_analysis::{
    CircuitGenerationOptions, CodeCircuitParams, CodeDistance, CodeTask, ErrorAnalyzerOptions,
    RepetitionCodeTask, RoundCount, circuit_to_detector_error_model,
};
use stab_decoder::{DecodeBatchStatus, DecoderInputBatchView, decode_batch};
use stab_engine::{
    MeasurementToDetectionCompiler, RandomPolicy, SamplingCompiler, SamplingSessionOptions, Seed,
    ShotCount,
};
use stab_model::{Cancellation, Probability};
use stab_records::{
    BitIndex, CorrectionWidth, DetectionBatchView, DetectionSink, ObservablePredictionBatch,
    ShotIndex,
};
use stab_reference_decoder::ExactMlDecoderSession;

use super::error::{PipelineError, PipelineStage};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RustWorkerReceipt {
    pub(super) schema_version: u16,
    pub(super) elapsed_seconds: f64,
    pub(super) shots: u64,
    pub(super) logical_failures: u64,
    pub(super) output_sha256: String,
}

pub(super) fn execute(
    shots: u64,
    minimum_logical_failures: u64,
    maximum_logical_failures: u64,
    seed: u64,
) -> Result<RustWorkerReceipt, PipelineError> {
    let mut barrier = [0_u8; 1];
    std::io::stdin()
        .read_exact(&mut barrier)
        .map_err(PipelineError::Barrier)?;
    if barrier != *b"\n" {
        return Err(PipelineError::BarrierByte);
    }

    let params = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeDistance::try_new(3).map_err(PipelineError::stage(PipelineStage::CodeParameters))?,
        RoundCount::try_new(3).map_err(PipelineError::stage(PipelineStage::CodeParameters))?,
    )
    .map_err(PipelineError::stage(PipelineStage::CodeParameters))?
    .with_before_round_data_depolarization(probability(0.05)?)
    .with_before_measure_flip_probability(probability(0.025)?)
    .with_after_reset_flip_probability(probability(0.0125)?)
    .with_after_clifford_depolarization(probability(0.00625)?);
    let generated = params
        .generate(&CircuitGenerationOptions::default())
        .map_err(PipelineError::stage(PipelineStage::CircuitGeneration))?;
    let circuit = generated.circuit();
    let model = circuit_to_detector_error_model(circuit, &ErrorAnalyzerOptions::default())
        .map_err(PipelineError::stage(PipelineStage::ErrorAnalysis))?;
    let mut decoder = ExactMlDecoderSession::try_compile_model(&model)
        .map_err(PipelineError::stage(PipelineStage::DecoderCompilation))?;
    let sampling_plan = SamplingCompiler::default()
        .compile(circuit)
        .map_err(PipelineError::stage(PipelineStage::SamplingCompilation))?;
    let detection_plan = MeasurementToDetectionCompiler::default()
        .compile(circuit)
        .map_err(PipelineError::stage(PipelineStage::DetectionCompilation))?;
    if detection_plan.detector_width() != decoder.layout().detector_width()
        || detection_plan.observable_width() != decoder.layout().observable_width()
    {
        return Err(PipelineError::LayoutMismatch);
    }
    let mut sampling = sampling_plan
        .session(&SamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(seed)),
            ..SamplingSessionOptions::default()
        })
        .map_err(PipelineError::stage(PipelineStage::SamplingSession))?;
    let mut conversion = detection_plan
        .session(&Default::default())
        .map_err(PipelineError::stage(PipelineStage::DetectionSession))?;
    let mut report = PipelineReport::default();
    let mut sink = LogicalFailureSink::new(&mut decoder, &mut report)?;

    let started = Instant::now();
    {
        let mut transaction = conversion
            .start_transaction(&mut sink)
            .map_err(PipelineError::stage(PipelineStage::Transaction))?;
        let summary = sampling
            .run(ShotCount::new(shots), &mut transaction)
            .map_err(PipelineError::stage(PipelineStage::Sampling))?;
        if summary.committed_shots().get() != shots {
            return Err(PipelineError::CommittedShots {
                committed: summary.committed_shots().get(),
                shots,
            });
        }
    }
    let elapsed_seconds = started.elapsed().as_secs_f64();
    if !sink.finished {
        return Err(PipelineError::SinkUnfinished);
    }
    drop(sink);
    if report.shots != shots
        || report.logical_failures < minimum_logical_failures
        || report.logical_failures > maximum_logical_failures
    {
        return Err(PipelineError::FailureBand {
            failures: report.logical_failures,
            observed_shots: report.shots,
            minimum: minimum_logical_failures,
            maximum: maximum_logical_failures,
            shots,
        });
    }
    let output = format!("{}:{}", report.shots, report.logical_failures);
    Ok(RustWorkerReceipt {
        schema_version: 1,
        elapsed_seconds,
        shots,
        logical_failures: report.logical_failures,
        output_sha256: hex::encode(Sha256::digest(output.as_bytes())),
    })
}

pub(super) fn write_receipt(receipt: &RustWorkerReceipt) -> Result<(), PipelineError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, receipt).map_err(PipelineError::ReceiptJson)?;
    stdout.write_all(b"\n").map_err(PipelineError::ReceiptWrite)
}

fn probability(value: f64) -> Result<Probability, PipelineError> {
    Probability::try_new(value).map_err(PipelineError::stage(PipelineStage::CodeParameters))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineReport {
    shots: u64,
    logical_failures: u64,
}

struct LogicalFailureSink<'a> {
    decoder: &'a mut ExactMlDecoderSession,
    report: &'a mut PipelineReport,
    predictions: ObservablePredictionBatch,
    cancellation: Cancellation,
    finished: bool,
}

impl<'a> LogicalFailureSink<'a> {
    fn new(
        decoder: &'a mut ExactMlDecoderSession,
        report: &'a mut PipelineReport,
    ) -> Result<Self, PipelineError> {
        let correction_width = CorrectionWidth::new(decoder.layout().observable_width().get());
        Ok(Self {
            decoder,
            report,
            predictions: ObservablePredictionBatch::zeros(
                stab_engine::MAX_BATCH_SHOTS,
                correction_width,
            )
            .map_err(PipelineError::stage(PipelineStage::PredictionBatch))?,
            cancellation: Cancellation::new(),
            finished: false,
        })
    }
}

impl DetectionSink for LogicalFailureSink<'_> {
    type Error = PipelineError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let summary = decode_batch(
            self.decoder,
            DecoderInputBatchView::from_detection(batch),
            &mut self.predictions,
            &self.cancellation,
        )
        .map_err(PipelineError::stage(PipelineStage::Decoding))?;
        if summary.status() != DecodeBatchStatus::Completed
            || summary.completed_shots() != batch.shot_count()
        {
            return Err(PipelineError::DecoderIncomplete {
                completed: summary.completed_shots(),
                records: batch.shot_count(),
            });
        }
        for shot_index in 0..batch.shot_count() {
            let predicted = self
                .predictions
                .records()
                .get(ShotIndex(shot_index), BitIndex(0))
                .ok_or(PipelineError::MissingPrediction { shot: shot_index })?;
            let actual = batch
                .observables()
                .get(ShotIndex(shot_index), BitIndex(0))
                .ok_or(PipelineError::MissingObservable { shot: shot_index })?;
            self.report.logical_failures = self
                .report
                .logical_failures
                .checked_add(u64::from(predicted != actual))
                .ok_or(PipelineError::FailureOverflow)?;
        }
        self.report.shots = self
            .report
            .shots
            .checked_add(u64::try_from(batch.shot_count()).map_err(|_| PipelineError::BatchShots)?)
            .ok_or(PipelineError::ShotOverflow)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finished = true;
        Ok(())
    }
}
