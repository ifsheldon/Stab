use std::fmt::{self, Display, Formatter};
use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use sha2::{Digest as _, Sha256};
use stab_analysis::{
    CodeDistance, ErrorAnalyzerOptions, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    circuit_to_detector_error_model, generate_repetition_code_circuit,
};
use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeCancellation, DecoderInputBatchView, decode_batch,
};
use stab_engine::{
    MeasurementToDetectionCompiler, MeasurementToDetectionSession, RandomPolicy, SamplingCompiler,
    SamplingSession, Seed, ShotCount,
};
use stab_model::{DetectorErrorModel, Probability};
use stab_records::{
    CorrectionWidth, DetectionBatchView, DetectionSink, ObservablePredictionBatch, PackedShotBatch,
};
use stab_reference_decoder::{ExactMlCompileError, ExactMlDecodeError, ExactMlDecoderSession};
use thiserror::Error;

use super::{byte_digest, semantic_digest, workload::WorkerWorkload};

const PIPELINE_SEED: u64 = 0xA7D3_C0DE;
const MAX_BATCH_SHOTS: u64 = 262_144;
const COMPILE_SCALES: [CompileScale; 3] = [
    CompileScale::new(6, 12),
    CompileScale::new(10, 32),
    CompileScale::accepted_maximum(),
];
const DECODE_MODEL_SCALE: CompileScale = CompileScale::new(14, 64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DecoderDiagnosticKind {
    ExactMlCompile,
    ExactMlReusedDecode,
    SampleDetectDecodePipeline,
}

impl DecoderDiagnosticKind {
    const fn from_workload(workload: WorkerWorkload) -> Option<Self> {
        match workload {
            WorkerWorkload::ExactMlCompile => Some(Self::ExactMlCompile),
            WorkerWorkload::ExactMlReusedDecode => Some(Self::ExactMlReusedDecode),
            WorkerWorkload::SampleDetectDecodePipeline => Some(Self::SampleDetectDecodePipeline),
            _ => None,
        }
    }

    const fn marker(self) -> u8 {
        match self {
            Self::ExactMlCompile => 1,
            Self::ExactMlReusedDecode => 2,
            Self::SampleDetectDecodePipeline => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DecoderDiagnosticOutput {
    Compile {
        completed_iterations: u64,
    },
    Decode {
        completed_iterations: u64,
        completed_shots: u64,
    },
    Pipeline(ExperimentReport),
}

pub(super) struct DecoderDiagnosticFixture {
    kind: DecoderDiagnosticKind,
    state: DecoderDiagnosticState,
    input_material: Vec<u8>,
}

enum DecoderDiagnosticState {
    Compile(CompileFixture),
    Decode(DecodeFixture),
    Pipeline(Box<PipelineFixture>),
}

impl DecoderDiagnosticFixture {
    pub(super) fn prepare(
        workload: WorkerWorkload,
        iterations: u64,
        work_items: u64,
    ) -> Result<Self, DecoderDiagnosticError> {
        let kind = DecoderDiagnosticKind::from_workload(workload)
            .ok_or(DecoderDiagnosticError::WrongWorkload(workload.id()))?;
        let (state, input_material) = match kind {
            DecoderDiagnosticKind::ExactMlCompile => {
                let fixture = CompileFixture::prepare(work_items)?;
                let input = fixture.input_material.clone();
                (DecoderDiagnosticState::Compile(fixture), input)
            }
            DecoderDiagnosticKind::ExactMlReusedDecode => {
                let fixture = DecodeFixture::prepare(work_items)?;
                let input = fixture.input_material.clone();
                (DecoderDiagnosticState::Decode(fixture), input)
            }
            DecoderDiagnosticKind::SampleDetectDecodePipeline => {
                let fixture = PipelineFixture::prepare(iterations, work_items)?;
                let input = fixture.input_material.clone();
                (DecoderDiagnosticState::Pipeline(Box::new(fixture)), input)
            }
        };
        Ok(Self {
            kind,
            state,
            input_material,
        })
    }

    pub(super) fn execute(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<DecoderDiagnosticOutput, DecoderDiagnosticError> {
        match &mut self.state {
            DecoderDiagnosticState::Compile(fixture) => fixture.execute(iterations, work_items),
            DecoderDiagnosticState::Decode(fixture) => fixture.execute(iterations, work_items),
            DecoderDiagnosticState::Pipeline(fixture) => fixture.execute(iterations, work_items),
        }
    }

    pub(super) fn validate(
        &self,
        output: DecoderDiagnosticOutput,
        iterations: u64,
        work_items: u64,
    ) -> Result<String, DecoderDiagnosticError> {
        let witness = match &self.state {
            DecoderDiagnosticState::Compile(fixture) => {
                fixture.validate(output, iterations, work_items)?
            }
            DecoderDiagnosticState::Decode(fixture) => {
                fixture.validate(output, iterations, work_items)?
            }
            DecoderDiagnosticState::Pipeline(fixture) => {
                fixture.validate(output, iterations, work_items)?
            }
        };
        let mut material = Vec::with_capacity(1 + witness.len());
        material.push(self.kind.marker());
        material.extend_from_slice(&witness);
        Ok(semantic_digest(byte_digest(&material)))
    }

    pub(super) fn input_bytes(&self) -> Result<u64, DecoderDiagnosticError> {
        u64::try_from(self.input_material.len()).map_err(|_| DecoderDiagnosticError::InputSizeRange)
    }

    pub(super) fn input_digest(&self) -> String {
        semantic_digest(byte_digest(&self.input_material))
    }
}

#[derive(Clone, Copy)]
struct CompileScale {
    detector_count: usize,
    mechanism_count: usize,
    high_precision_passes: usize,
    profile: CompileScaleProfile,
}

#[derive(Clone, Copy)]
enum CompileScaleProfile {
    Throughput,
    AcceptedMaximum,
}

impl CompileScale {
    const fn new(detector_count: usize, mechanism_count: usize) -> Self {
        Self {
            detector_count,
            mechanism_count,
            high_precision_passes: 1,
            profile: CompileScaleProfile::Throughput,
        }
    }

    const fn accepted_maximum() -> Self {
        Self {
            detector_count: 20,
            mechanism_count: 1,
            high_precision_passes: 2,
            profile: CompileScaleProfile::AcceptedMaximum,
        }
    }

    const fn transition_count(self) -> u64 {
        let joint_width = self.detector_count + 1;
        (1_u64 << joint_width) * self.mechanism_count as u64 * self.high_precision_passes as u64
    }
}

struct CompileFixture {
    model: DetectorErrorModel,
    input_material: Vec<u8>,
}

impl CompileFixture {
    fn prepare(work_items: u64) -> Result<Self, DecoderDiagnosticError> {
        let scale = COMPILE_SCALES
            .into_iter()
            .find(|scale| scale.transition_count() == work_items)
            .ok_or(DecoderDiagnosticError::CompileWorkShape(work_items))?;
        let model = compile_model(scale)?;
        let input_material =
            input_material(b"a7-exact-ml-compile-v1", model.to_dem_string().as_bytes());
        Ok(Self {
            model,
            input_material,
        })
    }

    fn execute(
        &self,
        iterations: u64,
        work_items: u64,
    ) -> Result<DecoderDiagnosticOutput, DecoderDiagnosticError> {
        require_compile_work_items(work_items)?;
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let compiled = ExactMlDecoderSession::try_compile_model(black_box(&self.model))?;
            drop(black_box(compiled));
        }
        Ok(DecoderDiagnosticOutput::Compile {
            completed_iterations: iterations,
        })
    }

    fn validate(
        &self,
        output: DecoderDiagnosticOutput,
        iterations: u64,
        work_items: u64,
    ) -> Result<Vec<u8>, DecoderDiagnosticError> {
        require_compile_work_items(work_items)?;
        if output
            != (DecoderDiagnosticOutput::Compile {
                completed_iterations: iterations,
            })
        {
            return Err(DecoderDiagnosticError::OutputMismatch("exact ML compile"));
        }
        let observed = compile_witness(&self.model)?;
        Ok(observed.encode())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompileWitness {
    model_digest: [u8; 32],
    detector_count: usize,
    observable_count: usize,
    syndrome_count: usize,
    retained_prediction_bytes: usize,
    prediction_digest: [u8; 32],
}

impl CompileWitness {
    fn encode(&self) -> Vec<u8> {
        let mut material = Vec::with_capacity(32 * 2 + 32);
        material.extend_from_slice(&self.model_digest);
        for value in [
            self.detector_count,
            self.observable_count,
            self.syndrome_count,
            self.retained_prediction_bytes,
        ] {
            material.extend_from_slice(&(value as u128).to_le_bytes());
        }
        material.extend_from_slice(&self.prediction_digest);
        material
    }
}

fn compile_witness(model: &DetectorErrorModel) -> Result<CompileWitness, DecoderDiagnosticError> {
    let session = ExactMlDecoderSession::try_compile_model(model)?;
    let mut predictions = Vec::with_capacity(session.syndrome_count());
    for syndrome in 0..session.syndrome_count() {
        match session.prediction_for_syndrome(syndrome as u64) {
            Ok(prediction) => predictions.push(u8::from(prediction)),
            Err(ExactMlDecodeError::ImpossibleSyndrome { .. }) => predictions.push(2),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(CompileWitness {
        model_digest: session.model_fingerprint().digest(),
        detector_count: session.layout().detector_width().get(),
        observable_count: session.layout().observable_width().get(),
        syndrome_count: session.syndrome_count(),
        retained_prediction_bytes: session.retained_prediction_bytes(),
        prediction_digest: Sha256::digest(&predictions).into(),
    })
}

struct DecodeFixture {
    session: ExactMlDecoderSession,
    detectors: PackedShotBatch,
    predictions: ObservablePredictionBatch,
    cancellation: DecodeCancellation,
    input_material: Vec<u8>,
}

impl DecodeFixture {
    fn prepare(work_items: u64) -> Result<Self, DecoderDiagnosticError> {
        require_batch_shots(work_items)?;
        let model = compile_model(DECODE_MODEL_SCALE)?;
        let session = ExactMlDecoderSession::try_compile_model(&model)?;
        let shot_count = usize::try_from(work_items)
            .map_err(|_| DecoderDiagnosticError::ShotCountRange(work_items))?;
        let detector_count = session.layout().detector_width().get();
        let detectors = detector_batch(shot_count, detector_count)?;
        let predictions = ObservablePredictionBatch::zeros(
            shot_count,
            CorrectionWidth::new(session.layout().observable_width().get()),
        )
        .map_err(|error| DecoderDiagnosticError::Records(error.to_string()))?;
        let input_material = decode_input_material(&model, shot_count, detector_count);
        Ok(Self {
            session,
            detectors,
            predictions,
            cancellation: DecodeCancellation::new(),
            input_material,
        })
    }

    fn execute(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<DecoderDiagnosticOutput, DecoderDiagnosticError> {
        require_batch_shots(work_items)?;
        let expected_shots = usize::try_from(work_items)
            .map_err(|_| DecoderDiagnosticError::ShotCountRange(work_items))?;
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let summary = decode_batch(
                black_box(&mut self.session),
                DecoderInputBatchView::from_detectors(black_box(self.detectors.view())),
                black_box(&mut self.predictions),
                black_box(&self.cancellation),
            )
            .map_err(|error| DecoderDiagnosticError::Decode(error.to_string()))?;
            if summary.status() != DecodeBatchStatus::Completed
                || summary.completed_shots() != expected_shots
            {
                return Err(DecoderDiagnosticError::Progress {
                    expected: expected_shots,
                    actual: summary.completed_shots(),
                });
            }
            black_box(summary);
        }
        Ok(DecoderDiagnosticOutput::Decode {
            completed_iterations: iterations,
            completed_shots: iterations
                .checked_mul(work_items)
                .ok_or(DecoderDiagnosticError::WorkOverflow)?,
        })
    }

    fn validate(
        &self,
        output: DecoderDiagnosticOutput,
        iterations: u64,
        work_items: u64,
    ) -> Result<Vec<u8>, DecoderDiagnosticError> {
        let completed_shots = iterations
            .checked_mul(work_items)
            .ok_or(DecoderDiagnosticError::WorkOverflow)?;
        if output
            != (DecoderDiagnosticOutput::Decode {
                completed_iterations: iterations,
                completed_shots,
            })
        {
            return Err(DecoderDiagnosticError::OutputMismatch(
                "reused exact ML decode",
            ));
        }
        let actual = prediction_batch_digest(&self.predictions)?;
        let mut witness = Vec::with_capacity(48);
        witness.extend_from_slice(&actual);
        witness
            .extend_from_slice(&(self.session.retained_prediction_bytes() as u128).to_le_bytes());
        Ok(witness)
    }
}

struct PipelineFixture {
    state: PipelineState,
    input_material: Vec<u8>,
}

impl PipelineFixture {
    fn prepare(_iterations: u64, work_items: u64) -> Result<Self, DecoderDiagnosticError> {
        require_batch_shots(work_items)?;
        let (state, input_material) = PipelineState::prepare(work_items)?;
        Ok(Self {
            state,
            input_material,
        })
    }

    fn execute(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<DecoderDiagnosticOutput, DecoderDiagnosticError> {
        let report = self.state.execute(iterations, work_items)?;
        Ok(DecoderDiagnosticOutput::Pipeline(report))
    }

    fn validate(
        &self,
        output: DecoderDiagnosticOutput,
        iterations: u64,
        work_items: u64,
    ) -> Result<Vec<u8>, DecoderDiagnosticError> {
        let DecoderDiagnosticOutput::Pipeline(report) = output else {
            return Err(DecoderDiagnosticError::OutputMismatch(
                "sample-detect-decode pipeline",
            ));
        };
        let expected_shots = iterations
            .checked_mul(work_items)
            .ok_or(DecoderDiagnosticError::WorkOverflow)?;
        if report.shots != expected_shots || report.logical_failures > report.shots {
            return Err(DecoderDiagnosticError::OutputMismatch(
                "sample-detect-decode report",
            ));
        }
        let mut witness = Vec::with_capacity(16);
        witness.extend_from_slice(&report.shots.to_le_bytes());
        witness.extend_from_slice(&report.logical_failures.to_le_bytes());
        Ok(witness)
    }
}

struct PipelineState {
    sampling: SamplingSession,
    conversion: MeasurementToDetectionSession,
    decoder: ExactMlDecoderSession,
    predictions: ObservablePredictionBatch,
    cancellation: DecodeCancellation,
    report: ExperimentReport,
}

impl PipelineState {
    fn prepare(work_items: u64) -> Result<(Self, Vec<u8>), DecoderDiagnosticError> {
        let params = repetition_params()?;
        let generated = generate_repetition_code_circuit(&params)
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        let circuit = generated.circuit();
        let model = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        let decoder = ExactMlDecoderSession::try_compile_model(&model)?;
        let sampling_plan = SamplingCompiler::new()
            .compile(circuit)
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        let detection_plan = MeasurementToDetectionCompiler::new()
            .compile(circuit)
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        if detection_plan.detector_width() != decoder.layout().detector_width()
            || detection_plan.observable_width() != decoder.layout().observable_width()
        {
            return Err(DecoderDiagnosticError::Pipeline(
                "pipeline compiler layouts differ".to_string(),
            ));
        }
        let sampling = sampling_plan
            .session(RandomPolicy::Seeded(Seed::new(PIPELINE_SEED)))
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        let conversion = detection_plan
            .session()
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
        let predictions = ObservablePredictionBatch::zeros(
            64,
            CorrectionWidth::new(decoder.layout().observable_width().get()),
        )
        .map_err(|error| DecoderDiagnosticError::Records(error.to_string()))?;
        let mut descriptor = Vec::new();
        descriptor.extend_from_slice(b"a7-sample-detect-decode-v1\0");
        descriptor.extend_from_slice(&PIPELINE_SEED.to_le_bytes());
        descriptor.extend_from_slice(&work_items.to_le_bytes());
        descriptor.extend_from_slice(circuit.to_stim_string().as_bytes());
        descriptor.extend_from_slice(&decoder.model_fingerprint().digest());
        Ok((
            Self {
                sampling,
                conversion,
                decoder,
                predictions,
                cancellation: DecodeCancellation::new(),
                report: ExperimentReport::default(),
            },
            descriptor,
        ))
    }

    fn execute(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<ExperimentReport, DecoderDiagnosticError> {
        require_batch_shots(work_items)?;
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            self.run_once(work_items)?;
        }
        black_box(self.report);
        Ok(self.report)
    }

    fn run_once(&mut self, shots: u64) -> Result<(), DecoderDiagnosticError> {
        let mut sink = PipelineSink {
            decoder: &mut self.decoder,
            predictions: &mut self.predictions,
            cancellation: &self.cancellation,
            report: &mut self.report,
            finished: false,
        };
        {
            let mut adapter = self
                .conversion
                .start_delivery(&mut sink)
                .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
            let summary = self
                .sampling
                .run(ShotCount::new(shots), &mut adapter)
                .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?;
            if summary.committed_shots().get() != shots {
                return Err(DecoderDiagnosticError::Progress {
                    expected: usize::try_from(shots)
                        .map_err(|_| DecoderDiagnosticError::ShotCountRange(shots))?,
                    actual: usize::try_from(summary.committed_shots().get()).map_err(|_| {
                        DecoderDiagnosticError::ShotCountRange(summary.committed_shots().get())
                    })?,
                });
            }
        }
        if !sink.finished {
            return Err(DecoderDiagnosticError::Pipeline(
                "pipeline sink was not finished".to_string(),
            ));
        }
        Ok(())
    }
}

struct PipelineSink<'a> {
    decoder: &'a mut ExactMlDecoderSession,
    predictions: &'a mut ObservablePredictionBatch,
    cancellation: &'a DecodeCancellation,
    report: &'a mut ExperimentReport,
    finished: bool,
}

impl DetectionSink for PipelineSink<'_> {
    type Error = PipelineSinkError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let summary = decode_batch(
            self.decoder,
            DecoderInputBatchView::from_detection(batch),
            self.predictions,
            self.cancellation,
        )?;
        if summary.status() != DecodeBatchStatus::Completed
            || summary.completed_shots() != batch.shot_count()
        {
            return Err(PipelineSinkError::Progress {
                expected: batch.shot_count(),
                actual: summary.completed_shots(),
            });
        }
        for shot_index in 0..batch.shot_count() {
            let predicted = self.predictions.records().get(shot_index, 0).ok_or(
                PipelineSinkError::MissingBit {
                    kind: "prediction",
                    shot_index,
                },
            )?;
            let actual =
                batch
                    .observables()
                    .get(shot_index, 0)
                    .ok_or(PipelineSinkError::MissingBit {
                        kind: "observable truth",
                        shot_index,
                    })?;
            self.report.logical_failures = self
                .report
                .logical_failures
                .checked_add(u64::from(predicted != actual))
                .ok_or(PipelineSinkError::CounterOverflow)?;
        }
        self.report.shots = self
            .report
            .shots
            .checked_add(
                u64::try_from(batch.shot_count())
                    .map_err(|_| PipelineSinkError::ShotCountRange(batch.shot_count()))?,
            )
            .ok_or(PipelineSinkError::CounterOverflow)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finished = true;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ExperimentReport {
    shots: u64,
    logical_failures: u64,
}

#[derive(Debug, Error)]
enum PipelineSinkError {
    #[error(transparent)]
    Decode(#[from] DecodeBatchError<ExactMlDecodeError>),
    #[error("pipeline decoder completed {actual} of {expected} records")]
    Progress { expected: usize, actual: usize },
    #[error("pipeline {kind} bit is missing for shot {shot_index}")]
    MissingBit {
        kind: &'static str,
        shot_index: usize,
    },
    #[error("pipeline shot count {0} cannot be represented as u64")]
    ShotCountRange(usize),
    #[error("pipeline report counter overflowed")]
    CounterOverflow,
}

#[derive(Debug, Error)]
pub(in crate::qualification::runtime) enum DecoderDiagnosticError {
    #[error("qualification workload {0} is not an A7 decoder diagnostic")]
    WrongWorkload(&'static str),
    #[error("exact ML compile work count {0} is not a source-owned transition scale")]
    CompileWorkShape(u64),
    #[error("decoder batch has {actual} shots, maximum {maximum}")]
    ShotLimit { actual: u64, maximum: u64 },
    #[error("decoder shot count {0} cannot be represented on this host")]
    ShotCountRange(u64),
    #[error("decoder semantic work count overflowed")]
    WorkOverflow,
    #[error("decoder input byte count cannot be represented as u64")]
    InputSizeRange,
    #[error("decoder record fixture failed: {0}")]
    Records(String),
    #[error("decoder execution failed: {0}")]
    Decode(String),
    #[error("decoder pipeline failed: {0}")]
    Pipeline(String),
    #[error("decoder completed {actual} of {expected} records")]
    Progress { expected: usize, actual: usize },
    #[error("decoder diagnostic {0} differed from its untimed exact witness")]
    OutputMismatch(&'static str),
    #[error(transparent)]
    Compile(#[from] ExactMlCompileError),
    #[error(transparent)]
    ExactDecode(#[from] ExactMlDecodeError),
    #[error("decoder fixture invariant failed: {0}")]
    Invariant(String),
}

fn require_compile_work_items(work_items: u64) -> Result<(), DecoderDiagnosticError> {
    if COMPILE_SCALES
        .into_iter()
        .any(|scale| scale.transition_count() == work_items)
    {
        Ok(())
    } else {
        Err(DecoderDiagnosticError::CompileWorkShape(work_items))
    }
}

fn require_batch_shots(work_items: u64) -> Result<(), DecoderDiagnosticError> {
    if work_items <= MAX_BATCH_SHOTS {
        Ok(())
    } else {
        Err(DecoderDiagnosticError::ShotLimit {
            actual: work_items,
            maximum: MAX_BATCH_SHOTS,
        })
    }
}

fn compile_model(scale: CompileScale) -> Result<DetectorErrorModel, DecoderDiagnosticError> {
    if matches!(scale.profile, CompileScaleProfile::AcceptedMaximum) {
        let model = DetectorErrorModel::from_dem_str("error(0) D19\nerror(0.5) L0\n")
            .map_err(|error| DecoderDiagnosticError::Invariant(error.to_string()))?;
        if scale.transition_count() != 4_194_304 {
            return Err(DecoderDiagnosticError::Invariant(
                "accepted-maximum compile work count drifted".to_string(),
            ));
        }
        return Ok(model);
    }
    let mut text = String::new();
    for mechanism in 0..scale.mechanism_count {
        let probability_millis = 5 + (mechanism * 17) % 190;
        text.push_str(&format!("error(0.{probability_millis:03})"));
        if mechanism < scale.detector_count {
            text.push_str(&format!(" D{mechanism}"));
            if mechanism == 0 {
                text.push_str(" L0");
            }
        } else if mechanism == scale.detector_count {
            text.push_str(" L0");
        } else {
            let first = mechanism % scale.detector_count;
            let second = (first + 1 + mechanism / scale.detector_count) % scale.detector_count;
            text.push_str(&format!(" D{first} D{second}"));
            if mechanism % 3 == 0 {
                text.push_str(" L0");
            }
        }
        text.push('\n');
    }
    let model = DetectorErrorModel::from_dem_str(&text)
        .map_err(|error| DecoderDiagnosticError::Invariant(error.to_string()))?;
    let expected_transitions = (1_u64 << (scale.detector_count + 1))
        .checked_mul(scale.mechanism_count as u64)
        .and_then(|work| work.checked_mul(scale.high_precision_passes as u64))
        .ok_or(DecoderDiagnosticError::WorkOverflow)?;
    if expected_transitions != scale.transition_count() {
        return Err(DecoderDiagnosticError::Invariant(
            "compile transition count drifted".to_string(),
        ));
    }
    Ok(model)
}

fn detector_batch(
    shot_count: usize,
    detector_count: usize,
) -> Result<PackedShotBatch, DecoderDiagnosticError> {
    let mut detectors = PackedShotBatch::zeros(shot_count, detector_count)
        .map_err(|error| DecoderDiagnosticError::Records(error.to_string()))?;
    for shot_index in 0..shot_count {
        let syndrome = syndrome_for_shot(shot_index, detector_count);
        for detector in 0..detector_count {
            if syndrome & (1_usize << detector) != 0 {
                detectors
                    .set(shot_index, detector, true)
                    .map_err(|error| DecoderDiagnosticError::Records(error.to_string()))?;
            }
        }
    }
    Ok(detectors)
}

fn syndrome_for_shot(shot_index: usize, detector_count: usize) -> usize {
    let mask = (1_usize << detector_count) - 1;
    shot_index
        .wrapping_mul(0x9e37)
        .wrapping_add((shot_index >> 3).wrapping_mul(0x45d9))
        .wrapping_add(0xa7)
        & mask
}

fn prediction_batch_digest(
    predictions: &ObservablePredictionBatch,
) -> Result<[u8; 32], DecoderDiagnosticError> {
    let mut digest = Sha256::new();
    for shot_index in 0..predictions.records().shot_count() {
        let prediction = predictions.records().get(shot_index, 0).ok_or_else(|| {
            DecoderDiagnosticError::Invariant(format!(
                "prediction bit missing for shot {shot_index}"
            ))
        })?;
        digest.update([u8::from(prediction)]);
    }
    Ok(digest.finalize().into())
}

fn decode_input_material(
    model: &DetectorErrorModel,
    shot_count: usize,
    detector_count: usize,
) -> Vec<u8> {
    let mut material = input_material(
        b"a7-exact-ml-reused-decode-v1",
        model.to_dem_string().as_bytes(),
    );
    material.extend_from_slice(&(shot_count as u128).to_le_bytes());
    material.extend_from_slice(&(detector_count as u128).to_le_bytes());
    for shot_index in 0..shot_count {
        material.extend_from_slice(
            &(syndrome_for_shot(shot_index, detector_count) as u64).to_le_bytes(),
        );
    }
    material
}

fn input_material(label: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut material = Vec::with_capacity(label.len() + 1 + payload.len());
    material.extend_from_slice(label);
    material.push(0);
    material.extend_from_slice(payload);
    material
}

fn repetition_params() -> Result<RepetitionCodeParams, DecoderDiagnosticError> {
    RepetitionCodeParams::new(
        RoundCount::try_new(3)
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?,
        CodeDistance::try_new(3)
            .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))?,
        RepetitionCodeTask::Memory,
    )
    .map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))
    .and_then(|params| {
        Ok(params
            .with_before_round_data_depolarization(probability(0.05)?)
            .with_before_measure_flip_probability(probability(0.025)?)
            .with_after_reset_flip_probability(probability(0.0125)?)
            .with_after_clifford_depolarization(probability(0.00625)?))
    })
}

fn probability(value: f64) -> Result<Probability, DecoderDiagnosticError> {
    Probability::try_new(value).map_err(|error| DecoderDiagnosticError::Pipeline(error.to_string()))
}

impl Display for ExperimentReport {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} shots with {} logical failures",
            self.shots, self.logical_failures
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPILE_WORK: [u64; 3] = [1_536, 65_536, 4_194_304];
    const DECODE_SHOT_SCALES: [u64; 3] = [1_024, 65_536, 262_144];
    const PIPELINE_SHOT_SCALES: [u64; 3] = [1_024, 16_384, 262_144];

    #[test]
    fn decoder_diagnostics_match_frozen_source_owned_witnesses_at_every_scale() {
        let root = crate::root::RepoRoot::resolve(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
        )
        .expect("repository root");
        let suite = crate::qualification::read(&root).expect("performance inventory");
        for (group_id, workload) in [
            (
                crate::qualification::runtime::invocation::A7_EXACT_ML_COMPILE_GROUP_ID,
                WorkerWorkload::ExactMlCompile,
            ),
            (
                crate::qualification::runtime::invocation::A7_EXACT_ML_REUSED_DECODE_GROUP_ID,
                WorkerWorkload::ExactMlReusedDecode,
            ),
            (
                crate::qualification::runtime::invocation::A7_PIPELINE_GROUP_ID,
                WorkerWorkload::SampleDetectDecodePipeline,
            ),
        ] {
            let resolved = crate::qualification::runtime::group::load_group(
                &root,
                &suite.semantic_digest,
                group_id,
            )
            .expect("runtime group");
            let policy = resolved
                .product_diagnostic_policy
                .expect("source-owned witness policy");
            for scale in &resolved.contract.scales {
                let work_items = scale.work_items.get();
                let mut fixture = DecoderDiagnosticFixture::prepare(workload, 1, work_items)
                    .expect("decoder fixture");
                assert_eq!(
                    fixture.input_bytes().expect("input bytes"),
                    scale.input_bytes
                );
                assert_eq!(fixture.input_digest(), scale.input_digest.as_str());
                let output = fixture.execute(1, work_items).expect("decoder output");
                let digest = fixture
                    .validate(output, 1, work_items)
                    .expect("decoder witness");
                assert_eq!(
                    digest,
                    policy
                        .scale(&scale.id)
                        .expect("scale witness")
                        .expected_output_digest
                        .as_str(),
                    "{group_id}/{}",
                    scale.id,
                );
            }
        }
    }

    #[test]
    fn decoder_input_identities_are_distinct_and_work_sensitive() {
        let mut identities = std::collections::BTreeSet::new();
        for work_items in COMPILE_WORK {
            let fixture =
                DecoderDiagnosticFixture::prepare(WorkerWorkload::ExactMlCompile, 1, work_items)
                    .expect("compile fixture");
            assert!(identities.insert(fixture.input_digest()));
        }
        for work_items in DECODE_SHOT_SCALES {
            let fixture = DecoderDiagnosticFixture::prepare(
                WorkerWorkload::ExactMlReusedDecode,
                1,
                work_items,
            )
            .expect("decode fixture");
            assert!(identities.insert(fixture.input_digest()));
        }
        for work_items in PIPELINE_SHOT_SCALES {
            let fixture = DecoderDiagnosticFixture::prepare(
                WorkerWorkload::SampleDetectDecodePipeline,
                1,
                work_items,
            )
            .expect("pipeline fixture");
            assert!(identities.insert(fixture.input_digest()));
        }
        assert_eq!(identities.len(), 9);
    }

    #[test]
    fn decoder_output_witnesses_are_independent_of_calibrated_repeat_count() {
        for (workload, work_items) in [
            (WorkerWorkload::ExactMlCompile, COMPILE_WORK[0]),
            (WorkerWorkload::ExactMlReusedDecode, DECODE_SHOT_SCALES[0]),
        ] {
            let digest = |iterations| {
                let mut fixture =
                    DecoderDiagnosticFixture::prepare(workload, iterations, work_items)
                        .expect("fixture");
                let output = fixture
                    .execute(iterations, work_items)
                    .expect("diagnostic output");
                fixture
                    .validate(output, iterations, work_items)
                    .expect("diagnostic witness")
            };
            assert_eq!(digest(1), digest(2), "{}", workload.id());
        }
    }

    #[test]
    fn pipeline_witness_covers_the_complete_single_pass_report() {
        let work_items = PIPELINE_SHOT_SCALES[0];
        let digest = |iterations| {
            let mut fixture = DecoderDiagnosticFixture::prepare(
                WorkerWorkload::SampleDetectDecodePipeline,
                iterations,
                work_items,
            )
            .expect("pipeline fixture");
            let output = fixture
                .execute(iterations, work_items)
                .expect("pipeline output");
            fixture
                .validate(output, iterations, work_items)
                .expect("pipeline witness")
        };
        assert_ne!(digest(1), digest(2));
    }

    #[test]
    fn decoder_diagnostics_reject_noncontractual_work() {
        assert!(matches!(
            DecoderDiagnosticFixture::prepare(WorkerWorkload::ExactMlCompile, 1, 1_535),
            Err(DecoderDiagnosticError::CompileWorkShape(1_535))
        ));
        assert!(matches!(
            DecoderDiagnosticFixture::prepare(
                WorkerWorkload::ExactMlReusedDecode,
                1,
                MAX_BATCH_SHOTS + 1,
            ),
            Err(DecoderDiagnosticError::ShotLimit { .. })
        ));
        assert!(matches!(
            DecoderDiagnosticFixture::prepare(
                WorkerWorkload::SampleDetectDecodePipeline,
                1,
                MAX_BATCH_SHOTS + 1,
            ),
            Err(DecoderDiagnosticError::ShotLimit { .. })
        ));
    }

    #[test]
    fn pipeline_is_seeded_and_partition_equivalent_at_small_scale() {
        let (mut whole, _) = PipelineState::prepare(1_024).expect("whole pipeline");
        let (mut partitioned, _) = PipelineState::prepare(1_024).expect("partitioned pipeline");
        let whole_report = whole.execute(1, 1_024).expect("whole run");
        for shots in [17, 63, 64, 113, 767] {
            partitioned.run_once(shots).expect("partitioned run");
        }
        assert_eq!(whole_report, partitioned.report);
        assert_eq!(whole_report.shots, 1_024);
        assert_eq!(whole_report.logical_failures, 37);
    }

    #[cfg(feature = "count-allocations")]
    #[test]
    fn reused_decode_allocates_nothing_after_preparation() {
        let mut fixture =
            DecoderDiagnosticFixture::prepare(WorkerWorkload::ExactMlReusedDecode, 2, 1_024)
                .expect("decode fixture");
        let allocations = allocation_counter::measure(|| {
            black_box(fixture.execute(2, 1_024).expect("decode output"));
        });
        assert_eq!(allocations.count_total, 0, "{allocations:?}");
        assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    }
}
