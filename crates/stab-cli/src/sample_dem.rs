use std::io::{BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use stab_core::{
    CircuitError, DemSampleBatchView, DemSampleSink, DetectionObservableOutputMode,
    DetectorErrorModel, RandomPolicy, RecordFormat, SampleFormat, Seed, ShotCount,
    advanced::records::for_each_sparse_record,
    advanced::records::{read_measurement_records, validate_ptb64_shot_count},
    execution::{DemSamplingCompiler, DemSamplingRunError},
};

use super::{
    CliError, SampleOutFormatArg,
    batch_output::DemSampleBatchEncoder,
    input::{read_limited_input_file, read_limited_line, read_limited_stdin},
    io_plan::{FileRole, InputFile, PendingIo},
    streaming::{FileOutputSink, OutputSink},
};

const MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES: usize = 1_048_576;
const MAX_SAMPLE_DEM_INPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SampleDemRecordFormatArg {
    #[value(name = "01")]
    ZeroOne,
    #[value(name = "b8")]
    B8,
    #[value(name = "r8")]
    R8,
    #[value(name = "ptb64")]
    Ptb64,
    #[value(name = "hits")]
    Hits,
    #[value(name = "dets")]
    Dets,
}

impl SampleDemRecordFormatArg {
    const fn record_format(self) -> RecordFormat {
        match self {
            Self::ZeroOne => RecordFormat::ZeroOne,
            Self::B8 => RecordFormat::B8,
            Self::R8 => RecordFormat::R8,
            Self::Ptb64 => RecordFormat::Ptb64,
            Self::Hits => RecordFormat::Hits,
            Self::Dets => RecordFormat::Dets,
        }
    }

    fn sample_format(self) -> Result<SampleFormat, CliError> {
        match self {
            Self::ZeroOne => Ok(SampleFormat::ZeroOne),
            Self::B8 => Ok(SampleFormat::B8),
            Self::R8 => Ok(SampleFormat::R8),
            Self::Hits => Ok(SampleFormat::Hits),
            Self::Dets => Ok(SampleFormat::Dets),
            Self::Ptb64 => Err(CliError::UnsupportedDetectionFormat { format: "ptb64" }),
        }
    }
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

    /// Append observable flips after detector-event bits.
    #[arg(long = "append_observables", hide = true)]
    append_observables: bool,

    /// Deprecated Stim alias that writes observable flips before detector bits.
    #[arg(long = "prepend_observables", hide = true)]
    prepend_observables: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: SampleDemRecordFormatArg,

    /// Optional sampled-error output path.
    #[arg(long = "err_out")]
    error_output: Option<PathBuf>,

    /// Sampled-error output format.
    #[arg(long = "err_out_format", value_enum, default_value = "01")]
    err_out_format: SampleDemRecordFormatArg,

    /// Optional sampled-error replay input path.
    #[arg(long = "replay_err_in")]
    replay_error_input: Option<PathBuf>,

    /// Sampled-error replay input format.
    #[arg(long = "replay_err_in_format", value_enum, default_value = "01")]
    replay_err_in_format: SampleDemRecordFormatArg,
}

pub(super) fn run_sample_dem<R, W>(
    args: SampleDemArgs,
    input: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    validate_observable_routing(&args)?;
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
    let input_bytes = if let Some(input_file) = io.input_mut(FileRole::Input) {
        read_limited_input_file(input_file, MAX_SAMPLE_DEM_INPUT_BYTES, "sample_dem input")?
    } else {
        read_limited_stdin(input, MAX_SAMPLE_DEM_INPUT_BYTES, "sample_dem input")?
    };
    let dem = parse_dem_bytes(&input_bytes)?;
    let plan = DemSamplingCompiler::new()
        .compile(&dem)
        .map_err(|error| CliError::from(CircuitError::from(error)))?;
    let shots = dem_shot_count(args.shots)?;
    if let Some(replay_input) = io.input_mut(FileRole::ReplayErrorInput) {
        plan.validate_replay(shots)
            .map_err(|error| CliError::from(CircuitError::from(error)))?;
        validate_replay_prefix(
            replay_input,
            args.replay_err_in_format,
            plan.error_count(),
            args.shots,
        )?;
    }
    let observable_mode = observable_output_mode(&args);
    let encoder = DemSampleBatchEncoder::try_new(
        plan.detector_count(),
        plan.observable_count(),
        plan.error_count(),
        observable_mode,
        args.out_format.record_format(),
        args.obs_output
            .as_ref()
            .map(|_| args.obs_out_format.record_format()),
        args.error_output
            .as_ref()
            .map(|_| args.err_out_format.record_format()),
    )?;
    let mut session = plan
        .session(dem_random_policy(args.seed))
        .map_err(|error| CliError::from(CircuitError::from(error)))?;
    let mut io = io.with_outputs(output_roles)?;
    let mut replay_input = io.take_input(FileRole::ReplayErrorInput);
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
    if let Some(replay_input) = replay_input.as_mut() {
        let mut replay_record = Vec::new();
        replay_record
            .try_reserve_exact(plan.error_count())
            .map_err(|error| {
                invalid_result_format(format!(
                    "sample_dem replay record could not reserve {} bits: {error}",
                    plan.error_count()
                ))
            })?;
        let mut replay = session
            .start_replay(shots, &mut sink)
            .map_err(map_dem_run_error)?;
        for_each_replay_error_record(
            replay_input,
            args.replay_err_in_format,
            plan.error_count(),
            args.shots,
            |error_record| {
                replay_record.clear();
                replay_record.extend_from_slice(error_record);
                replay
                    .write_batch(std::slice::from_ref(&replay_record))
                    .map(|_| ())
                    .map_err(map_dem_run_error)
            },
        )?;
        replay.finish().map(|_| ()).map_err(map_dem_run_error)
    } else if args.error_output.is_some() {
        session
            .run_with_sampled_errors(shots, &mut sink)
            .map(|_| ())
            .map_err(map_dem_run_error)
    } else {
        session
            .run(shots, &mut sink)
            .map(|_| ())
            .map_err(map_dem_run_error)
    }
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
        DemSamplingRunError::Engine { source, .. } => CliError::from(CircuitError::from(source)),
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

fn parse_dem_bytes(input: &[u8]) -> Result<DetectorErrorModel, CliError> {
    Ok(DetectorErrorModel::from_dem_bytes(input)?)
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::from(CircuitError::invalid_result_format(message))
}

fn validate_observable_routing(args: &SampleDemArgs) -> Result<(), CliError> {
    let selected_routes = usize::from(args.prepend_observables)
        + usize::from(args.append_observables)
        + usize::from(args.obs_output.is_some());
    if selected_routes > 1 {
        return Err(CliError::ConflictingObservableRouting);
    }
    Ok(())
}

fn validate_ptb64_routing(args: &SampleDemArgs) -> Result<(), CliError> {
    let uses_ptb64 = args.out_format == SampleOutFormatArg::Ptb64
        || (args.obs_output.is_some() && args.obs_out_format == SampleDemRecordFormatArg::Ptb64)
        || (args.error_output.is_some() && args.err_out_format == SampleDemRecordFormatArg::Ptb64)
        || (args.replay_error_input.is_some()
            && args.replay_err_in_format == SampleDemRecordFormatArg::Ptb64);
    if uses_ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }
    Ok(())
}

fn for_each_replay_error_record<F>(
    input: &mut InputFile,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
    visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    match format {
        SampleDemRecordFormatArg::Ptb64 => {
            for_each_ptb64_replay_error_record(input, error_count, expected_shots, visit)
        }
        SampleDemRecordFormatArg::B8 => {
            for_each_b8_replay_error_record(input, error_count, expected_shots, visit)
        }
        SampleDemRecordFormatArg::R8 => {
            for_each_r8_replay_error_record(input, error_count, expected_shots, visit)
        }
        SampleDemRecordFormatArg::ZeroOne
        | SampleDemRecordFormatArg::Hits
        | SampleDemRecordFormatArg::Dets => {
            for_each_line_replay_error_record(input, format, error_count, expected_shots, visit)
        }
    }
}

fn validate_replay_prefix(
    input: &mut InputFile,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
) -> Result<(), CliError> {
    for_each_replay_error_record(input, format, error_count, expected_shots, |_record| Ok(()))?;
    input.rewind()
}

fn for_each_ptb64_replay_error_record<F>(
    input: &mut InputFile,
    error_count: usize,
    expected_shots: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let path = input.path().to_path_buf();
    if expected_shots == 0 {
        return Ok(());
    }
    if error_count == 0 {
        return Err(invalid_result_format(
            "ptb64 input cannot represent a nonzero number of zero-width records",
        ));
    }
    let bytes_per_group = error_count
        .checked_mul(8)
        .ok_or(CliError::MeasurementCountOverflow)?;
    let expected_bytes = bytes_per_group
        .checked_mul(expected_shots / 64)
        .ok_or(CliError::MeasurementCountOverflow)?;
    let mut group_bytes = vec![0u8; bytes_per_group];
    let mut bytes_read = 0usize;
    for _ in 0..(expected_shots / 64) {
        read_exact_ptb64_replay_group(
            &path,
            input,
            &mut group_bytes,
            &mut bytes_read,
            expected_bytes,
            expected_shots,
            error_count,
        )?;
        let mut group = vec![vec![false; error_count]; 64];
        for (bit_index, word_chunk) in group_bytes.chunks_exact(8).enumerate() {
            let mut word_bytes = [0u8; 8];
            word_bytes.copy_from_slice(word_chunk);
            let word = u64::from_le_bytes(word_bytes);
            for (shot_offset, record) in group.iter_mut().enumerate() {
                if word & (1u64 << shot_offset) != 0 {
                    let bit = record.get_mut(bit_index).ok_or_else(|| {
                        invalid_result_format("ptb64 bit index was out of decoded record bounds")
                    })?;
                    *bit = true;
                }
            }
        }
        for record in &group {
            visit(record)?;
        }
    }
    Ok(())
}

fn read_exact_ptb64_replay_group(
    path: &Path,
    reader: &mut impl Read,
    buffer: &mut [u8],
    bytes_read: &mut usize,
    expected_bytes: usize,
    expected_shots: usize,
    error_count: usize,
) -> Result<(), CliError> {
    let mut offset = 0usize;
    while offset < buffer.len() {
        let remaining = buffer
            .get_mut(offset..)
            .ok_or_else(|| invalid_result_format("ptb64 replay byte cursor was out of range"))?;
        match reader.read(remaining) {
            Ok(0) => {
                return Err(invalid_result_format(format!(
                    "ptb64 input expected at least {expected_bytes} bytes for {expected_shots} records with {error_count} bits each, got {}",
                    *bytes_read
                )));
            }
            Ok(count) => {
                offset += count;
                *bytes_read = bytes_read.saturating_add(count);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(source) => {
                return Err(CliError::ReadPath {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(())
}

fn for_each_b8_replay_error_record<F>(
    input: &mut InputFile,
    error_count: usize,
    expected_shots: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let path = input.path().to_path_buf();
    let bytes_per_record = error_count.div_ceil(8);
    if bytes_per_record == 0 && expected_shots > 0 {
        return Err(invalid_result_format(
            "b8 input cannot represent zero-width records",
        ));
    }
    let mut record_bytes = vec![0u8; bytes_per_record];
    for records_read in 0..expected_shots {
        let mut offset = 0usize;
        while offset < record_bytes.len() {
            let remaining = record_bytes
                .get_mut(offset..)
                .ok_or_else(|| invalid_result_format("b8 replay byte cursor was out of range"))?;
            match input.read(remaining) {
                Ok(0) if offset == 0 => {
                    return Err(CliError::ReplayErrorRecordCountMismatch {
                        expected: expected_shots,
                        actual: records_read,
                    });
                }
                Ok(0) => {
                    return Err(invalid_result_format(format!(
                        "b8 input ended after {offset} bytes of a {bytes_per_record}-byte record"
                    )));
                }
                Ok(count) => offset += count,
                Err(source) => {
                    return Err(CliError::ReadPath {
                        path: path.clone(),
                        source,
                    });
                }
            }
        }
        let records = read_measurement_records(&record_bytes, SampleFormat::B8, error_count)?;
        let [record] = <[Vec<bool>; 1]>::try_from(records).map_err(|records| {
            CircuitError::invalid_result_format(format!(
                "b8 replay record decoded into {} records",
                records.len()
            ))
        })?;
        visit(&record)?;
    }
    Ok(())
}

fn for_each_line_replay_error_record<F>(
    input: &mut InputFile,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let sample_format = format.sample_format()?;
    let path = input.path().to_path_buf();
    let mut reader = BufReader::new(input);
    let mut records_read = 0usize;
    let mut skipped_dets_blank_bytes = 0usize;
    let mut byte_offset = 0usize;
    while records_read < expected_shots {
        let Some(line) = read_limited_line(
            &mut reader,
            Some(&path),
            MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES,
            "sample_dem replay text record",
        )?
        else {
            return Err(CliError::ReplayErrorRecordCountMismatch {
                expected: expected_shots,
                actual: records_read,
            });
        };
        let record_byte_offset = byte_offset;
        byte_offset =
            byte_offset
                .checked_add(line.len())
                .ok_or(CliError::InputByteOffsetOverflow {
                    kind: "sample_dem replay text record",
                })?;
        let parsed = if format == SampleDemRecordFormatArg::Hits {
            vec![
                read_hits_replay_record(&line, error_count).map_err(|error| match error {
                    CliError::Circuit(source) => CliError::InputRecord {
                        byte_offset: record_byte_offset,
                        source,
                    },
                    error => error,
                })?,
            ]
        } else {
            read_measurement_records(&line, sample_format, error_count).map_err(|source| {
                CliError::InputRecord {
                    byte_offset: record_byte_offset,
                    source,
                }
            })?
        };
        if format == SampleDemRecordFormatArg::Dets && parsed.is_empty() {
            skipped_dets_blank_bytes =
                checked_text_replay_scan_bytes(skipped_dets_blank_bytes, line.len())?;
            continue;
        }
        let [record] = <[Vec<bool>; 1]>::try_from(parsed).map_err(|records| {
            CircuitError::invalid_result_format(format!(
                "replay record decoded into {} records",
                records.len()
            ))
        })?;
        visit(&record)?;
        records_read += 1;
        skipped_dets_blank_bytes = 0;
    }
    Ok(())
}

fn read_hits_replay_record(input: &[u8], error_count: usize) -> Result<Vec<bool>, CliError> {
    let mut record = None;
    for_each_sparse_record(input, SampleFormat::Hits, error_count, |hits| {
        if record.is_some() {
            return Err(CircuitError::invalid_result_format(
                "HITS replay line decoded into multiple records",
            ));
        }
        let mut decoded = vec![false; error_count];
        for hit in hits {
            let index = usize::try_from(*hit).map_err(|_| {
                CircuitError::invalid_result_format(format!(
                    "HITS replay index {hit} does not fit usize"
                ))
            })?;
            let bit = decoded.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "HITS replay index {index} exceeds error count {error_count}"
                ))
            })?;
            *bit = true;
        }
        record = Some(decoded);
        Ok(())
    })?;
    record.ok_or_else(|| {
        CliError::from(CircuitError::invalid_result_format(
            "HITS replay line did not contain one record",
        ))
    })
}

fn checked_text_replay_scan_bytes(current: usize, added: usize) -> Result<usize, CliError> {
    let updated = current.saturating_add(added);
    if updated > MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES {
        return Err(CliError::InputTooLarge {
            kind: "sample_dem replay text record",
            limit: u64::try_from(MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES).unwrap_or(u64::MAX),
        });
    }
    Ok(updated)
}

fn for_each_r8_replay_error_record<F>(
    input: &mut InputFile,
    error_count: usize,
    expected_shots: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let path = input.path().to_path_buf();
    for records_read in 0..expected_shots {
        let Some(record) = read_r8_replay_record(&path, input, error_count)? else {
            return Err(CliError::ReplayErrorRecordCountMismatch {
                expected: expected_shots,
                actual: records_read,
            });
        };
        visit(&record)?;
    }
    Ok(())
}

fn read_r8_replay_record(
    path: &Path,
    reader: &mut impl Read,
    bits_per_record: usize,
) -> Result<Option<Vec<bool>>, CliError> {
    let mut record = vec![false; bits_per_record];
    let mut bit_index = 0usize;
    let mut read_any = false;
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) if !read_any => return Ok(None),
            Ok(0) => {
                return Err(CliError::from(CircuitError::invalid_result_format(
                    "r8 input ended before record completed",
                )));
            }
            Ok(_) => {
                read_any = true;
            }
            Err(source) => {
                return Err(CliError::ReadPath {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }

        if byte[0] == u8::MAX {
            bit_index = bit_index.checked_add(usize::from(u8::MAX)).ok_or_else(|| {
                CliError::from(CircuitError::invalid_result_format(
                    "r8 run-length offset overflowed",
                ))
            })?;
            if bit_index > bits_per_record {
                return Err(CliError::from(CircuitError::invalid_result_format(
                    "r8 run-length overshot record width",
                )));
            }
            continue;
        }
        bit_index = bit_index.checked_add(usize::from(byte[0])).ok_or_else(|| {
            CliError::from(CircuitError::invalid_result_format(
                "r8 run-length offset overflowed",
            ))
        })?;
        if bit_index > bits_per_record {
            return Err(CliError::from(CircuitError::invalid_result_format(
                "r8 run-length overshot record width",
            )));
        }
        if bit_index == bits_per_record {
            return Ok(Some(record));
        }
        let Some(bit) = record.get_mut(bit_index) else {
            return Err(CliError::from(CircuitError::invalid_result_format(
                format!("r8 hit index {bit_index} exceeds record width {bits_per_record}"),
            )));
        };
        *bit = true;
        bit_index += 1;
    }
}

fn observable_output_mode(args: &SampleDemArgs) -> DetectionObservableOutputMode {
    if args.append_observables {
        DetectionObservableOutputMode::Append
    } else if args.prepend_observables {
        DetectionObservableOutputMode::Prepend
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
}
