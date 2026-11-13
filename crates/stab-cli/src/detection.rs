use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_analysis::circuit_with_inlined_feedback_with_memory_budget;
use stab_engine::{
    DetectionConversionLimits, DetectionRunError, DetectionSamplingCompiler,
    MeasurementToDetectionCompiler, MeasurementToDetectionTransaction, RandomPolicy,
    ReferenceSampleMode, Seed, ShotCount,
};
use stab_records::{
    ByteSpan, DetectionBatchView, DetectionSink, FormatError, FormatErrorCode,
    MeasurementBatchView, PackedShotBatchView, RecordFormat, RecordFraming, RecordStreamReadError,
    RecordStreamReader, validate_ptb64_shot_count,
};

use crate::{
    CliError, MemoryBudgetArg, RecordFormatArg, SampleOutFormatArg,
    batch_output::{DetectionBatchEncoder, DetectionObservableOutputMode},
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, InputFile, PendingIo, StandardStreams, StreamUse},
    parse_circuit_bytes,
    streaming::{FileOutputSink, OutputSink, flush_before_failure},
};

const M2D_BATCH_SHOTS: usize = u64::BITS as usize;

#[derive(Debug, Args)]
pub(crate) struct DetectArgs {
    /// Number of shots to sample.
    #[arg(long, default_value_t = 1, value_parser = super::parse_stim_usize)]
    shots: usize,

    /// Input circuit path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detection-event path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Output detection-event format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: SampleOutFormatArg,

    /// Partially deterministic random seed for noisy detection.
    #[arg(long, value_parser = super::parse_stim_u64)]
    seed: Option<u64>,

    /// Append observable flips after detector-event bits.
    #[arg(long = "append_observables")]
    append_observables: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,

    #[command(flatten)]
    memory: MemoryBudgetArg,
}

#[derive(Debug, Args)]
pub(crate) struct M2dArgs {
    /// Circuit path used to interpret measurement records.
    #[arg(long)]
    circuit: PathBuf,

    /// Input measurement path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detection-event path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Input measurement format.
    #[arg(long = "in_format", value_enum)]
    in_format: RecordFormatArg,

    /// Optional sweep-bit input path.
    #[arg(long = "sweep")]
    sweep: Option<PathBuf>,

    /// Input sweep-bit format.
    #[arg(long = "sweep_format", value_enum, default_value = "01")]
    sweep_format: RecordFormatArg,

    /// Output detection-event format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: RecordFormatArg,

    /// Append observable flips after detector-event bits.
    #[arg(long = "append_observables")]
    append_observables: bool,

    /// Compare measurements directly instead of subtracting the circuit reference sample.
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,

    /// Stim compatibility flag for externally transformed circuits.
    #[arg(long = "ran_without_feedback")]
    ran_without_feedback: bool,

    #[command(flatten)]
    memory: MemoryBudgetArg,
}

pub(crate) fn run_detect<R, W>(
    args: DetectArgs,
    input: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    validate_detect_observable_routing(&args)?;
    validate_detect_ptb64_shots(&args)?;
    let mut io = PendingIo::preflight(
        [(FileRole::Input, args.input.as_deref())],
        [
            (FileRole::Output, args.output.as_deref()),
            (FileRole::ObservableOutput, args.obs_output.as_deref()),
        ],
    )?;
    io.reject_standard_stream_aliases(
        streams,
        StreamUse {
            reads_stdin: args.input.is_none() && args.shots != 0,
            writes_stdout: args.output.is_none(),
        },
    )?;
    if args.shots == 0 {
        let mut outputs = io.activate()?;
        let mut primary_output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
        primary_output.write_with(|writer| writer.write_all(&[]))?;
        if let Some(output) = outputs.take(FileRole::ObservableOutput) {
            let mut output = FileOutputSink::from_output(output);
            output.write_with(|writer| writer.write_all(&[]))?;
        }
        return Ok(());
    }
    let input_bytes = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(
            &mut input_file,
            args.memory.input_bytes(),
            "detect circuit input",
        )?
    } else {
        read_limited_stdin(input, args.memory.input_bytes(), "detect circuit input")?
    };
    let circuit = parse_circuit_bytes(&input_bytes, args.memory)?;
    let observable_mode = detect_observable_output_mode(&args);
    let plan = DetectionSamplingCompiler::new()
        .limits(DetectionConversionLimits::new(args.memory.get()))
        .compile(&circuit)
        .map_err(CliError::from)?;
    let encoder = DetectionBatchEncoder::try_new(
        plan.detector_width().get(),
        plan.observable_width().get(),
        observable_mode,
        args.out_format.record_format(),
        args.obs_output
            .as_ref()
            .map(|_| detection_record_format(args.obs_out_format))
            .transpose()?,
    )?;
    let mut sink = DeferredDetectionBatchSink {
        outputs: DeferredDetectOutputs::new(io, stdout),
        encoder,
    };
    let mut session = plan
        .session(random_policy(args.seed))
        .map_err(CliError::from)?;
    session
        .run(shot_count(args.shots)?, &mut sink)
        .map(|_| ())
        .map_err(map_detection_run_error)
}

pub(crate) fn run_m2d<R, W>(
    args: M2dArgs,
    input: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    validate_m2d_output_formats(&args)?;
    let mut io = PendingIo::preflight(
        [
            (FileRole::Circuit, Some(args.circuit.as_path())),
            (FileRole::Input, args.input.as_deref()),
            (FileRole::Sweep, args.sweep.as_deref()),
        ],
        [
            (FileRole::Output, args.output.as_deref()),
            (FileRole::ObservableOutput, args.obs_output.as_deref()),
        ],
    )?;
    io.reject_standard_stream_aliases(
        streams,
        StreamUse {
            reads_stdin: args.input.is_none(),
            writes_stdout: args.output.is_none(),
        },
    )?;
    let mut circuit_file = io
        .take_input(FileRole::Circuit)
        .ok_or(CliError::IoPlanInvariant {
            message: "m2d preflight omitted its required circuit input",
        })?;
    let circuit_bytes = read_limited_input_file(
        &mut circuit_file,
        args.memory.input_bytes(),
        "m2d circuit input",
    )?;
    let circuit = parse_circuit_bytes(&circuit_bytes, args.memory)?;
    let circuit = if args.ran_without_feedback {
        circuit_with_inlined_feedback_with_memory_budget(&circuit, args.memory.get())?
    } else {
        circuit
    };
    let measurement_input = io.take_input(FileRole::Input);
    let observable_mode = observable_output_mode(args.append_observables);
    let sweep_input = io.take_input(FileRole::Sweep);
    let plan = MeasurementToDetectionCompiler::new()
        .limits(DetectionConversionLimits::new(args.memory.get()))
        .reference_sample_mode(reference_sample_mode(args.skip_reference_sample))
        .compile(&circuit)
        .map_err(CliError::from)?;
    let mut session = plan.session().map_err(CliError::from)?;
    let encoder = DetectionBatchEncoder::try_new(
        plan.detector_width().get(),
        plan.observable_width().get(),
        observable_mode,
        detection_record_format(args.out_format)?,
        args.obs_output
            .as_ref()
            .map(|_| detection_record_format(args.obs_out_format))
            .transpose()?,
    )?;
    let mut measurements = match measurement_input {
        Some(file) => M2dInput::from_file(
            file,
            args.in_format,
            plan.measurement_width().get(),
            args.memory.record_bytes(),
        ),
        None => M2dInput::new(
            Box::new(input),
            None,
            args.in_format,
            (plan.measurement_width().get(), args.memory.record_bytes()),
            M2D_MEASUREMENT_KIND,
        ),
    };
    let mut sweeps = sweep_input.map(|file| {
        M2dInput::from_file(
            file,
            args.sweep_format,
            plan.sweep_width().get(),
            args.memory.record_bytes(),
        )
        .into_sweep()
    });
    let mut outputs = io.activate()?;
    let primary = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    let observable = outputs
        .take(FileRole::ObservableOutput)
        .map(FileOutputSink::from_output);
    let mut sink = M2dBatchSink {
        primary,
        observable,
        encoder,
    };
    let result = (|| -> Result<(), CliError> {
        let mut delivery = session
            .start_transaction(&mut sink)
            .map_err(CliError::from)?;
        while let Some(measurement_batch) = measurements.read_batch(M2D_BATCH_SHOTS)? {
            let shots = measurement_batch.shot_count();
            let Some(sweeps) = sweeps.as_mut() else {
                write_m2d_batch(&mut delivery, measurement_batch, None)?;
                continue;
            };
            let Some(sweep_batch) = sweeps.read_batch(shots)? else {
                return Err(invalid_result_format(
                    "m2d measurement input has more records than sweep input",
                ));
            };
            let paired = sweep_batch.shot_count();
            let measurement_batch = if paired < shots {
                measurement_batch.prefix(paired)?
            } else {
                measurement_batch
            };
            write_m2d_batch(&mut delivery, measurement_batch, Some(sweep_batch))?;
            if paired < shots {
                // The sweep input ended or failed before this measurement batch did.
                sweeps.read_batch(1)?;
                return Err(invalid_result_format(
                    "m2d measurement input has more records than sweep input",
                ));
            }
        }
        if let Some(sweeps) = sweeps.as_mut()
            && sweeps.has_more_records()?
        {
            return Err(invalid_result_format(
                "m2d sweep input has more records than measurement input",
            ));
        }
        finish_m2d(delivery)
    })();
    flush_before_failure(result, || sink.flush_outputs())
}

struct M2dBatchSink<'a, W>
where
    W: Write,
{
    primary: OutputSink<'a, W>,
    observable: Option<FileOutputSink>,
    encoder: DetectionBatchEncoder,
}

impl<W> M2dBatchSink<'_, W>
where
    W: Write,
{
    fn flush_outputs(&mut self) -> Result<(), CliError> {
        self.primary.flush()?;
        if let Some(observable) = self.observable.as_mut() {
            observable.flush()?;
        }
        Ok(())
    }
}

impl<W> DetectionSink for M2dBatchSink<'_, W>
where
    W: Write,
{
    type Error = CliError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.encoder
            .write_batch(batch, &mut self.primary, self.observable.as_mut())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.encoder
            .finish(&mut self.primary, self.observable.as_mut())
    }
}

struct DeferredDetectionBatchSink<'a, W>
where
    W: Write,
{
    outputs: DeferredDetectOutputs<'a, W>,
    encoder: DetectionBatchEncoder,
}

impl<W> DetectionSink for DeferredDetectionBatchSink<'_, W>
where
    W: Write,
{
    type Error = CliError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        let (primary, observable) = self.outputs.activate()?;
        self.encoder.write_batch(batch, primary, observable)
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        let (primary, observable) = self.outputs.activate()?;
        self.encoder.finish(primary, observable)
    }
}

fn write_m2d_batch<Sink>(
    delivery: &mut MeasurementToDetectionTransaction<'_, '_, Sink>,
    measurements: PackedShotBatchView<'_>,
    sweeps: Option<PackedShotBatchView<'_>>,
) -> Result<(), CliError>
where
    Sink: DetectionSink<Error = CliError>,
{
    delivery
        .write_batch_with_sweep(
            MeasurementBatchView::new(measurements),
            sweeps.map(MeasurementBatchView::new),
        )
        .map(|_| ())
        .map_err(map_detection_run_error)
}

fn finish_m2d<Sink>(
    delivery: MeasurementToDetectionTransaction<'_, '_, Sink>,
) -> Result<(), CliError>
where
    Sink: DetectionSink<Error = CliError>,
{
    delivery.finish().map_err(map_detection_run_error)
}

fn map_detection_run_error(error: DetectionRunError<CliError>) -> CliError {
    match error {
        DetectionRunError::Engine { source, .. } => CliError::from(source),
        DetectionRunError::Sink { source, .. } => source,
    }
}

fn random_policy(seed: Option<u64>) -> RandomPolicy {
    match seed {
        Some(seed) => RandomPolicy::Seeded(Seed::new(seed)),
        None => RandomPolicy::Entropy,
    }
}

fn shot_count(shots: usize) -> Result<ShotCount, CliError> {
    u64::try_from(shots)
        .map(ShotCount::new)
        .map_err(|_| CliError::MeasurementCountOverflow)
}

fn reference_sample_mode(skip_reference_sample: bool) -> ReferenceSampleMode {
    if skip_reference_sample {
        ReferenceSampleMode::SkipReferenceSample
    } else {
        ReferenceSampleMode::UseReferenceSample
    }
}

fn detection_record_format(format: RecordFormatArg) -> Result<RecordFormat, CliError> {
    format
        .record_format()
        .ok_or(CliError::UnsupportedDetectionFormat { format: "stim" })
}

fn validate_m2d_output_formats(args: &M2dArgs) -> Result<(), CliError> {
    validate_m2d_output_format(args.out_format)?;
    if args.obs_output.is_some() {
        validate_m2d_output_format(args.obs_out_format)?;
    }
    Ok(())
}

fn validate_m2d_output_format(format: RecordFormatArg) -> Result<(), CliError> {
    let format = format.required_record_format()?;
    if format == RecordFormat::Ptb64 {
        return Err(CliError::UnsupportedDetectionFormat { format: "ptb64" });
    }
    Ok(())
}

fn validate_detect_observable_routing(args: &DetectArgs) -> Result<(), CliError> {
    let primary_includes_observables =
        args.append_observables || args.out_format == SampleOutFormatArg::Dets;
    if primary_includes_observables && args.obs_output.is_some() {
        return Err(CliError::ConflictingObservableRouting);
    }
    Ok(())
}

fn validate_detect_ptb64_shots(args: &DetectArgs) -> Result<(), CliError> {
    let uses_ptb64 = args.out_format == SampleOutFormatArg::Ptb64
        || (args.obs_output.is_some() && args.obs_out_format == RecordFormatArg::Ptb64);
    if uses_ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }
    Ok(())
}

fn detect_observable_output_mode(args: &DetectArgs) -> DetectionObservableOutputMode {
    if args.append_observables {
        DetectionObservableOutputMode::Append
    } else if args.out_format == SampleOutFormatArg::Dets {
        DetectionObservableOutputMode::Prepend
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
}

fn observable_output_mode(append_observables: bool) -> DetectionObservableOutputMode {
    if append_observables {
        DetectionObservableOutputMode::Append
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::Record(FormatError::new(
        FormatErrorCode::InvalidData,
        message,
        None,
    ))
}

const M2D_MEASUREMENT_KIND: &str = "m2d measurement input";
const M2D_SWEEP_KIND: &str = "m2d sweep input";

/// One m2d record input: the Records stream reader plus the m2d diagnostic vocabulary.
///
/// Decoding, framing, and zero-width sweep handling belong to the reader; this adapter only
/// names the input in diagnostics and keeps Stim-format inputs failing at their first read.
struct M2dInput<'a> {
    reader: Option<RecordStreamReader<Box<dyn Read + 'a>>>,
    input_path: Option<PathBuf>,
    format: RecordFormatArg,
    kind: &'static str,
}

impl<'a> M2dInput<'a> {
    fn new(
        source: Box<dyn Read + 'a>,
        input_path: Option<PathBuf>,
        format: RecordFormatArg,
        (bits_per_record, max_record_bytes): (usize, usize),
        kind: &'static str,
    ) -> Self {
        let reader = format.record_format().map(|record_format| {
            RecordStreamReader::measurements(
                source,
                record_format,
                bits_per_record,
                max_record_bytes,
            )
            .with_framing(RecordFraming::Records)
        });
        Self {
            reader,
            input_path,
            format,
            kind,
        }
    }

    fn from_file(
        file: InputFile,
        format: RecordFormatArg,
        bits_per_record: usize,
        max_record_bytes: usize,
    ) -> Self {
        let input_path = Some(file.path().to_path_buf());
        Self::new(
            Box::new(file),
            input_path,
            format,
            (bits_per_record, max_record_bytes),
            M2D_MEASUREMENT_KIND,
        )
    }

    /// Relabels this input as the sweep stream, whose zero-width `b8` form must be empty and
    /// then supplies an empty record for every measurement record.
    fn into_sweep(mut self) -> Self {
        self.kind = M2D_SWEEP_KIND;
        self.reader = self
            .reader
            .map(RecordStreamReader::with_empty_zero_width_input);
        self
    }

    fn reader(&mut self) -> Result<&mut RecordStreamReader<Box<dyn Read + 'a>>, CliError> {
        self.reader.as_mut().ok_or(CliError::UnsupportedConversion)
    }

    fn read_batch(
        &mut self,
        max_records: usize,
    ) -> Result<Option<PackedShotBatchView<'_>>, CliError> {
        let (format, kind, input_path) = (self.format, self.kind, self.input_path.clone());
        self.reader()?
            .read_batch(max_records)
            .map_err(|error| m2d_read_error(error, format, kind, input_path.as_deref()))
    }

    fn has_more_records(&mut self) -> Result<bool, CliError> {
        let (format, kind, input_path) = (self.format, self.kind, self.input_path.clone());
        self.reader()?
            .has_more_records()
            .map_err(|error| m2d_read_error(error, format, kind, input_path.as_deref()))
    }
}

/// Renders a Records reader failure with m2d's established diagnostics.
fn m2d_read_error(
    error: RecordStreamReadError,
    format: RecordFormatArg,
    kind: &'static str,
    input_path: Option<&std::path::Path>,
) -> CliError {
    match error {
        RecordStreamReadError::Record {
            byte_offset,
            source,
        } => CliError::InputRecord {
            byte_offset,
            source,
        },
        RecordStreamReadError::UnterminatedRecord {
            byte_offset,
            line_len,
            ..
        } => CliError::InputRecord {
            byte_offset,
            source: FormatError::new(
                FormatErrorCode::MissingRecordTerminator,
                format!("{kind} {} record must end with a newline", format.name()),
                ByteSpan::try_new(line_len, 0),
            ),
        },
        RecordStreamReadError::RecordTooLarge { limit } => CliError::InputTooLarge {
            kind,
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
        },
        RecordStreamReadError::ByteOffsetOverflow => CliError::InputByteOffsetOverflow { kind },
        RecordStreamReadError::TruncatedRecord {
            bytes_read,
            record_bytes,
        } => invalid_result_format(format!(
            "{kind} ended after {bytes_read} bytes of a {record_bytes}-byte record"
        )),
        RecordStreamReadError::ZeroWidthRecords if format == RecordFormatArg::Ptb64 => {
            invalid_result_format(format!(
                "{kind} ptb64 input cannot infer a shot count for zero-width records"
            ))
        }
        RecordStreamReadError::ZeroWidthRecords => invalid_result_format(format!(
            "{kind} b8 input cannot represent zero-width records"
        )),
        RecordStreamReadError::NonEmptyZeroWidthInput => {
            invalid_result_format(format!("{kind} b8 zero-width input must be empty"))
        }
        error @ (RecordStreamReadError::Io(_) | RecordStreamReadError::Format(_)) => {
            record_stream_error(error, input_path)
        }
    }
}

enum DeferredDetectOutputs<'a, W>
where
    W: Write,
{
    Pending {
        io: PendingIo,
        stdout: &'a mut W,
    },
    Active {
        primary: OutputSink<'a, W>,
        observable: Option<FileOutputSink>,
    },
    Transition,
}

impl<'a, W> DeferredDetectOutputs<'a, W>
where
    W: Write,
{
    fn new(io: PendingIo, stdout: &'a mut W) -> Self {
        Self::Pending { io, stdout }
    }

    fn activate(
        &mut self,
    ) -> Result<(&mut OutputSink<'a, W>, Option<&mut FileOutputSink>), CliError> {
        if matches!(self, Self::Pending { .. }) {
            let pending = std::mem::replace(self, Self::Transition);
            let Self::Pending { io, stdout } = pending else {
                return Err(CliError::IoPlanInvariant {
                    message: "detect deferred output transition lost pending state",
                });
            };
            let mut outputs = io.activate()?;
            *self = Self::Active {
                primary: OutputSink::from_output(outputs.take(FileRole::Output), stdout),
                observable: outputs
                    .take(FileRole::ObservableOutput)
                    .map(FileOutputSink::from_output),
            };
        }
        let Self::Active {
            primary,
            observable,
        } = self
        else {
            return Err(CliError::IoPlanInvariant {
                message: "detect deferred output activation did not produce active sinks",
            });
        };
        Ok((primary, observable.as_mut()))
    }
}
