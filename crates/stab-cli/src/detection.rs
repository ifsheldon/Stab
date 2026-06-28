use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_core::{
    CircuitError, CompiledDetectionConverter, DetectionConversionOptions, DetectionEventRecord,
    DetectionObservableOutputMode,
    result_formats::{read_measurement_records, validate_ptb64_shot_count},
    try_for_each_sampled_detection_event, validate_detection_sampling_circuit,
};

use crate::{
    CliError, MAX_CIRCUIT_INPUT_BYTES, RecordFormatArg, SampleOutFormatArg, parse_circuit_bytes,
    read_limited_input,
    streaming::{
        FileOutputSink, OutputSink, detection_record_bits, write_detection_record,
        write_observable_record, write_ptb64_group,
    },
    write_empty_observables, write_output,
};

const MAX_M2D_TEXT_RECORD_BYTES: usize = 1_048_576;

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
    let observable_mode = detect_observable_output_mode(&args);
    let mut primary_output = OutputSink::create(args.output.as_ref(), stdout)?;
    let mut observable_output = args
        .obs_output
        .as_ref()
        .map(FileOutputSink::create)
        .transpose()?;
    let mut state = DetectionStreamState::default();
    try_for_each_sampled_detection_event(&circuit, args.shots, args.seed, |record| {
        write_detect_stream_record(
            record,
            observable_mode,
            args.out_format,
            args.obs_out_format,
            &mut primary_output,
            observable_output.as_mut(),
            &mut state,
        )
    })?;
    state.finish()
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
    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: args.skip_reference_sample,
        },
    )?;
    let observable_mode = observable_output_mode(args.append_observables);
    let mut primary_output = OutputSink::create(args.output.as_ref(), stdout)?;
    let mut observable_output = args
        .obs_output
        .as_ref()
        .map(FileOutputSink::create)
        .transpose()?;
    for_each_m2d_measurement_record(
        args.input.as_ref(),
        input,
        args.in_format,
        converter.measurement_count(),
        |measurement_record| {
            converter.try_for_each_detection_event(std::iter::once(measurement_record), |record| {
                write_m2d_stream_record(
                    record,
                    observable_mode,
                    args.out_format,
                    args.obs_out_format,
                    &mut primary_output,
                    observable_output.as_mut(),
                )
            })
        },
    )
}

fn validate_m2d_output_formats(args: &M2dArgs) -> Result<(), CliError> {
    args.out_format.sample_format()?;
    if args.obs_output.is_some() {
        args.obs_out_format.sample_format()?;
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

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::from(CircuitError::InvalidResultFormat {
        message: message.into(),
    })
}

fn read_error(path: Option<&PathBuf>, source: std::io::Error) -> CliError {
    if let Some(path) = path {
        return CliError::ReadPath {
            path: path.clone(),
            source,
        };
    }
    CliError::ReadInput(source)
}

fn for_each_m2d_measurement_record<R, F>(
    input_path: Option<&PathBuf>,
    stdin: &mut R,
    format: RecordFormatArg,
    measurement_width: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    R: Read,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    if let Some(path) = input_path {
        let file = File::open(path).map_err(|source| CliError::ReadPath {
            path: path.clone(),
            source,
        })?;
        let mut reader = BufReader::new(file);
        return for_each_m2d_measurement_record_from_reader(
            &mut reader,
            Some(path),
            format,
            measurement_width,
            visit,
        );
    }
    let mut reader = BufReader::new(stdin);
    for_each_m2d_measurement_record_from_reader(
        &mut reader,
        None,
        format,
        measurement_width,
        &mut visit,
    )
}

fn for_each_m2d_measurement_record_from_reader<R, F>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    format: RecordFormatArg,
    measurement_width: usize,
    visit: F,
) -> Result<(), CliError>
where
    R: BufRead,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    match format {
        RecordFormatArg::ZeroOne | RecordFormatArg::Hits | RecordFormatArg::Dets => {
            for_each_m2d_text_record(reader, input_path, format, measurement_width, visit)
        }
        RecordFormatArg::B8 => for_each_m2d_b8_record(reader, input_path, measurement_width, visit),
        RecordFormatArg::R8 => for_each_m2d_r8_record(reader, input_path, measurement_width, visit),
        RecordFormatArg::Ptb64 => {
            for_each_m2d_ptb64_record(reader, input_path, measurement_width, visit)
        }
        RecordFormatArg::Stim => Err(CliError::UnsupportedConversion),
    }
}

fn for_each_m2d_text_record<R, F>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    format: RecordFormatArg,
    measurement_width: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    R: BufRead,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let sample_format = format.sample_format()?;
    while let Some(line) = read_m2d_line(reader, input_path)? {
        let record = decode_single_m2d_record(&line, sample_format, measurement_width)?;
        visit(&record)?;
    }
    Ok(())
}

fn read_m2d_line<R>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
) -> Result<Option<Vec<u8>>, CliError>
where
    R: BufRead,
{
    let mut line = Vec::new();
    let bytes = reader
        .read_until(b'\n', &mut line)
        .map_err(|source| read_error(input_path, source))?;
    if bytes == 0 {
        return Ok(None);
    }
    if line.len() > MAX_M2D_TEXT_RECORD_BYTES {
        return Err(CliError::InputTooLarge {
            kind: "m2d text record",
            limit: u64::try_from(MAX_M2D_TEXT_RECORD_BYTES).unwrap_or(u64::MAX),
        });
    }
    Ok(Some(line))
}

fn decode_single_m2d_record(
    input: &[u8],
    format: stab_core::SampleFormat,
    measurement_width: usize,
) -> Result<Vec<bool>, CliError> {
    let records = read_measurement_records(input, format, measurement_width)?;
    let [record] = <[Vec<bool>; 1]>::try_from(records).map_err(|records| {
        CircuitError::InvalidResultFormat {
            message: format!("m2d record decoded into {} records", records.len()),
        }
    })?;
    Ok(record)
}

fn for_each_m2d_b8_record<R, F>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    measurement_width: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    R: Read,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let bytes_per_record = measurement_width.div_ceil(8);
    if bytes_per_record == 0 {
        return Err(invalid_result_format(
            "b8 input cannot represent zero-width records",
        ));
    }
    let mut record_bytes = vec![0u8; bytes_per_record];
    loop {
        match read_m2d_exact_record_bytes(reader, input_path, &mut record_bytes)? {
            RecordRead::Complete => {
                let record = decode_single_m2d_record(
                    &record_bytes,
                    stab_core::SampleFormat::B8,
                    measurement_width,
                )?;
                visit(&record)?;
            }
            RecordRead::EofBeforeRecord => return Ok(()),
        }
    }
}

fn for_each_m2d_r8_record<R, F>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    measurement_width: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    R: Read,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    while let Some(record) = read_m2d_r8_record(reader, input_path, measurement_width)? {
        visit(&record)?;
    }
    Ok(())
}

fn read_m2d_r8_record<R>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    bits_per_record: usize,
) -> Result<Option<Vec<bool>>, CliError>
where
    R: Read,
{
    let mut record = vec![false; bits_per_record];
    let mut bit_index = 0usize;
    let mut read_any = false;
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) if !read_any => return Ok(None),
            Ok(0) => {
                return Err(invalid_result_format(
                    "r8 input ended before record completed",
                ));
            }
            Ok(_) => read_any = true,
            Err(source) => return Err(read_error(input_path, source)),
        }

        if byte[0] == u8::MAX {
            bit_index = bit_index
                .checked_add(usize::from(u8::MAX))
                .ok_or_else(|| invalid_result_format("r8 run-length offset overflowed"))?;
            if bit_index > bits_per_record {
                return Err(invalid_result_format("r8 run-length overshot record width"));
            }
            continue;
        }
        bit_index = bit_index
            .checked_add(usize::from(byte[0]))
            .ok_or_else(|| invalid_result_format("r8 run-length offset overflowed"))?;
        if bit_index > bits_per_record {
            return Err(invalid_result_format("r8 run-length overshot record width"));
        }
        if bit_index == bits_per_record {
            return Ok(Some(record));
        }
        let Some(bit) = record.get_mut(bit_index) else {
            return Err(invalid_result_format(format!(
                "r8 hit index {bit_index} exceeds record width {bits_per_record}"
            )));
        };
        *bit = true;
        bit_index += 1;
    }
}

fn for_each_m2d_ptb64_record<R, F>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    measurement_width: usize,
    mut visit: F,
) -> Result<(), CliError>
where
    R: Read,
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    if measurement_width == 0 {
        return Err(invalid_result_format(
            "ptb64 input cannot infer a shot count for zero-width records",
        ));
    }
    let bytes_per_group = measurement_width
        .checked_mul(8)
        .ok_or(CliError::MeasurementCountOverflow)?;
    let mut group_bytes = vec![0u8; bytes_per_group];
    loop {
        match read_m2d_exact_record_bytes(reader, input_path, &mut group_bytes)? {
            RecordRead::Complete => {
                let group = decode_ptb64_group(&group_bytes, measurement_width)?;
                for record in &group {
                    visit(record)?;
                }
            }
            RecordRead::EofBeforeRecord => return Ok(()),
        }
    }
}

enum RecordRead {
    Complete,
    EofBeforeRecord,
}

fn read_m2d_exact_record_bytes<R>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    buffer: &mut [u8],
) -> Result<RecordRead, CliError>
where
    R: Read,
{
    let mut offset = 0usize;
    while offset < buffer.len() {
        let remaining = buffer
            .get_mut(offset..)
            .ok_or_else(|| invalid_result_format("m2d byte cursor was out of range"))?;
        match reader.read(remaining) {
            Ok(0) if offset == 0 => return Ok(RecordRead::EofBeforeRecord),
            Ok(0) => {
                return Err(invalid_result_format(format!(
                    "m2d input ended after {offset} bytes of a {}-byte record",
                    buffer.len()
                )));
            }
            Ok(count) => offset += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(source) => return Err(read_error(input_path, source)),
        }
    }
    Ok(RecordRead::Complete)
}

fn decode_ptb64_group(input: &[u8], bits_per_record: usize) -> Result<Vec<Vec<bool>>, CliError> {
    let mut records = vec![vec![false; bits_per_record]; 64];
    for (bit_index, word_chunk) in input.chunks_exact(8).enumerate() {
        let mut word_bytes = [0u8; 8];
        word_bytes.copy_from_slice(word_chunk);
        let word = u64::from_le_bytes(word_bytes);
        for (shot_offset, record) in records.iter_mut().enumerate() {
            if word & (1u64 << shot_offset) != 0 {
                let bit = record.get_mut(bit_index).ok_or_else(|| {
                    invalid_result_format("ptb64 bit index was out of decoded record bounds")
                })?;
                *bit = true;
            }
        }
    }
    Ok(records)
}

#[derive(Default)]
struct DetectionStreamState {
    primary_ptb64_records: Vec<Vec<bool>>,
    observable_ptb64_records: Vec<Vec<bool>>,
}

impl DetectionStreamState {
    fn finish(self) -> Result<(), CliError> {
        if self.primary_ptb64_records.is_empty() && self.observable_ptb64_records.is_empty() {
            return Ok(());
        }
        Err(invalid_result_format(
            "internal ptb64 stream ended with an incomplete 64-record group",
        ))
    }
}

fn write_detect_stream_record<W>(
    record: &DetectionEventRecord,
    observable_mode: DetectionObservableOutputMode,
    out_format: SampleOutFormatArg,
    obs_format: RecordFormatArg,
    primary_output: &mut OutputSink<'_, W>,
    observable_output: Option<&mut FileOutputSink>,
    state: &mut DetectionStreamState,
) -> Result<(), CliError>
where
    W: Write,
{
    match out_format {
        SampleOutFormatArg::Ptb64 => write_ptb64_output_record(
            detection_record_bits(record, observable_mode),
            primary_output,
            &mut state.primary_ptb64_records,
        )?,
        SampleOutFormatArg::ZeroOne
        | SampleOutFormatArg::B8
        | SampleOutFormatArg::R8
        | SampleOutFormatArg::Hits
        | SampleOutFormatArg::Dets => {
            let sample_format = out_format.sample_format()?;
            primary_output.write_with(|writer| {
                write_detection_record(record, observable_mode, sample_format, writer)
            })?;
        }
    }
    if let Some(output) = observable_output {
        match obs_format {
            RecordFormatArg::Ptb64 => write_ptb64_file_output_record(
                record.observables.clone(),
                output,
                &mut state.observable_ptb64_records,
            )?,
            _ => {
                let sample_format = obs_format.sample_format()?;
                output
                    .write_with(|writer| write_observable_record(record, sample_format, writer))?;
            }
        }
    }
    Ok(())
}

fn write_m2d_stream_record<W>(
    record: &DetectionEventRecord,
    observable_mode: DetectionObservableOutputMode,
    out_format: RecordFormatArg,
    obs_format: RecordFormatArg,
    primary_output: &mut OutputSink<'_, W>,
    observable_output: Option<&mut FileOutputSink>,
) -> Result<(), CliError>
where
    W: Write,
{
    let out_sample_format = out_format.sample_format()?;
    primary_output.write_with(|writer| {
        write_detection_record(record, observable_mode, out_sample_format, writer)
    })?;
    if let Some(output) = observable_output {
        let obs_sample_format = obs_format.sample_format()?;
        output.write_with(|writer| write_observable_record(record, obs_sample_format, writer))?;
    }
    Ok(())
}

fn write_ptb64_output_record<W>(
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

fn write_ptb64_file_output_record(
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
