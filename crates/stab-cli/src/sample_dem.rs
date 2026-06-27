use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use stab_core::{
    CircuitError, CompiledDemSampler, DetectionObservableOutputMode, DetectorErrorModel,
    SampleFormat,
    result_formats::{
        read_measurement_records, read_ptb64_records, validate_ptb64_shot_count,
        write_ptb64_records_checked, write_records,
    },
    write_detection_records, write_observable_records, write_ptb64_detection_records,
    write_ptb64_observable_records,
};

use super::{
    CliError, SampleOutFormatArg, read_limited_input, write_empty_observables, write_output,
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
    if args.shots == 0 {
        validate_zero_shot_input_paths(&args)?;
        write_output(args.output.as_ref(), stdout, &[])?;
        write_empty_observables(args.obs_output.as_ref(), stdout)?;
        return write_empty_errors(args.error_output.as_ref(), stdout);
    }
    let input_bytes = read_limited_input(
        args.input.as_ref(),
        input,
        MAX_SAMPLE_DEM_INPUT_BYTES,
        "sample_dem input",
    )?;
    let dem = parse_dem_bytes(&input_bytes)?;
    let sampler = CompiledDemSampler::compile(&dem)?;
    sampler.validate_sample_buffer_units(
        args.shots,
        args.error_output.is_some() || args.replay_error_input.is_some(),
    )?;
    let mut error_records = None;
    let output = if let Some(replay_path) = args.replay_error_input.as_ref() {
        let replayed_errors = read_replay_error_records(
            replay_path,
            args.replay_err_in_format,
            sampler.error_count(),
            args.shots,
        )?;
        let output = sampler.sample_detection_events_from_error_records(&replayed_errors)?;
        error_records = Some(replayed_errors);
        output
    } else if args.error_output.is_some() {
        let (output, sampled_errors) =
            sampler.sample_detection_events_and_errors_with_seed(args.shots, args.seed)?;
        error_records = Some(sampled_errors);
        output
    } else {
        sampler.sample_detection_events_with_seed(args.shots, args.seed)?
    };
    let observable_mode = observable_output_mode(&args);
    let bytes = write_sample_dem_detection_records(&output, observable_mode, args.out_format)?;
    write_output(args.output.as_ref(), stdout, &bytes)?;
    write_optional_sample_dem_observables(
        args.obs_output.as_ref(),
        args.obs_out_format,
        stdout,
        &output,
    )?;
    write_optional_error_records(
        args.error_output.as_ref(),
        args.err_out_format,
        stdout,
        error_records.as_deref().unwrap_or(&[]),
    )
}

fn parse_dem_bytes(input: &[u8]) -> Result<DetectorErrorModel, CliError> {
    let dem_text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    Ok(DetectorErrorModel::from_dem_str(dem_text)?)
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

fn validate_zero_shot_input_paths(args: &SampleDemArgs) -> Result<(), CliError> {
    if let Some(path) = args.input.as_ref() {
        ensure_readable_path(path)?;
    }
    if let Some(path) = args.replay_error_input.as_ref() {
        ensure_readable_path(path)?;
    }
    Ok(())
}

fn ensure_readable_path(path: &Path) -> Result<(), CliError> {
    std::fs::File::open(path)
        .map(|_| ())
        .map_err(|source| CliError::ReadPath {
            path: path.to_path_buf(),
            source,
        })
}

fn read_replay_error_records(
    path: &Path,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    match format {
        SampleDemRecordFormatArg::Ptb64 => {
            let byte_count = ptb64_replay_byte_count(error_count, expected_shots)?;
            let input = read_replay_prefix(path, byte_count)?;
            read_ptb64_records(&input, error_count, expected_shots).map_err(CliError::from)
        }
        SampleDemRecordFormatArg::B8 => {
            read_b8_replay_error_records(path, error_count, expected_shots)
        }
        SampleDemRecordFormatArg::R8 => {
            read_r8_replay_error_records(path, error_count, expected_shots)
        }
        SampleDemRecordFormatArg::ZeroOne
        | SampleDemRecordFormatArg::Hits
        | SampleDemRecordFormatArg::Dets => {
            read_line_replay_error_records(path, format, error_count, expected_shots)
        }
    }
}

fn ptb64_replay_byte_count(error_count: usize, expected_shots: usize) -> Result<usize, CliError> {
    let shot_groups = expected_shots / 64;
    error_count
        .checked_mul(8)
        .and_then(|bytes_per_group| shot_groups.checked_mul(bytes_per_group))
        .ok_or(CliError::MeasurementCountOverflow)
}

fn read_b8_replay_error_records(
    path: &Path,
    error_count: usize,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    let bytes_per_record = error_count.div_ceil(8);
    let byte_count = expected_shots
        .checked_mul(bytes_per_record)
        .ok_or(CliError::MeasurementCountOverflow)?;
    let input = read_replay_prefix(path, byte_count)?;
    let records = read_measurement_records(&input, SampleFormat::B8, error_count)?;
    require_expected_replay_records(records, expected_shots)
}

fn read_replay_prefix(path: &Path, byte_count: usize) -> Result<Vec<u8>, CliError> {
    let file = std::fs::File::open(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    let mut input = Vec::new();
    file.take(u64::try_from(byte_count).unwrap_or(u64::MAX))
        .read_to_end(&mut input)
        .map_err(|source| CliError::ReadPath {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(input)
}

fn read_line_replay_error_records(
    path: &Path,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    let sample_format = format.sample_format()?;
    let file = std::fs::File::open(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::with_capacity(expected_shots);
    let mut skipped_dets_blank_bytes = 0usize;
    while records.len() < expected_shots {
        let Some(line) = read_replay_line(path, &mut reader)? else {
            return Err(CliError::ReplayErrorRecordCountMismatch {
                expected: expected_shots,
                actual: records.len(),
            });
        };
        if format == SampleDemRecordFormatArg::Dets && is_blank_dets_replay_line(&line) {
            skipped_dets_blank_bytes =
                checked_text_replay_scan_bytes(skipped_dets_blank_bytes, line.len())?;
            continue;
        }
        let parsed = read_measurement_records(&line, sample_format, error_count)?;
        let [record] = <[Vec<bool>; 1]>::try_from(parsed).map_err(|records| {
            CircuitError::InvalidResultFormat {
                message: format!("replay record decoded into {} records", records.len()),
            }
        })?;
        records.push(record);
        skipped_dets_blank_bytes = 0;
    }
    Ok(records)
}

fn is_blank_dets_replay_line(line: &[u8]) -> bool {
    line.iter().all(|byte| byte.is_ascii_whitespace())
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

fn read_replay_line(path: &Path, reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, CliError> {
    let mut line = Vec::new();
    loop {
        let (consumed, found_newline) = {
            let available = reader.fill_buf().map_err(|source| CliError::ReadPath {
                path: path.to_path_buf(),
                source,
            })?;
            if available.is_empty() {
                return if line.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(line))
                };
            }
            let consumed = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if line.len().saturating_add(consumed) > MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES {
                return Err(CliError::InputTooLarge {
                    kind: "sample_dem replay text record",
                    limit: u64::try_from(MAX_SAMPLE_DEM_REPLAY_TEXT_RECORD_BYTES)
                        .unwrap_or(u64::MAX),
                });
            }
            let chunk = available.get(..consumed).ok_or_else(|| {
                CliError::from(CircuitError::InvalidResultFormat {
                    message: "replay line byte range was out of bounds".to_string(),
                })
            })?;
            line.extend_from_slice(chunk);
            (
                consumed,
                consumed
                    .checked_sub(1)
                    .and_then(|index| available.get(index))
                    .is_some_and(|byte| *byte == b'\n'),
            )
        };
        reader.consume(consumed);
        if found_newline {
            return Ok(Some(line));
        }
    }
}

fn read_r8_replay_error_records(
    path: &Path,
    error_count: usize,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    let mut file = std::fs::File::open(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    let mut records = Vec::with_capacity(expected_shots);
    for _ in 0..expected_shots {
        let Some(record) = read_r8_replay_record(path, &mut file, error_count)? else {
            return Err(CliError::ReplayErrorRecordCountMismatch {
                expected: expected_shots,
                actual: records.len(),
            });
        };
        records.push(record);
    }
    Ok(records)
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

fn require_expected_replay_records(
    records: Vec<Vec<bool>>,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    if records.len() < expected_shots {
        return Err(CliError::ReplayErrorRecordCountMismatch {
            expected: expected_shots,
            actual: records.len(),
        });
    }
    Ok(records)
}

fn write_sample_dem_detection_records(
    output: &stab_core::DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: SampleOutFormatArg,
) -> Result<Vec<u8>, CliError> {
    match format {
        SampleOutFormatArg::Ptb64 => Ok(write_ptb64_detection_records(output, observable_mode)?),
        SampleOutFormatArg::ZeroOne
        | SampleOutFormatArg::B8
        | SampleOutFormatArg::R8
        | SampleOutFormatArg::Hits
        | SampleOutFormatArg::Dets => Ok(write_detection_records(
            output,
            observable_mode,
            format.sample_format()?,
        )?),
    }
}

fn write_optional_sample_dem_observables<W>(
    output_path: Option<&PathBuf>,
    format: SampleDemRecordFormatArg,
    stdout: &mut W,
    detection_output: &stab_core::DetectionConversionOutput,
) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let output = if format == SampleDemRecordFormatArg::Ptb64 {
        write_ptb64_observable_records(detection_output)?
    } else {
        write_observable_records(detection_output, format.sample_format()?)?
    };
    write_output(Some(output_path), stdout, &output)
}

fn write_empty_errors<W>(output_path: Option<&PathBuf>, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    write_output(Some(output_path), stdout, &[])
}

fn write_optional_error_records<W>(
    output_path: Option<&PathBuf>,
    format: SampleDemRecordFormatArg,
    stdout: &mut W,
    error_records: &[Vec<bool>],
) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let output = if format == SampleDemRecordFormatArg::Ptb64 {
        write_ptb64_records_checked(error_records)?
    } else {
        write_records(error_records, format.sample_format()?)
    };
    write_output(Some(output_path), stdout, &output)
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
