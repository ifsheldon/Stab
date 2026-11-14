use stab_model::advanced::{
    CancellationCountdown, FoldedDemBlock, FoldedDemItem, dem_instruction_detector_shift,
};
use stab_model::{Cancellation, DemInstruction, DemInstructionKind, DemTarget};

use super::{DemError, DemResult};
use crate::bernoulli::SparseBernoulli;

/// The compiled operation holding error ordinals `first_error..` of its block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ErrorSpan {
    pub(super) first_error: usize,
    pub(super) operation: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct DemSampleBlock {
    pub(super) operations: Vec<DemSampleOperation>,
    /// Error-carrying operations in order, keyed by their first unrolled error ordinal, so a
    /// replayed error is located by binary search instead of a walk over the whole program.
    pub(super) error_spans: Vec<ErrorSpan>,
    pub(super) detector_shift: u64,
    pub(super) error_count: usize,
    pub(super) direct_sample_effect_count: usize,
    pub(super) direct_sample_work_count: usize,
    pub(super) direct_sample_has_stochastic_error: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DemSampleOperation {
    Error(DemSampleError),
    Repeat(DemSampleRepeat),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DemSampleError {
    probability: f64,
    /// Geometric-skip constants, present exactly when `probability` is on the sparse branch.
    sparse: Option<SparseBernoulli>,
    pub(super) detectors: Vec<u64>,
    pub(super) observables: Vec<usize>,
}

impl DemSampleError {
    pub(super) fn new(probability: f64, detectors: Vec<u64>, observables: Vec<usize>) -> Self {
        Self {
            probability,
            sparse: SparseBernoulli::new(probability),
            detectors,
            observables,
        }
    }

    pub(super) const fn probability(&self) -> f64 {
        self.probability
    }

    pub(super) const fn sparse(&self) -> Option<&SparseBernoulli> {
        self.sparse.as_ref()
    }

    fn set_probability(&mut self, probability: f64) {
        self.probability = probability;
        self.sparse = SparseBernoulli::new(probability);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DemSampleRepeat {
    pub(super) start_detector_shift: u64,
    pub(super) repeat_count: u64,
    pub(super) body: DemSampleBlock,
    pub(super) folded_zero_shift_errors: Option<Vec<DemSampleError>>,
}

pub(super) fn compile_block(source: &FoldedDemBlock<'_>) -> DemResult<DemSampleBlock> {
    let mut block = DemSampleBlock::default();
    let mut current_shift = 0;
    for item in source.items() {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                compile_instruction(instruction, current_shift, &mut block)?;
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    current_shift = current_shift
                        .checked_add(dem_instruction_detector_shift(instruction)?)
                        .ok_or_else(|| {
                            DemError::invalid_sampler_compilation(
                                "DEM sampler detector shift overflowed",
                            )
                        })?;
                }
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count == 0 {
                    continue;
                }
                let body = compile_block(body)?;
                let repeat_start_shift = current_shift;
                let repeated_shift =
                    body.detector_shift
                        .checked_mul(repeat_count)
                        .ok_or_else(|| {
                            DemError::invalid_sampler_compilation(
                                "DEM sampler repeat detector shift overflowed",
                            )
                        })?;
                current_shift = current_shift.checked_add(repeated_shift).ok_or_else(|| {
                    DemError::invalid_sampler_compilation("DEM sampler detector shift overflowed")
                })?;
                let first_error = block.error_count;
                block.error_count = checked_repeated_count(
                    block.error_count,
                    body.error_count,
                    repeat_count,
                    "DEM sampler error count",
                )?;
                block.direct_sample_effect_count = checked_repeated_count(
                    block.direct_sample_effect_count,
                    body.direct_sample_effect_count,
                    repeat_count,
                    "DEM sampler direct sample effect count",
                )?;
                block.direct_sample_has_stochastic_error |= body.direct_sample_has_stochastic_error;
                let folded_zero_shift_errors =
                    folded_zero_shift_repeat_errors(&body, repeat_count)?;
                let repeated_work = folded_direct_sample_repeat_work_count(
                    &body,
                    repeat_count,
                    folded_zero_shift_errors.as_deref(),
                )?;
                block.direct_sample_work_count = block
                    .direct_sample_work_count
                    .checked_add(repeated_work)
                    .ok_or_else(|| {
                        DemError::invalid_sampler_compilation(
                            "DEM sampler direct sample work count overflowed",
                        )
                    })?;
                if body.error_count > 0 {
                    block.error_spans.push(ErrorSpan {
                        first_error,
                        operation: block.operations.len(),
                    });
                }
                block
                    .operations
                    .push(DemSampleOperation::Repeat(DemSampleRepeat {
                        start_detector_shift: repeat_start_shift,
                        repeat_count,
                        body,
                        folded_zero_shift_errors,
                    }));
            }
        }
    }
    block.detector_shift = current_shift;
    Ok(block)
}

fn compile_instruction(
    instruction: &DemInstruction,
    detector_shift: u64,
    block: &mut DemSampleBlock,
) -> DemResult<()> {
    if instruction.kind() != DemInstructionKind::Error {
        return Ok(());
    }
    let probability = instruction
        .args()
        .first()
        .copied()
        .ok_or_else(|| DemError::invalid_sampler_compilation("error is missing probability"))?;
    let mut operation = DemSampleError::new(probability, Vec::new(), Vec::new());
    for target in instruction.targets() {
        match target {
            DemTarget::RelativeDetector(detector) => {
                let shifted = detector_shift
                    .checked_add(detector.get())
                    .ok_or_else(|| detector_index_overflow_error("detector"))?;
                operation.detectors.push(shifted);
            }
            DemTarget::LogicalObservable(observable) => {
                operation
                    .observables
                    .push(usize_from_u64(observable.get(), "observable index")?);
            }
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(DemError::invalid_sampler_compilation(
                    "error targets cannot include numeric DEM targets",
                ));
            }
        }
    }
    block.error_spans.push(ErrorSpan {
        first_error: block.error_count,
        operation: block.operations.len(),
    });
    block.error_count = block.error_count.checked_add(1).ok_or_else(|| {
        DemError::invalid_sampler_compilation("DEM sampler error count overflowed")
    })?;
    block.direct_sample_work_count =
        block
            .direct_sample_work_count
            .checked_add(1)
            .ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampler direct sample work count overflowed",
                )
            })?;
    if probability > 0.0 {
        block.direct_sample_effect_count = block
            .direct_sample_effect_count
            .checked_add(1)
            .ok_or_else(|| {
                DemError::invalid_sampler_compilation(
                    "DEM sampler direct sample effect count overflowed",
                )
            })?;
    }
    if probability > 0.0 && probability < 1.0 {
        block.direct_sample_has_stochastic_error = true;
    }
    block.operations.push(DemSampleOperation::Error(operation));
    Ok(())
}

/// Cancellation polled by DEM sampling and replay once per bounded step of a batch.
pub(super) struct DemPoll<'a> {
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl<'a> DemPoll<'a> {
    pub(super) const fn new(cancellation: &'a Cancellation) -> Self {
        Self {
            cancellation,
            countdown: CancellationCountdown::new(),
        }
    }

    /// Counts one sampled error, replayed error, or repeat iteration.
    #[inline]
    pub(super) fn tick(&mut self) -> DemResult<()> {
        Ok(self.countdown.tick(self.cancellation)?)
    }
}

fn checked_repeated_count(
    current: usize,
    body_count: usize,
    repeat_count: u64,
    kind: &'static str,
) -> DemResult<usize> {
    let repeat_count = usize_from_u64(repeat_count, "DEM sampler repeat count")?;
    let repeated = body_count.checked_mul(repeat_count).ok_or_else(|| {
        DemError::invalid_sampler_compilation(format!("repeated {kind} overflowed"))
    })?;
    current
        .checked_add(repeated)
        .ok_or_else(|| DemError::invalid_sampler_compilation(format!("{kind} overflowed")))
}

fn folded_direct_sample_repeat_work_count(
    body: &DemSampleBlock,
    repeat_count: u64,
    folded_zero_shift_errors: Option<&[DemSampleError]>,
) -> DemResult<usize> {
    if body.direct_sample_effect_count == 0 {
        return Ok(0);
    }
    if let Some(errors) = folded_zero_shift_errors {
        return Ok(errors.len());
    }
    if body.detector_shift == 0 && !body.direct_sample_has_stochastic_error {
        if repeat_count.is_multiple_of(2) {
            return Ok(0);
        }
        return Ok(body.direct_sample_work_count);
    }
    if flat_zero_shift_error_block(body) {
        return Ok(body.direct_sample_work_count);
    }
    checked_repeated_count(
        0,
        body.direct_sample_work_count,
        repeat_count,
        "DEM sampler direct sample work count",
    )
}

fn flat_zero_shift_error_block(block: &DemSampleBlock) -> bool {
    block.detector_shift == 0
        && !block.operations.is_empty()
        && block
            .operations
            .iter()
            .all(|operation| matches!(operation, DemSampleOperation::Error(_)))
}

fn folded_zero_shift_repeat_errors(
    block: &DemSampleBlock,
    repeat_count: u64,
) -> DemResult<Option<Vec<DemSampleError>>> {
    if !block.direct_sample_has_stochastic_error {
        return Ok(None);
    }
    let mut one_pass_errors = Vec::new();
    if !collect_zero_shift_effect_errors(block, 0, &mut one_pass_errors)? {
        return Ok(None);
    }
    let mut folded_errors = Vec::with_capacity(one_pass_errors.len());
    for mut folded_error in one_pass_errors {
        folded_error.set_probability(odd_parity_probability(
            folded_error.probability,
            repeat_count,
        ));
        if folded_error.probability > 0.0 {
            folded_errors.push(folded_error);
        }
    }
    Ok(Some(folded_errors))
}

fn collect_zero_shift_effect_errors(
    block: &DemSampleBlock,
    detector_offset: u64,
    errors: &mut Vec<DemSampleError>,
) -> DemResult<bool> {
    if block.detector_shift != 0 {
        return Ok(false);
    }
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                errors.push(shifted_sample_error(error, detector_offset)?);
            }
            DemSampleOperation::Repeat(repeat) => {
                if repeat.body.detector_shift != 0 {
                    return Ok(false);
                }
                let nested_offset = detector_offset
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                if repeat.body.direct_sample_has_stochastic_error {
                    let Some(nested_errors) =
                        folded_zero_shift_repeat_errors(&repeat.body, repeat.repeat_count)?
                    else {
                        return Ok(false);
                    };
                    for nested_error in nested_errors {
                        errors.push(shifted_sample_error(&nested_error, nested_offset)?);
                    }
                } else if !repeat.repeat_count.is_multiple_of(2)
                    && !collect_zero_shift_effect_errors(&repeat.body, nested_offset, errors)?
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn shifted_sample_error(error: &DemSampleError, detector_offset: u64) -> DemResult<DemSampleError> {
    if detector_offset == 0 {
        return Ok(error.clone());
    }
    let detectors = error
        .detectors
        .iter()
        .map(|detector| {
            detector
                .checked_add(detector_offset)
                .ok_or_else(|| detector_index_overflow_error("detector"))
        })
        .collect::<DemResult<Vec<_>>>()?;
    Ok(DemSampleError {
        detectors,
        observables: error.observables.clone(),
        ..*error
    })
}

pub(super) fn odd_parity_probability(probability: f64, repeat_count: u64) -> f64 {
    if repeat_count == 0 || probability <= 0.0 {
        return 0.0;
    }
    if probability >= 1.0 {
        return if repeat_count.is_multiple_of(2) {
            0.0
        } else {
            1.0
        };
    }
    if probability == 0.5 {
        return 0.5;
    }

    if probability < 0.5 {
        let log_bias = (repeat_count as f64) * (-2.0 * probability).ln_1p();
        return (-0.5 * log_bias.exp_m1()).clamp(0.0, 0.5);
    }

    let complement = 1.0 - probability;
    let log_magnitude = (repeat_count as f64) * (-2.0 * complement).ln_1p();
    if repeat_count.is_multiple_of(2) {
        (-0.5 * log_magnitude.exp_m1()).clamp(0.0, 0.5)
    } else {
        (1.0 + 0.5 * log_magnitude.exp_m1()).clamp(0.5, 1.0)
    }
}

pub(super) fn usize_from_u64(value: u64, kind: &'static str) -> DemResult<usize> {
    usize::try_from(value).map_err(|_| {
        DemError::invalid_sampler_compilation(format!("{kind} {value} does not fit in usize"))
    })
}

pub(super) fn detector_index_overflow_error(kind: &'static str) -> DemError {
    DemError::invalid_sampler_compilation(format!("{kind} index overflowed"))
}

pub(super) fn detector_shift_overflow_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM sampler detector shift overflowed")
}
