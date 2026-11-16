use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use stab_engine::{
    DemReplayRecordStore, DemReplaySession, DemSamplerLimits, DemSamplingCompiler,
    DemSamplingRunError, DemSamplingSession, RandomPolicy, Seed, ShotCount,
};
use stab_records::{
    DemSampleBatchView, DemSampleSink, FormatError, FormatErrorCode as RecordFormatErrorCode,
    HitsDuplicates, RecordFormat, RecordFraming, RecordStreamReadError, RecordStreamReader,
    validate_ptb64_shot_count,
};

use super::{
    CliError, MemoryBudgetArg, MemoryLedger, RecordFormatArg, SampleOutFormatArg,
    batch_output::{DemSampleBatchEncoder, DetectionObservableOutputMode},
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, InputFile, PendingIo, StandardStreams, StreamUse},
    streaming::{FileOutputSink, OutputSink, flush_before_failure},
};

const REPLAY_TEXT_RECORD_KIND: &str = "sample_dem replay text record";
/// Replay records delivered to the engine per transaction batch.
const REPLAY_BATCH_SHOTS: usize = u64::BITS as usize;

enum SampleDemExecution {
    Replay {
        input: InputFile,
        session: DemReplaySession,
    },
    Sample(DemSamplingSession),
}

#[derive(Debug, Args)]
pub(super) struct SampleDemArgs {
    /// Number of shots to sample.
    #[arg(long, default_value_t = 1, value_parser = super::parse_stim_usize)]
    shots: usize,

    /// Input detector error model path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detection-event path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Output detection-event format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: SampleOutFormatArg,

    /// Partially deterministic random seed for noisy DEM sampling.
    #[arg(long, value_parser = super::parse_stim_u64)]
    seed: Option<u64>,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(
        long = "obs_out_format",
        value_parser = super::result_record_format_parser(),
        default_value = "01"
    )]
    obs_out_format: RecordFormatArg,

    /// Optional sampled-error output path.
    #[arg(long = "err_out")]
    error_output: Option<PathBuf>,

    /// Sampled-error output format.
    #[arg(
        long = "err_out_format",
        value_parser = super::result_record_format_parser(),
        default_value = "01"
    )]
    err_out_format: RecordFormatArg,

    /// Optional sampled-error replay input path.
    #[arg(long = "replay_err_in")]
    replay_error_input: Option<PathBuf>,

    /// Sampled-error replay input format.
    #[arg(
        long = "replay_err_in_format",
        value_parser = super::result_record_format_parser(),
        default_value = "01"
    )]
    replay_err_in_format: RecordFormatArg,

    #[command(flatten)]
    memory: MemoryBudgetArg,
}

pub(super) fn run_sample_dem<R, W>(
    args: SampleDemArgs,
    input: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    validate_ptb64_routing(&args)?;
    let input_roles = [
        (FileRole::Input, args.input.as_deref()),
        (
            FileRole::ReplayErrorInput,
            args.replay_error_input.as_deref(),
        ),
    ];
    let output_roles = [
        (FileRole::Output, args.output.as_deref()),
        (FileRole::ObservableOutput, args.obs_output.as_deref()),
        (FileRole::ErrorOutput, args.error_output.as_deref()),
    ];
    PendingIo::reject_aliases_without_opening(input_roles, output_roles)?;
    let mut io = PendingIo::preflight_inputs(input_roles)?;
    if args.shots == 0 {
        let io = io.with_outputs(output_roles)?;
        io.reject_standard_stream_aliases(
            streams,
            StreamUse {
                reads_stdin: false,
                writes_stdout: args.output.is_none(),
            },
        )?;
        let mut outputs = io.activate()?;
        let mut primary_output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
        primary_output.write_with(|writer| writer.write_all(&[]))?;
        for role in [FileRole::ObservableOutput, FileRole::ErrorOutput] {
            if let Some(output) = outputs.take(role) {
                let mut output = FileOutputSink::from_output(output);
                output.write_with(|writer| writer.write_all(&[]))?;
            }
        }
        return Ok(());
    }
    let mut ledger = MemoryLedger::new(args.memory);
    let input_cap = ledger.stage().input_bytes();
    let input_bytes = if let Some(input_file) = io.input_mut(FileRole::Input) {
        read_limited_input_file(input_file, input_cap, "sample_dem input")?
    } else {
        read_limited_stdin(input, input_cap, "sample_dem input")?
    };
    let (dem, dem_hold) = ledger.parse_dem(input_bytes)?;
    // DEM compilation admits nothing and reports no size, so the plan holds no ledger bytes.
    let plan = DemSamplingCompiler::new()
        .compile(&dem)
        .map_err(CliError::from)?;
    drop(dem);
    ledger.release(dem_hold);
    let session_stage = ledger.stage();
    let limits = DemSamplerLimits::new(session_stage.get());
    let record_bytes = session_stage.record_bytes();
    let shots = dem_shot_count(args.shots)?;
    // Records the validation pass keeps, so a replay that fits the budget is decoded once.
    let mut retained = None;
    if let Some(replay_input) = io.input_mut(FileRole::ReplayErrorInput) {
        plan.validate_replay(shots).map_err(CliError::from)?;
        let mut store = plan.replay_record_store(&limits, shots);
        validate_replay_input(
            replay_input,
            args.replay_err_in_format,
            plan.error_count(),
            args.shots,
            record_bytes,
            &mut store,
        )?;
        retained = Some(store).filter(DemReplayRecordStore::is_retaining);
    }
    let encoder = DemSampleBatchEncoder::try_new(
        plan.detector_count(),
        plan.observable_count(),
        plan.error_count(),
        DetectionObservableOutputMode::DetectorsOnly,
        args.out_format.record_format(),
        args.obs_output
            .as_ref()
            .map(|_| sample_dem_record_format(args.obs_out_format))
            .transpose()?,
        args.error_output
            .as_ref()
            .map(|_| sample_dem_record_format(args.err_out_format))
            .transpose()?,
    )?;
    let mut io = io.with_outputs(output_roles)?;
    io.reject_standard_stream_aliases(
        streams,
        StreamUse {
            reads_stdin: args.input.is_none(),
            writes_stdout: args.output.is_none(),
        },
    )?;
    let mut execution = if let Some(input) = io.take_input(FileRole::ReplayErrorInput) {
        SampleDemExecution::Replay {
            input,
            session: plan
                .replay_session_with_limits(shots, limits)
                .map_err(CliError::from)?,
        }
    } else {
        SampleDemExecution::Sample(
            plan.session_with_limits(dem_random_policy(args.seed), limits)
                .map_err(CliError::from)?,
        )
    };
    let mut outputs = io.activate()?;
    let primary = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    let observable = outputs
        .take(FileRole::ObservableOutput)
        .map(FileOutputSink::from_output);
    let sampled_errors = outputs
        .take(FileRole::ErrorOutput)
        .map(FileOutputSink::from_output);
    let mut sink = SampleDemBatchSink {
        primary,
        observable,
        sampled_errors,
        encoder,
    };
    let result = match &mut execution {
        SampleDemExecution::Replay { input, session } => (|| {
            let mut transaction = session
                .start_transaction(&mut sink)
                .map_err(map_dem_run_error)?;
            if let Some(retained) = &retained {
                transaction
                    .write_retained(retained)
                    .map_err(map_dem_run_error)?;
            } else {
                ReplayInput::new(
                    input,
                    args.replay_err_in_format,
                    plan.error_count(),
                    args.shots,
                    record_bytes,
                )?
                .for_each_batch(|batch| {
                    transaction
                        .write_packed_batch(batch)
                        .map(|_| ())
                        .map_err(map_dem_run_error)
                })?;
            }
            transaction.finish().map(|_| ()).map_err(map_dem_run_error)
        })(),
        SampleDemExecution::Sample(session) if args.error_output.is_some() => session
            .run_with_sampled_errors(shots, &mut sink)
            .map(|_| ())
            .map_err(map_dem_run_error),
        SampleDemExecution::Sample(session) => session
            .run(shots, &mut sink)
            .map(|_| ())
            .map_err(map_dem_run_error),
    };
    flush_before_failure(result, || sink.flush_outputs())
}

struct SampleDemBatchSink<'a, W>
where
    W: Write,
{
    primary: OutputSink<'a, W>,
    observable: Option<FileOutputSink>,
    sampled_errors: Option<FileOutputSink>,
    encoder: DemSampleBatchEncoder,
}

impl<W> SampleDemBatchSink<'_, W>
where
    W: Write,
{
    fn flush_outputs(&mut self) -> Result<(), CliError> {
        self.primary.flush()?;
        for output in [self.observable.as_mut(), self.sampled_errors.as_mut()]
            .into_iter()
            .flatten()
        {
            output.flush()?;
        }
        Ok(())
    }
}

impl<W> DemSampleSink for SampleDemBatchSink<'_, W>
where
    W: Write,
{
    type Error = CliError;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.encoder.write_batch(
            batch,
            &mut self.primary,
            self.observable.as_mut(),
            self.sampled_errors.as_mut(),
        )
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.encoder.finish(
            &mut self.primary,
            self.observable.as_mut(),
            self.sampled_errors.as_mut(),
        )
    }
}

fn map_dem_run_error(error: DemSamplingRunError<CliError>) -> CliError {
    match error {
        DemSamplingRunError::Engine { source, .. } => CliError::from(source),
        DemSamplingRunError::Sink { source, .. } => source,
    }
}

fn dem_shot_count(shots: usize) -> Result<ShotCount, CliError> {
    u64::try_from(shots)
        .map(ShotCount::new)
        .map_err(|_| CliError::MeasurementCountOverflow)
}

fn dem_random_policy(seed: Option<u64>) -> RandomPolicy {
    match seed {
        Some(seed) => RandomPolicy::Seeded(Seed::new(seed)),
        None => RandomPolicy::Entropy,
    }
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::Record(FormatError::new(
        RecordFormatErrorCode::InvalidData,
        message,
        None,
    ))
}

fn sample_dem_record_format(format: RecordFormatArg) -> Result<RecordFormat, CliError> {
    format
        .record_format()
        .ok_or(CliError::UnsupportedDetectionFormat { format: "stim" })
}

fn validate_ptb64_routing(args: &SampleDemArgs) -> Result<(), CliError> {
    let uses_ptb64 = args.out_format == SampleOutFormatArg::Ptb64
        || (args.obs_output.is_some() && args.obs_out_format == RecordFormatArg::Ptb64)
        || (args.error_output.is_some() && args.err_out_format == RecordFormatArg::Ptb64)
        || (args.replay_error_input.is_some()
            && args.replay_err_in_format == RecordFormatArg::Ptb64);
    if uses_ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }
    Ok(())
}

/// Checks that the replay file holds `expected_shots` well-formed records before any output
/// file is activated, so a malformed replay produces no partial output, then rewinds it.
///
/// This pass decodes into the reader's reusable batch and hands each batch to `retained`, which
/// keeps the records while the whole replay fits the budget the session leaves and they are
/// dense enough to be worth keeping; once it releases, the remaining records are decoded and
/// discarded, and the replay is decoded again for delivery.
fn validate_replay_input(
    input: &mut InputFile,
    format: RecordFormatArg,
    error_count: usize,
    expected_shots: usize,
    max_record_bytes: usize,
    retained: &mut DemReplayRecordStore,
) -> Result<(), CliError> {
    {
        let mut replay = ReplayInput::new(
            &mut *input,
            format,
            error_count,
            expected_shots,
            max_record_bytes,
        )?;
        replay.retain_or_skip_all(retained)?;
    }
    input.rewind()
}

/// The sampled-error replay stream: exactly `expected_shots` records from the Records reader,
/// rendered with sample_dem's replay diagnostics. Input beyond those records is ignored.
struct ReplayInput<'a> {
    reader: RecordStreamReader<Box<dyn Read + 'a>>,
    path: PathBuf,
    format: RecordFormat,
    error_count: usize,
    expected_shots: usize,
}

impl<'a> ReplayInput<'a> {
    fn new(
        input: &'a mut InputFile,
        format: RecordFormatArg,
        error_count: usize,
        expected_shots: usize,
        max_record_bytes: usize,
    ) -> Result<Self, CliError> {
        let format = sample_dem_record_format(format)?;
        if format == RecordFormat::Ptb64 && error_count > 0 {
            ptb64_expected_bytes(error_count, expected_shots)?;
        }
        let path = input.path().to_path_buf();
        let source: Box<dyn Read + 'a> = Box::new(input);
        let reader =
            RecordStreamReader::measurements(source, format, error_count, max_record_bytes)
                .with_framing(RecordFraming::Records)
                .with_hits_duplicates(HitsDuplicates::Set)
                .with_blank_dets_line_budget();
        Ok(Self {
            reader,
            path,
            format,
            error_count,
            expected_shots,
        })
    }

    /// Hands every expected record to `write` in batches of at most 64 records.
    fn for_each_batch(
        &mut self,
        mut write: impl FnMut(stab_records::PackedShotBatchView<'_>) -> Result<(), CliError>,
    ) -> Result<(), CliError> {
        let mut delivered = 0_usize;
        while delivered < self.expected_shots {
            let wanted = (self.expected_shots - delivered).min(REPLAY_BATCH_SHOTS);
            match self.reader.read_batch(wanted) {
                Ok(Some(batch)) => {
                    delivered += batch.shot_count();
                    write(batch)?;
                }
                Ok(None) => return Err(self.shortfall_error(delivered)),
                Err(error) => return Err(self.read_error(error)),
            }
        }
        Ok(())
    }

    /// Decodes every expected record, keeping them in `retained` while it admits them and
    /// discarding the rest once it releases.
    fn retain_or_skip_all(&mut self, retained: &mut DemReplayRecordStore) -> Result<(), CliError> {
        let mut decoded = 0_usize;
        while decoded < self.expected_shots && retained.is_retaining() {
            let wanted = (self.expected_shots - decoded).min(REPLAY_BATCH_SHOTS);
            match self.reader.read_batch(wanted) {
                Ok(Some(batch)) => {
                    decoded += batch.shot_count();
                    retained.try_retain(batch).map_err(CliError::from)?;
                }
                Ok(None) => return Err(self.shortfall_error(decoded)),
                Err(error) => return Err(self.read_error(error)),
            }
        }
        let skipped = self
            .reader
            .skip_records(self.expected_shots - decoded)
            .map_err(|error| self.read_error(error))?;
        if decoded + skipped < self.expected_shots {
            return Err(self.shortfall_error(decoded + skipped));
        }
        Ok(())
    }

    fn shortfall_error(&self, actual: usize) -> CliError {
        if self.format == RecordFormat::Ptb64 {
            return self.ptb64_truncated_error();
        }
        CliError::ReplayErrorRecordCountMismatch {
            expected: self.expected_shots,
            actual,
        }
    }

    fn ptb64_truncated_error(&self) -> CliError {
        match ptb64_expected_bytes(self.error_count, self.expected_shots) {
            Ok(expected_bytes) => invalid_result_format(format!(
                "ptb64 input expected at least {expected_bytes} bytes for {} records with {} bits each, got {}",
                self.expected_shots,
                self.error_count,
                self.reader.bytes_read()
            )),
            Err(error) => error,
        }
    }

    fn read_error(&self, error: RecordStreamReadError) -> CliError {
        replay_read_error(error, self.format, &self.path, || {
            self.ptb64_truncated_error()
        })
    }
}

/// Bytes of the PTB64 groups that hold `expected_shots` records of `error_count` bits.
fn ptb64_expected_bytes(error_count: usize, expected_shots: usize) -> Result<usize, CliError> {
    error_count
        .checked_mul(8)
        .and_then(|bytes_per_group| bytes_per_group.checked_mul(expected_shots / 64))
        .ok_or(CliError::MeasurementCountOverflow)
}

/// Renders a Records reader failure with sample_dem's established replay diagnostics.
fn replay_read_error(
    error: RecordStreamReadError,
    format: RecordFormat,
    path: &Path,
    ptb64_truncated: impl FnOnce() -> CliError,
) -> CliError {
    match error {
        RecordStreamReadError::Io(source) => CliError::ReadPath {
            path: path.to_path_buf(),
            source,
        },
        RecordStreamReadError::Format(error)
            if format == RecordFormat::Ptb64
                && error.code() == RecordFormatErrorCode::InvalidPackedLength =>
        {
            // A trailing partial group keeps the replay contract's byte-count diagnostic.
            ptb64_truncated()
        }
        RecordStreamReadError::Record {
            byte_offset,
            source,
        }
        | RecordStreamReadError::UnterminatedRecord {
            byte_offset,
            source,
            ..
        } => CliError::InputRecord {
            byte_offset,
            source,
        },
        RecordStreamReadError::RecordTooLarge { limit } => CliError::InputTooLarge {
            kind: REPLAY_TEXT_RECORD_KIND,
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
        },
        RecordStreamReadError::ByteOffsetOverflow => CliError::InputByteOffsetOverflow {
            kind: REPLAY_TEXT_RECORD_KIND,
        },
        RecordStreamReadError::TruncatedRecord {
            bytes_read,
            record_bytes,
        } => invalid_result_format(format!(
            "b8 input ended after {bytes_read} bytes of a {record_bytes}-byte record"
        )),
        RecordStreamReadError::ZeroWidthRecords if format == RecordFormat::Ptb64 => {
            invalid_result_format(
                "ptb64 input cannot represent a nonzero number of zero-width records",
            )
        }
        RecordStreamReadError::ZeroWidthRecords => {
            invalid_result_format("b8 input cannot represent zero-width records")
        }
        error @ (RecordStreamReadError::Format(_)
        | RecordStreamReadError::NonEmptyZeroWidthInput) => record_stream_error(error, Some(path)),
    }
}
