use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, MeasureRecordOffset, RepeatNestingLimit,
};

use super::buffers::{resource_amount, validate_vector_capacity};
use super::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use super::{
    DetectionConversionLimits, DetectionRecordBuffer, DetectionRecordLimitSubject,
    UNSUPPORTED_SWEEP_DETECTION_MESSAGE,
};

mod execution;

use self::execution::{ConversionCursor, RecordValues, execute_program, validate_repeat_marker};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConversionOperation {
    AdvanceMeasurements(usize),
    Detector(Vec<MeasureRecordOffset>),
    Observable {
        id: usize,
        terms: Vec<MeasureRecordOffset>,
    },
    Repeat {
        count: u64,
        body_end: usize,
        measurement_count: usize,
        detector_count: usize,
        requires_iteration: bool,
    },
    EndRepeat,
}

/// A compact measurement-to-detection program.
///
/// Repeat bodies are retained once. Expanded counts are computed arithmetically for admission,
/// while record offsets remain relative until a record is converted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConversionPlan {
    pub(super) limits: DetectionConversionLimits,
    pub(super) measurement_count: usize,
    pub(super) sweep_bit_count: usize,
    pub(super) detector_count: usize,
    pub(super) observable_count: usize,
    program: Vec<ConversionOperation>,
    compiled_term_count: u64,
    retained_term_capacity: u64,
    compact_operation_count: u64,
    iteration_operation_count: u64,
    expanded_instruction_count: u64,
    repeat_iteration_count: u64,
    collect_program: bool,
}

#[derive(Clone, Copy)]
struct RepeatSnapshot {
    measurement_count: usize,
    detector_count: usize,
    expanded_instruction_count: u64,
    repeat_iteration_count: u64,
    compact_operation_count: u64,
    iteration_operation_count: u64,
}

#[derive(Clone, Copy)]
struct RepeatFinish {
    count: u64,
    snapshot: RepeatSnapshot,
    marker_index: Option<usize>,
}

#[derive(Clone, Copy)]
struct CircuitVisitFrame<'a> {
    circuit: &'a Circuit,
    next_item: usize,
    depth: usize,
    finish: Option<RepeatFinish>,
}

impl ConversionPlan {
    pub(super) fn from_circuit_with_limits(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<Self> {
        let admission = Self::admission_from_circuit_with_limits(circuit, limits)?;
        Self::materialize_circuit_from_admission(circuit, admission)
    }

    pub(super) fn admission_from_circuit_with_limits(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<Self> {
        Self::admission_from_visitor(limits, |plan| plan.visit_circuit(circuit, 0))
    }

    pub(super) fn materialize_circuit_from_admission(
        circuit: &Circuit,
        admission: Self,
    ) -> DetectionResult<Self> {
        Self::materialize_from_admission(admission, |plan| plan.visit_circuit(circuit, 0))
    }

    #[cfg(test)]
    pub(super) fn from_visitor(
        limits: DetectionConversionLimits,
        mut visit: impl FnMut(&mut Self) -> DetectionResult<()>,
    ) -> DetectionResult<Self> {
        let admission = Self::admission_from_visitor(limits, &mut visit)?;
        Self::materialize_from_admission(admission, visit)
    }

    pub(super) fn admission_from_visitor(
        limits: DetectionConversionLimits,
        mut visit: impl FnMut(&mut Self) -> DetectionResult<()>,
    ) -> DetectionResult<Self> {
        let mut admission = Self::new(limits, false);
        visit(&mut admission)?;
        admission.validate_compiled_shape()?;
        Ok(admission)
    }

    pub(super) fn materialize_from_admission(
        admission: Self,
        mut visit: impl FnMut(&mut Self) -> DetectionResult<()>,
    ) -> DetectionResult<Self> {
        let mut plan = Self::new(admission.limits, true);
        let operation_count = usize::try_from(admission.compact_operation_count)
            .map_err(|_| compilation_overflow())?;
        plan.program
            .try_reserve_exact(operation_count)
            .map_err(|error| {
                DetectionError::invalid_sampler_compilation(format!(
                    "unable to reserve {operation_count} compact conversion operations: {error}"
                ))
            })?;
        plan.validate_materialized_storage_with_terms(admission.compiled_term_count)?;
        visit(&mut plan)?;
        plan.validate_against_admission(&admission)?;
        plan.validate_compiled_shape()?;
        Ok(plan)
    }

    pub(super) fn new(limits: DetectionConversionLimits, collect_program: bool) -> Self {
        Self {
            limits,
            measurement_count: 0,
            sweep_bit_count: 0,
            detector_count: 0,
            observable_count: 0,
            program: Vec::new(),
            compiled_term_count: 0,
            retained_term_capacity: 0,
            compact_operation_count: 0,
            iteration_operation_count: 0,
            expanded_instruction_count: 0,
            repeat_iteration_count: 0,
            collect_program,
        }
    }

    pub(super) fn visit_circuit(&mut self, circuit: &Circuit, depth: usize) -> DetectionResult<()> {
        if depth > RepeatNestingLimit::HARD_MAX {
            return Err(ResourceLimitError::detection_repeat_nesting(
                depth,
                RepeatNestingLimit::HARD_MAX,
            )
            .into());
        }
        let mut stack = [None; RepeatNestingLimit::HARD_MAX + 1];
        stack[0] = Some(CircuitVisitFrame {
            circuit,
            next_item: 0,
            depth,
            finish: None,
        });
        let mut stack_len = 1_usize;

        while stack_len != 0 {
            let frame_index = stack_len.checked_sub(1).ok_or_else(compilation_overflow)?;
            let (item, frame_depth) = {
                let frame = stack
                    .get_mut(frame_index)
                    .and_then(Option::as_mut)
                    .ok_or_else(compilation_shape_mismatch)?;
                let item = frame.circuit.items().get(frame.next_item);
                if item.is_some() {
                    frame.next_item = frame
                        .next_item
                        .checked_add(1)
                        .ok_or_else(compilation_overflow)?;
                }
                (item, frame.depth)
            };
            let Some(item) = item else {
                let frame = stack
                    .get_mut(frame_index)
                    .and_then(Option::take)
                    .ok_or_else(compilation_shape_mismatch)?;
                stack_len = frame_index;
                if let Some(finish) = frame.finish {
                    self.integrate_repeated_body(
                        finish.count,
                        finish.snapshot,
                        finish.marker_index,
                    )?;
                }
                continue;
            };
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    let next_depth = frame_depth.checked_add(1).ok_or_else(|| {
                        DetectionError::invalid_sampler_compilation(
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
                    let (snapshot, marker_index) = self.begin_repeated_body()?;
                    let slot = stack
                        .get_mut(stack_len)
                        .ok_or_else(compilation_shape_mismatch)?;
                    *slot = Some(CircuitVisitFrame {
                        circuit: repeat.body(),
                        next_item: 0,
                        depth: next_depth,
                        finish: Some(RepeatFinish {
                            count: repeat.repeat_count().get(),
                            snapshot,
                            marker_index,
                        }),
                    });
                    stack_len = stack_len.checked_add(1).ok_or_else(compilation_overflow)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn visit_repeated_body<F>(
        &mut self,
        repeat_count: u64,
        mut visit_body: F,
    ) -> DetectionResult<()>
    where
        F: FnMut(&mut Self) -> DetectionResult<()>,
    {
        let (snapshot, marker_index) = self.begin_repeated_body()?;
        visit_body(self)?;
        self.integrate_repeated_body(repeat_count, snapshot, marker_index)
    }

    fn begin_repeated_body(&mut self) -> DetectionResult<(RepeatSnapshot, Option<usize>)> {
        let snapshot = RepeatSnapshot {
            measurement_count: self.measurement_count,
            detector_count: self.detector_count,
            expanded_instruction_count: self.expanded_instruction_count,
            repeat_iteration_count: self.repeat_iteration_count,
            compact_operation_count: self.compact_operation_count,
            iteration_operation_count: self.iteration_operation_count,
        };
        let marker_index = if self.collect_program && self.program.len() < self.program.capacity() {
            let index = self.program.len();
            self.program.push(ConversionOperation::Repeat {
                count: 0,
                body_end: 0,
                measurement_count: 0,
                detector_count: 0,
                requires_iteration: false,
            });
            Some(index)
        } else {
            None
        };
        Ok((snapshot, marker_index))
    }

    fn integrate_repeated_body(
        &mut self,
        repeat_count: u64,
        snapshot: RepeatSnapshot,
        marker_index: Option<usize>,
    ) -> DetectionResult<()> {
        let body_measurements = self
            .measurement_count
            .checked_sub(snapshot.measurement_count)
            .ok_or_else(compilation_overflow)?;
        let body_detectors = self
            .detector_count
            .checked_sub(snapshot.detector_count)
            .ok_or_else(compilation_overflow)?;
        self.measurement_count = repeated_usize_total(
            snapshot.measurement_count,
            body_measurements,
            repeat_count,
            "measurement record count",
        )?;
        self.validate_measurement_width()?;
        self.detector_count = repeated_usize_total(
            snapshot.detector_count,
            body_detectors,
            repeat_count,
            "detector count",
        )?;
        self.validate_record_width_value(self.output_bit_count()?)?;

        let body_expanded_instructions = self
            .expanded_instruction_count
            .checked_sub(snapshot.expanded_instruction_count)
            .ok_or_else(compilation_overflow)?;
        self.expanded_instruction_count = repeated_u64_total(
            snapshot.expanded_instruction_count,
            body_expanded_instructions,
            repeat_count,
        )?;
        self.validate_expanded_instruction_count()?;

        let body_repeat_iterations = self
            .repeat_iteration_count
            .checked_sub(snapshot.repeat_iteration_count)
            .ok_or_else(compilation_overflow)?;
        let nested_iterations = body_repeat_iterations
            .checked_mul(repeat_count)
            .ok_or_else(compilation_overflow)?;
        self.repeat_iteration_count = snapshot
            .repeat_iteration_count
            .checked_add(
                repeat_count
                    .checked_add(nested_iterations)
                    .ok_or_else(compilation_overflow)?,
            )
            .ok_or_else(compilation_overflow)?;
        self.validate_repeat_iteration_count()?;

        let body_operation_count = self
            .compact_operation_count
            .checked_sub(snapshot.compact_operation_count)
            .ok_or_else(compilation_overflow)?;
        if body_operation_count == 0 {
            if let Some(index) = marker_index {
                if index.checked_add(1) != Some(self.program.len()) {
                    return Err(compilation_shape_mismatch());
                }
                self.program.pop().ok_or_else(compilation_shape_mismatch)?;
            }
            return Ok(());
        }
        self.add_compact_operations(2)?;
        if self.collect_program {
            let index = marker_index.ok_or_else(compilation_shape_mismatch)?;
            self.try_push_operation(ConversionOperation::EndRepeat)?;
            let body_end = self
                .program
                .len()
                .checked_sub(1)
                .ok_or_else(compilation_shape_mismatch)?;
            let requires_iteration =
                self.iteration_operation_count > snapshot.iteration_operation_count;
            let marker = self
                .program
                .get_mut(index)
                .ok_or_else(compilation_shape_mismatch)?;
            *marker = ConversionOperation::Repeat {
                count: repeat_count,
                body_end,
                measurement_count: body_measurements,
                detector_count: body_detectors,
                requires_iteration,
            };
        }
        Ok(())
    }

    pub(super) fn visit_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> DetectionResult<()> {
        self.add_expanded_instructions(1)?;
        self.record_sweep_bits(instruction)?;
        self.visit_instruction_semantics(instruction)
    }

    fn visit_instruction_semantics(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> DetectionResult<()> {
        match instruction.gate().canonical_name() {
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            // SPP instructions do not produce measurement results. Frame execution validates and
            // applies their quantum effect separately, so detection-conversion admission must not
            // allocate a temporary decomposed circuit for them.
            "SPP" | "SPP_DAG" => Ok(()),
            _ => self.add_measurements(instruction),
        }
    }

    pub(super) fn record_detector(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> DetectionResult<()> {
        let next_detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            DetectionError::invalid_result_format(
                "detector count overflowed while planning conversion",
            )
        })?;
        let next_width = next_detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                DetectionError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })?;
        self.validate_record_width_value(next_width)?;

        let mut terms = Vec::new();
        if self.collect_program {
            terms
                .try_reserve_exact(instruction.targets().len())
                .map_err(|error| {
                    DetectionError::invalid_sampler_compilation(format!(
                        "unable to reserve {} detector measurement references: {error}",
                        instruction.targets().len()
                    ))
                })?;
        }
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                DetectionError::invalid_result_format(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            self.validate_measurement_offset(offset)?;
            if self.collect_program {
                terms.push(offset);
            }
        }
        self.add_compiled_terms(resource_amount(
            instruction.targets().len(),
            "detector measurement-reference count",
        )?)?;
        if self.collect_program {
            self.add_retained_term_capacity(resource_amount(
                terms.capacity(),
                "detector measurement-reference capacity",
            )?)?;
        }
        self.add_root_operation(true)?;
        self.detector_count = next_detector_count;
        if self.collect_program {
            self.try_push_operation(ConversionOperation::Detector(terms))?;
        }
        Ok(())
    }

    fn record_sweep_bits(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let mut found_sweep = None;
        let mut next_sweep_bit_count = self.sweep_bit_count;
        for target in instruction.targets() {
            if let Some(sweep_id) = target.sweep_bit_id() {
                found_sweep = Some(target.clone());
                let next_count = usize::try_from(sweep_id)
                    .ok()
                    .and_then(|id| id.checked_add(1))
                    .ok_or_else(|| {
                        DetectionError::invalid_result_format(format!(
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
            name => Err(DetectionError::invalid_result_format(format!(
                "{UNSUPPORTED_SWEEP_DETECTION_MESSAGE}; found {target} in {name}"
            ))),
        }
    }

    pub(super) fn record_observable(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> DetectionResult<()> {
        let observable_id = instruction.observable_id_argument()?.ok_or_else(|| {
            DetectionError::invalid_result_format("OBSERVABLE_INCLUDE missing id")
        })?;
        let observable_id = usize::try_from(observable_id.get()).map_err(|_| {
            DetectionError::invalid_result_format(format!(
                "observable id {} does not fit usize",
                observable_id.get()
            ))
        })?;
        self.ensure_observable(observable_id)?;

        let term_count = instruction
            .targets()
            .iter()
            .filter(|target| target.measurement_record_offset().is_some())
            .count();
        let mut terms = Vec::new();
        if self.collect_program {
            terms.try_reserve_exact(term_count).map_err(|error| {
                DetectionError::invalid_sampler_compilation(format!(
                    "unable to reserve {term_count} observable measurement references: {error}"
                ))
            })?;
        }
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                self.validate_measurement_offset(offset)?;
                if self.collect_program {
                    terms.push(offset);
                }
            } else if !target.is_pauli_target() {
                return Err(DetectionError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        if term_count == 0 {
            return Ok(());
        }
        self.add_compiled_terms(resource_amount(
            term_count,
            "observable measurement-reference count",
        )?)?;
        if self.collect_program {
            self.add_retained_term_capacity(resource_amount(
                terms.capacity(),
                "observable measurement-reference capacity",
            )?)?;
        }
        self.add_root_operation(true)?;
        if self.collect_program {
            self.try_push_operation(ConversionOperation::Observable {
                id: observable_id,
                terms,
            })?;
        }
        Ok(())
    }

    fn ensure_observable(&mut self, observable_id: usize) -> DetectionResult<()> {
        if observable_id >= self.limits.max_record_bits {
            let actual = observable_id.checked_add(1).ok_or_else(|| {
                DetectionError::invalid_result_format(
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
            DetectionError::invalid_result_format(
                "observable count overflowed while planning detection conversion",
            )
        })?;
        let next_width = self
            .detector_count
            .checked_add(next_observable_count)
            .ok_or_else(|| {
                DetectionError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })?;
        self.validate_record_width_value(next_width)?;
        self.observable_count = self.observable_count.max(next_observable_count);
        Ok(())
    }

    fn add_measurements(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let count = stab_model::advanced::circuit_instruction_measurement_result_count(instruction);
        if count == 0 {
            return Ok(());
        }
        self.measurement_count = self.measurement_count.checked_add(count).ok_or_else(|| {
            DetectionError::invalid_result_format(
                "measurement record count overflowed during detection conversion planning",
            )
        })?;
        self.validate_measurement_width()?;
        self.add_root_operation(false)?;
        if self.collect_program {
            self.try_push_operation(ConversionOperation::AdvanceMeasurements(count))?;
        }
        Ok(())
    }

    fn validate_measurement_width(&self) -> DetectionResult<()> {
        if self.measurement_count > self.limits.max_record_bits {
            return Err(ResourceLimitError::detection_record_bits(
                DetectionRecordLimitSubject::MeasurementRecord,
                resource_amount(self.measurement_count, "measurement record width")?,
                resource_amount(
                    self.limits.max_record_bits,
                    "measurement record width limit",
                )?,
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn output_bit_count(&self) -> DetectionResult<usize> {
        self.detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                DetectionError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })
    }

    fn validate_record_width_value(&self, width: usize) -> DetectionResult<()> {
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

    fn validate_measurement_offset(&self, offset: MeasureRecordOffset) -> DetectionResult<()> {
        measurement_index_from_offset(self.measurement_count, offset).map(|_| ())
    }

    fn add_root_operation(&mut self, requires_iteration: bool) -> DetectionResult<()> {
        self.add_compact_operations(1)?;
        if requires_iteration {
            self.iteration_operation_count = self
                .iteration_operation_count
                .checked_add(1)
                .ok_or_else(compilation_overflow)?;
        }
        Ok(())
    }

    fn try_push_operation(&mut self, operation: ConversionOperation) -> DetectionResult<()> {
        if self.program.len() == self.program.capacity() {
            self.program.try_reserve(1).map_err(|error| {
                DetectionError::invalid_sampler_compilation(format!(
                    "unable to reserve compact conversion operation: {error}"
                ))
            })?;
            self.validate_compiled_bytes()?;
        }
        self.program.push(operation);
        Ok(())
    }

    fn add_expanded_instructions(&mut self, count: u64) -> DetectionResult<()> {
        self.expanded_instruction_count = self
            .expanded_instruction_count
            .checked_add(count)
            .ok_or_else(compilation_overflow)?;
        self.validate_expanded_instruction_count()
    }

    fn validate_expanded_instruction_count(&self) -> DetectionResult<()> {
        if self.expanded_instruction_count > self.limits.max_expanded_instructions {
            return Err(ResourceLimitError::detection_expanded_instructions(
                self.expanded_instruction_count,
                self.limits.max_expanded_instructions,
            )
            .into());
        }
        Ok(())
    }

    fn validate_repeat_iteration_count(&self) -> DetectionResult<()> {
        if self.repeat_iteration_count > self.limits.max_repeat_iterations {
            return Err(ResourceLimitError::detection_repeat_iterations(
                self.repeat_iteration_count,
                self.limits.max_repeat_iterations,
            )
            .into());
        }
        Ok(())
    }

    fn add_compiled_terms(&mut self, count: u64) -> DetectionResult<()> {
        self.compiled_term_count = self
            .compiled_term_count
            .checked_add(count)
            .ok_or_else(compilation_overflow)?;
        if self.compiled_term_count > self.limits.max_compiled_terms {
            return Err(ResourceLimitError::detection_compiled_terms(
                self.compiled_term_count,
                self.limits.max_compiled_terms,
            )
            .into());
        }
        self.validate_compiled_bytes()
    }

    fn add_retained_term_capacity(&mut self, count: u64) -> DetectionResult<()> {
        self.retained_term_capacity = self
            .retained_term_capacity
            .checked_add(count)
            .ok_or_else(compilation_overflow)?;
        self.validate_compiled_bytes()
    }

    fn add_compact_operations(&mut self, operations: u64) -> DetectionResult<()> {
        self.compact_operation_count = self
            .compact_operation_count
            .checked_add(operations)
            .ok_or_else(compilation_overflow)?;
        self.validate_compiled_bytes()
    }

    fn validate_compiled_shape(&self) -> DetectionResult<()> {
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
        let operation_count =
            usize::try_from(self.compact_operation_count).map_err(|_| compilation_overflow())?;
        validate_vector_capacity::<ConversionOperation>(
            operation_count,
            "compact detection conversion operations",
        )?;
        let term_count =
            usize::try_from(self.compiled_term_count).map_err(|_| compilation_overflow())?;
        validate_vector_capacity::<MeasureRecordOffset>(
            term_count,
            "detection conversion measurement references",
        )?;
        self.validate_compiled_bytes()?;
        if self.collect_program {
            self.validate_materialized_program()?;
        }
        Ok(())
    }

    fn validate_against_admission(&self, admission: &Self) -> DetectionResult<()> {
        for (field, expected, actual) in [
            (
                "measurement count",
                resource_amount(admission.measurement_count, "admitted measurement count")?,
                resource_amount(self.measurement_count, "materialized measurement count")?,
            ),
            (
                "sweep-bit count",
                resource_amount(admission.sweep_bit_count, "admitted sweep-bit count")?,
                resource_amount(self.sweep_bit_count, "materialized sweep-bit count")?,
            ),
            (
                "detector count",
                resource_amount(admission.detector_count, "admitted detector count")?,
                resource_amount(self.detector_count, "materialized detector count")?,
            ),
            (
                "observable count",
                resource_amount(admission.observable_count, "admitted observable count")?,
                resource_amount(self.observable_count, "materialized observable count")?,
            ),
            (
                "compiled term count",
                admission.compiled_term_count,
                self.compiled_term_count,
            ),
            (
                "compact operation count",
                admission.compact_operation_count,
                self.compact_operation_count,
            ),
            (
                "iteration operation count",
                admission.iteration_operation_count,
                self.iteration_operation_count,
            ),
            (
                "expanded instruction count",
                admission.expanded_instruction_count,
                self.expanded_instruction_count,
            ),
            (
                "repeat iteration count",
                admission.repeat_iteration_count,
                self.repeat_iteration_count,
            ),
        ] {
            if actual != expected {
                return Err(compilation_admission_mismatch(field, expected, actual));
            }
        }
        Ok(())
    }

    fn validate_materialized_program(&self) -> DetectionResult<()> {
        let retained_operations = resource_amount(
            self.program.len(),
            "materialized compact conversion operation count",
        )?;
        if retained_operations != self.compact_operation_count {
            return Err(compilation_admission_mismatch(
                "retained compact operation count",
                self.compact_operation_count,
                retained_operations,
            ));
        }

        let mut retained_terms = 0_u64;
        let mut retained_term_capacity = 0_u64;
        let mut repeat_ends = [None; RepeatNestingLimit::HARD_MAX];
        let mut repeat_depth = 0_usize;
        for (index, operation) in self.program.iter().enumerate() {
            let terms = match operation {
                ConversionOperation::Detector(terms)
                | ConversionOperation::Observable { terms, .. } => Some(terms),
                ConversionOperation::Repeat { body_end, .. } => {
                    validate_repeat_marker(&self.program, index, *body_end)?;
                    let slot = repeat_ends
                        .get_mut(repeat_depth)
                        .ok_or_else(compilation_shape_mismatch)?;
                    *slot = Some(*body_end);
                    repeat_depth = repeat_depth
                        .checked_add(1)
                        .ok_or_else(compilation_overflow)?;
                    None
                }
                ConversionOperation::EndRepeat => {
                    let frame_index = repeat_depth
                        .checked_sub(1)
                        .ok_or_else(compilation_shape_mismatch)?;
                    let expected_end = repeat_ends
                        .get_mut(frame_index)
                        .and_then(Option::take)
                        .ok_or_else(compilation_shape_mismatch)?;
                    if expected_end != index {
                        return Err(compilation_shape_mismatch());
                    }
                    repeat_depth = frame_index;
                    None
                }
                ConversionOperation::AdvanceMeasurements(_) => None,
            };
            if let Some(terms) = terms {
                retained_terms = retained_terms
                    .checked_add(resource_amount(terms.len(), "retained conversion terms")?)
                    .ok_or_else(compilation_overflow)?;
                retained_term_capacity = retained_term_capacity
                    .checked_add(resource_amount(
                        terms.capacity(),
                        "retained conversion term capacity",
                    )?)
                    .ok_or_else(compilation_overflow)?;
            }
        }
        if repeat_depth != 0 {
            return Err(compilation_shape_mismatch());
        }
        if retained_terms != self.compiled_term_count {
            return Err(compilation_admission_mismatch(
                "retained compiled term count",
                self.compiled_term_count,
                retained_terms,
            ));
        }
        if retained_term_capacity != self.retained_term_capacity {
            return Err(compilation_admission_mismatch(
                "retained compiled term capacity",
                self.retained_term_capacity,
                retained_term_capacity,
            ));
        }
        Ok(())
    }

    fn validate_compiled_bytes(&self) -> DetectionResult<()> {
        let bytes = self.compiled_storage_bytes()?;
        if bytes > self.limits.max_compiled_bytes {
            return Err(ResourceLimitError::detection_compiled_bytes(
                bytes,
                self.limits.max_compiled_bytes,
            )
            .into());
        }
        Ok(())
    }

    fn validate_materialized_storage_with_terms(&self, term_count: u64) -> DetectionResult<()> {
        let operation_count = resource_amount(
            self.program.capacity(),
            "compact conversion operation capacity",
        )?;
        let bytes = compiled_storage_bytes(operation_count, term_count)?;
        if bytes > self.limits.max_compiled_bytes {
            return Err(ResourceLimitError::detection_compiled_bytes(
                bytes,
                self.limits.max_compiled_bytes,
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn compiled_storage_bytes(&self) -> DetectionResult<u64> {
        let operation_count = if self.collect_program {
            resource_amount(
                self.program.capacity(),
                "compact conversion operation capacity",
            )?
        } else {
            self.compact_operation_count
        };
        let term_count = if self.collect_program {
            self.retained_term_capacity
        } else {
            self.compiled_term_count
        };
        compiled_storage_bytes(operation_count, term_count)
    }

    pub(super) fn reference_signs_into(
        &self,
        reference_sample: &[bool],
        record: &mut DetectionRecordBuffer,
    ) -> DetectionResult<()> {
        super::reference::validate_reference_sample_len(reference_sample, self.measurement_count)?;
        self.execute_record(
            RecordValues::Zero,
            RecordValues::Values(reference_sample),
            record,
        )
    }

    fn execute_record(
        &self,
        measurements: RecordValues<'_, bool>,
        reference_sample: RecordValues<'_, bool>,
        record: &mut DetectionRecordBuffer,
    ) -> DetectionResult<()> {
        record.detectors.resize(self.detector_count, false);
        record.detectors.fill(false);
        record.observables.resize(self.observable_count, false);
        record.observables.fill(false);
        let mut cursor = ConversionCursor::default();
        execute_program(
            &self.program,
            measurements,
            reference_sample,
            &mut record.detectors,
            &mut record.observables,
            &mut cursor,
        )?;
        if cursor.measurement != self.measurement_count || cursor.detector != self.detector_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "compact conversion finished at measurement {} and detector {}, expected {} and {}",
                cursor.measurement, cursor.detector, self.measurement_count, self.detector_count
            )));
        }
        Ok(())
    }

    pub(super) fn convert_word_planes_into(
        &self,
        measurement_planes: &[u64],
        detector_planes: &mut Vec<u64>,
        observable_planes: &mut Vec<u64>,
    ) -> DetectionResult<()> {
        if measurement_planes.len() != self.measurement_count {
            return Err(DetectionError::invalid_result_format(format!(
                "measurement plane count {} does not match the compiled width {}",
                measurement_planes.len(),
                self.measurement_count
            )));
        }
        detector_planes.resize(self.detector_count, 0);
        detector_planes.fill(0);
        observable_planes.resize(self.observable_count, 0);
        observable_planes.fill(0);
        let mut cursor = ConversionCursor::default();
        execute_program(
            &self.program,
            RecordValues::Values(measurement_planes),
            RecordValues::Zero,
            detector_planes,
            observable_planes,
            &mut cursor,
        )?;
        if cursor.measurement != self.measurement_count || cursor.detector != self.detector_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "compact word conversion finished at measurement {} and detector {}, expected {} and {}",
                cursor.measurement, cursor.detector, self.measurement_count, self.detector_count
            )));
        }
        Ok(())
    }
}

fn measurement_index_from_offset(
    measurement_count: usize,
    offset: MeasureRecordOffset,
) -> DetectionResult<usize> {
    let current = i64::try_from(measurement_count)
        .map_err(|_| DetectionError::invalid_result_format("measurement count does not fit i64"))?;
    let index = current
        .checked_add(i64::from(offset.get()))
        .ok_or_else(|| DetectionError::invalid_result_format("measurement reference overflow"))?;
    let index = usize::try_from(index).map_err(|_| unavailable_measurement(offset))?;
    if index >= measurement_count {
        return Err(unavailable_measurement(offset));
    }
    Ok(index)
}

fn unavailable_measurement(offset: MeasureRecordOffset) -> DetectionError {
    DetectionError::invalid_result_format(format!(
        "measurement record target rec[{}] is not available",
        offset.stim_text()
    ))
}

fn repeated_usize_total(
    initial: usize,
    per_iteration: usize,
    repeat_count: u64,
    subject: &str,
) -> DetectionResult<usize> {
    let repeated = u64::try_from(per_iteration)
        .ok()
        .and_then(|count| count.checked_mul(repeat_count))
        .ok_or_else(compilation_overflow)?;
    let repeated = usize::try_from(repeated).map_err(|_| compilation_overflow())?;
    initial
        .checked_add(repeated)
        .ok_or_else(|| DetectionError::invalid_sampler_compilation(format!("{subject} overflowed")))
}

fn repeated_u64_total(initial: u64, per_iteration: u64, repeat_count: u64) -> DetectionResult<u64> {
    per_iteration
        .checked_mul(repeat_count)
        .and_then(|repeated| initial.checked_add(repeated))
        .ok_or_else(compilation_overflow)
}

fn compiled_storage_bytes(operation_count: u64, term_count: u64) -> DetectionResult<u64> {
    let operation_bytes = operation_count
        .checked_mul(size_of::<ConversionOperation>() as u64)
        .ok_or_else(compilation_overflow)?;
    let term_bytes = term_count
        .checked_mul(size_of::<MeasureRecordOffset>() as u64)
        .ok_or_else(compilation_overflow)?;
    operation_bytes
        .checked_add(term_bytes)
        .ok_or_else(compilation_overflow)
}

fn compilation_shape_mismatch() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detection conversion materialization disagreed with admission",
    )
}

fn compilation_admission_mismatch(field: &str, expected: u64, actual: u64) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "compact detection conversion {field} disagreed with admission: expected {expected}, got {actual}"
    ))
}

fn compilation_overflow() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "compact detection conversion resource accounting overflowed",
    )
}
