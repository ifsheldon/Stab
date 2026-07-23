use std::io::{BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use stab_core::{
    CircuitError, CompiledDemSampler, DetectionEventRecord, DetectionObservableOutputMode,
    DetectorErrorModel, SampleFormat,
    result_formats::{read_measurement_records, validate_ptb64_shot_count},
    result_streaming::for_each_sparse_record,
};

use super::{
    CliError, SampleOutFormatArg,
    input::{read_limited_input_file, read_limited_line, read_limited_stdin},
    io_plan::{FileRole, InputFile, PendingIo},
    streaming::{
        FileOutputSink, OutputSink, detection_record_bits, write_bits_record,
        write_detection_record, write_observable_record, write_ptb64_group,
    },
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
    #[arg(long = "append_observables")]
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
    let mut io = PendingIo::preflight(
        [
            (FileRole::Input, args.input.as_deref()),
            (
                FileRole::ReplayErrorInput,
                args.replay_error_input.as_deref(),
            ),
        ],
        [
            (FileRole::Output, args.output.as_deref()),
            (FileRole::ObservableOutput, args.obs_output.as_deref()),
            (FileRole::ErrorOutput, args.error_output.as_deref()),
        ],
    )?;
    if args.shots == 0 {
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
    let input_bytes = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(
            &mut input_file,
            MAX_SAMPLE_DEM_INPUT_BYTES,
            "sample_dem input",
        )?
    } else {
        read_limited_stdin(input, MAX_SAMPLE_DEM_INPUT_BYTES, "sample_dem input")?
    };
    let dem = parse_dem_bytes(&input_bytes)?;
    let sampler = CompiledDemSampler::compile(&dem)?;
    let mut replay_input = io.take_input(FileRole::ReplayErrorInput);
    if let Some(replay_input) = replay_input.as_mut() {
        validate_replay_prefix(
            replay_input,
            args.replay_err_in_format,
            sampler.error_count(),
            args.shots,
        )?;
    }
    let observable_mode = observable_output_mode(&args);
    let stream_formats = SampleDemStreamFormats::from_args(&args, observable_mode);
    let mut outputs = io.activate()?;
    let mut primary_output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    let mut observable_output = outputs
        .take(FileRole::ObservableOutput)
        .map(FileOutputSink::from_output);
    let mut error_output = outputs
        .take(FileRole::ErrorOutput)
        .map(FileOutputSink::from_output);
    let mut stream_state = SampleDemStreamState::default();
    if let Some(replay_input) = replay_input.as_mut() {
        for_each_replay_error_record(
            replay_input,
            args.replay_err_in_format,
            sampler.error_count(),
            args.shots,
            |error_record| {
                sampler.try_for_each_detection_event_from_error_records(
                    std::iter::once(error_record),
                    |record, replayed_error_record| {
                        write_sample_dem_stream_record(
                            record,
                            Some(replayed_error_record),
                            stream_formats,
                            &mut primary_output,
                            observable_output.as_mut(),
                            error_output.as_mut(),
                            &mut stream_state,
                        )
                    },
                )
            },
        )?;
    } else if args.error_output.is_some() {
        sampler.try_for_each_detection_event_and_error_with_seed(
            args.shots,
            args.seed,
            |record, error_record| {
                write_sample_dem_stream_record(
                    record,
                    Some(error_record),
                    stream_formats,
                    &mut primary_output,
                    observable_output.as_mut(),
                    error_output.as_mut(),
                    &mut stream_state,
                )
            },
        )?;
    } else {
        sampler.try_for_each_detection_event_with_seed(args.shots, args.seed, |record| {
            write_sample_dem_stream_record(
                record,
                None,
                stream_formats,
                &mut primary_output,
                observable_output.as_mut(),
                error_output.as_mut(),
                &mut stream_state,
            )
        })?;
    }
    stream_state.finish()
}

fn parse_dem_bytes(input: &[u8]) -> Result<DetectorErrorModel, CliError> {
    let dem_text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    Ok(DetectorErrorModel::from_dem_str(dem_text)?)
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::from(CircuitError::InvalidResultFormat {
        message: message.into(),
    })
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
            CircuitError::InvalidResultFormat {
                message: format!("b8 replay record decoded into {} records", records.len()),
            }
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
        let parsed = if format == SampleDemRecordFormatArg::Hits {
            vec![read_hits_replay_record(&line, error_count)?]
        } else {
            read_measurement_records(&line, sample_format, error_count)?
        };
        if format == SampleDemRecordFormatArg::Dets && parsed.is_empty() {
            skipped_dets_blank_bytes =
                checked_text_replay_scan_bytes(skipped_dets_blank_bytes, line.len())?;
            continue;
        }
        let [record] = <[Vec<bool>; 1]>::try_from(parsed).map_err(|records| {
            CircuitError::InvalidResultFormat {
                message: format!("replay record decoded into {} records", records.len()),
            }
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
            return Err(CircuitError::InvalidResultFormat {
                message: "HITS replay line decoded into multiple records".to_string(),
            });
        }
        let mut decoded = vec![false; error_count];
        for hit in hits {
            let index = usize::try_from(*hit).map_err(|_| CircuitError::InvalidResultFormat {
                message: format!("HITS replay index {hit} does not fit usize"),
            })?;
            let bit = decoded
                .get_mut(index)
                .ok_or_else(|| CircuitError::InvalidResultFormat {
                    message: format!("HITS replay index {index} exceeds error count {error_count}"),
                })?;
            *bit = true;
        }
        record = Some(decoded);
        Ok(())
    })?;
    record.ok_or_else(|| {
        CliError::from(CircuitError::InvalidResultFormat {
            message: "HITS replay line did not contain one record".to_string(),
        })
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
                return Err(CliError::from(CircuitError::InvalidResultFormat {
                    message: "r8 input ended before record completed".to_string(),
                }));
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
                CliError::from(CircuitError::InvalidResultFormat {
                    message: "r8 run-length offset overflowed".to_string(),
                })
            })?;
            if bit_index > bits_per_record {
                return Err(CliError::from(CircuitError::InvalidResultFormat {
                    message: "r8 run-length overshot record width".to_string(),
                }));
            }
            continue;
        }
        bit_index = bit_index.checked_add(usize::from(byte[0])).ok_or_else(|| {
            CliError::from(CircuitError::InvalidResultFormat {
                message: "r8 run-length offset overflowed".to_string(),
            })
        })?;
        if bit_index > bits_per_record {
            return Err(CliError::from(CircuitError::InvalidResultFormat {
                message: "r8 run-length overshot record width".to_string(),
            }));
        }
        if bit_index == bits_per_record {
            return Ok(Some(record));
        }
        let Some(bit) = record.get_mut(bit_index) else {
            return Err(CliError::from(CircuitError::InvalidResultFormat {
                message: format!("r8 hit index {bit_index} exceeds record width {bits_per_record}"),
            }));
        };
        *bit = true;
        bit_index += 1;
    }
}

#[derive(Default)]
struct SampleDemStreamState {
    primary_ptb64_records: Vec<Vec<bool>>,
    observable_ptb64_records: Vec<Vec<bool>>,
    error_ptb64_records: Vec<Vec<bool>>,
}

impl SampleDemStreamState {
    fn finish(self) -> Result<(), CliError> {
        if self.primary_ptb64_records.is_empty()
            && self.observable_ptb64_records.is_empty()
            && self.error_ptb64_records.is_empty()
        {
            return Ok(());
        }
        Err(invalid_result_format(
            "internal ptb64 stream ended with an incomplete 64-record group",
        ))
    }
}

#[derive(Clone, Copy)]
struct SampleDemStreamFormats {
    observable_mode: DetectionObservableOutputMode,
    out_format: SampleOutFormatArg,
    obs_out_format: SampleDemRecordFormatArg,
    err_out_format: SampleDemRecordFormatArg,
}

impl SampleDemStreamFormats {
    fn from_args(args: &SampleDemArgs, observable_mode: DetectionObservableOutputMode) -> Self {
        Self {
            observable_mode,
            out_format: args.out_format,
            obs_out_format: args.obs_out_format,
            err_out_format: args.err_out_format,
        }
    }
}

fn write_sample_dem_stream_record<W>(
    record: &DetectionEventRecord,
    error_record: Option<&[bool]>,
    formats: SampleDemStreamFormats,
    primary_output: &mut OutputSink<'_, W>,
    observable_output: Option<&mut FileOutputSink>,
    error_output: Option<&mut FileOutputSink>,
    state: &mut SampleDemStreamState,
) -> Result<(), CliError>
where
    W: Write,
{
    write_primary_detection_stream_record(
        record,
        formats.observable_mode,
        formats.out_format,
        primary_output,
        &mut state.primary_ptb64_records,
    )?;
    if let Some(output) = observable_output {
        write_observable_stream_record(
            record,
            formats.obs_out_format,
            output,
            &mut state.observable_ptb64_records,
        )?;
    }
    if let (Some(output), Some(error_record)) = (error_output, error_record) {
        write_error_stream_record(
            error_record,
            formats.err_out_format,
            output,
            &mut state.error_ptb64_records,
        )?;
    }
    Ok(())
}

fn write_primary_detection_stream_record<W>(
    record: &DetectionEventRecord,
    observable_mode: DetectionObservableOutputMode,
    format: SampleOutFormatArg,
    output: &mut OutputSink<'_, W>,
    ptb64_records: &mut Vec<Vec<bool>>,
) -> Result<(), CliError>
where
    W: Write,
{
    match format {
        SampleOutFormatArg::Ptb64 => write_ptb64_stream_record(
            detection_record_bits(record, observable_mode),
            output,
            ptb64_records,
        ),
        SampleOutFormatArg::ZeroOne
        | SampleOutFormatArg::B8
        | SampleOutFormatArg::R8
        | SampleOutFormatArg::Hits
        | SampleOutFormatArg::Dets => {
            let sample_format = format.sample_format()?;
            output.write_with(|writer| {
                write_detection_record(record, observable_mode, sample_format, writer)
            })
        }
    }
}

fn write_observable_stream_record(
    record: &DetectionEventRecord,
    format: SampleDemRecordFormatArg,
    output: &mut FileOutputSink,
    ptb64_records: &mut Vec<Vec<bool>>,
) -> Result<(), CliError> {
    match format {
        SampleDemRecordFormatArg::Ptb64 => {
            write_ptb64_file_stream_record(record.observables.clone(), output, ptb64_records)
        }
        SampleDemRecordFormatArg::ZeroOne
        | SampleDemRecordFormatArg::B8
        | SampleDemRecordFormatArg::R8
        | SampleDemRecordFormatArg::Hits
        | SampleDemRecordFormatArg::Dets => {
            let sample_format = format.sample_format()?;
            output.write_with(|writer| write_observable_record(record, sample_format, writer))
        }
    }
}

fn write_error_stream_record(
    error_record: &[bool],
    format: SampleDemRecordFormatArg,
    output: &mut FileOutputSink,
    ptb64_records: &mut Vec<Vec<bool>>,
) -> Result<(), CliError> {
    match format {
        SampleDemRecordFormatArg::Ptb64 => {
            write_ptb64_file_stream_record(error_record.to_vec(), output, ptb64_records)
        }
        SampleDemRecordFormatArg::ZeroOne
        | SampleDemRecordFormatArg::B8
        | SampleDemRecordFormatArg::R8
        | SampleDemRecordFormatArg::Hits
        | SampleDemRecordFormatArg::Dets => {
            let sample_format = format.sample_format()?;
            output.write_with(|writer| write_bits_record(error_record, sample_format, writer))
        }
    }
}

fn write_ptb64_stream_record<W>(
    bits: Vec<bool>,
    output: &mut OutputSink<'_, W>,
    ptb64_records: &mut Vec<Vec<bool>>,
) -> Result<(), CliError>
where
    W: Write,
{
    ptb64_records.push(bits);
    if ptb64_records.len() == 64 {
        output.write_with(|writer| write_ptb64_group(ptb64_records, writer))?;
        ptb64_records.clear();
    }
    Ok(())
}

fn write_ptb64_file_stream_record(
    bits: Vec<bool>,
    output: &mut FileOutputSink,
    ptb64_records: &mut Vec<Vec<bool>>,
) -> Result<(), CliError> {
    ptb64_records.push(bits);
    if ptb64_records.len() == 64 {
        output.write_with(|writer| write_ptb64_group(ptb64_records, writer))?;
        ptb64_records.clear();
    }
    Ok(())
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
