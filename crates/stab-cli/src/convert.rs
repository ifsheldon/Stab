use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use stab_engine::detection_record_width;
use stab_model::{Circuit, DetectorErrorModel};
use stab_records::{
    DetsLayout, DetsResultType, FormatError, FormatErrorCode, PackedShotBatch, PackedShotBatchView,
    RecordBatchPart, RecordEncoder, RecordStreamReader, RecordStreamStorage,
};

use crate::{
    CliError, MemoryBudgetArg, MemoryLedger, RecordFormatArg, StageBudget,
    batch_output::record_write_io,
    input::{read_limited_input_file, read_limited_stdin, record_stream_error},
    io_plan::{FileRole, OUTPUT_BUFFER_BYTES, PendingIo, StandardStreams, StreamUse},
    output_storage::{EncodedBatches, OutputStorage},
    streaming::{FileOutputSink, OutputSink},
};

/// Upper bound on the packed bytes one conversion batch holds, so very wide records are
/// converted a few at a time instead of reserving 64 rows at once.
const CONVERT_BATCH_BYTES: usize = 1024 * 1024;
const CONVERT_BATCH_RECORDS: usize = u64::BITS as usize;

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

    #[command(flatten)]
    memory: MemoryBudgetArg,
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
        // Compiling the conversion plan still validates measurement-record targets and the
        // detection conversion limits before any count is trusted; the counts come from the
        // model, which agrees with the plan on every circuit the plan accepts.
        detection_record_width(circuit)?;
        self.num_measurements = usize_from_u64(circuit.count_measurements()?, "measurement count")?;
        self.num_detectors = usize_from_u64(circuit.count_detectors()?, "detector count")?;
        self.num_observables = usize_from_u64(circuit.count_observables()?, "observable count")?;
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

    /// Each record type's included width, in record order.
    fn part_widths(self) -> [(DetsResultType, usize); 3] {
        let included = |include: bool, count: usize| if include { count } else { 0 };
        [
            (
                DetsResultType::Measurement,
                included(self.include_measurements, self.num_measurements),
            ),
            (
                DetsResultType::Detector,
                included(self.include_detectors, self.num_detectors),
            ),
            (
                DetsResultType::Observable,
                included(self.include_observables, self.num_observables),
            ),
        ]
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
        return run_convert_stim(io, stdin, stdout, args.memory, args.output.is_some());
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

    let mut ledger = MemoryLedger::new(args.memory);
    let layout = resolve_convert_layout(&args, &mut io, &mut ledger)?;
    let input_file = io.take_input(FileRole::Input);
    let input_path = input_file.as_ref().map(|input| input.path().to_path_buf());
    let input: Box<dyn Read + '_> = match input_file {
        Some(input_file) => Box::new(input_file),
        None => Box::new(stdin),
    };
    let storage = ConvertStorage::admit(&args, layout, ledger.stage())?;
    let _storage_hold = ledger.hold(storage.bytes);
    let record_bytes = ledger.stage().record_bytes();
    stream_convert_records(
        input,
        input_path.as_deref(),
        layout,
        &args,
        RecordLimits {
            batch_records: storage.batch_records,
            text_record_bytes: record_bytes,
        },
        io,
        stdout,
    )
}

/// The storage one record conversion retains while it streams, admitted before any is allocated.
struct ConvertStorage {
    batch_records: usize,
    bytes: u64,
}

impl ConvertStorage {
    /// Admits the reader's batch, PTB64 group, first `r8` record, and packed-format transport
    /// buffer (fixed for `r8`, whose records decode as they arrive) for the input width and
    /// batch size, the column batches split from each batch, each encoder's scratch, and the
    /// write buffer of each named `--out` and `--obs_out` file. Standard output's buffer exists
    /// before the budget is known and is not charged. Text inputs bound their transport buffer
    /// through the per-record text cap that the stage after this one receives.
    fn admit(
        args: &ConvertArgs,
        layout: ConvertLayout,
        stage: StageBudget,
    ) -> Result<Self, CliError> {
        let width = layout.input_width()?;
        let batch_records = convert_batch_records(width);
        let split_observables = args.obs_output.is_some();
        let reader = RecordStreamStorage::new(
            args.in_format.required_record_format()?,
            width,
            batch_records,
        )
        .bytes();
        let single = layout.single_part();
        let columns = layout
            .part_widths()
            .into_iter()
            .map(|(namespace, width)| {
                PackedShotBatch::storage_bytes(
                    batch_records,
                    column_width(width, single, namespace),
                )
            })
            .fold(0_u128, u128::saturating_add);
        let mut encoders = RecordEncoder::shot_major_scratch_bytes(
            args.out_format.required_record_format()?,
            layout.primary_width(split_observables)?,
        );
        if split_observables {
            encoders = encoders.saturating_add(RecordEncoder::shot_major_scratch_bytes(
                args.obs_out_format.required_record_format()?,
                layout.observable_width(),
            ));
        }
        let file_buffers = [args.output.is_some(), split_observables]
            .into_iter()
            .filter(|named| *named)
            .fold(0_u128, |total, _| {
                total.saturating_add(OUTPUT_BUFFER_BYTES as u128)
            });
        let required = reader
            .saturating_add(columns)
            .saturating_add(encoders)
            .saturating_add(file_buffers);
        let budget = stage.get();
        match u64::try_from(required) {
            Ok(bytes) if budget.admits(required) => Ok(Self {
                batch_records,
                bytes,
            }),
            // A requirement beyond `u64` is reported as the largest byte count, still a true
            // lower bound for the refused storage.
            _ => Err(stage.refusal(CliError::ConvertStorageBudget {
                width,
                batch_records,
                exceeded: stab_model::MemoryBudgetExceeded::new(
                    stab_model::ResourceAmount::saturating_from_u128(required),
                    budget,
                ),
            })),
        }
    }
}

/// The record shape a conversion streams with, fixed by its storage admission.
#[derive(Clone, Copy, Debug)]
struct RecordLimits {
    batch_records: usize,
    text_record_bytes: usize,
}

fn run_convert_stim<R, W>(
    mut io: PendingIo,
    stdin: &mut R,
    stdout: &mut W,
    memory: MemoryBudgetArg,
    named_output: bool,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let mut ledger = MemoryLedger::new(memory);
    let input_cap = ledger.stage().input_bytes();
    let input = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(&mut input_file, input_cap, "convert input")?
    } else {
        read_limited_stdin(stdin, input_cap, "convert input")?
    };
    let (circuit, _circuit_hold) = ledger.parse_circuit(input)?;
    // Canonical text streams to the output instead of materializing a second copy, so the only
    // output storage is a named file's write buffer, admitted beside the parsed circuit before
    // the file is activated.
    let _output_hold = OutputStorage::new("convert", EncodedBatches::ShotMajor, None)
        .file_buffer(named_output)
        .admit(&mut ledger)?;
    let mut outputs = io.activate()?;
    let mut primary_output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    primary_output.write_with(|mut writer| circuit.write_stim_io(&mut writer))?;
    primary_output.flush()
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

/// Reads each side input's counts and releases it before the next one or the records stream.
fn resolve_convert_layout(
    args: &ConvertArgs,
    io: &mut PendingIo,
    ledger: &mut MemoryLedger,
) -> Result<ConvertLayout, CliError> {
    let mut layout = ConvertLayout::from_explicit_counts(args);
    if args.dem.is_some() {
        let dem_bytes = read_side_input(io, FileRole::Dem, "convert dem input", ledger)?;
        let (dem, dem_hold) = ledger.parse_dem(dem_bytes)?;
        layout.overwrite_dem_counts(&dem)?;
        drop(dem);
        ledger.release(dem_hold);
    }
    if args.circuit.is_some() {
        let Some(types) = args.types.as_deref() else {
            return Err(CliError::MissingConvertTypes);
        };
        let circuit_bytes =
            read_side_input(io, FileRole::Circuit, "convert circuit input", ledger)?;
        let (circuit, circuit_hold) = ledger.parse_circuit(circuit_bytes)?;
        layout.overwrite_circuit_counts(&circuit, types)?;
        drop(circuit);
        ledger.release(circuit_hold);
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
    max_record_bytes: usize,
) -> Result<RecordStreamReader<Box<dyn Read + 'a>>, CliError> {
    let width = layout.input_width()?;
    Ok(match format {
        RecordFormatArg::Dets => {
            RecordStreamReader::dets(input, layout.dets_layout()?, max_record_bytes)
        }
        RecordFormatArg::Stim => return Err(CliError::UnsupportedConversion),
        _ => RecordStreamReader::measurements(
            input,
            format.required_record_format()?,
            width,
            max_record_bytes,
        ),
    })
}

fn stream_convert_records<W>(
    input: Box<dyn Read + '_>,
    input_path: Option<&Path>,
    layout: ConvertLayout,
    args: &ConvertArgs,
    limits: RecordLimits,
    io: PendingIo,
    stdout: &mut W,
) -> Result<(), CliError>
where
    W: Write,
{
    let mut reader =
        convert_record_reader(input, args.in_format, layout, limits.text_record_bytes)?;
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
    let batch_records = limits.batch_records;
    let mut parts = ConvertParts::new(layout);

    let conversion_result = (|| -> Result<(), CliError> {
        loop {
            let batch = match reader.read_batch(batch_records) {
                Ok(Some(batch)) => batch,
                Ok(None) => break,
                Err(error) => return Err(record_stream_error(error, input_path)),
            };
            parts.with_batch(batch, split_observables, |record, observable| {
                primary_sink.write_with(|output| {
                    primary.write_batch(record, output).map_err(record_write_io)
                })?;
                if let Some(encoder) = observables.as_mut() {
                    let sink = observable_sink.as_mut().ok_or(CliError::IoPlanInvariant {
                        message: "convert observable encoder has no output sink",
                    })?;
                    sink.write_with(|output| {
                        encoder
                            .write_batch(observable, output)
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

/// Records per conversion batch: up to 64, fewer when records are wide.
fn convert_batch_records(width: usize) -> usize {
    let row_bytes = width.div_ceil(u8::BITS as usize).max(1);
    (CONVERT_BATCH_BYTES / row_bytes).clamp(1, CONVERT_BATCH_RECORDS)
}

/// Splits decoded conversion batches into measurement, detector, and observable column groups
/// for the shared encoder. Column storage is allocated only after a batch has been decoded.
struct ConvertParts {
    layout: ConvertLayout,
    columns: Option<[PackedShotBatch; 3]>,
}

impl ConvertParts {
    const fn new(layout: ConvertLayout) -> Self {
        Self {
            layout,
            columns: None,
        }
    }

    /// Calls `write` with the primary record's parts and the observable-output record's parts.
    fn with_batch(
        &mut self,
        batch: PackedShotBatchView<'_>,
        split_observables: bool,
        write: impl FnOnce(&[RecordBatchPart<'_>], &[RecordBatchPart<'_>]) -> Result<(), CliError>,
    ) -> Result<(), CliError> {
        let layout = self.layout;
        let widths = layout.part_widths();
        let shots = batch.shot_count();
        let single = layout.single_part();
        let columns = match &mut self.columns {
            Some(columns) if columns[0].shot_count() >= shots => columns,
            columns => columns.insert([
                PackedShotBatch::zeros(shots, column_width(widths[0].1, single, widths[0].0))?,
                PackedShotBatch::zeros(shots, column_width(widths[1].1, single, widths[1].0))?,
                PackedShotBatch::zeros(shots, column_width(widths[2].1, single, widths[2].0))?,
            ]),
        };
        let mut offset = 0_usize;
        for ((namespace, width), column) in widths.iter().zip(columns.iter_mut()) {
            if *width > 0 && single != Some(*namespace) {
                column.copy_columns_from(batch, offset, shots)?;
            }
            offset = offset
                .checked_add(*width)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        let mut views = Vec::with_capacity(widths.len());
        for ((namespace, _), column) in widths.iter().zip(columns.iter()) {
            let records = if single == Some(*namespace) {
                batch
            } else {
                column.view_prefix(shots)?
            };
            views.push(RecordBatchPart::new(*namespace, records));
        }
        let (primary, observables) = if split_observables {
            views.split_at(2)
        } else {
            (views.as_slice(), &[][..])
        };
        write(primary, observables)
    }
}

/// Columns stored outside the decoded batch: none for the layout's only included type, which
/// is encoded straight from the batch.
fn column_width(width: usize, single: Option<DetsResultType>, namespace: DetsResultType) -> usize {
    if single == Some(namespace) { 0 } else { width }
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

fn read_side_input(
    io: &mut PendingIo,
    role: FileRole,
    kind: &'static str,
    ledger: &MemoryLedger,
) -> Result<Vec<u8>, CliError> {
    let mut input = io.take_input(role).ok_or(CliError::IoPlanInvariant {
        message: "convert preflight omitted a requested side input",
    })?;
    read_limited_input_file(&mut input, ledger.stage().input_bytes(), kind)
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
