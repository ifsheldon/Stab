use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use stab_engine::{
    DemReplayRecordStore, DemReplayRetention, DemReplaySession, DemReplaySessionOptions,
    DemReplayStoreState, DemReplayTransaction, DemSamplingCompiler, DemSamplingExecutionError,
    DemSamplingPlan, DemSamplingSession, DemSamplingSessionOptions, RandomPolicy, RunError,
    RunStatus, Seed, ShotCount,
};
use stab_records::{
    DemSampleBatchView, DemSampleSink, FormatError, FormatErrorCode as RecordFormatErrorCode,
    HitsDuplicates, RecordFormat, RecordFraming, RecordStreamReadError, RecordStreamReader,
    RecordStreamStorage, validate_ptb64_shot_count,
};

use super::{
    CliError, MemoryBudgetArg, MemoryLedger, RecordFormatArg, SampleOutFormatArg, StageBudget,
    batch_output::{DemSampleBatchEncoder, DetectionObservableOutputMode},
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, InputFile, PendingIo, StandardStreams, StreamUse},
    streaming::{FileOutputSink, OutputSink, flush_before_failure},
};

const REPLAY_TEXT_RECORD_KIND: &str = "sample_dem replay text record";
/// Replay records delivered to the engine per transaction batch.
const REPLAY_BATCH_SHOTS: usize = u64::BITS as usize;
/// A text replay reader's transport buffer may use one part in this many of the budget its fixed
/// storage and the smallest replay session leave.
const TEXT_BUFFER_SHARE: u64 = 4;

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
    let compile = ledger.stage();
    let plan = DemSamplingCompiler {
        memory_budget: compile.get(),
        ..DemSamplingCompiler::default()
    }
    .compile(&dem)
    .map_err(|error| compile.refusal(error))?;
    drop(dem);
    ledger.release(dem_hold);
    let _plan_hold = ledger.hold(plan.retained_bytes());
    let shots = dem_shot_count(args.shots)?;
    let mut replay_reader = None;
    if args.replay_error_input.is_some() {
        plan.validate_replay(shots).map_err(CliError::from)?;
        let replay_format = sample_dem_record_format(args.replay_err_in_format)?;
        let reader = ReplayReaderStorage::admit(&plan, replay_format, ledger.stage())?;
        replay_reader = Some((reader, ledger.hold(reader.bytes)));
    }
    let session_stage = ledger.stage();
    let replay_options = DemReplaySessionOptions {
        memory_budget: Some(session_stage.get()),
        ..DemReplaySessionOptions::default()
    };
    let record_bytes = replay_reader
        .as_ref()
        .map_or(0, |(reader, _)| reader.text_record_bytes);
    // Records the validation pass keeps, so the retained prefix of a replay is decoded once and
    // delivery decodes only the records after it; fixed-width binary files whose length already
    // proves `expected_shots` stream directly.
    let mut retained = None;
    let mut delivery = ReplayDelivery::FromInput(ReplayResume::START);
    if let Some(replay_input) = io.input_mut(FileRole::ReplayErrorInput) {
        let replay_format = sample_dem_record_format(args.replay_err_in_format)?;
        if !preflight_fixed_width_replay_file(
            replay_input,
            replay_format,
            plan.error_count(),
            args.shots,
        )? {
            let mut store = plan.replay_record_store(&replay_options, shots);
            delivery = validate_replay_input(
                replay_input,
                args.replay_err_in_format,
                plan.error_count(),
                args.shots,
                record_bytes,
                &mut store,
            )?;
            retained = Some(store).filter(|store| store.shot_count() > 0);
        }
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
                .replay_session(&replay_options, shots)
                .map_err(|error| session_stage.refusal(error))?,
        }
    } else {
        SampleDemExecution::Sample(
            plan.session(&DemSamplingSessionOptions {
                random: dem_random_policy(args.seed),
                memory_budget: Some(session_stage.get()),
                ..DemSamplingSessionOptions::default()
            })
            .map_err(|error| session_stage.refusal(error))?,
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
            let mut status = RunStatus::Completed;
            if let Some(retained) = &retained {
                status = transaction
                    .write_retained(retained)
                    .map_err(map_dem_run_error)?;
            }
            // A store that kept every record is the whole replay: the input is never read again,
            // which is what lets a pipe replay without a rewind.
            if let ReplayDelivery::FromInput(resume) = delivery
                && status.is_completed()
                && resume.records < args.shots
            {
                ReplayInput::new(
                    input,
                    args.replay_err_in_format,
                    plan.error_count(),
                    args.shots,
                    record_bytes,
                )?
                .resuming_at(resume)
                .deliver_to(&mut transaction)?;
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

/// The replay reader's storage, admitted against the command budget before the reader exists.
#[derive(Clone, Copy, Debug)]
struct ReplayReaderStorage {
    /// The per-record cap of a line-framed text replay input; packed formats ignore it.
    text_record_bytes: usize,
    /// The reader's batch, PTB64 group, first `r8` record, and transport buffer, with the text
    /// transport buffer bounded by `text_record_bytes`.
    bytes: u64,
}

impl ReplayReaderStorage {
    /// Admits the reader's fixed storage for 64-record batches beside the smallest replay
    /// session, then gives a text reader's transport buffer a quarter of what both leave.
    ///
    /// The buffer grows only to the longest line framed, which for realistic records is far
    /// below the batch, so the rest stays for the session's batch shape and the records a
    /// validation pass retains instead of being reserved for lines that rarely occur.
    ///
    /// A bound beyond `u64` saturates, which only ever refuses.
    fn admit(
        plan: &DemSamplingPlan,
        format: RecordFormat,
        stage: StageBudget,
    ) -> Result<Self, CliError> {
        let options = DemReplaySessionOptions {
            memory_budget: Some(stage.get()),
            ..DemReplaySessionOptions::default()
        };
        let saturating = |bytes: u128| u64::try_from(bytes).unwrap_or(u64::MAX);
        // PTB64 replays read whole groups as bit planes and never request a shot-major batch.
        let batch_records = if format == RecordFormat::Ptb64 {
            1
        } else {
            REPLAY_BATCH_SHOTS
        };
        let fixed =
            saturating(RecordStreamStorage::new(format, plan.error_count(), batch_records).bytes());
        let free = plan
            .admit_replay_input(&options, fixed)
            .map_err(|error| stage.refusal(error))?;
        if matches!(
            format,
            RecordFormat::B8 | RecordFormat::R8 | RecordFormat::Ptb64
        ) {
            return Ok(Self {
                text_record_bytes: 0,
                bytes: fixed,
            });
        }
        // Every other format is line-framed text, whose transport buffer follows its cap.
        let text_record_bytes = RecordStreamStorage::max_text_record_bytes(u128::from(
            free.bytes() / TEXT_BUFFER_SHARE,
        ));
        // Admitting the whole reader again also refuses the one-byte buffer of a zero cap when
        // nothing is left for it.
        let bytes = fixed.saturating_add(saturating(RecordStreamStorage::text_transport_bytes(
            text_record_bytes,
        )));
        plan.admit_replay_input(&options, bytes)
            .map_err(|error| stage.refusal(error))?;
        Ok(Self {
            text_record_bytes,
            bytes,
        })
    }
}

fn map_dem_run_error(error: RunError<DemSamplingExecutionError, CliError>) -> CliError {
    match error {
        RunError::Engine { source, .. } => CliError::from(source),
        RunError::Sink { source, .. } => source,
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

/// Proves in O(1) metadata time that a regular `b8` or `ptb64` file holds at least
/// `expected_shots` records of `error_count` bits, since neither format has data-dependent
/// grammar errors inside a complete frame.
///
/// Returns `Ok(false)` for variable-width formats, non-regular files, and files shorter than
/// `expected_bytes` (including virtual files whose reported size is zero), so the streaming
/// validator handles them and owns every shortfall diagnostic.
fn preflight_fixed_width_replay_file(
    input: &InputFile,
    format: RecordFormat,
    error_count: usize,
    expected_shots: usize,
) -> Result<bool, CliError> {
    let expected_bytes = match format {
        RecordFormat::Ptb64 => {
            if error_count == 0 {
                return Err(invalid_result_format(
                    "ptb64 input cannot represent a nonzero number of zero-width records",
                ));
            }
            ptb64_expected_bytes(error_count, expected_shots)?
        }
        RecordFormat::B8 => {
            if error_count == 0 {
                return Err(invalid_result_format(
                    "b8 input cannot represent zero-width records",
                ));
            }
            error_count
                .div_ceil(8)
                .checked_mul(expected_shots)
                .ok_or(CliError::MeasurementCountOverflow)?
        }
        _ => return Ok(false),
    };
    let expected_bytes_u64 =
        u64::try_from(expected_bytes).map_err(|_| CliError::MeasurementCountOverflow)?;
    Ok(input
        .regular_file_len()?
        .is_some_and(|len| len >= expected_bytes_u64))
}

/// Where delivery resumes after the records a validation pass retained.
#[derive(Clone, Copy, Debug)]
struct ReplayResume {
    /// Records the store holds, which are every record before `byte_offset`.
    records: usize,
    /// Input offset of the first record the store does not hold, a record boundary.
    byte_offset: usize,
}

impl ReplayResume {
    const START: Self = Self {
        records: 0,
        byte_offset: 0,
    };
}

/// Where the replay records that delivery hands the session come from.
#[derive(Clone, Copy, Debug)]
enum ReplayDelivery {
    /// The store, when it holds any, and then the input from `resume`, which the input has
    /// already been positioned at.
    FromInput(ReplayResume),
    /// The store alone: it kept every expected record, so the input is never read again.
    StoreOnly,
}

/// What a validation pass decoded and kept.
#[derive(Clone, Copy, Debug)]
enum ReplayRetained {
    /// The store holds every expected record.
    All,
    /// The store declined records from `resume.records` on, so delivery decodes them again from
    /// the input.
    Prefix(ReplayResume),
}

/// Checks that the replay file holds `expected_shots` well-formed records before any output
/// file is activated, so a malformed replay produces no partial output, and says where delivery
/// takes its records from.
///
/// This pass decodes into the reader's reusable batch and hands each batch to `retained`, which
/// keeps dense and sparse records while they fit the budget the session leaves. When it keeps
/// every record, delivery reads the store alone and the input is never touched again, so a pipe
/// replays as well as a file. Once a batch does not fit, the store keeps the whole batches before
/// it, the remaining records are decoded and discarded, and the input is seeked back to the first
/// record the store does not hold, so delivery replays the kept prefix from the store and decodes
/// only the records after it again. A non-seekable input fails that seek here, before any output
/// exists, with [`CliError::ReplayInputNotRewindable`].
fn validate_replay_input(
    input: &mut InputFile,
    format: RecordFormatArg,
    error_count: usize,
    expected_shots: usize,
    max_record_bytes: usize,
    retained: &mut DemReplayRecordStore,
) -> Result<ReplayDelivery, CliError> {
    let kept = {
        let mut replay = ReplayInput::new(
            &mut *input,
            format,
            error_count,
            expected_shots,
            max_record_bytes,
        )?;
        replay.retain_or_skip_all(retained)?
    };
    let resume = match kept {
        ReplayRetained::All => return Ok(ReplayDelivery::StoreOnly),
        ReplayRetained::Prefix(resume) => resume,
    };
    let offset =
        u64::try_from(resume.byte_offset).map_err(|_| CliError::InputByteOffsetOverflow {
            kind: REPLAY_TEXT_RECORD_KIND,
        })?;
    input.seek_to(offset).map_err(|error| match error {
        CliError::ReadPath { path, source } => CliError::ReplayInputNotRewindable {
            path,
            retained_records: resume.records,
            expected_records: expected_shots,
            source,
        },
        error => error,
    })?;
    Ok(ReplayDelivery::FromInput(resume))
}

/// The sampled-error replay stream: exactly `expected_shots` records from the Records reader,
/// rendered with sample_dem's replay diagnostics. Input beyond those records is ignored.
struct ReplayInput<'a> {
    reader: RecordStreamReader<Box<dyn Read + 'a>>,
    path: PathBuf,
    format: RecordFormat,
    error_count: usize,
    expected_shots: usize,
    /// Records before the transport's first byte, which another source delivers.
    first_record: usize,
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
            first_record: 0,
        })
    }

    /// Continues a replay whose transport was positioned at `resume.byte_offset` after its first
    /// `resume.records` records were delivered from a store, so record counts and diagnostic
    /// offsets stay those of the whole input.
    fn resuming_at(mut self, resume: ReplayResume) -> Self {
        self.reader = self.reader.starting_at_offset(resume.byte_offset);
        self.first_record = resume.records;
        self
    }

    /// Delivers every expected record to `transaction` in batches of at most 64 records, reading
    /// PTB64 groups directly as bit planes without a shot-major transpose.
    fn deliver_to<Sink>(
        &mut self,
        transaction: &mut DemReplayTransaction<'_, '_, Sink>,
    ) -> Result<(), CliError>
    where
        Sink: DemSampleSink<Error = CliError>,
    {
        let mut delivered = self.first_record;
        if self.format == RecordFormat::Ptb64 {
            while delivered < self.expected_shots {
                match self.reader.read_ptb64_group() {
                    Ok(Some(planes)) => {
                        delivered += planes.shot_count();
                        transaction
                            .write_bit_plane_batch(planes)
                            .map(|_| ())
                            .map_err(map_dem_run_error)?;
                    }
                    Ok(None) => return Err(self.shortfall_error(delivered)),
                    Err(error) => return Err(self.read_error(error)),
                }
            }
            return Ok(());
        }
        while delivered < self.expected_shots {
            let wanted = (self.expected_shots - delivered).min(REPLAY_BATCH_SHOTS);
            match self.reader.read_batch(wanted) {
                Ok(Some(batch)) => {
                    delivered += batch.shot_count();
                    transaction
                        .write_packed_batch(batch)
                        .map(|_| ())
                        .map_err(map_dem_run_error)?;
                }
                Ok(None) => return Err(self.shortfall_error(delivered)),
                Err(error) => return Err(self.read_error(error)),
            }
        }
        Ok(())
    }

    /// Decodes every expected record, keeping whole batches in `retained` while it admits them
    /// and discarding the rest, and reports whether it kept them all or where delivery resumes
    /// after the kept prefix.
    fn retain_or_skip_all(
        &mut self,
        retained: &mut DemReplayRecordStore,
    ) -> Result<ReplayRetained, CliError> {
        let mut decoded = 0_usize;
        let mut declined = None;
        while decoded < self.expected_shots && retained.state() == DemReplayStoreState::Retaining {
            let byte_offset = self
                .reader
                .consumed_offset()
                .map_err(|error| self.read_error(error))?;
            let (shots, retention) = if self.format == RecordFormat::Ptb64 {
                match self.reader.read_ptb64_group() {
                    Ok(Some(planes)) => (
                        planes.shot_count(),
                        retained
                            .try_retain_bit_planes(planes)
                            .map_err(CliError::from)?,
                    ),
                    Ok(None) => return Err(self.shortfall_error(decoded)),
                    Err(error) => return Err(self.read_error(error)),
                }
            } else {
                let wanted = (self.expected_shots - decoded).min(REPLAY_BATCH_SHOTS);
                match self.reader.read_batch(wanted) {
                    Ok(Some(batch)) => (
                        batch.shot_count(),
                        retained.try_retain(batch).map_err(CliError::from)?,
                    ),
                    Ok(None) => return Err(self.shortfall_error(decoded)),
                    Err(error) => return Err(self.read_error(error)),
                }
            };
            if retention != DemReplayRetention::Retained {
                declined = Some(ReplayResume {
                    records: decoded,
                    byte_offset,
                });
            }
            decoded += shots;
        }
        let resume = match declined {
            Some(resume) => resume,
            None => ReplayResume {
                records: decoded,
                byte_offset: self
                    .reader
                    .consumed_offset()
                    .map_err(|error| self.read_error(error))?,
            },
        };
        if u64::try_from(resume.records).ok() != Some(retained.shot_count()) {
            return Err(CliError::IoPlanInvariant {
                message: "the replay record store disagreed with the records it was offered",
            });
        }
        let skipped = self
            .reader
            .skip_records(self.expected_shots - decoded)
            .map_err(|error| self.read_error(error))?;
        if decoded + skipped < self.expected_shots {
            return Err(self.shortfall_error(decoded + skipped));
        }
        if resume.records >= self.expected_shots {
            return Ok(ReplayRetained::All);
        }
        Ok(ReplayRetained::Prefix(resume))
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
