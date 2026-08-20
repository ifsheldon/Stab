use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use stab_core::{
    Circuit, CircuitError, CircuitItem, DetectorErrorModel, RepeatBlock, SampleFormat,
    advanced::records::{DetsLayout, DetsResultType, MeasureRecordWriter, RecordStreamReader},
    detection_record_width, measurement_record_count,
};

use crate::{
    CliError, MAX_CIRCUIT_INPUT_BYTES, RecordFormatArg,
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, PendingIo},
    parse_circuit_bytes,
    streaming::{FileOutputSink, OutputSink, write_ptb64_group},
};

/// Upper bound on one framed text record's bytes while streaming conversion input.
///
/// Decision D5 (docs/plans/post-review-remediation-plan.md) replaced the whole-input 64 MiB cap
/// with streaming conversion; this per-record bound keeps memory bounded without rejecting any
/// input the capped route accepted, because no record in a cap-compliant input could exceed it.
const MAX_CONVERT_RECORD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Args)]
pub(crate) struct ConvertArgs {
    /// Input record format.
    #[arg(long = "in_format", value_enum)]
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

    fn dets_layout(self) -> Result<DetsLayout, CliError> {
        DetsLayout::try_new(
            if self.include_measurements {
                self.num_measurements
            } else {
                0
            },
            if self.include_detectors {
                self.num_detectors
            } else {
                0
            },
            if self.include_observables {
                self.num_observables
            } else {
                0
            },
        )
        .map_err(CircuitError::from)
        .map_err(CliError::from)
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
        validate_stim_conversion_options(&args)?;
        let io = PendingIo::preflight(
            [(FileRole::Input, args.input.as_deref())],
            [(FileRole::Output, args.output.as_deref())],
        )?;
        return run_convert_stim(io, stdin, stdout);
    }
    if args.obs_out_format == RecordFormatArg::Stim {
        return Err(CliError::UnsupportedConversion);
    }

    let mut io = PendingIo::preflight(
        [
            (FileRole::Input, args.input.as_deref()),
            (FileRole::Circuit, args.circuit.as_deref()),
            (FileRole::Dem, args.dem.as_deref()),
        ],
        [
            (FileRole::Output, args.output.as_deref()),
            (FileRole::ObservableOutput, args.obs_output.as_deref()),
        ],
    )?;

    let layout = resolve_convert_layout(&args, &mut io)?;
    let input_file = io.take_input(FileRole::Input);
    let input_path = input_file.as_ref().map(|input| input.path().to_path_buf());
    let input: Box<dyn Read + '_> = match input_file {
        Some(input_file) => Box::new(input_file),
        None => Box::new(stdin),
    };
    stream_convert_records(input, input_path.as_deref(), layout, &args, io, stdout)
}

fn run_convert_stim<R, W>(mut io: PendingIo, stdin: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let input = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(&mut input_file, MAX_CIRCUIT_INPUT_BYTES, "convert input")?
    } else {
        read_limited_stdin(stdin, MAX_CIRCUIT_INPUT_BYTES, "convert input")?
    };
    let circuit = parse_circuit_bytes(&input)?;
    let output = circuit.to_stim_bytes();
    write_convert_outputs(io, stdout, &output)
}

fn validate_stim_conversion_options(args: &ConvertArgs) -> Result<(), CliError> {
    if args.in_format != RecordFormatArg::Stim || args.out_format != RecordFormatArg::Stim {
        return Err(CliError::UnsupportedConversion);
    }
    let invalid_option = [
        (args.circuit.is_some(), "--circuit"),
        (args.dem.is_some(), "--dem"),
        (args.types.is_some(), "--types"),
        (args.obs_output.is_some(), "--obs_out"),
        (
            args.obs_out_format != RecordFormatArg::ZeroOne,
            "--obs_out_format",
        ),
        (args.num_measurements != 0, "--num_measurements"),
        (args.num_detectors != 0, "--num_detectors"),
        (args.num_observables != 0, "--num_observables"),
        (args.bits_per_shot != 0, "--bits_per_shot"),
    ]
    .into_iter()
    .find_map(|(is_present, flag)| is_present.then_some(flag));
    if let Some(flag) = invalid_option {
        return Err(CliError::UnsupportedStimConversionOption { flag });
    }
    Ok(())
}

fn resolve_convert_layout(
    args: &ConvertArgs,
    io: &mut PendingIo,
) -> Result<ConvertLayout, CliError> {
    let mut layout = ConvertLayout::from_explicit_counts(args);
    if args.dem.is_some() {
        let dem_bytes = read_side_input(io, FileRole::Dem, "convert dem input")?;
        let dem = DetectorErrorModel::from_dem_bytes(&dem_bytes)?;
        layout.overwrite_dem_counts(&dem)?;
    }
    if args.circuit.is_some() {
        let Some(types) = args.types.as_deref() else {
            return Err(CliError::MissingConvertTypes);
        };
        let circuit_bytes = read_side_input(io, FileRole::Circuit, "convert circuit input")?;
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

/// Builds the shared per-record streaming reader for one conversion input.
fn convert_record_reader<'a>(
    input: Box<dyn Read + 'a>,
    format: RecordFormatArg,
    layout: ConvertLayout,
) -> Result<RecordStreamReader<Box<dyn Read + 'a>>, CliError> {
    let width = layout.input_width()?;
    Ok(match format {
        RecordFormatArg::Ptb64 => RecordStreamReader::ptb64(input, width),
        RecordFormatArg::Dets => {
            RecordStreamReader::dets(input, layout.dets_layout()?, MAX_CONVERT_RECORD_BYTES)
        }
        RecordFormatArg::Stim => return Err(CliError::UnsupportedConversion),
        _ => RecordStreamReader::measurements(
            input,
            format.sample_format()?,
            width,
            MAX_CONVERT_RECORD_BYTES,
        ),
    })
}

fn stream_convert_records<W>(
    input: Box<dyn Read + '_>,
    input_path: Option<&Path>,
    layout: ConvertLayout,
    args: &ConvertArgs,
    io: PendingIo,
    stdout: &mut W,
) -> Result<(), CliError>
where
    W: Write,
{
    let mut reader = convert_record_reader(input, args.in_format, layout)?;
    let mut outputs = io.activate()?;
    let mut primary_sink = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    if args.in_format == RecordFormatArg::B8
        && args.out_format == RecordFormatArg::B8
        && args.obs_output.is_none()
    {
        return stream_b8_identity_records(
            &mut reader,
            input_path,
            layout.input_width()?,
            &mut primary_sink,
        );
    }
    let mut observable_sink = args
        .obs_output
        .as_ref()
        .map(|_| {
            outputs
                .take(FileRole::ObservableOutput)
                .map(FileOutputSink::from_output)
                .ok_or(CliError::IoPlanInvariant {
                    message: "convert observable output was not activated",
                })
        })
        .transpose()?;
    let mut primary = ConvertOutput::new(args.out_format)?;
    let mut observables = args
        .obs_output
        .as_ref()
        .map(|_| ConvertOutput::new(args.obs_out_format))
        .transpose()?;

    loop {
        let record = match reader.next_record() {
            Ok(Some(record)) => record,
            Ok(None) => break,
            Err(error) => return Err(record_stream_error(error, input_path)),
        };
        let parts = layout.parts(record)?;
        primary.write_primary(parts, observables.is_some())?;
        let ready = primary.take_ready();
        if !ready.is_empty() {
            primary_sink.write_with(|writer| writer.write_all(&ready))?;
        }
        if let Some(observable_output) = observables.as_mut() {
            observable_output.write_observables(parts.observables)?;
            let ready = observable_output.take_ready();
            if !ready.is_empty() {
                observable_sink
                    .as_mut()
                    .ok_or(CliError::IoPlanInvariant {
                        message: "convert observable encoder has no output sink",
                    })?
                    .write_with(|writer| writer.write_all(&ready))?;
            }
        }
    }

    let primary_output = primary.finish()?;
    if !primary_output.is_empty() {
        primary_sink.write_with(|writer| writer.write_all(&primary_output))?;
    }
    if let Some(observable_output) = observables.map(ConvertOutput::finish).transpose()?
        && !observable_output.is_empty()
    {
        observable_sink
            .as_mut()
            .ok_or(CliError::IoPlanInvariant {
                message: "convert observable output has no output sink",
            })?
            .write_with(|writer| writer.write_all(&observable_output))?;
    }
    Ok(())
}

fn stream_b8_identity_records<R, W>(
    reader: &mut RecordStreamReader<R>,
    input_path: Option<&Path>,
    bits_per_record: usize,
    output: &mut OutputSink<'_, W>,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let tail_bits = bits_per_record % 8;
    let tail_mask = if tail_bits == 0 {
        u8::MAX
    } else {
        u8::MAX >> (8 - tail_bits)
    };
    loop {
        let record = match reader.next_b8_packed_record() {
            Ok(Some(record)) => record,
            Ok(None) => return Ok(()),
            Err(error) => return Err(record_stream_error(error, input_path)),
        };
        output.write_with(|writer| {
            if tail_bits == 0 {
                return writer.write_all(record);
            }
            let Some((last, prefix)) = record.split_last() else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "b8 identity conversion produced an empty record frame",
                ));
            };
            writer.write_all(prefix)?;
            writer.write_all(&[*last & tail_mask])
        })?;
    }
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
                (DetsResultType::Measurement, parts.measurements),
                (DetsResultType::Detector, parts.detectors),
                (
                    DetsResultType::Observable,
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
            return self.write_typed_dets_record(&[(DetsResultType::Observable, observables)]);
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

    fn write_typed_dets_record(
        &mut self,
        types: &[(DetsResultType, &[bool])],
    ) -> Result<(), CliError> {
        let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
        for (result_type, bits) in types {
            writer.begin_dets_result_type(*result_type);
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

    fn take_ready(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
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

fn read_side_input(
    io: &mut PendingIo,
    role: FileRole,
    kind: &'static str,
) -> Result<Vec<u8>, CliError> {
    let mut input = io.take_input(role).ok_or(CliError::IoPlanInvariant {
        message: "convert preflight omitted a requested side input",
    })?;
    read_limited_input_file(&mut input, MAX_CIRCUIT_INPUT_BYTES, kind)
}

fn write_convert_outputs<W>(io: PendingIo, stdout: &mut W, primary: &[u8]) -> Result<(), CliError>
where
    W: Write,
{
    let mut outputs = io.activate()?;
    let mut primary_output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    primary_output.write_with(|writer| writer.write_all(primary))?;
    Ok(())
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
    CliError::Circuit(CircuitError::invalid_result_format(message))
}
