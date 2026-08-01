use std::ops::ControlFlow;

use stab_decoder::{
    DecodeBatchSummary, DecodeCancellation, DecodeSessionFailure, DecoderLayout, DecoderModelView,
    DecoderModelViewError, DecoderSession, ValidatedDecodeBatch,
};
use stab_model::{
    DemErrorMechanismTraversalLimits, DemErrorMechanismView, DemErrorMechanismVisitError,
    DemErrorMechanismVisitor, DemErrorTarget, ModelError, ModelFingerprint,
};
use stab_records::FormatError;
use thiserror::Error;

const MAX_DETECTORS: usize = 20;
const MAX_OBSERVABLES: usize = 1;
const MAX_MECHANISMS: u64 = 256;
const MAX_MECHANISM_STORAGE: usize = 256;
const MAX_INSTRUCTION_VISITS: u64 = 65_536;
const MAX_JOINT_STATES: usize = 1 << 21;
const MAX_PRIMARY_WORKSPACE_BYTES: u128 = 32 * 1024 * 1024;
const MAX_TIE_WORKSPACE_BYTES: u128 = 32 * 1024 * 1024;
const MAX_PEAK_WORKSPACE_BYTES: u128 = MAX_TIE_WORKSPACE_BYTES;
const MAX_PAIR_TRANSITIONS_PER_PASS: u128 = 1 << 28;
const MAX_TOTAL_PAIR_TRANSITIONS: u128 = 1 << 29;
const MAX_EXACT_LIMB_TRANSITIONS: u128 = 1 << 28;

const _: () = assert!(
    (MAX_JOINT_STATES as u128) * (size_of::<ProbabilityInterval>() as u128)
        == MAX_PRIMARY_WORKSPACE_BYTES
);
const _: () = assert!(
    (MAX_MECHANISMS as u128) * ((MAX_JOINT_STATES / 2) as u128) == MAX_PAIR_TRANSITIONS_PER_PASS
);
const _: () = assert!(MAX_PAIR_TRANSITIONS_PER_PASS * 2 == MAX_TOTAL_PAIR_TRANSITIONS);

const PREDICT_ZERO: u8 = 0;
const PREDICT_ONE: u8 = 1;
const IMPOSSIBLE_SYNDROME: u8 = 2;
const AMBIGUOUS_SYNDROME: u8 = 3;

/// Reusable exact maximum-likelihood decoder for one admitted detector-error model.
#[derive(Debug)]
pub struct ExactMlDecoderSession {
    layout: DecoderLayout,
    model_fingerprint: ModelFingerprint,
    predictions: Vec<u8>,
}
impl ExactMlDecoderSession {
    pub const MAX_DETECTORS: usize = MAX_DETECTORS;
    pub const MAX_OBSERVABLES: usize = MAX_OBSERVABLES;
    pub const MAX_MECHANISMS: u64 = MAX_MECHANISMS;
    pub const MAX_INSTRUCTION_VISITS: u64 = MAX_INSTRUCTION_VISITS;
    pub const MAX_JOINT_STATES: usize = MAX_JOINT_STATES;
    pub const MAX_PRIMARY_WORKSPACE_BYTES: u128 = MAX_PRIMARY_WORKSPACE_BYTES;
    pub const MAX_TIE_WORKSPACE_BYTES: u128 = MAX_TIE_WORKSPACE_BYTES;
    pub const MAX_PEAK_WORKSPACE_BYTES: u128 = MAX_PEAK_WORKSPACE_BYTES;
    pub const MAX_PAIR_TRANSITIONS_PER_PASS: u128 = MAX_PAIR_TRANSITIONS_PER_PASS;
    pub const MAX_TOTAL_PAIR_TRANSITIONS: u128 = MAX_TOTAL_PAIR_TRANSITIONS;
    pub const MAX_EXACT_LIMB_TRANSITIONS: u128 = MAX_EXACT_LIMB_TRANSITIONS;

    pub fn try_compile_model(
        model: &stab_model::DetectorErrorModel,
    ) -> Result<Self, ExactMlCompileError> {
        Self::compile(DecoderModelView::try_new(model)?)
    }

    pub fn compile(model: DecoderModelView<'_>) -> Result<Self, ExactMlCompileError> {
        let layout = model.layout();
        admit_layout(layout)?;
        let detector_count = layout.detector_width().get();
        let observable_count = layout.observable_width().get();
        let joint_width = detector_count.checked_add(observable_count).ok_or(
            ExactMlCompileError::JointStateLimit {
                actual_at_least: u128::MAX,
                limit: MAX_JOINT_STATES,
            },
        )?;
        let joint_state_count = joint_state_count(joint_width)?;
        let mut collector = MechanismCollector::try_new(layout)?;
        let traversal = model
            .model()
            .try_visit_error_mechanisms(
                DemErrorMechanismTraversalLimits::new(MAX_MECHANISMS, MAX_INSTRUCTION_VISITS),
                &mut collector,
            )
            .map_err(map_traversal_error)?;
        if matches!(traversal, ControlFlow::Break(())) {
            return Err(ExactMlCompileError::InternalInvariant {
                message: "exact-ML mechanism collector stopped traversal early".to_owned(),
            });
        }
        let distribution = interval_joint_distribution(joint_state_count, &collector.mechanisms)?;
        let mut predictions =
            initial_prediction_table(detector_count, observable_count, &distribution)?;
        drop(distribution);
        if predictions.contains(&AMBIGUOUS_SYNDROME) {
            // The interval pass certifies ordinary comparisons. Recompute unresolved cases using
            // exact dyadic arithmetic because every finite f64 probability is a binary rational.
            let exact_distribution =
                ExactDyadicDistribution::try_compute(joint_state_count, &collector.mechanisms)?;
            resolve_ambiguous_predictions(detector_count, &exact_distribution, &mut predictions)?;
        }
        Ok(Self {
            layout,
            model_fingerprint: model.fingerprint(),
            predictions,
        })
    }

    pub const fn layout(&self) -> DecoderLayout {
        self.layout
    }

    pub const fn model_fingerprint(&self) -> ModelFingerprint {
        self.model_fingerprint
    }

    pub fn syndrome_count(&self) -> usize {
        self.predictions.len()
    }

    pub fn retained_prediction_bytes(&self) -> usize {
        self.predictions.len()
    }

    pub fn prediction_for_syndrome(&self, syndrome: u64) -> Result<bool, ExactMlDecodeError> {
        let syndrome_index = usize::try_from(syndrome)
            .ok()
            .filter(|index| *index < self.predictions.len())
            .ok_or(ExactMlDecodeError::SyndromeOutOfRange {
                syndrome,
                syndrome_count: self.predictions.len(),
            })?;
        match self.predictions.get(syndrome_index).copied() {
            Some(PREDICT_ZERO) => Ok(false),
            Some(PREDICT_ONE) => Ok(true),
            Some(IMPOSSIBLE_SYNDROME) => Err(ExactMlDecodeError::ImpossibleSyndrome { syndrome }),
            Some(code) => Err(ExactMlDecodeError::InternalInvariant {
                message: format!("prediction table contains unknown code {code}"),
            }),
            None => Err(ExactMlDecodeError::InternalInvariant {
                message: "admitted syndrome escaped the prediction table".to_owned(),
            }),
        }
    }

    fn prediction_code(&self, syndrome: usize) -> Result<u8, ExactMlDecodeError> {
        self.predictions.get(syndrome).copied().ok_or_else(|| {
            ExactMlDecodeError::InternalInvariant {
                message: format!(
                    "batch syndrome {syndrome} escaped {} retained entries",
                    self.predictions.len()
                ),
            }
        })
    }
}

impl DecoderSession for ExactMlDecoderSession {
    type Error = ExactMlDecodeError;

    fn layout(&self) -> DecoderLayout {
        self.layout
    }

    fn decode_validated_batch(
        &mut self,
        mut batch: ValidatedDecodeBatch<'_, '_>,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodeBatchSummary, DecodeSessionFailure<Self::Error>> {
        let requested = batch.shot_count();

        // Scan first so an impossible syndrome never leaves a partially updated output batch.
        for shot_index in 0..requested {
            if cancellation.is_cancelled() {
                return Ok(DecodeBatchSummary::cancelled(requested, 0));
            }
            let syndrome = batch_syndrome(&batch, shot_index)
                .map_err(|error| DecodeSessionFailure::new(error, 0))?;
            if self
                .prediction_code(syndrome)
                .map_err(|error| DecodeSessionFailure::new(error, 0))?
                == IMPOSSIBLE_SYNDROME
            {
                return Err(DecodeSessionFailure::new(
                    ExactMlDecodeError::ImpossibleBatchSyndrome {
                        shot_index,
                        syndrome: syndrome as u64,
                    },
                    0,
                ));
            }
        }

        let mut completed = 0;
        while completed < requested {
            if cancellation.is_cancelled() {
                return Ok(DecodeBatchSummary::cancelled(requested, completed));
            }
            let syndrome = batch_syndrome(&batch, completed)
                .map_err(|error| DecodeSessionFailure::new(error, completed))?;
            let prediction = self
                .prediction_code(syndrome)
                .map_err(|error| DecodeSessionFailure::new(error, completed))?;
            if self.layout.observable_width().get() == 1 {
                batch
                    .set_prediction(completed, 0, prediction == PREDICT_ONE)
                    .map_err(|error| {
                        DecodeSessionFailure::new(ExactMlDecodeError::Record(error), completed)
                    })?;
            }
            completed += 1;
        }
        Ok(DecodeBatchSummary::completed(requested))
    }
}

/// Failure while compiling the bounded exact maximum-likelihood table.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExactMlCompileError {
    #[error(transparent)]
    ModelView(#[from] DecoderModelViewError),

    #[error("exact ML supports at most {limit} detectors, got {actual}")]
    DetectorWidth { actual: usize, limit: usize },

    #[error("exact ML supports at most {limit} observables, got {actual}")]
    ObservableWidth { actual: usize, limit: usize },

    #[error("exact ML supports at most {limit} represented mechanisms, got {actual}")]
    MechanismLimit { actual: u64, limit: u64 },

    #[error(
        "exact ML supports at most {limit} represented instruction visits, got at least {actual_at_least}"
    )]
    InstructionVisitLimit { actual_at_least: u64, limit: u64 },

    #[error("exact ML joint-state limit is {limit}, got at least {actual_at_least}")]
    JointStateLimit { actual_at_least: u128, limit: usize },

    #[error(
        "exact ML tie resolution needs at least {actual_at_least} bytes, exceeding the {limit}-byte workspace limit"
    )]
    ExactWorkspaceLimit { actual_at_least: u128, limit: u128 },

    #[error(
        "exact ML tie resolution needs at least {actual_at_least} limb transitions, exceeding the {limit}-transition work limit"
    )]
    ExactWorkLimit { actual_at_least: u128, limit: u128 },

    #[error("invalid detector-error model during exact ML traversal: {0}")]
    ModelTraversal(#[source] ModelError),

    #[error("exact ML mechanism target is outside the derived decoder layout: {message}")]
    InvalidTarget { message: String },

    #[error("exact ML could not allocate {component}: {message}")]
    Allocation {
        component: &'static str,
        message: String,
    },

    #[error("exact ML compilation invariant failed: {message}")]
    InternalInvariant { message: String },
}

/// Failure while applying a compiled exact maximum-likelihood table.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExactMlDecodeError {
    #[error("syndrome {syndrome} is out of range for {syndrome_count} retained syndromes")]
    SyndromeOutOfRange {
        syndrome: u64,
        syndrome_count: usize,
    },

    #[error("syndrome {syndrome} has zero probability under the compiled detector-error model")]
    ImpossibleSyndrome { syndrome: u64 },

    #[error(
        "shot {shot_index} has impossible syndrome {syndrome} under the compiled detector-error model"
    )]
    ImpossibleBatchSyndrome { shot_index: usize, syndrome: u64 },

    #[error("prediction record update failed: {0}")]
    Record(#[source] FormatError),

    #[error("exact ML decode invariant failed: {message}")]
    InternalInvariant { message: String },
}

#[derive(Clone, Copy, Debug)]
struct Mechanism {
    probability: f64,
    effect: usize,
}

#[derive(Debug)]
struct MechanismCollector {
    layout: DecoderLayout,
    mechanisms: Vec<Mechanism>,
}

impl MechanismCollector {
    fn try_new(layout: DecoderLayout) -> Result<Self, ExactMlCompileError> {
        let mut mechanisms = Vec::new();
        mechanisms
            .try_reserve_exact(MAX_MECHANISM_STORAGE)
            .map_err(|error| ExactMlCompileError::Allocation {
                component: "temporary mechanism table",
                message: error.to_string(),
            })?;
        Ok(Self { layout, mechanisms })
    }
}

impl DemErrorMechanismVisitor for MechanismCollector {
    type Error = MechanismCollectorError;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        let detector_count = self.layout.detector_width().get();
        let observable_count = self.layout.observable_width().get();
        let mut effect = 0_usize;
        for target in mechanism.targets() {
            match target.map_err(MechanismCollectorError::Model)? {
                DemErrorTarget::Detector(detector) => {
                    let index = usize::try_from(detector.get()).map_err(|_| {
                        MechanismCollectorError::InvalidTarget(format!(
                            "detector {} does not fit usize",
                            detector.get()
                        ))
                    })?;
                    if index >= detector_count {
                        return Err(MechanismCollectorError::InvalidTarget(format!(
                            "detector {index} is outside width {detector_count}"
                        )));
                    }
                    effect ^= 1_usize << index;
                }
                DemErrorTarget::Observable(observable) => {
                    let observable_index = usize::try_from(observable.get()).map_err(|_| {
                        MechanismCollectorError::InvalidTarget(format!(
                            "observable {} does not fit usize",
                            observable.get()
                        ))
                    })?;
                    if observable_index >= observable_count {
                        return Err(MechanismCollectorError::InvalidTarget(format!(
                            "observable {observable_index} is outside width {observable_count}"
                        )));
                    }
                    effect ^= 1_usize << (detector_count + observable_index);
                }
                DemErrorTarget::Separator => {}
            }
        }
        self.mechanisms.push(Mechanism {
            probability: mechanism.probability().get(),
            effect,
        });
        Ok(ControlFlow::Continue(()))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
enum MechanismCollectorError {
    #[error("{0}")]
    Model(ModelError),
    #[error("{0}")]
    InvalidTarget(String),
}

fn map_traversal_error(
    error: DemErrorMechanismVisitError<MechanismCollectorError>,
) -> ExactMlCompileError {
    match error {
        DemErrorMechanismVisitError::Model(error) => ExactMlCompileError::ModelTraversal(error),
        DemErrorMechanismVisitError::MechanismLimit { actual, limit } => {
            ExactMlCompileError::MechanismLimit { actual, limit }
        }
        DemErrorMechanismVisitError::InstructionVisitLimit {
            actual_at_least,
            limit,
        } => ExactMlCompileError::InstructionVisitLimit {
            actual_at_least,
            limit,
        },
        DemErrorMechanismVisitError::Visitor(MechanismCollectorError::Model(error)) => {
            ExactMlCompileError::ModelTraversal(error)
        }
        DemErrorMechanismVisitError::Visitor(MechanismCollectorError::InvalidTarget(message)) => {
            ExactMlCompileError::InvalidTarget { message }
        }
    }
}

fn admit_layout(layout: DecoderLayout) -> Result<(), ExactMlCompileError> {
    let detector_count = layout.detector_width().get();
    if detector_count > MAX_DETECTORS {
        return Err(ExactMlCompileError::DetectorWidth {
            actual: detector_count,
            limit: MAX_DETECTORS,
        });
    }
    let observable_count = layout.observable_width().get();
    if observable_count > MAX_OBSERVABLES {
        return Err(ExactMlCompileError::ObservableWidth {
            actual: observable_count,
            limit: MAX_OBSERVABLES,
        });
    }
    Ok(())
}

fn joint_state_count(joint_width: usize) -> Result<usize, ExactMlCompileError> {
    let shift = u32::try_from(joint_width).map_err(|_| ExactMlCompileError::JointStateLimit {
        actual_at_least: u128::MAX,
        limit: MAX_JOINT_STATES,
    })?;
    let count = 1_usize
        .checked_shl(shift)
        .ok_or(ExactMlCompileError::JointStateLimit {
            actual_at_least: u128::MAX,
            limit: MAX_JOINT_STATES,
        })?;
    if count > MAX_JOINT_STATES {
        return Err(ExactMlCompileError::JointStateLimit {
            actual_at_least: count as u128,
            limit: MAX_JOINT_STATES,
        });
    }
    Ok(count)
}

#[derive(Clone, Copy, Debug)]
struct ProbabilityInterval {
    lower: f64,
    upper: f64,
}

impl ProbabilityInterval {
    const ZERO: Self = Self {
        lower: 0.0,
        upper: 0.0,
    };
    const ONE: Self = Self {
        lower: 1.0,
        upper: 1.0,
    };

    const fn exact(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    fn complement(probability: f64) -> Self {
        if probability == 0.0 {
            return Self::ONE;
        }
        if probability == 1.0 {
            return Self::ZERO;
        }
        let rounded = 1.0 - probability;
        Self {
            lower: next_down_probability(rounded),
            upper: next_up_probability(rounded),
        }
    }

    fn weighted_sum(left: Self, left_weight: Self, right: Self, right_weight: Self) -> Self {
        interval_add(
            interval_multiply(left, left_weight),
            interval_multiply(right, right_weight),
        )
    }

    const fn is_exactly_zero(self) -> bool {
        self.upper == 0.0
    }
}

fn interval_joint_distribution(
    state_count: usize,
    mechanisms: &[Mechanism],
) -> Result<Vec<ProbabilityInterval>, ExactMlCompileError> {
    let mut distribution = Vec::new();
    distribution
        .try_reserve_exact(state_count)
        .map_err(|error| ExactMlCompileError::Allocation {
            component: "temporary directed-interval workspace",
            message: error.to_string(),
        })?;
    distribution.resize(state_count, ProbabilityInterval::ZERO);
    let Some(initial) = distribution.first_mut() else {
        return Err(ExactMlCompileError::InternalInvariant {
            message: "joint-state table was empty after positive admission".to_owned(),
        });
    };
    *initial = ProbabilityInterval::ONE;

    for mechanism in mechanisms {
        if mechanism.effect == 0 || mechanism.probability == 0.0 {
            continue;
        }
        if mechanism.effect >= state_count {
            return Err(ExactMlCompileError::InternalInvariant {
                message: format!(
                    "mechanism effect {} escaped {state_count} joint states",
                    mechanism.effect
                ),
            });
        }
        let error_probability = ProbabilityInterval::exact(mechanism.probability);
        let no_error_probability = ProbabilityInterval::complement(mechanism.probability);
        for left in 0..state_count {
            let right = left ^ mechanism.effect;
            if left >= right {
                continue;
            }
            let left_probability =
                *distribution
                    .get(left)
                    .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                        message: "left joint state escaped admitted workspace".to_owned(),
                    })?;
            let right_probability =
                *distribution
                    .get(right)
                    .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                        message: "right joint state escaped admitted workspace".to_owned(),
                    })?;
            let next_left = ProbabilityInterval::weighted_sum(
                left_probability,
                no_error_probability,
                right_probability,
                error_probability,
            );
            let next_right = ProbabilityInterval::weighted_sum(
                right_probability,
                no_error_probability,
                left_probability,
                error_probability,
            );
            let Some(left_slot) = distribution.get_mut(left) else {
                return Err(ExactMlCompileError::InternalInvariant {
                    message: "left joint state disappeared during update".to_owned(),
                });
            };
            *left_slot = next_left;
            let Some(right_slot) = distribution.get_mut(right) else {
                return Err(ExactMlCompileError::InternalInvariant {
                    message: "right joint state disappeared during update".to_owned(),
                });
            };
            *right_slot = next_right;
        }
    }
    Ok(distribution)
}

fn initial_prediction_table(
    detector_count: usize,
    observable_count: usize,
    distribution: &[ProbabilityInterval],
) -> Result<Vec<u8>, ExactMlCompileError> {
    let syndrome_count = joint_state_count(detector_count)?;
    let mut predictions = Vec::new();
    predictions
        .try_reserve_exact(syndrome_count)
        .map_err(|error| ExactMlCompileError::Allocation {
            component: "retained syndrome prediction table",
            message: error.to_string(),
        })?;
    predictions.resize(syndrome_count, IMPOSSIBLE_SYNDROME);

    for syndrome in 0..syndrome_count {
        let zero =
            *distribution
                .get(syndrome)
                .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                    message: "zero-observable state escaped joint distribution".to_owned(),
                })?;
        let code = if observable_count == 0 {
            if zero.is_exactly_zero() {
                IMPOSSIBLE_SYNDROME
            } else {
                PREDICT_ZERO
            }
        } else {
            let observable_one_state = syndrome | syndrome_count;
            let one = *distribution.get(observable_one_state).ok_or_else(|| {
                ExactMlCompileError::InternalInvariant {
                    message: "one-observable state escaped joint distribution".to_owned(),
                }
            })?;
            match (zero.is_exactly_zero(), one.is_exactly_zero()) {
                (true, true) => IMPOSSIBLE_SYNDROME,
                (true, false) => PREDICT_ONE,
                (false, true) => PREDICT_ZERO,
                (false, false) if one.lower > zero.upper => PREDICT_ONE,
                (false, false) if zero.lower > one.upper => PREDICT_ZERO,
                (false, false) => AMBIGUOUS_SYNDROME,
            }
        };
        let Some(slot) = predictions.get_mut(syndrome) else {
            return Err(ExactMlCompileError::InternalInvariant {
                message: "syndrome disappeared from retained prediction table".to_owned(),
            });
        };
        *slot = code;
    }
    Ok(predictions)
}

fn interval_multiply(left: ProbabilityInterval, right: ProbabilityInterval) -> ProbabilityInterval {
    if left.is_exactly_zero() || right.is_exactly_zero() {
        return ProbabilityInterval::ZERO;
    }
    ProbabilityInterval {
        lower: next_down_probability(left.lower * right.lower),
        upper: next_up_probability(left.upper * right.upper),
    }
}

fn interval_add(left: ProbabilityInterval, right: ProbabilityInterval) -> ProbabilityInterval {
    if left.is_exactly_zero() && right.is_exactly_zero() {
        return ProbabilityInterval::ZERO;
    }
    ProbabilityInterval {
        lower: next_down_probability(left.lower + right.lower),
        upper: next_up_probability(left.upper + right.upper),
    }
}

fn next_down_probability(value: f64) -> f64 {
    if value <= 0.0 {
        0.0
    } else if value >= 1.0 {
        f64::from_bits(1.0_f64.to_bits() - 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

fn next_up_probability(value: f64) -> f64 {
    if value <= 0.0 {
        f64::from_bits(1)
    } else if value >= 1.0 {
        1.0
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

fn resolve_ambiguous_predictions(
    detector_count: usize,
    distribution: &ExactDyadicDistribution,
    predictions: &mut [u8],
) -> Result<(), ExactMlCompileError> {
    let syndrome_count = joint_state_count(detector_count)?;
    for (syndrome, prediction) in predictions.iter_mut().enumerate() {
        if *prediction != AMBIGUOUS_SYNDROME {
            continue;
        }
        let one_state = syndrome.checked_add(syndrome_count).ok_or_else(|| {
            ExactMlCompileError::InternalInvariant {
                message: "ambiguous observable state index overflowed".to_owned(),
            }
        })?;
        let zero = distribution.state(syndrome)?;
        let one = distribution.state(one_state)?;
        if exact_is_zero(zero) && exact_is_zero(one) {
            return Err(ExactMlCompileError::InternalInvariant {
                message: "reachable interval state became impossible during exact resolution"
                    .to_owned(),
            });
        }
        *prediction = if exact_compare(one, zero).is_gt() {
            PREDICT_ONE
        } else {
            PREDICT_ZERO
        };
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactProbability {
    numerator: u64,
    denominator_exponent: usize,
}

impl ExactProbability {
    fn from_f64(probability: f64) -> Result<Self, ExactMlCompileError> {
        if probability == 0.0 {
            return Ok(Self {
                numerator: 0,
                denominator_exponent: 0,
            });
        }
        if probability == 1.0 {
            return Ok(Self {
                numerator: 1,
                denominator_exponent: 0,
            });
        }
        let bits = probability.to_bits();
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, binary_exponent) = if exponent_bits == 0 {
            (fraction, -1074_i32)
        } else {
            (fraction | (1_u64 << 52), exponent_bits - 1023 - 52)
        };
        if significand == 0 || binary_exponent >= 0 {
            return Err(ExactMlCompileError::InternalInvariant {
                message: format!(
                    "probability {probability:?} escaped the finite dyadic fraction contract"
                ),
            });
        }
        let denominator_exponent = usize::try_from(-binary_exponent).map_err(|_| {
            ExactMlCompileError::InternalInvariant {
                message: "probability denominator exponent does not fit usize".to_owned(),
            }
        })?;
        let reduction = usize::try_from(significand.trailing_zeros())
            .unwrap_or(usize::MAX)
            .min(denominator_exponent);
        Ok(Self {
            numerator: significand >> reduction,
            denominator_exponent: denominator_exponent - reduction,
        })
    }
}

struct ExactDyadicDistribution {
    words: Vec<u64>,
    state_words: usize,
    state_count: usize,
    limbs_per_state: usize,
}

impl ExactDyadicDistribution {
    fn try_compute(
        state_count: usize,
        mechanisms: &[Mechanism],
    ) -> Result<Self, ExactMlCompileError> {
        let mut denominator_exponent = 0_usize;
        let mut active_mechanisms = 0_u128;
        for mechanism in mechanisms {
            if mechanism.effect == 0 || mechanism.probability == 0.0 {
                continue;
            }
            active_mechanisms =
                active_mechanisms
                    .checked_add(1)
                    .ok_or(ExactMlCompileError::ExactWorkLimit {
                        actual_at_least: u128::MAX,
                        limit: MAX_EXACT_LIMB_TRANSITIONS,
                    })?;
            denominator_exponent = denominator_exponent
                .checked_add(
                    ExactProbability::from_f64(mechanism.probability)?.denominator_exponent,
                )
                .ok_or(ExactMlCompileError::ExactWorkspaceLimit {
                    actual_at_least: u128::MAX,
                    limit: MAX_TIE_WORKSPACE_BYTES,
                })?;
        }
        let significant_bits = denominator_exponent.checked_add(1).ok_or(
            ExactMlCompileError::ExactWorkspaceLimit {
                actual_at_least: u128::MAX,
                limit: MAX_TIE_WORKSPACE_BYTES,
            },
        )?;
        let limbs_per_state = significant_bits
            .checked_add(63)
            .map(|bits| bits / 64)
            .filter(|limbs| *limbs > 0)
            .ok_or(ExactMlCompileError::ExactWorkspaceLimit {
                actual_at_least: u128::MAX,
                limit: MAX_TIE_WORKSPACE_BYTES,
            })?;
        let state_words = state_count.checked_mul(limbs_per_state).ok_or(
            ExactMlCompileError::ExactWorkspaceLimit {
                actual_at_least: u128::MAX,
                limit: MAX_TIE_WORKSPACE_BYTES,
            },
        )?;
        let scratch_words =
            limbs_per_state
                .checked_mul(2)
                .ok_or(ExactMlCompileError::ExactWorkspaceLimit {
                    actual_at_least: u128::MAX,
                    limit: MAX_TIE_WORKSPACE_BYTES,
                })?;
        let total_words = state_words.checked_add(scratch_words).ok_or(
            ExactMlCompileError::ExactWorkspaceLimit {
                actual_at_least: u128::MAX,
                limit: MAX_TIE_WORKSPACE_BYTES,
            },
        )?;
        let actual_bytes = (total_words as u128) * (size_of::<u64>() as u128);
        if actual_bytes > MAX_TIE_WORKSPACE_BYTES {
            return Err(ExactMlCompileError::ExactWorkspaceLimit {
                actual_at_least: actual_bytes,
                limit: MAX_TIE_WORKSPACE_BYTES,
            });
        }
        let pair_transitions = (state_count as u128 / 2).saturating_mul(active_mechanisms);
        admit_exact_limb_work(pair_transitions, limbs_per_state)?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(total_words)
            .map_err(|error| ExactMlCompileError::Allocation {
                component: "exact dyadic tie-resolution workspace",
                message: error.to_string(),
            })?;
        words.resize(total_words, 0);
        let Some(initial) = words.first_mut() else {
            return Err(ExactMlCompileError::InternalInvariant {
                message: "exact dyadic workspace was empty after positive admission".to_owned(),
            });
        };
        *initial = 1;
        let mut result = Self {
            words,
            state_words,
            state_count,
            limbs_per_state,
        };
        for mechanism in mechanisms {
            result.apply(*mechanism)?;
        }
        Ok(result)
    }

    fn apply(&mut self, mechanism: Mechanism) -> Result<(), ExactMlCompileError> {
        if mechanism.effect == 0 || mechanism.probability == 0.0 {
            return Ok(());
        }
        if mechanism.effect >= self.state_count {
            return Err(ExactMlCompileError::InternalInvariant {
                message: format!(
                    "mechanism effect {} escaped {} exact states",
                    mechanism.effect, self.state_count
                ),
            });
        }
        let probability = ExactProbability::from_f64(mechanism.probability)?;
        let (states, scratch) = self
            .words
            .split_at_mut_checked(self.state_words)
            .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                message: "exact state and scratch workspace boundary drifted".to_owned(),
            })?;
        let (left_scratch, right_scratch) = scratch
            .split_at_mut_checked(self.limbs_per_state)
            .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                message: "exact scratch workspace boundary drifted".to_owned(),
            })?;
        for left_index in 0..self.state_count {
            let right_index = left_index ^ mechanism.effect;
            if left_index >= right_index {
                continue;
            }
            let (left, right) =
                exact_state_pair_mut(states, self.limbs_per_state, left_index, right_index)?;
            exact_multiply_small(left, probability.numerator, left_scratch)?;
            exact_multiply_small(right, probability.numerator, right_scratch)?;
            exact_shift_left(left, probability.denominator_exponent)?;
            exact_shift_left(right, probability.denominator_exponent)?;
            exact_sub_assign(left, left_scratch)?;
            exact_add_assign(left, right_scratch)?;
            exact_sub_assign(right, right_scratch)?;
            exact_add_assign(right, left_scratch)?;
        }
        Ok(())
    }

    fn state(&self, index: usize) -> Result<&[u64], ExactMlCompileError> {
        if index >= self.state_count {
            return Err(ExactMlCompileError::InternalInvariant {
                message: format!(
                    "exact state {index} escaped admitted count {}",
                    self.state_count
                ),
            });
        }
        let start = index.checked_mul(self.limbs_per_state).ok_or_else(|| {
            ExactMlCompileError::InternalInvariant {
                message: "exact state offset overflowed".to_owned(),
            }
        })?;
        let end = start.checked_add(self.limbs_per_state).ok_or_else(|| {
            ExactMlCompileError::InternalInvariant {
                message: "exact state range overflowed".to_owned(),
            }
        })?;
        self.words
            .get(start..end)
            .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                message: "exact state escaped allocated workspace".to_owned(),
            })
    }
}

fn admit_exact_limb_work(
    pair_transitions: u128,
    limbs_per_state: usize,
) -> Result<(), ExactMlCompileError> {
    let limb_transitions = pair_transitions.saturating_mul(limbs_per_state as u128);
    if limb_transitions > MAX_EXACT_LIMB_TRANSITIONS {
        return Err(ExactMlCompileError::ExactWorkLimit {
            actual_at_least: limb_transitions,
            limit: MAX_EXACT_LIMB_TRANSITIONS,
        });
    }
    Ok(())
}

fn exact_state_pair_mut(
    states: &mut [u64],
    limbs_per_state: usize,
    left_index: usize,
    right_index: usize,
) -> Result<(&mut [u64], &mut [u64]), ExactMlCompileError> {
    let left_start = left_index.checked_mul(limbs_per_state).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "left exact-state offset overflowed".to_owned(),
        }
    })?;
    let left_end = left_start.checked_add(limbs_per_state).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "left exact-state range overflowed".to_owned(),
        }
    })?;
    let right_start = right_index.checked_mul(limbs_per_state).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "right exact-state offset overflowed".to_owned(),
        }
    })?;
    let right_end = right_start.checked_add(limbs_per_state).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "right exact-state range overflowed".to_owned(),
        }
    })?;
    let (before_right, from_right) = states.split_at_mut_checked(right_start).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "right exact-state boundary escaped workspace".to_owned(),
        }
    })?;
    let left = before_right.get_mut(left_start..left_end).ok_or_else(|| {
        ExactMlCompileError::InternalInvariant {
            message: "left exact state escaped workspace".to_owned(),
        }
    })?;
    let right = from_right
        .get_mut(..right_end - right_start)
        .ok_or_else(|| ExactMlCompileError::InternalInvariant {
            message: "right exact state escaped workspace".to_owned(),
        })?;
    Ok((left, right))
}

fn exact_multiply_small(
    input: &[u64],
    multiplier: u64,
    output: &mut [u64],
) -> Result<(), ExactMlCompileError> {
    output.fill(0);
    let mut carry = 0_u128;
    for (input_limb, output_limb) in input.iter().copied().zip(output.iter_mut()) {
        let product = (input_limb as u128) * (multiplier as u128) + carry;
        let low_limb = product & u128::from(u64::MAX);
        *output_limb =
            u64::try_from(low_limb).map_err(|_| ExactMlCompileError::InternalInvariant {
                message: "masked exact product limb did not fit u64".to_owned(),
            })?;
        carry = product >> 64;
    }
    if carry != 0 {
        return Err(ExactMlCompileError::InternalInvariant {
            message: "exact small multiplication exceeded admitted limb width".to_owned(),
        });
    }
    Ok(())
}

fn exact_shift_left(value: &mut [u64], bits: usize) -> Result<(), ExactMlCompileError> {
    if bits == 0 {
        return Ok(());
    }
    let word_shift = bits / 64;
    let bit_shift = bits % 64;
    for destination in (0..value.len()).rev() {
        let shifted = if destination < word_shift {
            0
        } else {
            let source = destination - word_shift;
            let lower = value.get(source).copied().ok_or_else(|| {
                ExactMlCompileError::InternalInvariant {
                    message: "exact shift source escaped workspace".to_owned(),
                }
            })?;
            let mut shifted = lower << bit_shift;
            if bit_shift != 0 && source > 0 {
                let carry = value.get(source - 1).copied().ok_or_else(|| {
                    ExactMlCompileError::InternalInvariant {
                        message: "exact shift carry source escaped workspace".to_owned(),
                    }
                })?;
                shifted |= carry >> (64 - bit_shift);
            }
            shifted
        };
        let slot =
            value
                .get_mut(destination)
                .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                    message: "exact shift destination escaped workspace".to_owned(),
                })?;
        *slot = shifted;
    }
    Ok(())
}

fn exact_sub_assign(left: &mut [u64], right: &[u64]) -> Result<(), ExactMlCompileError> {
    let mut borrow = false;
    for (left_limb, right_limb) in left.iter_mut().zip(right.iter().copied()) {
        let (without_right, first_borrow) = left_limb.overflowing_sub(right_limb);
        let (result, second_borrow) = without_right.overflowing_sub(u64::from(borrow));
        *left_limb = result;
        borrow = first_borrow || second_borrow;
    }
    if borrow {
        return Err(ExactMlCompileError::InternalInvariant {
            message: "exact probability update became negative".to_owned(),
        });
    }
    Ok(())
}

fn exact_add_assign(left: &mut [u64], right: &[u64]) -> Result<(), ExactMlCompileError> {
    let mut carry = false;
    for (left_limb, right_limb) in left.iter_mut().zip(right.iter().copied()) {
        let (with_right, first_carry) = left_limb.overflowing_add(right_limb);
        let (result, second_carry) = with_right.overflowing_add(u64::from(carry));
        *left_limb = result;
        carry = first_carry || second_carry;
    }
    if carry {
        return Err(ExactMlCompileError::InternalInvariant {
            message: "exact probability update exceeded admitted limb width".to_owned(),
        });
    }
    Ok(())
}

fn exact_is_zero(value: &[u64]) -> bool {
    value.iter().all(|limb| *limb == 0)
}

fn exact_compare(left: &[u64], right: &[u64]) -> std::cmp::Ordering {
    left.iter().rev().cmp(right.iter().rev())
}

fn batch_syndrome(
    batch: &ValidatedDecodeBatch<'_, '_>,
    shot_index: usize,
) -> Result<usize, ExactMlDecodeError> {
    let detector_count = batch.layout().detector_width().get();
    let mut syndrome = 0_usize;
    for detector_index in 0..detector_count {
        let value = batch.detector(shot_index, detector_index).ok_or_else(|| {
            ExactMlDecodeError::InternalInvariant {
                message: format!(
                    "validated detector ({shot_index}, {detector_index}) was inaccessible"
                ),
            }
        })?;
        if value {
            syndrome |= 1_usize << detector_index;
        }
    }
    Ok(syndrome)
}

#[cfg(test)]
mod tests;
