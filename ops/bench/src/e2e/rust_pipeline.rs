use std::io::{Read as _, Write as _};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use stab_analysis::{
    CodeDistance, ErrorAnalyzerOptions, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    circuit_to_detector_error_model, generate_repetition_code_circuit,
};
use stab_decoder::{DecodeBatchStatus, DecodeCancellation, DecoderInputBatchView, decode_batch};
use stab_engine::{
    MeasurementToDetectionCompiler, RandomPolicy, SamplingCompiler, Seed, ShotCount,
};
use stab_model::Probability;
use stab_records::{CorrectionWidth, DetectionBatchView, DetectionSink, ObservablePredictionBatch};
use stab_reference_decoder::ExactMlDecoderSession;

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
) -> Result<RustWorkerReceipt, String> {
    let mut barrier = [0_u8; 1];
    std::io::stdin()
        .read_exact(&mut barrier)
        .map_err(|source| format!("cannot read start barrier: {source}"))?;
    if barrier != *b"\n" {
        return Err("start barrier must be one newline".to_string());
    }

    let params = RepetitionCodeParams::new(
        RoundCount::try_new(3).map_err(|source| source.to_string())?,
        CodeDistance::try_new(3).map_err(|source| source.to_string())?,
        RepetitionCodeTask::Memory,
    )
    .map_err(|source| source.to_string())?
    .with_before_round_data_depolarization(probability(0.05)?)
    .with_before_measure_flip_probability(probability(0.025)?)
    .with_after_reset_flip_probability(probability(0.0125)?)
    .with_after_clifford_depolarization(probability(0.00625)?);
    let generated =
        generate_repetition_code_circuit(&params).map_err(|source| source.to_string())?;
    let circuit = generated.circuit();
    let model = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
        .map_err(|source| source.to_string())?;
    let mut decoder =
        ExactMlDecoderSession::try_compile_model(&model).map_err(|source| source.to_string())?;
    let sampling_plan = SamplingCompiler::new()
        .compile(circuit)
        .map_err(|source| source.to_string())?;
    let detection_plan = MeasurementToDetectionCompiler::new()
        .compile(circuit)
        .map_err(|source| source.to_string())?;
    if detection_plan.detector_width() != decoder.layout().detector_width()
        || detection_plan.observable_width() != decoder.layout().observable_width()
    {
        return Err("compiled sampling and decoder layouts disagree".to_string());
    }
    let mut sampling = sampling_plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .map_err(|source| source.to_string())?;
    let mut conversion = detection_plan
        .session()
        .map_err(|source| source.to_string())?;
    let mut report = PipelineReport::default();
    let mut sink = LogicalFailureSink::new(&mut decoder, &mut report)?;

    let started = Instant::now();
    {
        let mut transaction = conversion
            .start_transaction(&mut sink)
            .map_err(|source| source.to_string())?;
        let summary = sampling
            .run(ShotCount::new(shots), &mut transaction)
            .map_err(|source| source.to_string())?;
        if summary.committed_shots().get() != shots {
            return Err(format!(
                "sampling committed {} of {shots} shots",
                summary.committed_shots().get()
            ));
        }
    }
    let elapsed_seconds = started.elapsed().as_secs_f64();
    if !sink.finished {
        return Err("detection sink did not finish".to_string());
    }
    drop(sink);
    if report.shots != shots
        || report.logical_failures < minimum_logical_failures
        || report.logical_failures > maximum_logical_failures
    {
        return Err(format!(
            "pipeline produced {} failures in {} shots, expected {minimum_logical_failures}..={maximum_logical_failures} in {shots}",
            report.logical_failures, report.shots
        ));
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

pub(super) fn write_receipt(receipt: &RustWorkerReceipt) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, receipt).map_err(|source| source.to_string())?;
    stdout
        .write_all(b"\n")
        .map_err(|source| format!("cannot write worker receipt: {source}"))
}

fn probability(value: f64) -> Result<Probability, String> {
    Probability::try_new(value).map_err(|source| source.to_string())
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
    cancellation: DecodeCancellation,
    finished: bool,
}

impl<'a> LogicalFailureSink<'a> {
    fn new(
        decoder: &'a mut ExactMlDecoderSession,
        report: &'a mut PipelineReport,
    ) -> Result<Self, String> {
        let correction_width = CorrectionWidth::new(decoder.layout().observable_width().get());
        Ok(Self {
            decoder,
            report,
            predictions: ObservablePredictionBatch::zeros(64, correction_width)
                .map_err(|source| source.to_string())?,
            cancellation: DecodeCancellation::new(),
            finished: false,
        })
    }
}

impl DetectionSink for LogicalFailureSink<'_> {
    type Error = String;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let summary = decode_batch(
            self.decoder,
            DecoderInputBatchView::from_detection(batch),
            &mut self.predictions,
            &self.cancellation,
        )
        .map_err(|source| source.to_string())?;
        if summary.status() != DecodeBatchStatus::Completed
            || summary.completed_shots() != batch.shot_count()
        {
            return Err(format!(
                "decoder completed {} of {} records",
                summary.completed_shots(),
                batch.shot_count()
            ));
        }
        for shot_index in 0..batch.shot_count() {
            let predicted = self
                .predictions
                .records()
                .get(shot_index, 0)
                .ok_or_else(|| format!("prediction bit is missing for shot {shot_index}"))?;
            let actual = batch
                .observables()
                .get(shot_index, 0)
                .ok_or_else(|| format!("observable bit is missing for shot {shot_index}"))?;
            self.report.logical_failures = self
                .report
                .logical_failures
                .checked_add(u64::from(predicted != actual))
                .ok_or_else(|| "logical failure count overflow".to_string())?;
        }
        self.report.shots = self
            .report
            .shots
            .checked_add(
                u64::try_from(batch.shot_count())
                    .map_err(|_| "batch shot count does not fit in u64".to_string())?,
            )
            .ok_or_else(|| "pipeline shot count overflow".to_string())?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finished = true;
        Ok(())
    }
}
