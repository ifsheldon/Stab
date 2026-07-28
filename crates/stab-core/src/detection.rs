mod api;
mod buffers;
mod frame;
mod limits;
mod output;
mod prepared;
mod reference;
mod requirements;

pub use api::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary, DetectionSamplingCompiler, DetectionSamplingPlan,
    DetectionSamplingSession, MeasurementToDetectionCompiler, MeasurementToDetectionPlan,
    MeasurementToDetectionSession, MeasurementToDetectionSinkAdapter,
};
pub use limits::DetectionConversionLimits;
pub use output::{
    write_detection_records, write_observable_records, write_ptb64_detection_records,
    write_ptb64_observable_records,
};

use buffers::{
    resource_amount, try_clone_detection_record, try_false_vec, try_reserve_detection_record_slots,
    try_vec_with_capacity, validate_buffer_bits, validate_vector_capacity,
};
use frame::frame_conversion_plan_with_limits;
use prepared::PreparedDetectionSampling;
use reference::ReferenceSampleSource;
use requirements::circuit_requires_detector_frame;

use crate::resources::{DetectionBufferLimitSubject, DetectionRecordLimitSubject};
use crate::sampling::ReferenceSampleScratch;
use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, CompiledSampler,
    MeasureRecordOffset, RepeatBlock, RepeatNestingLimit, ResourceLimitError,
};

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

impl CompiledDetectionConverter {
    pub fn compile(circuit: &Circuit, options: DetectionConversionOptions) -> CircuitResult<Self> {
        Self::compile_with_limits(circuit, options, DetectionConversionLimits::default())
    }

    pub fn compile_with_limits(
        circuit: &Circuit,
        options: DetectionConversionOptions,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<Self> {
        let plan = ConversionPlan::from_circuit_with_limits(circuit, limits)?;
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
            ReferenceSampleSource::Static(reference::static_reference_sample(
                circuit,
                plan.measurement_count,
            )?)
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
        let mut record = self.try_reusable_detection_record()?;
        let mut reference_sample = self.try_reusable_reference_sample()?;
        let sweep_record =
            try_false_vec(self.sweep_bit_count(), "detection conversion sweep record")?;
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
        let mut record = self.try_reusable_detection_record()?;
        let mut reference_sample = self.try_reusable_reference_sample()?;
        let mut reference_scratch = self
            .reference_sample
            .reusable_scratch()
            .map_err(|error| E::from(error.into_circuit_error()))?;
        let sweep_record =
            try_false_vec(self.sweep_bit_count(), "detection conversion sweep record")?;
        for (shot_index, measurement_record) in measurements.into_iter().enumerate() {
            self.validate_measurement_record_width(measurement_record, Some(shot_index))?;
            self.convert_record_with_sweep_and_scratch_into(
                measurement_record,
                &sweep_record,
                &mut reference_sample,
                &mut record,
                reference_scratch.as_mut(),
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
        let mut record = self.try_reusable_detection_record()?;
        let mut reference_sample = self.try_reusable_reference_sample()?;
        let mut reference_scratch = self
            .reference_sample
            .reusable_scratch()
            .map_err(|error| E::from(error.into_circuit_error()))?;
        let mut shot_index = 0usize;
        loop {
            match (measurement_iter.next(), sweep_iter.next()) {
                (Some(measurement_record), Some(sweep_record)) => {
                    self.validate_measurement_record_width(measurement_record, Some(shot_index))?;
                    self.validate_sweep_record_width(sweep_record, Some(shot_index))?;
                    self.convert_record_with_sweep_and_scratch_into(
                        measurement_record,
                        sweep_record,
                        &mut reference_sample,
                        &mut record,
                        reference_scratch.as_mut(),
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

    fn try_reusable_detection_record(&self) -> CircuitResult<DetectionEventRecord> {
        Ok(DetectionEventRecord {
            detectors: try_false_vec(
                self.detector_count(),
                "detection conversion detector record",
            )?,
            observables: try_false_vec(
                self.observable_count(),
                "detection conversion observable record",
            )?,
        })
    }

    fn try_reusable_reference_sample(&self) -> CircuitResult<Vec<bool>> {
        try_false_vec(
            self.measurement_count(),
            "detection conversion reference sample",
        )
    }

    pub fn convert_record_with_sweep_into(
        &self,
        measurement_record: &[bool],
        sweep_record: &[bool],
        reference_sample: &mut Vec<bool>,
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        let mut reference_scratch = self
            .reference_sample
            .reusable_scratch()
            .map_err(crate::SamplingExecutionError::into_circuit_error)?;
        self.convert_record_with_sweep_and_scratch_into(
            measurement_record,
            sweep_record,
            reference_sample,
            record,
            reference_scratch.as_mut(),
        )
    }

    fn convert_record_with_sweep_and_scratch_into(
        &self,
        measurement_record: &[bool],
        sweep_record: &[bool],
        reference_sample: &mut Vec<bool>,
        record: &mut DetectionEventRecord,
        reference_scratch: Option<&mut ReferenceSampleScratch>,
    ) -> CircuitResult<()> {
        self.validate_measurement_record_width(measurement_record, None)?;
        self.validate_sweep_record_width(sweep_record, None)?;
        self.reference_sample.fill(
            sweep_record,
            self.measurement_count(),
            reference_sample,
            reference_scratch,
        )?;
        self.plan
            .convert_record_into(measurement_record, reference_sample, record)
    }

    fn from_plan_and_reference_sample(
        plan: ConversionPlan,
        reference_sample: ReferenceSampleSource,
    ) -> CircuitResult<Self> {
        if let ReferenceSampleSource::Static(reference_sample) = &reference_sample {
            reference::validate_reference_sample_len(reference_sample, plan.measurement_count)?;
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

pub fn convert_measurements_to_detection_events(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    convert_measurements_to_detection_events_with_limits(
        circuit,
        measurements,
        options,
        DetectionConversionLimits::default(),
    )
}

pub fn convert_measurements_to_detection_events_with_limits(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    let converter = CompiledDetectionConverter::compile_with_limits(circuit, options, limits)?;
    converter.plan.validate_shot_count(measurements.len())?;
    let mut records = Vec::new();
    try_reserve_detection_record_slots(&mut records, measurements.len())?;
    converter.try_for_each_detection_event(measurements.iter().map(Vec::as_slice), |record| {
        records.push(try_clone_detection_record(record)?);
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
    convert_measurements_to_detection_events_with_sweep_and_limits(
        circuit,
        measurements,
        sweeps,
        options,
        DetectionConversionLimits::default(),
    )
}

pub fn convert_measurements_to_detection_events_with_sweep_and_limits(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    sweeps: &[Vec<bool>],
    options: DetectionConversionOptions,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    if measurements.len() != sweeps.len() {
        return Err(CircuitError::invalid_result_format(format!(
            "measurement records have {} shots but sweep records have {} shots",
            measurements.len(),
            sweeps.len()
        )));
    }
    let converter = CompiledDetectionConverter::compile_with_limits(circuit, options, limits)?;
    converter.plan.validate_shot_count(measurements.len())?;
    validate_buffer_bits(
        DetectionBufferLimitSubject::SweepRecords,
        sweeps.len(),
        converter.sweep_bit_count(),
        limits.max_materialized_bits,
    )?;
    let mut records = Vec::new();
    try_reserve_detection_record_slots(&mut records, measurements.len())?;
    converter.try_for_each_detection_event_with_sweep(
        measurements.iter().map(Vec::as_slice),
        sweeps.iter().map(Vec::as_slice),
        |record| {
            records.push(try_clone_detection_record(record)?);
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
    sample_detection_events_with_limits(circuit, shots, seed, DetectionConversionLimits::default())
}

pub fn sample_detection_events_with_limits(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    api::sample_materialized(circuit, shots, seed, limits)
}

pub fn try_for_each_sampled_detection_event<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    try_for_each_sampled_detection_event_with_limits(
        circuit,
        shots,
        seed,
        DetectionConversionLimits::default(),
        visit,
    )
}

pub fn try_for_each_sampled_detection_event_with_limits<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
    visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    api::try_for_each(circuit, shots, seed, limits, visit)
}

pub fn measurement_record_count(circuit: &Circuit) -> CircuitResult<usize> {
    measurement_record_count_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn measurement_record_count_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<usize> {
    Ok(detection_conversion_plan_with_limits(circuit, limits)?.measurement_count)
}

pub fn detection_record_width(circuit: &Circuit) -> CircuitResult<usize> {
    detection_record_width_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn detection_record_width_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<usize> {
    detection_conversion_plan_with_limits(circuit, limits)?.output_bit_count()
}

pub fn validate_detection_sampling_circuit(circuit: &Circuit) -> CircuitResult<()> {
    validate_detection_sampling_circuit_with_limits(circuit, DetectionConversionLimits::default())
}

pub fn validate_detection_sampling_circuit_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<()> {
    api::validate(circuit, limits)
}

fn detection_conversion_plan_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<ConversionPlan> {
    if circuit_requires_detector_frame(circuit)? {
        return frame_conversion_plan_with_limits(circuit, limits);
    }
    ConversionPlan::from_circuit_with_limits(circuit, limits)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConversionPlan {
    limits: DetectionConversionLimits,
    measurement_count: usize,
    sweep_bit_count: usize,
    detector_count: usize,
    observable_count: usize,
    detector_terms: Vec<Vec<usize>>,
    observable_terms: Vec<Vec<usize>>,
    compiled_term_count: u64,
    expanded_instruction_count: u64,
    repeat_iteration_count: u64,
    collect_terms: bool,
}

impl ConversionPlan {
    fn from_circuit_with_limits(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<Self> {
        circuit_requires_detector_frame(circuit)?;
        Self::from_visitor(limits, |plan| plan.visit_circuit(circuit, 0))
    }

    fn from_visitor(
        limits: DetectionConversionLimits,
        mut visit: impl FnMut(&mut Self) -> CircuitResult<()>,
    ) -> CircuitResult<Self> {
        let admission = Self::admission_from_visitor(limits, &mut visit)?;
        Self::materialize_from_admission(admission, visit)
    }

    fn admission_from_visitor(
        limits: DetectionConversionLimits,
        mut visit: impl FnMut(&mut Self) -> CircuitResult<()>,
    ) -> CircuitResult<Self> {
        let mut admission = Self::new(limits, false);
        visit(&mut admission)?;
        admission.validate_compiled_shape()?;
        Ok(admission)
    }

    fn materialize_from_admission(
        admission: Self,
        mut visit: impl FnMut(&mut Self) -> CircuitResult<()>,
    ) -> CircuitResult<Self> {
        let mut plan = Self::new(admission.limits, true);
        plan.detector_terms
            .try_reserve_exact(admission.detector_count)
            .map_err(|error| {
                CircuitError::invalid_sampler_compilation(format!(
                    "unable to reserve {} detector terms: {error}",
                    admission.detector_count
                ))
            })?;
        plan.observable_terms
            .try_reserve_exact(admission.observable_count)
            .map_err(|error| {
                CircuitError::invalid_sampler_compilation(format!(
                    "unable to reserve {} observable terms: {error}",
                    admission.observable_count
                ))
            })?;
        visit(&mut plan)?;
        debug_assert_eq!(plan.measurement_count, admission.measurement_count);
        debug_assert_eq!(plan.sweep_bit_count, admission.sweep_bit_count);
        debug_assert_eq!(plan.detector_count, admission.detector_count);
        debug_assert_eq!(plan.observable_count, admission.observable_count);
        debug_assert_eq!(plan.compiled_term_count, admission.compiled_term_count);
        Ok(plan)
    }

    fn new(limits: DetectionConversionLimits, collect_terms: bool) -> Self {
        Self {
            limits,
            measurement_count: 0,
            sweep_bit_count: 0,
            detector_count: 0,
            observable_count: 0,
            detector_terms: Vec::new(),
            observable_terms: Vec::new(),
            compiled_term_count: 0,
            expanded_instruction_count: 0,
            repeat_iteration_count: 0,
            collect_terms,
        }
    }

    fn visit_circuit(&mut self, circuit: &Circuit, depth: usize) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => self.visit_repeat(repeat, depth)?,
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock, depth: usize) -> CircuitResult<()> {
        let next_depth = depth.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "detection conversion repeat nesting overflowed",
            )
        })?;
        if next_depth > RepeatNestingLimit::HARD_MAX {
            return Err(ResourceLimitError::detection_repeat_nesting(
                next_depth,
                RepeatNestingLimit::HARD_MAX,
            )
            .into());
        }
        self.visit_repeated_body(repeat.repeat_count().get(), |plan| {
            plan.visit_circuit(repeat.body(), next_depth)
        })
    }

    fn visit_repeated_body<F>(&mut self, repeat_count: u64, mut visit_body: F) -> CircuitResult<()>
    where
        F: FnMut(&mut Self) -> CircuitResult<()>,
    {
        if repeat_count > self.limits.max_repeat_unroll {
            return Err(ResourceLimitError::detection_repeat_count(
                repeat_count,
                self.limits.max_repeat_unroll,
            )
            .into());
        }
        let next_repeat_iterations = self
            .repeat_iteration_count
            .checked_add(repeat_count)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "detection conversion repeat iteration count overflowed",
                )
            })?;
        if next_repeat_iterations > self.limits.max_repeat_iterations {
            return Err(ResourceLimitError::detection_repeat_iterations(
                next_repeat_iterations,
                self.limits.max_repeat_iterations,
            )
            .into());
        }
        self.repeat_iteration_count = next_repeat_iterations;
        for _ in 0..repeat_count {
            visit_body(self)?;
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.record_expanded_instruction()?;
        self.record_sweep_bits(instruction)?;
        self.visit_instruction_semantics(instruction)
    }

    fn visit_frame_instruction_without_sweep(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        self.record_expanded_instruction()?;
        self.visit_instruction_semantics(instruction)
    }

    fn record_expanded_instruction(&mut self) -> CircuitResult<()> {
        let next_expanded_instruction_count = self
            .expanded_instruction_count
            .checked_add(1)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "detection conversion expanded instruction count overflowed",
                )
            })?;
        if next_expanded_instruction_count > self.limits.max_expanded_instructions {
            return Err(ResourceLimitError::detection_expanded_instructions(
                next_expanded_instruction_count,
                self.limits.max_expanded_instructions,
            )
            .into());
        }
        self.expanded_instruction_count = next_expanded_instruction_count;
        Ok(())
    }

    fn visit_instruction_semantics(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "SPP" | "SPP_DAG" => self.visit_decomposed_instruction(instruction),
            _ => self.add_measurements(instruction),
        }
    }

    fn visit_decomposed_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let decomposed =
            crate::analysis::decomposed_single_instruction(instruction).map_err(|error| {
                CircuitError::invalid_sampler_compilation(format!(
                    "{} cannot be converted via decomposition: {error}",
                    instruction.gate().canonical_name()
                ))
            })?;
        self.visit_circuit(&decomposed, 0)
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let next_width = self.output_bit_count()?.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_result_format(
                "detection record width overflowed while planning conversion",
            )
        })?;
        self.validate_record_width_value(next_width)?;
        let mut terms = Vec::new();
        if self.collect_terms {
            terms
                .try_reserve_exact(instruction.targets().len())
                .map_err(|error| {
                    CircuitError::invalid_sampler_compilation(format!(
                        "unable to reserve {} detector measurement references: {error}",
                        instruction.targets().len()
                    ))
                })?;
        }
        let mut term_count = 0_u64;
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            let measurement_index = self.measurement_index_from_offset(offset)?;
            term_count = term_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "detection conversion compiled term count overflowed",
                )
            })?;
            if self.collect_terms {
                terms.push(measurement_index);
            }
        }
        self.add_compiled_terms(term_count)?;
        self.detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_result_format(
                "detector count overflowed while planning conversion",
            )
        })?;
        if self.collect_terms {
            self.detector_terms.push(terms);
        }
        Ok(())
    }

    fn record_sweep_bits(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let mut found_sweep = None;
        let mut next_sweep_bit_count = self.sweep_bit_count;
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
                if next_count > self.limits.max_record_bits {
                    return Err(ResourceLimitError::detection_record_bits(
                        DetectionRecordLimitSubject::SweepRecord,
                        resource_amount(next_count, "sweep bit width")?,
                        resource_amount(self.limits.max_record_bits, "sweep bit limit")?,
                    )
                    .into());
                }
                next_sweep_bit_count = next_sweep_bit_count.max(next_count);
            }
        }
        let Some(target) = found_sweep else {
            return Ok(());
        };
        match instruction.gate().canonical_name() {
            "CX" | "CY" | "CZ" | "XCZ" | "YCZ" => {
                self.sweep_bit_count = next_sweep_bit_count;
                Ok(())
            }
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

        let mut terms = Vec::new();
        if self.collect_terms {
            terms
                .try_reserve_exact(instruction.targets().len())
                .map_err(|error| {
                    CircuitError::invalid_sampler_compilation(format!(
                        "unable to reserve {} observable measurement references: {error}",
                        instruction.targets().len()
                    ))
                })?;
        }
        let mut term_count = 0_u64;
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                let measurement_index = self.measurement_index_from_offset(offset)?;
                term_count = term_count.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "detection conversion compiled term count overflowed",
                    )
                })?;
                if self.collect_terms {
                    terms.push(measurement_index);
                }
            } else if target.is_pauli_target() {
                continue;
            } else {
                return Err(CircuitError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        self.add_compiled_terms(term_count)?;
        self.ensure_observable(observable_id)?;
        if !self.collect_terms {
            return Ok(());
        }
        let observable_terms = self
            .observable_terms
            .get_mut(observable_id)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "observable id {observable_id} was not initialized"
                ))
            })?;
        observable_terms.try_reserve(terms.len()).map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "unable to reserve {} observable measurement references: {error}",
                terms.len()
            ))
        })?;
        observable_terms.extend(terms);
        Ok(())
    }

    fn ensure_observable(&mut self, observable_id: usize) -> CircuitResult<()> {
        if observable_id >= self.limits.max_record_bits {
            let actual = observable_id.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "observable count overflowed while planning detection conversion",
                )
            })?;
            return Err(ResourceLimitError::detection_record_bits(
                DetectionRecordLimitSubject::ObservableCount,
                resource_amount(actual, "observable count")?,
                resource_amount(self.limits.max_record_bits, "observable count limit")?,
            )
            .into());
        }
        let next_observable_count = observable_id.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_result_format(
                "observable count overflowed while planning detection conversion",
            )
        })?;
        let next_width = self
            .detector_count
            .checked_add(next_observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })?;
        self.validate_record_width_value(next_width)?;
        self.observable_count = self.observable_count.max(next_observable_count);
        if self.collect_terms && self.observable_terms.len() < next_observable_count {
            let additional = next_observable_count - self.observable_terms.len();
            self.observable_terms
                .try_reserve_exact(additional)
                .map_err(|error| {
                    CircuitError::invalid_sampler_compilation(format!(
                        "unable to reserve {additional} observable term slots: {error}"
                    ))
                })?;
            self.observable_terms
                .resize_with(next_observable_count, Vec::new);
        }
        Ok(())
    }

    fn add_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let measurement_count = instruction.measurement_result_count();
        let next_measurement_count = self
            .measurement_count
            .checked_add(measurement_count)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "measurement record count overflowed during detection conversion planning",
                )
            })?;
        if next_measurement_count > self.limits.max_record_bits {
            return Err(ResourceLimitError::detection_record_bits(
                DetectionRecordLimitSubject::MeasurementRecord,
                resource_amount(next_measurement_count, "measurement record width")?,
                resource_amount(
                    self.limits.max_record_bits,
                    "measurement record width limit",
                )?,
            )
            .into());
        }
        self.measurement_count = next_measurement_count;
        Ok(())
    }

    fn output_bit_count(&self) -> CircuitResult<usize> {
        self.detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })
    }

    fn validate_record_width_value(&self, width: usize) -> CircuitResult<()> {
        if width > self.limits.max_record_bits {
            return Err(ResourceLimitError::detection_record_bits(
                DetectionRecordLimitSubject::DetectionRecord,
                resource_amount(width, "detection record width")?,
                resource_amount(self.limits.max_record_bits, "detection record width limit")?,
            )
            .into());
        }
        Ok(())
    }

    fn add_compiled_terms(&mut self, count: u64) -> CircuitResult<()> {
        self.compiled_term_count =
            self.compiled_term_count.checked_add(count).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "detection conversion compiled term count overflowed",
                )
            })?;
        if self.compiled_term_count > self.limits.max_compiled_terms {
            return Err(ResourceLimitError::detection_compiled_terms(
                self.compiled_term_count,
                self.limits.max_compiled_terms,
            )
            .into());
        }
        Ok(())
    }

    fn validate_compiled_shape(&self) -> CircuitResult<()> {
        validate_vector_capacity::<bool>(
            self.measurement_count,
            "detection conversion measurement record",
        )?;
        validate_vector_capacity::<bool>(
            self.sweep_bit_count,
            "detection conversion sweep record",
        )?;
        validate_vector_capacity::<bool>(
            self.detector_count,
            "detection conversion detector record",
        )?;
        validate_vector_capacity::<bool>(
            self.observable_count,
            "detection conversion observable record",
        )?;
        validate_vector_capacity::<Vec<usize>>(
            self.output_bit_count()?,
            "detection conversion term table",
        )?;
        let term_count = usize::try_from(self.compiled_term_count).map_err(|_| {
            ResourceLimitError::detection_compiled_terms(
                self.compiled_term_count,
                usize::MAX as u64,
            )
        })?;
        validate_vector_capacity::<usize>(
            term_count,
            "detection conversion measurement references",
        )?;
        let compiled_bytes = self.compiled_storage_bytes()?;
        if compiled_bytes > self.limits.max_compiled_bytes {
            return Err(ResourceLimitError::detection_compiled_bytes(
                compiled_bytes,
                self.limits.max_compiled_bytes,
            )
            .into());
        }
        Ok(())
    }

    fn compiled_storage_bytes(&self) -> CircuitResult<u64> {
        let outer_bytes = resource_amount(
            self.output_bit_count()?
                .checked_mul(std::mem::size_of::<Vec<usize>>())
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "detection conversion compiled byte count overflowed",
                    )
                })?,
            "compiled term table bytes",
        )?;
        let term_bytes = self
            .compiled_term_count
            .checked_mul(std::mem::size_of::<usize>() as u64)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "detection conversion compiled byte count overflowed",
                )
            })?;
        let compiled_bytes = outer_bytes.checked_add(term_bytes).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "detection conversion compiled byte count overflowed",
            )
        })?;
        Ok(compiled_bytes)
    }

    fn validate_shot_count(&self, shots: usize) -> CircuitResult<()> {
        validate_buffer_bits(
            DetectionBufferLimitSubject::MeasurementSamples,
            shots,
            self.measurement_count,
            self.limits.max_materialized_bits,
        )?;
        self.validate_detection_record_shot_count(shots)
    }

    fn validate_detection_record_shot_count(&self, shots: usize) -> CircuitResult<()> {
        validate_buffer_bits(
            DetectionBufferLimitSubject::DetectionRecords,
            shots,
            self.output_bit_count()?,
            self.limits.max_materialized_bits,
        )
    }

    fn measurement_index_from_offset(&self, offset: MeasureRecordOffset) -> CircuitResult<usize> {
        let current = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_result_format("measurement count does not fit i64")
        })?;
        let index = current
            .checked_add(i64::from(offset.get()))
            .ok_or_else(|| CircuitError::invalid_result_format("measurement reference overflow"))?;
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "measurement record target rec[{}] is not available",
                offset.stim_text()
            ))
        })?;
        if index >= self.measurement_count {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record target rec[{}] is not available",
                offset.stim_text()
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

#[cfg(test)]
mod tests;
