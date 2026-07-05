mod frame;

use frame::{
    circuit_has_pauli_observable_targets, sample_detection_events_with_frame,
    try_for_each_detection_event_with_frame, validate_frame_detection_circuit,
};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, CompiledSampler,
    RepeatBlock, SampleFormat,
    result_formats::{MeasureRecordWriter, write_ptb64_records_checked},
};

const MAX_DETECTION_RECORD_BITS: usize = 1_000_000;
const MAX_DETECTION_BUFFER_BITS: usize = 64_000_000;
const MAX_DETECTION_REPEAT_UNROLL: u64 = 100_000;
const UNSUPPORTED_SWEEP_DETECTION_MESSAGE: &str =
    "sweep-conditioned detection conversion requires sweep input support";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionConversionOptions {
    pub skip_reference_sample: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionEventRecord {
    pub detectors: Vec<bool>,
    pub observables: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionConversionOutput {
    pub records: Vec<DetectionEventRecord>,
    pub detector_count: usize,
    pub observable_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectionObservableOutputMode {
    DetectorsOnly,
    Append,
    Prepend,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDetectionConverter {
    plan: ConversionPlan,
    reference_sample: ReferenceSampleSource,
}

#[derive(Clone, Debug, PartialEq)]
enum ReferenceSampleSource {
    Zero,
    Static(Vec<bool>),
    Sweep(CompiledSampler),
}

impl CompiledDetectionConverter {
    pub fn compile(circuit: &Circuit, options: DetectionConversionOptions) -> CircuitResult<Self> {
        let plan = ConversionPlan::from_circuit(circuit)?;
        let reference_sample = if options.skip_reference_sample {
            ReferenceSampleSource::Zero
        } else if plan.sweep_bit_count > 0 {
            let sampler = CompiledSampler::compile_allowing_sweep(circuit)?;
            if sampler.sweep_bit_count() != plan.sweep_bit_count {
                return Err(CircuitError::invalid_result_format(format!(
                    "sweep reference sampler has {} sweep bits but detection conversion expected {}",
                    sampler.sweep_bit_count(),
                    plan.sweep_bit_count
                )));
            }
            ReferenceSampleSource::Sweep(sampler)
        } else {
            ReferenceSampleSource::Static(reference_sample(circuit, plan.measurement_count)?)
        };
        Self::from_plan_and_reference_sample(plan, reference_sample)
    }

    pub fn measurement_count(&self) -> usize {
        self.plan.measurement_count
    }

    pub fn sweep_bit_count(&self) -> usize {
        self.plan.sweep_bit_count
    }

    pub fn detector_count(&self) -> usize {
        self.plan.detector_terms.len()
    }

    pub fn observable_count(&self) -> usize {
        self.plan.observable_terms.len()
    }

    pub fn convert_record(
        &self,
        measurement_record: &[bool],
    ) -> CircuitResult<DetectionEventRecord> {
        let mut record = self.reusable_detection_record();
        let mut reference_sample = self.reusable_reference_sample();
        let sweep_record = vec![false; self.sweep_bit_count()];
        self.convert_record_with_sweep_into(
            measurement_record,
            &sweep_record,
            &mut reference_sample,
            &mut record,
        )?;
        Ok(record)
    }

    pub fn try_for_each_detection_event<'a, E, I, F>(
        &self,
        measurements: I,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        let mut record = self.reusable_detection_record();
        let mut reference_sample = self.reusable_reference_sample();
        let sweep_record = vec![false; self.sweep_bit_count()];
        for (shot_index, measurement_record) in measurements.into_iter().enumerate() {
            self.validate_measurement_record_width(measurement_record, Some(shot_index))?;
            self.convert_record_with_sweep_into(
                measurement_record,
                &sweep_record,
                &mut reference_sample,
                &mut record,
            )?;
            visit(&record)?;
        }
        Ok(())
    }

    pub fn try_for_each_detection_event_with_sweep<'a, 'b, E, M, S, F>(
        &self,
        measurements: M,
        sweeps: S,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        M: IntoIterator<Item = &'a [bool]>,
        S: IntoIterator<Item = &'b [bool]>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        let mut measurement_iter = measurements.into_iter();
        let mut sweep_iter = sweeps.into_iter();
        let mut record = self.reusable_detection_record();
        let mut reference_sample = self.reusable_reference_sample();
        let mut shot_index = 0usize;
        loop {
            match (measurement_iter.next(), sweep_iter.next()) {
                (Some(measurement_record), Some(sweep_record)) => {
                    self.validate_measurement_record_width(measurement_record, Some(shot_index))?;
                    self.validate_sweep_record_width(sweep_record, Some(shot_index))?;
                    self.convert_record_with_sweep_into(
                        measurement_record,
                        sweep_record,
                        &mut reference_sample,
                        &mut record,
                    )?;
                    visit(&record)?;
                    shot_index += 1;
                }
                (None, None) => return Ok(()),
                (Some(_), None) => {
                    return Err(CircuitError::invalid_result_format(
                        "measurement records have more shots than sweep records",
                    )
                    .into());
                }
                (None, Some(_)) => {
                    return Err(CircuitError::invalid_result_format(
                        "sweep records have more shots than measurement records",
                    )
                    .into());
                }
            }
        }
    }

    pub fn reusable_detection_record(&self) -> DetectionEventRecord {
        DetectionEventRecord {
            detectors: vec![false; self.detector_count()],
            observables: vec![false; self.observable_count()],
        }
    }

    pub fn reusable_reference_sample(&self) -> Vec<bool> {
        vec![false; self.measurement_count()]
    }

    pub fn convert_record_with_sweep_into(
        &self,
        measurement_record: &[bool],
        sweep_record: &[bool],
        reference_sample: &mut Vec<bool>,
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.validate_measurement_record_width(measurement_record, None)?;
        self.validate_sweep_record_width(sweep_record, None)?;
        self.reference_sample
            .fill(sweep_record, self.measurement_count(), reference_sample)?;
        self.plan
            .convert_record_into(measurement_record, reference_sample, record)
    }

    fn from_plan_and_reference_sample(
        plan: ConversionPlan,
        reference_sample: ReferenceSampleSource,
    ) -> CircuitResult<Self> {
        if let ReferenceSampleSource::Static(reference_sample) = &reference_sample {
            validate_reference_sample_len(reference_sample, plan.measurement_count)?;
        }
        Ok(Self {
            plan,
            reference_sample,
        })
    }

    fn validate_measurement_record_width(
        &self,
        measurement_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        if measurement_record.len() == self.plan.measurement_count {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record {shot_index} expected {} bits, got {}",
                self.plan.measurement_count,
                measurement_record.len()
            )));
        }
        Err(CircuitError::invalid_result_format(format!(
            "measurement record expected {} bits, got {}",
            self.plan.measurement_count,
            measurement_record.len()
        )))
    }

    fn validate_sweep_record_width(
        &self,
        sweep_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        if sweep_record.len() == self.plan.sweep_bit_count {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(CircuitError::invalid_result_format(format!(
                "sweep record {shot_index} expected {} bits, got {}",
                self.plan.sweep_bit_count,
                sweep_record.len()
            )));
        }
        Err(CircuitError::invalid_result_format(format!(
            "sweep record expected {} bits, got {}",
            self.plan.sweep_bit_count,
            sweep_record.len()
        )))
    }
}

impl ReferenceSampleSource {
    fn fill(
        &self,
        sweep_record: &[bool],
        measurement_count: usize,
        output: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        output.clear();
        match self {
            Self::Zero => output.resize(measurement_count, false),
            Self::Static(reference_sample) => output.extend_from_slice(reference_sample),
            Self::Sweep(sampler) => {
                sampler.reference_sample_with_sweep_into(sweep_record, output)?
            }
        }
        validate_reference_sample_len(output, measurement_count)
    }
}

pub fn convert_measurements_to_detection_events(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    let converter = CompiledDetectionConverter::compile(circuit, options)?;
    converter.plan.validate_shot_count(measurements.len())?;
    let mut records = Vec::with_capacity(measurements.len());
    converter.try_for_each_detection_event(measurements.iter().map(Vec::as_slice), |record| {
        records.push(record.clone());
        Ok::<(), CircuitError>(())
    })?;

    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub fn convert_measurements_to_detection_events_with_sweep(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    if measurements.len() != sweeps.len() {
        return Err(CircuitError::invalid_result_format(format!(
            "measurement records have {} shots but sweep records have {} shots",
            measurements.len(),
            sweeps.len()
        )));
    }
    let converter = CompiledDetectionConverter::compile(circuit, options)?;
    converter.plan.validate_shot_count(measurements.len())?;
    validate_buffer_bits("sweep records", sweeps.len(), converter.sweep_bit_count())?;
    let mut records = Vec::with_capacity(measurements.len());
    converter.try_for_each_detection_event_with_sweep(
        measurements.iter().map(Vec::as_slice),
        sweeps.iter().map(Vec::as_slice),
        |record| {
            records.push(record.clone());
            Ok::<(), CircuitError>(())
        },
    )?;

    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub fn sample_detection_events(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> CircuitResult<DetectionConversionOutput> {
    if circuit_has_pauli_observable_targets(circuit) {
        return sample_detection_events_with_frame(circuit, shots, seed);
    }
    validate_detection_sampling_circuit(circuit)?;
    let plan = ConversionPlan::from_circuit(circuit)?;
    plan.validate_shot_count(shots)?;
    let sampler = CompiledSampler::compile_allowing_sweep(circuit)?;
    let reference_sample = sampler.reference_sample();
    let converter = CompiledDetectionConverter::from_plan_and_reference_sample(
        plan,
        ReferenceSampleSource::Static(reference_sample),
    )?;
    let measurements = sampler.sample_zero_one_with_seed(shots, seed);
    let mut records = Vec::with_capacity(measurements.len());
    converter.try_for_each_detection_event(measurements.iter().map(Vec::as_slice), |record| {
        records.push(record.clone());
        Ok::<(), CircuitError>(())
    })?;
    Ok(DetectionConversionOutput {
        records,
        detector_count: converter.detector_count(),
        observable_count: converter.observable_count(),
    })
}

pub fn try_for_each_sampled_detection_event<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    mut visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    if circuit_has_pauli_observable_targets(circuit) {
        return try_for_each_detection_event_with_frame(circuit, shots, seed, visit);
    }
    validate_detection_sampling_circuit(circuit)?;
    let plan = ConversionPlan::from_circuit(circuit)?;
    let sampler = CompiledSampler::compile_allowing_sweep(circuit)?;
    let reference_sample = sampler.reference_sample();
    let converter = CompiledDetectionConverter::from_plan_and_reference_sample(
        plan,
        ReferenceSampleSource::Static(reference_sample),
    )?;
    let mut record = converter.reusable_detection_record();
    let mut reference_sample = converter.reusable_reference_sample();
    let sweep_record = vec![false; converter.sweep_bit_count()];
    sampler.for_each_sample_with_seed_and_reference_mode(shots, seed, false, |measurement_record| {
        converter.validate_measurement_record_width(measurement_record, None)?;
        converter.convert_record_with_sweep_into(
            measurement_record,
            &sweep_record,
            &mut reference_sample,
            &mut record,
        )?;
        visit(&record)
    })
}

pub fn measurement_record_count(circuit: &Circuit) -> CircuitResult<usize> {
    Ok(ConversionPlan::from_circuit(circuit)?.measurement_count)
}

pub fn detection_record_width(circuit: &Circuit) -> CircuitResult<usize> {
    ConversionPlan::from_circuit(circuit)?.output_bit_count()
}

pub fn validate_detection_sampling_circuit(circuit: &Circuit) -> CircuitResult<()> {
    if circuit_has_pauli_observable_targets(circuit) {
        validate_frame_detection_circuit(circuit)
    } else {
        ConversionPlan::from_circuit(circuit)?;
        CompiledSampler::compile_allowing_sweep(circuit)?;
        Ok(())
    }
}

pub fn write_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        if format == SampleFormat::Dets {
            if observable_mode == DetectionObservableOutputMode::Prepend {
                writer.begin_result_type(b'L');
                writer.write_bits(&record.observables);
            }
            writer.begin_result_type(b'D');
            writer.write_bits(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                writer.begin_result_type(b'L');
                writer.write_bits(&record.observables);
            }
        } else {
            if observable_mode == DetectionObservableOutputMode::Prepend {
                writer.write_bits(&record.observables);
            }
            writer.write_bits(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                writer.write_bits(&record.observables);
            }
        }
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_observable_records(
    output: &DetectionConversionOutput,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        if format == SampleFormat::Dets {
            writer.begin_result_type(b'L');
        }
        writer.write_bits(&record.observables);
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_ptb64_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<u8>> {
    let records = detection_records_as_bits(output, observable_mode)?;
    write_ptb64_records_checked(&records)
}

pub fn write_ptb64_observable_records(
    output: &DetectionConversionOutput,
) -> CircuitResult<Vec<u8>> {
    let records = observable_records_as_bits(output)?;
    write_ptb64_records_checked(&records)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConversionPlan {
    measurement_count: usize,
    sweep_bit_count: usize,
    detector_terms: Vec<Vec<usize>>,
    observable_terms: Vec<Vec<usize>>,
}

impl ConversionPlan {
    fn from_circuit(circuit: &Circuit) -> CircuitResult<Self> {
        let mut plan = Self {
            measurement_count: 0,
            sweep_bit_count: 0,
            detector_terms: Vec::new(),
            observable_terms: Vec::new(),
        };
        plan.visit_circuit(circuit)?;
        Ok(plan)
    }

    fn visit_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => self.visit_repeat(repeat)?,
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock) -> CircuitResult<()> {
        let repeat_count = repeat.repeat_count().get();
        if repeat_count > MAX_DETECTION_REPEAT_UNROLL {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "detection conversion currently supports repeat counts up to {MAX_DETECTION_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        for _ in 0..repeat.repeat_count().get() {
            self.visit_circuit(repeat.body())?;
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.record_sweep_bits(instruction)?;
        match instruction.gate().canonical_name() {
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "SPP" | "SPP_DAG" => Err(CircuitError::invalid_sampler_compilation(format!(
                "detection conversion does not yet support {}",
                instruction.gate().canonical_name()
            ))),
            _ => self.add_measurements(instruction),
        }
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let terms = instruction
            .targets()
            .iter()
            .map(|target| {
                target
                    .measurement_record_offset()
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format(format!(
                            "DETECTOR target {target} is not a measurement record"
                        ))
                    })
                    .and_then(|offset| self.measurement_index_from_offset(offset.get()))
            })
            .collect::<CircuitResult<Vec<_>>>()?;
        self.detector_terms.push(terms);
        self.validate_record_width()?;
        Ok(())
    }

    fn record_sweep_bits(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let mut found_sweep = None;
        for target in instruction.targets() {
            if let Some(sweep_id) = target.sweep_bit_id() {
                found_sweep = Some(target.clone());
                let next_count = usize::try_from(sweep_id)
                    .ok()
                    .and_then(|id| id.checked_add(1))
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format(format!(
                            "sweep bit id {sweep_id} does not fit this platform"
                        ))
                    })?;
                if next_count > MAX_DETECTION_RECORD_BITS {
                    return Err(CircuitError::invalid_result_format(format!(
                        "sweep bit width {next_count} exceeds current detection conversion limit {MAX_DETECTION_RECORD_BITS}"
                    )));
                }
                self.sweep_bit_count = self.sweep_bit_count.max(next_count);
            }
        }
        let Some(target) = found_sweep else {
            return Ok(());
        };
        match instruction.gate().canonical_name() {
            "CX" | "CY" | "CZ" => Ok(()),
            name => Err(CircuitError::invalid_result_format(format!(
                "{UNSUPPORTED_SWEEP_DETECTION_MESSAGE}; found {target} in {name}"
            ))),
        }
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable_id = instruction
            .observable_id_argument()?
            .ok_or_else(|| CircuitError::invalid_result_format("OBSERVABLE_INCLUDE missing id"))?;
        let observable_id = usize::try_from(observable_id.get()).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "observable id {} does not fit usize",
                observable_id.get()
            ))
        })?;
        self.ensure_observable(observable_id)?;

        let mut terms = Vec::new();
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                terms.push(self.measurement_index_from_offset(offset.get())?);
            } else if target.is_pauli_target() {
                continue;
            } else {
                return Err(CircuitError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        let observable_terms = self
            .observable_terms
            .get_mut(observable_id)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "observable id {observable_id} was not initialized"
                ))
            })?;
        observable_terms.extend(terms);
        Ok(())
    }

    fn ensure_observable(&mut self, observable_id: usize) -> CircuitResult<()> {
        if observable_id >= MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "observable id {observable_id} exceeds current detection record limit {MAX_DETECTION_RECORD_BITS}"
            )));
        }
        while self.observable_terms.len() <= observable_id {
            self.observable_terms.push(Vec::new());
        }
        self.validate_record_width()
    }

    fn add_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let measurement_count = instruction_measurement_count(instruction);
        self.measurement_count = self
            .measurement_count
            .checked_add(measurement_count)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "measurement record count overflowed during detection conversion planning",
                )
            })?;
        if self.measurement_count > MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record width {} exceeds current detection conversion limit {MAX_DETECTION_RECORD_BITS}",
                self.measurement_count
            )));
        }
        Ok(())
    }

    fn output_bit_count(&self) -> CircuitResult<usize> {
        self.detector_terms
            .len()
            .checked_add(self.observable_terms.len())
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })
    }

    fn validate_record_width(&self) -> CircuitResult<()> {
        let width = self.output_bit_count()?;
        if width > MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "detection record width {width} exceeds current limit {MAX_DETECTION_RECORD_BITS}"
            )));
        }
        Ok(())
    }

    fn validate_shot_count(&self, shots: usize) -> CircuitResult<()> {
        validate_buffer_bits("measurement samples", shots, self.measurement_count)?;
        validate_buffer_bits("detection records", shots, self.output_bit_count()?)
    }

    fn measurement_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let current = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_result_format("measurement count does not fit i64")
        })?;
        let index = current
            .checked_add(i64::from(offset))
            .ok_or_else(|| CircuitError::invalid_result_format("measurement reference overflow"))?;
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "measurement record target rec[{offset}] is not available"
            ))
        })?;
        if index >= self.measurement_count {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record target rec[{offset}] is not available"
            )));
        }
        Ok(index)
    }

    fn convert_record_into(
        &self,
        measurement_record: &[bool],
        reference_sample: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        record.detectors.clear();
        for terms in &self.detector_terms {
            record.detectors.push(parity_of_terms(
                terms,
                measurement_record,
                reference_sample,
            )?);
        }
        record.observables.clear();
        for terms in &self.observable_terms {
            record.observables.push(parity_of_terms(
                terms,
                measurement_record,
                reference_sample,
            )?);
        }
        Ok(())
    }
}

fn reference_sample(circuit: &Circuit, measurement_count: usize) -> CircuitResult<Vec<bool>> {
    let reference_sample = CompiledSampler::compile(circuit)?.reference_sample();
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

fn validate_reference_sample_len(
    reference_sample: &[bool],
    measurement_count: usize,
) -> CircuitResult<()> {
    if reference_sample.len() == measurement_count {
        return Ok(());
    }
    Err(CircuitError::invalid_result_format(format!(
        "reference sample has {} measurement bits but detection conversion expected {measurement_count}",
        reference_sample.len()
    )))
}

fn validate_buffer_bits(kind: &str, shots: usize, bits_per_shot: usize) -> CircuitResult<()> {
    let total = shots.checked_mul(bits_per_shot).ok_or_else(|| {
        CircuitError::invalid_result_format(format!("{kind} bit count overflowed"))
    })?;
    if total > MAX_DETECTION_BUFFER_BITS {
        return Err(CircuitError::invalid_result_format(format!(
            "{kind} would require {total} buffered bits; current limit is {MAX_DETECTION_BUFFER_BITS}"
        )));
    }
    Ok(())
}

fn detection_records_as_bits(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<Vec<bool>>> {
    output
        .records
        .iter()
        .map(|record| {
            validate_record_widths(output, record)?;
            let capacity = match observable_mode {
                DetectionObservableOutputMode::DetectorsOnly => output.detector_count,
                DetectionObservableOutputMode::Append | DetectionObservableOutputMode::Prepend => {
                    output
                        .detector_count
                        .checked_add(output.observable_count)
                        .ok_or_else(|| {
                            CircuitError::invalid_result_format(
                                "detection record width overflowed while writing ptb64 output",
                            )
                        })?
                }
            };
            let mut bits = Vec::with_capacity(capacity);
            if observable_mode == DetectionObservableOutputMode::Prepend {
                bits.extend_from_slice(&record.observables);
            }
            bits.extend_from_slice(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                bits.extend_from_slice(&record.observables);
            }
            Ok(bits)
        })
        .collect()
}

fn observable_records_as_bits(output: &DetectionConversionOutput) -> CircuitResult<Vec<Vec<bool>>> {
    output
        .records
        .iter()
        .map(|record| {
            validate_record_widths(output, record)?;
            Ok(record.observables.clone())
        })
        .collect()
}

pub(crate) fn instruction_measurement_count(instruction: &CircuitInstruction) -> usize {
    match instruction.gate().canonical_name() {
        "M"
        | "MX"
        | "MY"
        | "MR"
        | "MRX"
        | "MRY"
        | "MPAD"
        | "HERALDED_ERASE"
        | "HERALDED_PAULI_CHANNEL_1" => instruction.targets().len(),
        "MXX" | "MYY" | "MZZ" | "MPP" => instruction.target_groups().len(),
        _ => 0,
    }
}

fn parity_of_terms(
    terms: &[usize],
    measurement_record: &[bool],
    reference_sample: &[bool],
) -> CircuitResult<bool> {
    let mut parity = false;
    for index in terms {
        let measurement = measurement_record.get(*index).copied().ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "measurement index {index} is out of range"
            ))
        })?;
        let reference = reference_sample.get(*index).copied().ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "reference sample index {index} is out of range"
            ))
        })?;
        parity ^= measurement ^ reference;
    }
    Ok(parity)
}

fn validate_record_widths(
    output: &DetectionConversionOutput,
    record: &DetectionEventRecord,
) -> CircuitResult<()> {
    if record.detectors.len() != output.detector_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} detector bits but expected {}",
            record.detectors.len(),
            output.detector_count
        )));
    }
    if record.observables.len() != output.observable_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} observable bits but expected {}",
            record.observables.len(),
            output.observable_count
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
