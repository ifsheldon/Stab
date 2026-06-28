use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_core::{
    DetectionConversionOptions, DetectionConversionOutput, DetectionObservableOutputMode,
    convert_measurements_to_detection_events, measurement_record_count,
    result_formats::{
        ptb64_record_count, read_measurement_records, read_ptb64_records_all,
        validate_ptb64_shot_count,
    },
    sample_detection_events, validate_detection_sampling_circuit, write_detection_records,
    write_observable_records, write_ptb64_detection_records, write_ptb64_observable_records,
};

use crate::{
    CliError, MAX_CIRCUIT_INPUT_BYTES, RecordFormatArg, SampleOutFormatArg, parse_circuit_bytes,
    read_limited_input, write_empty_observables, write_output,
};

const MAX_M2D_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_M2D_PTB64_DECODED_SHOTS: usize = 1_000_000;
const MAX_M2D_PTB64_DECODED_RECORD_BITS: usize = 64_000_000;

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

    /// Deprecated Stim alias that writes observable flips before detector bits.
    #[arg(long = "prepend_observables", hide = true)]
    prepend_observables: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,
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
}

pub(crate) fn run_detect<R, W, E>(
    args: DetectArgs,
    input: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
    E: Write,
{
    if args.prepend_observables {
        writeln!(
            stderr,
            "[DEPRECATION] Avoid using `--prepend_observables`. Data readers assume observables are appended, not prepended."
        )
        .map_err(CliError::WriteOutput)?;
    }
    validate_detect_observable_routing(&args)?;
    validate_detect_ptb64_shots(&args)?;
    if args.shots == 0 {
        write_output(args.output.as_ref(), stdout, &[])?;
        return write_empty_observables(args.obs_output.as_ref(), stdout);
    }
    let input_bytes = read_limited_input(
        args.input.as_ref(),
        input,
        MAX_CIRCUIT_INPUT_BYTES,
        "detect circuit input",
    )?;
    let circuit = parse_circuit_bytes(&input_bytes)?;
    validate_detection_sampling_circuit(&circuit)?;
    let detection_output = sample_detection_events(&circuit, args.shots, args.seed)?;
    let observable_mode = detect_observable_output_mode(&args);
    let output = write_detect_records(&detection_output, observable_mode, args.out_format)?;
    write_output(args.output.as_ref(), stdout, &output)?;
    write_optional_observables(
        args.obs_output.as_ref(),
        args.obs_out_format,
        stdout,
        &detection_output,
    )
}

pub(crate) fn run_m2d<R, W>(args: M2dArgs, input: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    if args.ran_without_feedback {
        return Err(CliError::UnsupportedRanWithoutFeedback);
    }
    validate_m2d_output_formats(&args)?;
    let circuit_bytes = read_limited_input(
        Some(&args.circuit),
        input,
        MAX_CIRCUIT_INPUT_BYTES,
        "m2d circuit input",
    )?;
    let circuit = parse_circuit_bytes(&circuit_bytes)?;
    let measurement_width = measurement_record_count(&circuit)?;
    let input_bytes =
        read_limited_input(args.input.as_ref(), input, MAX_M2D_INPUT_BYTES, "m2d input")?;
    let measurements =
        read_m2d_measurement_records(&input_bytes, args.in_format, measurement_width)?;
    let detection_output = convert_measurements_to_detection_events(
        &circuit,
        &measurements,
        DetectionConversionOptions {
            skip_reference_sample: args.skip_reference_sample,
        },
    )?;
    let output = write_m2d_detection_records(
        &detection_output,
        observable_output_mode(args.append_observables),
        args.out_format,
    )?;
    write_output(args.output.as_ref(), stdout, &output)?;
    write_m2d_optional_observables(
        args.obs_output.as_ref(),
        args.obs_out_format,
        stdout,
        &detection_output,
    )
}

fn read_m2d_measurement_records(
    input: &[u8],
    format: RecordFormatArg,
    measurement_width: usize,
) -> Result<Vec<Vec<bool>>, CliError> {
    match format {
        RecordFormatArg::Ptb64 => {
            let shots = ptb64_record_count(input, measurement_width)?;
            validate_m2d_ptb64_decoded_size(shots, measurement_width)?;
            Ok(read_ptb64_records_all(input, measurement_width)?)
        }
        _ => Ok(read_measurement_records(
            input,
            format.sample_format()?,
            measurement_width,
        )?),
    }
}

fn validate_m2d_output_formats(args: &M2dArgs) -> Result<(), CliError> {
    args.out_format.sample_format()?;
    if args.obs_output.is_some() {
        args.obs_out_format.sample_format()?;
    }
    Ok(())
}

fn validate_m2d_ptb64_decoded_size(shots: usize, bits_per_record: usize) -> Result<(), CliError> {
    if shots > MAX_M2D_PTB64_DECODED_SHOTS {
        return Err(CliError::DecodedRecordCountTooLarge {
            kind: "m2d ptb64 input",
            actual: shots,
            limit: MAX_M2D_PTB64_DECODED_SHOTS,
        });
    }
    let record_bits =
        shots
            .checked_mul(bits_per_record)
            .ok_or(CliError::DecodedRecordBitsTooLarge {
                kind: "m2d ptb64 input",
                actual: usize::MAX,
                limit: MAX_M2D_PTB64_DECODED_RECORD_BITS,
            })?;
    if record_bits > MAX_M2D_PTB64_DECODED_RECORD_BITS {
        return Err(CliError::DecodedRecordBitsTooLarge {
            kind: "m2d ptb64 input",
            actual: record_bits,
            limit: MAX_M2D_PTB64_DECODED_RECORD_BITS,
        });
    }
    Ok(())
}

fn validate_detect_observable_routing(args: &DetectArgs) -> Result<(), CliError> {
    let effective_prepend = args.prepend_observables
        || (args.out_format == SampleOutFormatArg::Dets && !args.append_observables);
    let selected_routes = usize::from(effective_prepend)
        + usize::from(args.append_observables)
        + usize::from(args.obs_output.is_some());
    if selected_routes > 1 {
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
    } else if args.prepend_observables || args.out_format == SampleOutFormatArg::Dets {
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

fn write_detect_records(
    detection_output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: SampleOutFormatArg,
) -> Result<Vec<u8>, CliError> {
    match format {
        SampleOutFormatArg::Ptb64 => Ok(write_ptb64_detection_records(
            detection_output,
            observable_mode,
        )?),
        _ => Ok(write_detection_records(
            detection_output,
            observable_mode,
            format.sample_format()?,
        )?),
    }
}

fn write_m2d_detection_records(
    detection_output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: RecordFormatArg,
) -> Result<Vec<u8>, CliError> {
    Ok(write_detection_records(
        detection_output,
        observable_mode,
        format.sample_format()?,
    )?)
}

fn write_optional_observables<W>(
    output_path: Option<&PathBuf>,
    format: RecordFormatArg,
    stdout: &mut W,
    detection_output: &DetectionConversionOutput,
) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let output = match format {
        RecordFormatArg::Ptb64 => write_ptb64_observable_records(detection_output)?,
        _ => write_observable_records(detection_output, format.sample_format()?)?,
    };
    write_output(Some(output_path), stdout, &output)
}

fn write_m2d_optional_observables<W>(
    output_path: Option<&PathBuf>,
    format: RecordFormatArg,
    stdout: &mut W,
    detection_output: &DetectionConversionOutput,
) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let output = write_observable_records(detection_output, format.sample_format()?)?;
    write_output(Some(output_path), stdout, &output)
}
