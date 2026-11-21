use arrayvec::ArrayVec;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, GateKind, MeasureRecordOffset, RepeatNestingLimit,
};

use super::buffers::{resource_amount, validate_vector_capacity};
use super::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use super::frame::{TraversalEvent, visit_circuit};
use super::{
    DetectionConversionLimits, DetectionRecordBuffer, UNSUPPORTED_SWEEP_DETECTION_MESSAGE,
};
use crate::ReferenceSample;
use crate::work_poll::{WORD_OP_UNITS, WorkPoll};

mod execution;

use self::execution::{
    ConversionCursor, ConversionIo, RecordValues, execute_program, validate_repeat_marker,
};
pub(super) use self::execution::{ConversionRunner, ConversionStop};

/// A run of consecutive measurement references in a [`CompactProgram`]'s term arena.
///
/// Spans are cut only by [`CompactProgram::span_since`], so `start <= end <= terms.len()`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TermSpan {
    start: usize,
    end: usize,
}

impl TermSpan {
    const fn len(self) -> usize {
        self.end - self.start
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConversionOperation {
    AdvanceMeasurements(usize),
    /// A detector of at most [`execution::SHORT_PARITY_TERMS`] terms, charged to the work poll
    /// as one constant step.
    Detector(TermSpan),
    /// A detector with more terms, charged in proportion to them; lowering picks the variant so
    /// the common detector's hot loop never compares its length.
    WideDetector(TermSpan),
    Observable {
        id: usize,
        terms: TermSpan,
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

/// The operations of a compact conversion and the one arena holding every detector and
/// observable term, so a circuit of millions of detectors reserves one term vector instead of
/// one per detector.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CompactProgram {
    operations: Vec<ConversionOperation>,
    terms: Vec<MeasureRecordOffset>,
}

impl CompactProgram {
    /// The term arena, borrowed once per run so the hot loop keeps its bounds in registers
    /// instead of reloading them through the program after every detector store.
    fn arena(&self) -> TermArena<'_> {
        TermArena(&self.terms)
    }

    /// Appends `offset` within the term reservation admission sized exactly.
    fn push_term(&mut self, offset: MeasureRecordOffset) -> DetectionResult<()> {
        if self.terms.len() == self.terms.capacity() {
            return Err(compilation_shape_mismatch());
        }
        self.terms.push(offset);
        Ok(())
    }

    /// The span of the terms pushed since the arena held `start` terms.
    fn span_since(&self, start: usize) -> DetectionResult<TermSpan> {
        let end = self.terms.len();
        if start > end {
            return Err(compilation_shape_mismatch());
        }
        Ok(TermSpan { start, end })
    }
}

/// A borrowed [`CompactProgram`] term arena.
#[derive(Clone, Copy)]
struct TermArena<'a>(&'a [MeasureRecordOffset]);

impl<'a> TermArena<'a> {
    /// The terms of `span`.
    #[inline(always)]
    fn terms(self, span: TermSpan) -> DetectionResult<&'a [MeasureRecordOffset]> {
        self.0
            .get(span.start..span.end)
            .ok_or_else(terms_escaped_arena)
    }
}

#[cold]
#[inline(never)]
fn terms_escaped_arena() -> DetectionError {
    DetectionError::invalid_sampler_compilation("compact conversion terms escaped their arena")
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
    program: CompactProgram,
    compiled_term_count: u64,
    compact_operation_count: u64,
    iteration_operation_count: u64,
    collect_program: bool,
}

#[derive(Clone, Copy)]
struct RepeatSnapshot {
    measurement_count: usize,
    detector_count: usize,
    compact_operation_count: u64,
    iteration_operation_count: u64,
}

#[derive(Clone, Copy)]
struct RepeatFinish {
    count: u64,
    snapshot: RepeatSnapshot,
    marker_index: Option<usize>,
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
        Self::admission_from_visitor(limits, |plan| plan.visit_circuit(circuit))
    }

    pub(super) fn materialize_circuit_from_admission(
        circuit: &Circuit,
        admission: Self,
    ) -> DetectionResult<Self> {
        Self::materialize_from_admission(admission, |plan| plan.visit_circuit(circuit))
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
        let mut plan = Self::new(admission.limits.clone(), true);
        let operation_count = usize::try_from(admission.compact_operation_count)
            .map_err(|_| compilation_overflow())?;
        let term_count =
            usize::try_from(admission.compiled_term_count).map_err(|_| compilation_overflow())?;
        // Both reservations are admitted before either is made: admission counted them exactly.
        plan.admit_bytes(
            compiled_storage_bytes(
                admission.compact_operation_count,
                admission.compiled_term_count,
            )?
            .checked_add(plan.record_storage_bytes()?)
            .ok_or_else(compilation_overflow)?,
        )?;
        plan.program
            .operations
            .try_reserve_exact(operation_count)
            .map_err(|error| {
                DetectionError::storage_allocation(format!(
                    "unable to reserve {operation_count} compact conversion operations: {error}"
                ))
            })?;
        plan.program
            .terms
            .try_reserve_exact(term_count)
            .map_err(|error| {
                DetectionError::storage_allocation(format!(
                    "unable to reserve {term_count} detection conversion measurement references: {error}"
                ))
            })?;
        plan.validate_compiled_bytes()?;
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
            program: CompactProgram::default(),
            compiled_term_count: 0,
            compact_operation_count: 0,
            iteration_operation_count: 0,
            collect_program,
        }
    }

    /// Visits `circuit` through the engine's one folded circuit walker, integrating each
    /// repeat body arithmetically when the walker closes it.
    pub(super) fn visit_circuit(&mut self, circuit: &Circuit) -> DetectionResult<()> {
        let mut open_repeats = ArrayVec::<RepeatFinish, { RepeatNestingLimit::HARD_MAX }>::new();
        visit_circuit(circuit, |event| match event {
            TraversalEvent::Instruction(instruction) => self.visit_instruction(instruction),
            TraversalEvent::BeginRepeat(count) => {
                let (snapshot, marker_index) = self.begin_repeated_body()?;
                open_repeats
                    .try_push(RepeatFinish {
                        count,
                        snapshot,
                        marker_index,
                    })
                    .map_err(|_| compilation_shape_mismatch())
            }
            TraversalEvent::EndRepeat => {
                let finish = open_repeats.pop().ok_or_else(compilation_shape_mismatch)?;
                self.integrate_repeated_body(finish.count, finish.snapshot, finish.marker_index)
            }
        })
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
            compact_operation_count: self.compact_operation_count,
            iteration_operation_count: self.iteration_operation_count,
        };
        let operations = &mut self.program.operations;
        let marker_index = if self.collect_program && operations.len() < operations.capacity() {
            let index = operations.len();
            operations.push(ConversionOperation::Repeat {
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
        self.detector_count = repeated_usize_total(
            snapshot.detector_count,
            body_detectors,
            repeat_count,
            "detector count",
        )?;
        self.output_bit_count()?;
        self.validate_compiled_bytes()?;

        let body_operation_count = self
            .compact_operation_count
            .checked_sub(snapshot.compact_operation_count)
            .ok_or_else(compilation_overflow)?;
        if body_operation_count == 0 {
            if let Some(index) = marker_index {
                if index.checked_add(1) != Some(self.program.operations.len()) {
                    return Err(compilation_shape_mismatch());
                }
                self.program
                    .operations
                    .pop()
                    .ok_or_else(compilation_shape_mismatch)?;
            }
            return Ok(());
        }
        self.add_compact_operations(2)?;
        if self.collect_program {
            let index = marker_index.ok_or_else(compilation_shape_mismatch)?;
            self.try_push_operation(ConversionOperation::EndRepeat)?;
            let body_end = self
                .program
                .operations
                .len()
                .checked_sub(1)
                .ok_or_else(compilation_shape_mismatch)?;
            let requires_iteration =
                self.iteration_operation_count > snapshot.iteration_operation_count;
            let marker = self
                .program
                .operations
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
        self.record_sweep_bits(instruction)?;
        self.visit_instruction_semantics(instruction)
    }

    fn visit_instruction_semantics(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> DetectionResult<()> {
        match instruction.gate().kind() {
            GateKind::DETECTOR => self.record_detector(instruction),
            GateKind::OBSERVABLE_INCLUDE => self.record_observable(instruction),
            // SPP instructions do not produce measurement results. Frame execution validates and
            // applies their quantum effect separately, so detection-conversion admission must not
            // allocate a temporary decomposed circuit for them.
            GateKind::SPP | GateKind::SPP_DAG => Ok(()),
            GateKind::TICK
            | GateKind::QUBIT_COORDS
            | GateKind::SHIFT_COORDS
            | GateKind::REPEAT
            | GateKind::MPAD
            | GateKind::MX
            | GateKind::MY
            | GateKind::M
            | GateKind::MRX
            | GateKind::MRY
            | GateKind::MR
            | GateKind::RX
            | GateKind::RY
            | GateKind::R
            | GateKind::XCX
            | GateKind::XCY
            | GateKind::XCZ
            | GateKind::YCX
            | GateKind::YCY
            | GateKind::YCZ
            | GateKind::CX
            | GateKind::CY
            | GateKind::CZ
            | GateKind::H
            | GateKind::H_XY
            | GateKind::H_YZ
            | GateKind::H_NXY
            | GateKind::H_NXZ
            | GateKind::H_NYZ
            | GateKind::DEPOLARIZE1
            | GateKind::DEPOLARIZE2
            | GateKind::X_ERROR
            | GateKind::Y_ERROR
            | GateKind::Z_ERROR
            | GateKind::I_ERROR
            | GateKind::II_ERROR
            | GateKind::PAULI_CHANNEL_1
            | GateKind::PAULI_CHANNEL_2
            | GateKind::E
            | GateKind::ELSE_CORRELATED_ERROR
            | GateKind::HERALDED_ERASE
            | GateKind::HERALDED_PAULI_CHANNEL_1
            | GateKind::I
            | GateKind::X
            | GateKind::Y
            | GateKind::Z
            | GateKind::C_XYZ
            | GateKind::C_ZYX
            | GateKind::C_NXYZ
            | GateKind::C_XNYZ
            | GateKind::C_XYNZ
            | GateKind::C_NZYX
            | GateKind::C_ZNYX
            | GateKind::C_ZYNX
            | GateKind::SQRT_X
            | GateKind::SQRT_X_DAG
            | GateKind::SQRT_Y
            | GateKind::SQRT_Y_DAG
            | GateKind::S
            | GateKind::S_DAG
            | GateKind::II
            | GateKind::SQRT_XX
            | GateKind::SQRT_XX_DAG
            | GateKind::SQRT_YY
            | GateKind::SQRT_YY_DAG
            | GateKind::SQRT_ZZ
            | GateKind::SQRT_ZZ_DAG
            | GateKind::MPP
            | GateKind::SWAP
            | GateKind::ISWAP
            | GateKind::CXSWAP
            | GateKind::SWAPCX
            | GateKind::CZSWAP
            | GateKind::ISWAP_DAG
            | GateKind::MXX
            | GateKind::MYY
            | GateKind::MZZ => self.add_measurements(instruction),
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
        next_detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                DetectionError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })?;
        self.detector_count = next_detector_count;
        self.validate_compiled_bytes()?;

        let start = self.program.terms.len();
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                DetectionError::invalid_result_format(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            self.validate_measurement_offset(offset)?;
            if self.collect_program {
                self.program.push_term(offset)?;
            }
        }
        self.add_compiled_terms(resource_amount(
            instruction.targets().len(),
            "detector measurement-reference count",
        )?)?;
        self.add_root_operation(true)?;
        if self.collect_program {
            let terms = self.program.span_since(start)?;
            let operation = if terms.len() > execution::SHORT_PARITY_TERMS {
                ConversionOperation::WideDetector(terms)
            } else {
                ConversionOperation::Detector(terms)
            };
            self.try_push_operation(operation)?;
        }
        Ok(())
    }

    fn record_sweep_bits(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let mut found_sweep = None;
        let mut next_sweep_bit_count = self.sweep_bit_count;
        for target in instruction.targets() {
            if let Some(sweep_id) = target.sweep_bit_id() {
                found_sweep = Some(*target);
                let next_count = usize::try_from(sweep_id)
                    .ok()
                    .and_then(|id| id.checked_add(1))
                    .ok_or_else(|| {
                        DetectionError::invalid_result_format(format!(
                            "sweep bit id {sweep_id} does not fit this platform"
                        ))
                    })?;
                next_sweep_bit_count = next_sweep_bit_count.max(next_count);
            }
        }
        let Some(target) = found_sweep else {
            return Ok(());
        };
        let gate = instruction.gate();
        if gate.controlled_pauli().is_none() {
            return Err(DetectionError::invalid_result_format(format!(
                "{UNSUPPORTED_SWEEP_DETECTION_MESSAGE}; found {target} in {}",
                gate.canonical_name()
            )));
        }
        self.sweep_bit_count = next_sweep_bit_count;
        self.validate_compiled_bytes()
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
        let start = self.program.terms.len();
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                self.validate_measurement_offset(offset)?;
                if self.collect_program {
                    self.program.push_term(offset)?;
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
        self.add_root_operation(true)?;
        if self.collect_program {
            let terms = self.program.span_since(start)?;
            self.try_push_operation(ConversionOperation::Observable {
                id: observable_id,
                terms,
            })?;
        }
        Ok(())
    }

    fn ensure_observable(&mut self, observable_id: usize) -> DetectionResult<()> {
        let next_observable_count = observable_id.checked_add(1).ok_or_else(|| {
            DetectionError::invalid_result_format(
                "observable count overflowed while planning detection conversion",
            )
        })?;
        self.detector_count
            .checked_add(next_observable_count)
            .ok_or_else(|| {
                DetectionError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })?;
        self.observable_count = self.observable_count.max(next_observable_count);
        self.validate_compiled_bytes()
    }

    fn add_measurements(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let count = instruction.measurement_result_count();
        if count == 0 {
            return Ok(());
        }
        self.measurement_count = self.measurement_count.checked_add(count).ok_or_else(|| {
            DetectionError::invalid_result_format(
                "measurement record count overflowed during detection conversion planning",
            )
        })?;
        self.validate_compiled_bytes()?;
        self.add_root_operation(false)?;
        if self.collect_program {
            self.try_push_operation(ConversionOperation::AdvanceMeasurements(count))?;
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
        let operations = &mut self.program.operations;
        if operations.len() == operations.capacity() {
            operations.try_reserve(1).map_err(|error| {
                DetectionError::storage_allocation(format!(
                    "unable to reserve compact conversion operation: {error}"
                ))
            })?;
            self.validate_compiled_bytes()?;
        }
        self.program.operations.push(operation);
        Ok(())
    }

    fn add_compiled_terms(&mut self, count: u64) -> DetectionResult<()> {
        self.compiled_term_count = self
            .compiled_term_count
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
        ] {
            if actual != expected {
                return Err(compilation_admission_mismatch(field, expected, actual));
            }
        }
        Ok(())
    }

    fn validate_materialized_program(&self) -> DetectionResult<()> {
        let retained_operations = resource_amount(
            self.program.operations.len(),
            "materialized compact conversion operation count",
        )?;
        if retained_operations != self.compact_operation_count {
            return Err(compilation_admission_mismatch(
                "retained compact operation count",
                self.compact_operation_count,
                retained_operations,
            ));
        }

        // Every span is cut in order from the one arena, so the spans tile it exactly.
        let mut next_term = 0_usize;
        let mut repeat_ends = [None; RepeatNestingLimit::HARD_MAX];
        let mut repeat_depth = 0_usize;
        for (index, operation) in self.program.operations.iter().enumerate() {
            let terms = match operation {
                ConversionOperation::Detector(terms)
                | ConversionOperation::WideDetector(terms)
                | ConversionOperation::Observable { terms, .. } => Some(*terms),
                ConversionOperation::Repeat { body_end, .. } => {
                    validate_repeat_marker(&self.program.operations, index, *body_end)?;
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
                if terms.start != next_term {
                    return Err(compilation_shape_mismatch());
                }
                next_term = terms.end;
            }
        }
        if repeat_depth != 0 || next_term != self.program.terms.len() {
            return Err(compilation_shape_mismatch());
        }
        let retained_terms =
            resource_amount(self.program.terms.len(), "retained conversion terms")?;
        if retained_terms != self.compiled_term_count {
            return Err(compilation_admission_mismatch(
                "retained compiled term count",
                self.compiled_term_count,
                retained_terms,
            ));
        }
        Ok(())
    }

    fn validate_compiled_bytes(&self) -> DetectionResult<()> {
        let bytes = self.compiled_storage_bytes()?;
        self.admit_bytes(bytes)
    }

    fn admit_bytes(&self, bytes: u64) -> DetectionResult<()> {
        let budget = self.limits.memory_budget();
        if !budget.admits(u128::from(bytes)) {
            return Err(ResourceLimitError::detection_compiled_bytes(bytes, budget).into());
        }
        Ok(())
    }

    /// One byte per record bit of every record the plan's consumers hold at least once: the
    /// measurement, sweep, detector, and observable records.
    fn record_storage_bytes(&self) -> DetectionResult<u64> {
        [
            self.measurement_count,
            self.sweep_bit_count,
            self.detector_count,
            self.observable_count,
        ]
        .into_iter()
        .try_fold(0_u64, |total, width| {
            u64::try_from(width)
                .ok()
                .and_then(|width| total.checked_add(width))
                .ok_or_else(compilation_overflow)
        })
    }

    pub(super) fn compiled_storage_bytes(&self) -> DetectionResult<u64> {
        let (operation_count, term_count) = if self.collect_program {
            (
                resource_amount(
                    self.program.operations.capacity(),
                    "compact conversion operation capacity",
                )?,
                resource_amount(
                    self.program.terms.capacity(),
                    "detection conversion measurement-reference capacity",
                )?,
            )
        } else {
            (self.compact_operation_count, self.compiled_term_count)
        };
        compiled_storage_bytes(operation_count, term_count)?
            .checked_add(self.record_storage_bytes()?)
            .ok_or_else(compilation_overflow)
    }

    /// Writes the detector and observable parities of `reference_sample` alone, the all-zero
    /// sample when it is `None`.
    pub(super) fn reference_signs_into(
        &self,
        reference_sample: Option<&ReferenceSample>,
        record: &mut DetectionRecordBuffer,
    ) -> DetectionResult<()> {
        if let Some(reference_sample) = reference_sample {
            super::reference::validate_reference_sample_len(
                reference_sample,
                self.measurement_count,
            )?;
        }
        self.execute_record(RecordValues::Zero, reference_sample, record)
    }

    fn execute_record(
        &self,
        measurements: RecordValues<'_, bool>,
        reference_sample: Option<&ReferenceSample>,
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
            self.limits.cancellation(),
        )?;
        if cursor.measurement != self.measurement_count || cursor.detector != self.detector_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "compact conversion finished at measurement {} and detector {}, expected {} and {}",
                cursor.measurement, cursor.detector, self.measurement_count, self.detector_count
            )));
        }
        Ok(())
    }

    pub(super) fn convert_word_planes_into<Plane>(
        &self,
        measurement_planes: &[Plane],
        detector_planes: &mut Vec<Plane>,
        observable_planes: &mut Vec<Plane>,
        cancellation: &Cancellation,
    ) -> DetectionResult<()>
    where
        Plane: std::ops::BitXor<Output = Plane>
            + std::ops::BitXorAssign
            + std::ops::Not<Output = Plane>
            + Copy
            + Default,
    {
        if measurement_planes.len() != self.measurement_count {
            return Err(DetectionError::invalid_result_format(format!(
                "measurement plane count {} does not match the compiled width {}",
                measurement_planes.len(),
                self.measurement_count
            )));
        }
        // Clearing every output plane is as much work as the planes are wide, so it is polled
        // like the conversion itself; a cancellation here rejects the batch before any output.
        let units = WORD_OP_UNITS * size_of::<Plane>().div_ceil(size_of::<u64>());
        let mut poll = WorkPoll::new(cancellation);
        super::frame::fill_polled(
            detector_planes,
            self.detector_count,
            units,
            &mut poll,
            Plane::default(),
        )?;
        super::frame::fill_polled(
            observable_planes,
            self.observable_count,
            units,
            &mut poll,
            Plane::default(),
        )?;
        let mut cursor = ConversionCursor::default();
        execute_program(
            &self.program,
            RecordValues::Values(measurement_planes),
            None,
            detector_planes,
            observable_planes,
            &mut cursor,
            cancellation,
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

impl ConversionPlan {
    /// The most measurements back from the current one that any detector or observable reads.
    pub(super) fn max_record_lookback(&self) -> usize {
        // The arena holds exactly the terms of every detector and observable.
        self.program
            .terms
            .iter()
            .map(|offset| super::frame::lookback_distance(*offset))
            .max()
            .unwrap_or(0)
    }

    /// Continues `runner` over the measurement planes `base..base + measurements.len()`,
    /// writing detectors from absolute index `detector_base` into `detectors` and XOR-ing
    /// observables into `observables`, until the program ends or needs what it lacks.
    #[allow(
        clippy::too_many_arguments,
        reason = "the window of each record the conversion reads or writes is one argument"
    )]
    pub(super) fn convert_window<Plane>(
        &self,
        runner: &mut ConversionRunner,
        measurements: &[Plane],
        base: usize,
        detectors: &mut [Plane],
        detector_base: usize,
        observables: &mut [Plane],
        cancellation: &Cancellation,
    ) -> DetectionResult<ConversionStop>
    where
        Plane: std::ops::BitXor<Output = Plane>
            + std::ops::BitXorAssign
            + std::ops::Not<Output = Plane>
            + Copy
            + Default,
    {
        let mut io = ConversionIo {
            measurements: RecordValues::Values(measurements),
            measurement_base: base,
            reference_sample: None,
            available_measurements: base + measurements.len(),
            detectors,
            detector_base,
            observables,
        };
        runner.run(&self.program, &mut io, cancellation)
    }

    /// Checks that a finished streaming conversion covered every measurement and detector.
    pub(super) fn check_finished(&self, runner: &ConversionRunner) -> DetectionResult<()> {
        let cursor = runner.cursor;
        if cursor.measurement != self.measurement_count || cursor.detector != self.detector_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "streaming conversion finished at measurement {} and detector {}, expected {} and {}",
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

#[cold]
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
