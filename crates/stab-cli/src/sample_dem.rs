use std::io::{Read, Write};
use std::path::PathBuf;

use clap::{Args, ValueEnum};
use stab_core::{
    CompiledDemSampler, DetectionObservableOutputMode, DetectorErrorModel, SampleFormat,
    result_formats::{
        read_measurement_records, read_ptb64_records, validate_ptb64_shot_count,
        write_ptb64_records_checked, write_records,
    },
    write_detection_records, write_observable_records, write_ptb64_detection_records,
    write_ptb64_observable_records,
};

use super::{CliError, SampleOutFormatArg, read_input, write_empty_observables, write_output};

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
        write_output(args.output.as_ref(), stdout, &[])?;
        write_empty_observables(args.obs_output.as_ref(), stdout)?;
        return write_empty_errors(args.error_output.as_ref(), stdout);
    }
    let input_bytes = read_input(args.input.as_ref(), input)?;
    let dem = parse_dem_bytes(&input_bytes)?;
    let sampler = CompiledDemSampler::compile(&dem)?;
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

fn read_replay_error_records(
    path: &PathBuf,
    format: SampleDemRecordFormatArg,
    error_count: usize,
    expected_shots: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    let input = std::fs::read(path).map_err(|source| CliError::ReadPath {
        path: path.clone(),
        source,
    })?;
    let mut records = if format == SampleDemRecordFormatArg::Ptb64 {
        read_ptb64_records(&input, error_count, expected_shots)?
    } else {
        read_measurement_records(&input, format.sample_format()?, error_count)?
    };
    if records.len() < expected_shots {
        return Err(CliError::ReplayErrorRecordCountMismatch {
            expected: expected_shots,
            actual: records.len(),
        });
    }
    records.truncate(expected_shots);
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
