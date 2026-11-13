use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use stab_engine::{detection_record_width, measurement_record_count};
use stab_model::{Circuit, CircuitItem, DetectorErrorModel, RepeatBlock};
use stab_records::{
    DetsLayout, DetsResultType, FormatError, FormatErrorCode, PackedShotBatch, RecordEncoder,
    RecordPart, RecordStreamReader,
};

use crate::{
    CliError, MAX_CIRCUIT_INPUT_BYTES, RecordFormatArg,
    batch_output::record_write_io,
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, PendingIo, StandardStreams, StreamUse},
    parse_circuit_bytes,
    streaming::{FileOutputSink, OutputSink},
};

/// Upper bound on one framed text record's bytes while streaming conversion input.
///
/// Streaming conversion replaces the former whole-input 64 MiB cap. This per-record bound keeps
/// memory bounded without rejecting any input that route accepted, because no record in a
/// cap-compliant input could exceed it.
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

    /// Width of the primary output record, which omits observables routed to `--obs_out`.
    fn primary_width(self, split_observables: bool) -> Result<usize, CliError> {
        let width = self.input_width()?;
        if split_observables {
            return width
                .checked_sub(self.observable_width())
                .ok_or(CliError::MeasurementCountOverflow);
        }
        Ok(width)
    }

    fn observable_width(self) -> usize {
        if self.include_observables {
            self.num_observables
        } else {
            0
        }
    }

    /// The only included record type, when exactly one is included.
    fn single_part(self) -> Option<DetsResultType> {
        match (
            self.include_measurements,
            self.include_detectors,
            self.include_observables,
        ) {
            (true, false, false) => Some(DetsResultType::Measurement),
            (false, true, false) => Some(DetsResultType::Detector),
            (false, false, true) => Some(DetsResultType::Observable),
            _ => None,
        }
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
        .map_err(CliError::from)
    }
}

pub(crate) fn run_convert<R, W>(
    args: ConvertArgs,
    stdin: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let stream_use = StreamUse {
        reads_stdin: args.input.is_none(),
        writes_stdout: args.output.is_none(),
    };
    if args.in_format == RecordFormatArg::Stim || args.out_format == RecordFormatArg::Stim {
        validate_stim_conversion_options(&args)?;
        let io = PendingIo::preflight(
            [(FileRole::Input, args.input.as_deref())],
            [(FileRole::Output, args.output.as_deref())],
        )?;
        io.reject_standard_stream_aliases(streams, stream_use)?;
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
    io.reject_standard_stream_aliases(streams, stream_use)?;

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
            format.required_record_format()?,
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
    let split_observables = args.obs_output.is_some();
    let mut primary = RecordEncoder::try_new(
        args.out_format.required_record_format()?,
        layout.primary_width(split_observables)?,
    )?;
    let mut observables = args
        .obs_output
        .as_ref()
        .map(|_| -> Result<RecordEncoder, CliError> {
            Ok(RecordEncoder::try_new(
                args.obs_out_format.required_record_format()?,
                layout.observable_width(),
            )?)
        })
        .transpose()?;
    let mut outputs = io.activate()?;
    let mut primary_sink = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
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
    let mut parts = ConvertParts::try_new(layout)?;
    let packed_input = args.in_format == RecordFormatArg::B8 && layout.single_part().is_some();

    let conversion_result = (|| -> Result<(), CliError> {
        loop {
            let has_record = if packed_input {
                match reader.next_b8_packed_record() {
                    Ok(Some(record)) => {
                        parts.load_b8(record)?;
                        true
                    }
                    Ok(None) => false,
                    Err(error) => return Err(record_stream_error(error, input_path)),
                }
            } else {
                match reader.next_record() {
                    Ok(Some(record)) => {
                        parts.load_bools(layout.parts(record)?)?;
                        true
                    }
                    Ok(None) => false,
                    Err(error) => return Err(record_stream_error(error, input_path)),
                }
            };
            if !has_record {
                break;
            }
            parts.with_record(split_observables, |record, observable| {
                primary_sink.write_with(|output| {
                    primary
                        .write_record(record, output)
                        .map_err(record_write_io)
                })?;
                if let Some(encoder) = observables.as_mut() {
                    let sink = observable_sink.as_mut().ok_or(CliError::IoPlanInvariant {
                        message: "convert observable encoder has no output sink",
                    })?;
                    sink.write_with(|output| {
                        encoder
                            .write_record(observable, output)
                            .map_err(record_write_io)
                    })?;
                }
                Ok(())
            })?;
        }
        validate_complete_groups(&primary)?;
        if let Some(encoder) = observables.as_ref() {
            validate_complete_groups(encoder)?;
        }
        Ok(())
    })();

    // Unbuffered output reported a failing destination at the write that hit it, before a
    // later input or layout error, so pending bytes are flushed first and a flush failure takes
    // precedence. A write failure is returned as is, because retrying that stream repeats it.
    if matches!(
        conversion_result,
        Err(CliError::WriteOutput(_) | CliError::WritePath { .. })
    ) {
        return conversion_result;
    }
    let primary_flush_result = primary_sink.flush();
    let observable_flush_result = observable_sink
        .as_mut()
        .map_or(Ok(()), FileOutputSink::flush);
    primary_flush_result?;
    observable_flush_result?;
    conversion_result
}

fn validate_complete_groups(encoder: &RecordEncoder) -> Result<(), CliError> {
    match encoder.pending_group_records() {
        0 => Ok(()),
        count => Err(CliError::IncompletePtb64OutputGroup { count }),
    }
}

/// Reusable packed storage for one converted record's measurement, detector, and observable
/// columns, handed to the shared encoder as typed parts.
struct ConvertParts {
    layout: ConvertLayout,
    measurements: PackedShotBatch,
    detectors: PackedShotBatch,
    observables: PackedShotBatch,
}

impl ConvertParts {
    fn try_new(layout: ConvertLayout) -> Result<Self, CliError> {
        let width = |included: bool, count: usize| if included { count } else { 0 };
        Ok(Self {
            layout,
            measurements: PackedShotBatch::zeros(
                1,
                width(layout.include_measurements, layout.num_measurements),
            )?,
            detectors: PackedShotBatch::zeros(
                1,
                width(layout.include_detectors, layout.num_detectors),
            )?,
            observables: PackedShotBatch::zeros(
                1,
                width(layout.include_observables, layout.num_observables),
            )?,
        })
    }

    fn load_bools(&mut self, parts: ConvertRecordParts<'_>) -> Result<(), CliError> {
        self.measurements
            .copy_shot_from_bools(0, parts.measurements)?;
        self.detectors.copy_shot_from_bools(0, parts.detectors)?;
        self.observables
            .copy_shot_from_bools(0, parts.observables)?;
        Ok(())
    }

    /// Loads a raw B8 frame into the layout's only included column group.
    fn load_b8(&mut self, record: &[u8]) -> Result<(), CliError> {
        let target = match self.layout.single_part() {
            Some(DetsResultType::Measurement) => &mut self.measurements,
            Some(DetsResultType::Detector) => &mut self.detectors,
            Some(DetsResultType::Observable) => &mut self.observables,
            None => {
                return Err(CliError::IoPlanInvariant {
                    message: "packed b8 conversion requires exactly one included record type",
                });
            }
        };
        target.copy_shot_from_b8(0, record)?;
        Ok(())
    }

    /// Calls `write` with the primary record's parts and the observable-output record's parts.
    fn with_record(
        &self,
        split_observables: bool,
        write: impl FnOnce(&[RecordPart<'_>], &[RecordPart<'_>]) -> Result<(), CliError>,
    ) -> Result<(), CliError> {
        let measurements = RecordPart::new(DetsResultType::Measurement, self.measurements.shot(0)?);
        let detectors = RecordPart::new(DetsResultType::Detector, self.detectors.shot(0)?);
        let observables = RecordPart::new(DetsResultType::Observable, self.observables.shot(0)?);
        if split_observables {
            write(&[measurements, detectors], &[observables])
        } else {
            write(&[measurements, detectors, observables], &[])
        }
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
    primary_output.flush()
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
    CliError::Record(FormatError::new(
        FormatErrorCode::InvalidData,
        message,
        None,
    ))
}
