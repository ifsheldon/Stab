use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_core::{
    Circuit, CircuitError, CircuitItem, DetectorErrorModel, RepeatBlock, SampleFormat,
    detection_record_width, measurement_record_count,
    result_formats::MeasureRecordWriter,
    result_streaming::{for_each_ptb64_record_all, for_each_record},
};

use crate::{
    CliError, MAX_CONVERT_INPUT_BYTES, RecordFormatArg, parse_circuit_bytes, read_limited_input,
    streaming::write_ptb64_group, write_output,
};

#[derive(Debug, Args)]
pub(crate) struct ConvertArgs {
    /// Input record format.
    #[arg(long = "in_format", value_enum, default_value = "01")]
    in_format: RecordFormatArg,

    /// Output record format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: RecordFormatArg,

    /// Input path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Circuit path used to infer measurement, detector, and observable counts.
    #[arg(long = "circuit")]
    circuit: Option<PathBuf>,

    /// Detector error model path used to infer detector and observable counts.
    #[arg(long = "dem")]
    dem: Option<PathBuf>,

    /// Unique record types included when --circuit is present: M, D, and/or L.
    #[arg(long = "types")]
    types: Option<String>,

    /// Optional separate observable output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,

    /// Number of measurement bits per shot.
    #[arg(long = "num_measurements", default_value_t = 0)]
    num_measurements: usize,

    /// Number of detector bits per shot.
    #[arg(long = "num_detectors", default_value_t = 0)]
    num_detectors: usize,

    /// Number of observable bits per shot.
    #[arg(long = "num_observables", default_value_t = 0)]
    num_observables: usize,

    /// Raw bits per shot when no value type is known.
    #[arg(long = "bits_per_shot", default_value_t = 0)]
    bits_per_shot: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConvertLayout {
    num_measurements: usize,
    num_detectors: usize,
    num_observables: usize,
    include_measurements: bool,
    include_detectors: bool,
    include_observables: bool,
}

#[derive(Clone, Copy, Debug)]
struct ConvertRecordParts<'a> {
    measurements: &'a [bool],
    detectors: &'a [bool],
    observables: &'a [bool],
}

impl ConvertLayout {
    fn from_explicit_counts(args: &ConvertArgs) -> Self {
        Self {
            num_measurements: args.num_measurements,
            num_detectors: args.num_detectors,
            num_observables: args.num_observables,
            include_measurements: args.num_measurements > 0,
            include_detectors: args.num_detectors > 0,
            include_observables: args.num_observables > 0,
        }
    }

    fn overwrite_dem_counts(&mut self, model: &DetectorErrorModel) -> Result<(), CliError> {
        self.num_detectors = usize_from_u64(model.count_detectors()?, "detector count")?;
        self.num_observables = usize_from_u64(model.count_observables()?, "observable count")?;
        self.include_detectors = self.num_detectors > 0;
        self.include_observables = self.num_observables > 0;
        Ok(())
    }

    fn overwrite_circuit_counts(&mut self, circuit: &Circuit, types: &str) -> Result<(), CliError> {
        self.num_measurements = measurement_record_count(circuit)?;
        self.num_observables = circuit_observable_count(circuit)?;
        let detection_width = detection_record_width(circuit)?;
        self.num_detectors = detection_width
            .checked_sub(self.num_observables)
            .ok_or_else(|| {
                invalid_result_format(
                    "circuit detector/observable counts were internally inconsistent",
                )
            })?;
        self.include_measurements = false;
        self.include_detectors = false;
        self.include_observables = false;
        parse_convert_types(types, self)
    }

    fn has_included_bits(self) -> bool {
        self.include_measurements || self.include_detectors || self.include_observables
    }

    fn input_width(self) -> Result<usize, CliError> {
        let mut width = 0usize;
        if self.include_measurements {
            width = width
                .checked_add(self.num_measurements)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        if self.include_detectors {
            width = width
                .checked_add(self.num_detectors)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        if self.include_observables {
            width = width
                .checked_add(self.num_observables)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        Ok(width)
    }

    fn parts<'a>(self, record: &'a [bool]) -> Result<ConvertRecordParts<'a>, CliError> {
        let mut offset = 0usize;
        let measurements = if self.include_measurements {
            take_slice(record, &mut offset, self.num_measurements)?
        } else {
            &[]
        };
        let detectors = if self.include_detectors {
            take_slice(record, &mut offset, self.num_detectors)?
        } else {
            &[]
        };
        let observables = if self.include_observables {
            take_slice(record, &mut offset, self.num_observables)?
        } else {
            &[]
        };
        if offset != record.len() {
            return Err(invalid_result_format(format!(
                "convert record had {} bits but layout consumed {offset}",
                record.len()
            )));
        }
        Ok(ConvertRecordParts {
            measurements,
            detectors,
            observables,
        })
    }

    fn type_offset(self, prefix: char) -> Result<(usize, usize), CliError> {
        let mut offset = 0usize;
        if self.include_measurements {
            if prefix == 'M' {
                return Ok((offset, self.num_measurements));
            }
            offset = offset
                .checked_add(self.num_measurements)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        if self.include_detectors {
            if prefix == 'D' {
                return Ok((offset, self.num_detectors));
            }
            offset = offset
                .checked_add(self.num_detectors)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        if self.include_observables && prefix == 'L' {
            return Ok((offset, self.num_observables));
        }
        Err(invalid_result_format(format!(
            "dets token type {prefix} is not included by this convert layout"
        )))
    }
}

pub(crate) fn run_convert<R, W>(
    args: ConvertArgs,
    stdin: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    if args.in_format == RecordFormatArg::Stim || args.out_format == RecordFormatArg::Stim {
        return run_convert_stim(args, stdin, stdout);
    }

    if args.obs_out_format == RecordFormatArg::Stim {
        return Err(CliError::UnsupportedConversion);
    }

    let layout = resolve_convert_layout(&args)?;
    let input = read_limited_input(
        args.input.as_ref(),
        stdin,
        MAX_CONVERT_INPUT_BYTES,
        "convert input",
    )?;
    if try_write_b8_identity(&args, layout, &input, stdout)? {
        return Ok(());
    }
    let records = read_convert_records(&input, args.in_format, layout)?;
    let (primary_output, observable_output) = write_convert_records(&records, layout, &args)?;
    write_output(args.output.as_ref(), stdout, &primary_output)?;
    if let Some(observable_output) = observable_output {
        write_output(args.obs_output.as_ref(), stdout, &observable_output)?;
    }
    Ok(())
}

fn try_write_b8_identity<W>(
    args: &ConvertArgs,
    layout: ConvertLayout,
    input: &[u8],
    stdout: &mut W,
) -> Result<bool, CliError>
where
    W: Write,
{
    if args.in_format != RecordFormatArg::B8
        || args.out_format != RecordFormatArg::B8
        || args.obs_output.is_some()
    {
        return Ok(false);
    }
    let width = layout.input_width()?;
    if width == 0 || !width.is_multiple_of(8) {
        return Ok(false);
    }
    let bytes_per_record = width / 8;
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(invalid_result_format(format!(
            "b8 input length {} is not a multiple of record byte width {bytes_per_record}",
            input.len()
        )));
    }
    write_output(args.output.as_ref(), stdout, input)?;
    Ok(true)
}

fn run_convert_stim<R, W>(args: ConvertArgs, stdin: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    if args.in_format != RecordFormatArg::Stim || args.out_format != RecordFormatArg::Stim {
        return Err(CliError::UnsupportedConversion);
    }
    let input = read_limited_input(
        args.input.as_ref(),
        stdin,
        MAX_CONVERT_INPUT_BYTES,
        "convert input",
    )?;
    let circuit = parse_circuit_bytes(&input)?;
    let output = circuit.to_stim_string();
    write_output(args.output.as_ref(), stdout, output.as_bytes())
}

fn resolve_convert_layout(args: &ConvertArgs) -> Result<ConvertLayout, CliError> {
    let mut layout = ConvertLayout::from_explicit_counts(args);
    if let Some(dem_path) = args.dem.as_ref() {
        let dem_bytes = read_side_input(dem_path, "convert dem input")?;
        let dem_text = std::str::from_utf8(&dem_bytes).map_err(|_| CliError::InvalidUtf8Input)?;
        let dem = DetectorErrorModel::from_dem_str(dem_text)?;
        layout.overwrite_dem_counts(&dem)?;
    }
    if let Some(circuit_path) = args.circuit.as_ref() {
        let Some(types) = args.types.as_deref() else {
            return Err(CliError::MissingConvertTypes);
        };
        let circuit_bytes = read_side_input(circuit_path, "convert circuit input")?;
        let circuit = parse_circuit_bytes(&circuit_bytes)?;
        layout.overwrite_circuit_counts(&circuit, types)?;
    }
    if !layout.has_included_bits() {
        if args.out_format == RecordFormatArg::Dets {
            return Err(CliError::MissingRecordTypesForDets);
        }
        if args.bits_per_shot == 0 {
            return Err(CliError::MissingRecordWidth);
        }
        layout.num_measurements = args.bits_per_shot;
        layout.include_measurements = true;
    }
    if args.out_format == RecordFormatArg::Dets && layout.input_width()? == 0 {
        return Err(CliError::MissingRecordTypesForDets);
    }
    Ok(layout)
}

fn read_convert_records(
    input: &[u8],
    format: RecordFormatArg,
    layout: ConvertLayout,
) -> Result<Vec<Vec<bool>>, CliError> {
    let width = layout.input_width()?;
    let mut records = Vec::new();
    match format {
        RecordFormatArg::Ptb64 => {
            for_each_ptb64_record_all(input, width, |record| {
                records.push(record.to_vec());
                Ok(())
            })?;
        }
        RecordFormatArg::Dets => {
            for_each_typed_dets_record(input, layout, |record| {
                records.push(record.to_vec());
                Ok(())
            })?;
        }
        RecordFormatArg::Stim => return Err(CliError::UnsupportedConversion),
        _ => {
            let sample_format = format.sample_format()?;
            for_each_record(input, sample_format, width, |record| {
                records.push(record.to_vec());
                Ok(())
            })?;
        }
    }
    Ok(records)
}

fn write_convert_records(
    records: &[Vec<bool>],
    layout: ConvertLayout,
    args: &ConvertArgs,
) -> Result<(Vec<u8>, Option<Vec<u8>>), CliError> {
    let mut primary = ConvertOutput::new(args.out_format)?;
    let mut observables = args
        .obs_output
        .as_ref()
        .map(|_| ConvertOutput::new(args.obs_out_format))
        .transpose()?;

    for record in records {
        let parts = layout.parts(record)?;
        primary.write_primary(parts, observables.is_some())?;
        if let Some(observable_output) = observables.as_mut() {
            observable_output.write_observables(parts.observables)?;
        }
    }

    let primary_output = primary.finish()?;
    let observable_output = observables.map(ConvertOutput::finish).transpose()?;
    Ok((primary_output, observable_output))
}

struct ConvertOutput {
    format: RecordFormatArg,
    output: Vec<u8>,
    ptb64_group: Vec<Vec<bool>>,
}

impl ConvertOutput {
    fn new(format: RecordFormatArg) -> Result<Self, CliError> {
        if format == RecordFormatArg::Stim {
            return Err(CliError::UnsupportedConversion);
        }
        Ok(Self {
            format,
            output: Vec::new(),
            ptb64_group: Vec::new(),
        })
    }

    fn write_primary(
        &mut self,
        parts: ConvertRecordParts<'_>,
        split_observables: bool,
    ) -> Result<(), CliError> {
        if self.format == RecordFormatArg::Dets {
            let types = [
                (b'M', parts.measurements),
                (b'D', parts.detectors),
                (
                    b'L',
                    if split_observables {
                        &[][..]
                    } else {
                        parts.observables
                    },
                ),
            ];
            return self.write_typed_dets_record(&types);
        }

        let mut bits = Vec::with_capacity(
            parts.measurements.len()
                + parts.detectors.len()
                + if split_observables {
                    0
                } else {
                    parts.observables.len()
                },
        );
        bits.extend_from_slice(parts.measurements);
        bits.extend_from_slice(parts.detectors);
        if !split_observables {
            bits.extend_from_slice(parts.observables);
        }
        self.write_bits_record(bits)
    }

    fn write_observables(&mut self, observables: &[bool]) -> Result<(), CliError> {
        if self.format == RecordFormatArg::Dets {
            return self.write_typed_dets_record(&[(b'L', observables)]);
        }
        self.write_bits_record(observables.to_vec())
    }

    fn write_bits_record(&mut self, bits: Vec<bool>) -> Result<(), CliError> {
        if self.format == RecordFormatArg::Ptb64 {
            return self.write_ptb64_record(bits);
        }
        let sample_format = self.format.sample_format()?;
        let mut writer = MeasureRecordWriter::new(sample_format);
        writer.write_bits(&bits);
        writer.write_end();
        self.output.extend_from_slice(&writer.into_bytes());
        Ok(())
    }

    fn write_typed_dets_record(&mut self, types: &[(u8, &[bool])]) -> Result<(), CliError> {
        let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
        for (result_type, bits) in types {
            writer.begin_result_type(*result_type);
            writer.write_bits(bits);
        }
        writer.write_end();
        self.output.extend_from_slice(&writer.into_bytes());
        Ok(())
    }

    fn write_ptb64_record(&mut self, bits: Vec<bool>) -> Result<(), CliError> {
        self.ptb64_group.push(bits);
        if self.ptb64_group.len() == 64 {
            write_ptb64_group(&self.ptb64_group, &mut self.output)
                .map_err(CliError::WriteOutput)?;
            self.ptb64_group.clear();
        }
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, CliError> {
        if !self.ptb64_group.is_empty() {
            return Err(CliError::IncompletePtb64OutputGroup {
                count: self.ptb64_group.len(),
            });
        }
        Ok(self.output)
    }
}

fn parse_convert_types(types: &str, layout: &mut ConvertLayout) -> Result<(), CliError> {
    for result_type in types.chars() {
        match result_type {
            'M' => set_type_flag(&mut layout.include_measurements, result_type)?,
            'D' => set_type_flag(&mut layout.include_detectors, result_type)?,
            'L' => set_type_flag(&mut layout.include_observables, result_type)?,
            _ => return Err(CliError::UnknownConvertType { result_type }),
        }
    }
    Ok(())
}

fn set_type_flag(flag: &mut bool, result_type: char) -> Result<(), CliError> {
    if *flag {
        return Err(CliError::DuplicateConvertType { result_type });
    }
    *flag = true;
    Ok(())
}

fn for_each_typed_dets_record<F>(
    input: &[u8],
    layout: ConvertLayout,
    mut visit: F,
) -> Result<(), CliError>
where
    F: FnMut(&[bool]) -> Result<(), CliError>,
{
    let text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    let width = layout.input_width()?;
    let mut record = vec![false; width];
    for line in text.split_terminator('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("shot") else {
            return Err(invalid_result_format(format!(
                "dets record does not start with shot: {line:?}"
            )));
        };
        record.fill(false);
        parse_typed_dets_tokens(rest.trim(), layout, &mut record)?;
        visit(&record)?;
    }
    Ok(())
}

fn parse_typed_dets_tokens(
    tokens: &str,
    layout: ConvertLayout,
    record: &mut [bool],
) -> Result<(), CliError> {
    if tokens.is_empty() {
        return Ok(());
    }
    for token in tokens.split(' ') {
        if token.is_empty() {
            continue;
        }
        let mut chars = token.chars();
        let Some(prefix @ ('M' | 'D' | 'L')) = chars.next() else {
            return Err(invalid_result_format(format!(
                "invalid dets token {token:?}"
            )));
        };
        let index_text = chars.as_str();
        if index_text.is_empty() {
            return Err(invalid_result_format(format!(
                "invalid dets token {token:?}"
            )));
        }
        let index = index_text.parse::<usize>().map_err(|error| {
            invalid_result_format(format!("invalid dets token index {index_text:?}: {error}"))
        })?;
        let (offset, count) = layout.type_offset(prefix)?;
        if index >= count {
            return Err(invalid_result_format(format!(
                "dets token {token:?} exceeds {prefix} record width {count}"
            )));
        }
        let bit_index = offset
            .checked_add(index)
            .ok_or(CliError::MeasurementCountOverflow)?;
        let Some(bit) = record.get_mut(bit_index) else {
            return Err(invalid_result_format(format!(
                "dets token {token:?} exceeded convert record width {}",
                record.len()
            )));
        };
        *bit = true;
    }
    Ok(())
}

fn circuit_observable_count(circuit: &Circuit) -> Result<usize, CliError> {
    let mut max_observable = None;
    visit_circuit_observables(circuit, &mut max_observable)?;
    max_observable
        .map(|id| id.checked_add(1).ok_or(CliError::MeasurementCountOverflow))
        .transpose()?
        .map(|count| usize::try_from(count).map_err(|_| CliError::MeasurementCountOverflow))
        .transpose()
        .map(|count| count.unwrap_or(0))
}

fn visit_circuit_observables(
    circuit: &Circuit,
    max_observable: &mut Option<u64>,
) -> Result<(), CliError> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE"
                    && let Some(observable) = instruction.observable_id_argument()?
                {
                    let id = observable.get();
                    *max_observable = Some(max_observable.map_or(id, |current| current.max(id)));
                }
            }
            CircuitItem::RepeatBlock(repeat) => visit_repeat_observables(repeat, max_observable)?,
        }
    }
    Ok(())
}

fn visit_repeat_observables(
    repeat: &RepeatBlock,
    max_observable: &mut Option<u64>,
) -> Result<(), CliError> {
    visit_circuit_observables(repeat.body(), max_observable)
}

fn read_side_input(path: &PathBuf, kind: &'static str) -> Result<Vec<u8>, CliError> {
    let mut empty = std::io::empty();
    read_limited_input(Some(path), &mut empty, MAX_CONVERT_INPUT_BYTES, kind)
}

fn take_slice<'a>(
    record: &'a [bool],
    offset: &mut usize,
    len: usize,
) -> Result<&'a [bool], CliError> {
    let end = offset
        .checked_add(len)
        .ok_or(CliError::MeasurementCountOverflow)?;
    let slice = record.get(*offset..end).ok_or_else(|| {
        invalid_result_format(format!(
            "convert record expected slice {offset}..{end} within {} bits",
            record.len()
        ))
    })?;
    *offset = end;
    Ok(slice)
}

fn usize_from_u64(value: u64, kind: &'static str) -> Result<usize, CliError> {
    usize::try_from(value)
        .map_err(|_| invalid_result_format(format!("{kind} {value} does not fit in usize")))
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::Circuit(CircuitError::InvalidResultFormat {
        message: message.into(),
    })
}
