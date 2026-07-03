use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_core::{
    CircuitError, CompiledDetectionConverter, DetectionConversionOptions, DetectionEventRecord,
    DetectionObservableOutputMode, circuit_with_inlined_feedback,
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
    validate_m2d_output_formats(&args)?;
    let circuit_bytes = read_limited_input(
        Some(&args.circuit),
        input,
        MAX_CIRCUIT_INPUT_BYTES,
        "m2d circuit input",
    )?;
    let circuit = parse_circuit_bytes(&circuit_bytes)?;
    let circuit = if args.ran_without_feedback {
        circuit_with_inlined_feedback(&circuit)?
    } else {
        circuit
    };
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
    let mut reference_sample = converter.reusable_reference_sample();
    let mut detection_record = converter.reusable_detection_record();
    if let Some(sweep_path) = args.sweep.as_ref() {
        let mut measurements = M2dRecordStream::from_path_or_stdin(
            args.input.as_ref(),
            input,
            args.in_format,
            converter.measurement_count(),
            "m2d measurement input",
        )?;
        let mut sweeps = M2dRecordStream::from_path(
            sweep_path,
            args.sweep_format,
            converter.sweep_bit_count(),
            "m2d sweep input",
        )?;
        loop {
            match measurements.next_record()? {
                Some(measurement_record) => {
                    let Some(sweep_record) = sweeps.next_record()? else {
                        return Err(invalid_result_format(
                            "m2d measurement input has more records than sweep input",
                        ));
                    };
                    converter.convert_record_with_sweep_into(
                        &measurement_record,
                        &sweep_record,
                        &mut reference_sample,
                        &mut detection_record,
                    )?;
                    write_m2d_stream_record(
                        &detection_record,
                        observable_mode,
                        args.out_format,
                        args.obs_out_format,
                        &mut primary_output,
                        observable_output.as_mut(),
                    )?;
                }
                None => {
                    if sweeps.finish_empty_b8_zero_width_sweep_after_measurement_eof()? {
                        return Ok(());
                    }
                    if sweeps.next_record()?.is_none() {
                        return Ok(());
                    }
                    return Err(invalid_result_format(
                        "m2d sweep input has more records than measurement input",
                    ));
                }
            }
        }
    }
    let sweep_record = vec![false; converter.sweep_bit_count()];
    for_each_m2d_measurement_record(
        args.input.as_ref(),
        input,
        args.in_format,
        converter.measurement_count(),
        |measurement_record| {
            converter.convert_record_with_sweep_into(
                measurement_record,
                &sweep_record,
                &mut reference_sample,
                &mut detection_record,
            )?;
            write_m2d_stream_record(
                &detection_record,
                observable_mode,
                args.out_format,
                args.obs_out_format,
                &mut primary_output,
                observable_output.as_mut(),
            )
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

struct M2dRecordStream<'a> {
    reader: Box<dyn BufRead + 'a>,
    input_path: Option<PathBuf>,
    format: RecordFormatArg,
    bits_per_record: usize,
    kind: &'static str,
    ptb64_records: VecDeque<Vec<bool>>,
    empty_b8_zero_width_sweep_checked: bool,
}

impl<'a> M2dRecordStream<'a> {
    fn from_path_or_stdin<R>(
        input_path: Option<&PathBuf>,
        stdin: &'a mut R,
        format: RecordFormatArg,
        bits_per_record: usize,
        kind: &'static str,
    ) -> Result<Self, CliError>
    where
        R: Read + 'a,
    {
        let reader: Box<dyn BufRead + 'a> = if let Some(path) = input_path {
            Box::new(BufReader::new(File::open(path).map_err(|source| {
                CliError::ReadPath {
                    path: path.clone(),
                    source,
                }
            })?))
        } else {
            Box::new(BufReader::new(stdin))
        };
        Ok(Self {
            reader,
            input_path: input_path.cloned(),
            format,
            bits_per_record,
            kind,
            ptb64_records: VecDeque::new(),
            empty_b8_zero_width_sweep_checked: false,
        })
    }

    fn from_path(
        input_path: &PathBuf,
        format: RecordFormatArg,
        bits_per_record: usize,
        kind: &'static str,
    ) -> Result<Self, CliError> {
        let file = File::open(input_path).map_err(|source| CliError::ReadPath {
            path: input_path.clone(),
            source,
        })?;
        Ok(Self {
            reader: Box::new(BufReader::new(file)),
            input_path: Some(input_path.clone()),
            format,
            bits_per_record,
            kind,
            ptb64_records: VecDeque::new(),
            empty_b8_zero_width_sweep_checked: false,
        })
    }

    fn is_b8_zero_width_sweep(&self) -> bool {
        self.kind == "m2d sweep input"
            && self.format == RecordFormatArg::B8
            && self.bits_per_record == 0
    }

    fn finish_empty_b8_zero_width_sweep_after_measurement_eof(&mut self) -> Result<bool, CliError> {
        if !self.is_b8_zero_width_sweep() {
            return Ok(false);
        }
        self.validate_empty_b8_zero_width_sweep()?;
        Ok(true)
    }

    fn next_record(&mut self) -> Result<Option<Vec<bool>>, CliError> {
        match self.format {
            RecordFormatArg::ZeroOne | RecordFormatArg::Hits | RecordFormatArg::Dets => {
                let Some(line) =
                    read_m2d_line(&mut self.reader, self.input_path.as_ref(), self.kind)?
                else {
                    return Ok(None);
                };
                let sample_format = self.format.sample_format()?;
                decode_single_m2d_record(&line, sample_format, self.bits_per_record, self.kind)
                    .map(Some)
            }
            RecordFormatArg::B8 => self.next_b8_record(),
            RecordFormatArg::R8 => read_m2d_r8_record(
                &mut self.reader,
                self.input_path.as_ref(),
                self.bits_per_record,
                self.kind,
            ),
            RecordFormatArg::Ptb64 => self.next_ptb64_record(),
            RecordFormatArg::Stim => Err(CliError::UnsupportedConversion),
        }
    }

    fn next_b8_record(&mut self) -> Result<Option<Vec<bool>>, CliError> {
        let bytes_per_record = self.bits_per_record.div_ceil(8);
        if bytes_per_record == 0 {
            if self.is_b8_zero_width_sweep() {
                self.validate_empty_b8_zero_width_sweep()?;
                return Ok(Some(Vec::new()));
            }
            return Err(invalid_result_format(format!(
                "{} b8 input cannot represent zero-width records",
                self.kind
            )));
        }
        let mut record_bytes = vec![0u8; bytes_per_record];
        match read_m2d_exact_record_bytes(
            &mut self.reader,
            self.input_path.as_ref(),
            &mut record_bytes,
            self.kind,
        )? {
            RecordRead::Complete => decode_single_m2d_record(
                &record_bytes,
                stab_core::SampleFormat::B8,
                self.bits_per_record,
                self.kind,
            )
            .map(Some),
            RecordRead::EofBeforeRecord => Ok(None),
        }
    }

    fn validate_empty_b8_zero_width_sweep(&mut self) -> Result<(), CliError> {
        if self.empty_b8_zero_width_sweep_checked {
            return Ok(());
        }
        let mut byte = [0u8; 1];
        match self.reader.read(&mut byte) {
            Ok(0) => {
                self.empty_b8_zero_width_sweep_checked = true;
                Ok(())
            }
            Ok(_) => Err(invalid_result_format(
                "m2d sweep input b8 zero-width input must be empty",
            )),
            Err(source) => Err(read_error(self.input_path.as_ref(), source)),
        }
    }

    fn next_ptb64_record(&mut self) -> Result<Option<Vec<bool>>, CliError> {
        if let Some(record) = self.ptb64_records.pop_front() {
            return Ok(Some(record));
        }
        if self.bits_per_record == 0 {
            return Err(invalid_result_format(format!(
                "{} ptb64 input cannot infer a shot count for zero-width records",
                self.kind
            )));
        }
        let bytes_per_group = self
            .bits_per_record
            .checked_mul(8)
            .ok_or(CliError::MeasurementCountOverflow)?;
        let mut group_bytes = vec![0u8; bytes_per_group];
        match read_m2d_exact_record_bytes(
            &mut self.reader,
            self.input_path.as_ref(),
            &mut group_bytes,
            self.kind,
        )? {
            RecordRead::Complete => {
                self.ptb64_records = decode_ptb64_group(&group_bytes, self.bits_per_record)?
                    .into_iter()
                    .collect();
                Ok(self.ptb64_records.pop_front())
            }
            RecordRead::EofBeforeRecord => Ok(None),
        }
    }
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
    let mut records = M2dRecordStream::from_path_or_stdin(
        input_path,
        stdin,
        format,
        measurement_width,
        "m2d measurement input",
    )?;
    while let Some(record) = records.next_record()? {
        visit(&record)?;
    }
    Ok(())
}

fn read_m2d_line<R>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    kind: &'static str,
) -> Result<Option<Vec<u8>>, CliError>
where
    R: BufRead + ?Sized,
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
            kind,
            limit: u64::try_from(MAX_M2D_TEXT_RECORD_BYTES).unwrap_or(u64::MAX),
        });
    }
    Ok(Some(line))
}

fn decode_single_m2d_record(
    input: &[u8],
    format: stab_core::SampleFormat,
    measurement_width: usize,
    kind: &str,
) -> Result<Vec<bool>, CliError> {
    let records = read_measurement_records(input, format, measurement_width)?;
    let [record] = <[Vec<bool>; 1]>::try_from(records).map_err(|records| {
        CircuitError::InvalidResultFormat {
            message: format!("{kind} record decoded into {} records", records.len()),
        }
    })?;
    Ok(record)
}

fn read_m2d_r8_record<R>(
    reader: &mut R,
    input_path: Option<&PathBuf>,
    bits_per_record: usize,
    kind: &'static str,
) -> Result<Option<Vec<bool>>, CliError>
where
    R: Read + ?Sized,
{
    let mut record = vec![false; bits_per_record];
    let mut bit_index = 0usize;
    let mut read_any = false;
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) if !read_any => return Ok(None),
            Ok(0) => {
                return Err(invalid_result_format(format!(
                    "{kind} r8 input ended before record completed"
                )));
            }
            Ok(_) => read_any = true,
            Err(source) => return Err(read_error(input_path, source)),
        }

        if byte[0] == u8::MAX {
            bit_index = bit_index.checked_add(usize::from(u8::MAX)).ok_or_else(|| {
                invalid_result_format(format!("{kind} r8 run-length offset overflowed"))
            })?;
            if bit_index > bits_per_record {
                return Err(invalid_result_format(format!(
                    "{kind} r8 run-length overshot record width"
                )));
            }
            continue;
        }
        bit_index = bit_index.checked_add(usize::from(byte[0])).ok_or_else(|| {
            invalid_result_format(format!("{kind} r8 run-length offset overflowed"))
        })?;
        if bit_index > bits_per_record {
            return Err(invalid_result_format(format!(
                "{kind} r8 run-length overshot record width"
            )));
        }
        if bit_index == bits_per_record {
            return Ok(Some(record));
        }
        let Some(bit) = record.get_mut(bit_index) else {
            return Err(invalid_result_format(format!(
                "{kind} r8 hit index {bit_index} exceeds record width {bits_per_record}"
            )));
        };
        *bit = true;
        bit_index += 1;
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
    kind: &'static str,
) -> Result<RecordRead, CliError>
where
    R: Read + ?Sized,
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
                    "{kind} ended after {offset} bytes of a {}-byte record",
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
