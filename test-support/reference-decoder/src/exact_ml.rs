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
const MAX_WORKSPACE_BYTES: u128 = 16 * 1024 * 1024;
const MAX_PAIR_TRANSITIONS: u128 = 1 << 28;

const PREDICT_ZERO: u8 = 0;
const PREDICT_ONE: u8 = 1;
const IMPOSSIBLE_SYNDROME: u8 = 2;

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
    pub const MAX_WORKSPACE_BYTES: u128 = MAX_WORKSPACE_BYTES;
    pub const MAX_PAIR_TRANSITIONS: u128 = MAX_PAIR_TRANSITIONS;

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
        admit_workspace(joint_state_count)?;

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
        admit_pair_transitions(joint_state_count, &collector.mechanisms)?;

        let distribution = joint_distribution(joint_state_count, &collector.mechanisms)?;
        let predictions = prediction_table(detector_count, observable_count, &distribution)?;
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

    #[error("exact ML probability workspace limit is {limit_bytes} bytes, got {actual_bytes}")]
    WorkspaceLimit {
        actual_bytes: u128,
        limit_bytes: u128,
    },

    #[error("exact ML pair-transition limit is {limit}, got {actual}")]
    PairTransitionLimit { actual: u128, limit: u128 },

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

fn admit_workspace(joint_state_count: usize) -> Result<(), ExactMlCompileError> {
    let actual_bytes = (joint_state_count as u128) * (size_of::<f64>() as u128);
    if actual_bytes > MAX_WORKSPACE_BYTES {
        return Err(ExactMlCompileError::WorkspaceLimit {
            actual_bytes,
            limit_bytes: MAX_WORKSPACE_BYTES,
        });
    }
    Ok(())
}

fn admit_pair_transitions(
    joint_state_count: usize,
    mechanisms: &[Mechanism],
) -> Result<(), ExactMlCompileError> {
    let active_mechanisms = mechanisms
        .iter()
        .filter(|mechanism| mechanism.effect != 0 && mechanism.probability != 0.0)
        .count();
    let actual = (active_mechanisms as u128) * ((joint_state_count / 2) as u128);
    if actual > MAX_PAIR_TRANSITIONS {
        return Err(ExactMlCompileError::PairTransitionLimit {
            actual,
            limit: MAX_PAIR_TRANSITIONS,
        });
    }
    Ok(())
}

fn joint_distribution(
    state_count: usize,
    mechanisms: &[Mechanism],
) -> Result<Vec<f64>, ExactMlCompileError> {
    let mut distribution = Vec::new();
    distribution
        .try_reserve_exact(state_count)
        .map_err(|error| ExactMlCompileError::Allocation {
            component: "temporary log-probability workspace",
            message: error.to_string(),
        })?;
    distribution.resize(state_count, f64::NEG_INFINITY);
    let Some(initial) = distribution.first_mut() else {
        return Err(ExactMlCompileError::InternalInvariant {
            message: "joint-state table was empty after positive admission".to_owned(),
        });
    };
    *initial = 0.0;

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
        let log_error = if mechanism.probability == 1.0 {
            0.0
        } else {
            mechanism.probability.ln()
        };
        let log_no_error = if mechanism.probability == 1.0 {
            f64::NEG_INFINITY
        } else {
            (-mechanism.probability).ln_1p()
        };
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
            let next_left = log_add(
                left_probability + log_no_error,
                right_probability + log_error,
            );
            let next_right = log_add(
                right_probability + log_no_error,
                left_probability + log_error,
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

fn prediction_table(
    detector_count: usize,
    observable_count: usize,
    distribution: &[f64],
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
        let log_zero =
            *distribution
                .get(syndrome)
                .ok_or_else(|| ExactMlCompileError::InternalInvariant {
                    message: "zero-observable state escaped joint distribution".to_owned(),
                })?;
        let code = if observable_count == 0 {
            if log_zero == f64::NEG_INFINITY {
                IMPOSSIBLE_SYNDROME
            } else {
                PREDICT_ZERO
            }
        } else {
            let observable_one_state = syndrome | syndrome_count;
            let log_one = *distribution.get(observable_one_state).ok_or_else(|| {
                ExactMlCompileError::InternalInvariant {
                    message: "one-observable state escaped joint distribution".to_owned(),
                }
            })?;
            if log_zero == f64::NEG_INFINITY && log_one == f64::NEG_INFINITY {
                IMPOSSIBLE_SYNDROME
            } else if log_one > log_zero {
                PREDICT_ONE
            } else {
                PREDICT_ZERO
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

fn log_add(left: f64, right: f64) -> f64 {
    if left == f64::NEG_INFINITY {
        return right;
    }
    if right == f64::NEG_INFINITY {
        return left;
    }
    let (larger, smaller) = if left >= right {
        (left, right)
    } else {
        (right, left)
    };
    larger + (smaller - larger).exp().ln_1p()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "exact-ML unit tests use bounded generated tables"
    )]

    use super::{
        ExactMlCompileError, MAX_PAIR_TRANSITIONS, MAX_WORKSPACE_BYTES, Mechanism,
        admit_pair_transitions, admit_workspace, joint_distribution,
    };

    const PROBABILITIES: [f64; 4] = [0.0, 0.25, 0.5, 1.0];

    #[test]
    fn log_domain_distribution_matches_exhaustive_subset_enumeration() {
        for first_effect in 0..8 {
            for first_probability in PROBABILITIES {
                for second_effect in 0..8 {
                    for second_probability in PROBABILITIES {
                        let mechanisms = [
                            Mechanism {
                                probability: first_probability,
                                effect: first_effect,
                            },
                            Mechanism {
                                probability: second_probability,
                                effect: second_effect,
                            },
                        ];
                        let actual = joint_distribution(8, &mechanisms).expect("distribution");
                        let expected = direct_distribution(8, &mechanisms);
                        for state in 0..8 {
                            let actual_probability = actual[state].exp();
                            let expected_probability = expected[state];
                            assert!(
                                (actual_probability - expected_probability).abs() <= 1e-14,
                                "effects=({first_effect},{second_effect}) probabilities=({first_probability},{second_probability}) state={state}: actual={actual_probability} expected={expected_probability}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn workspace_and_pair_transition_arithmetic_accept_exact_limits() {
        assert!(admit_workspace((MAX_WORKSPACE_BYTES / 8) as usize).is_ok());
        assert!(matches!(
            admit_workspace((MAX_WORKSPACE_BYTES / 8 + 1) as usize),
            Err(ExactMlCompileError::WorkspaceLimit { .. })
        ));

        let mechanisms = vec![
            Mechanism {
                probability: 0.25,
                effect: 1,
            };
            256
        ];
        assert!(admit_pair_transitions(1 << 21, &mechanisms).is_ok());
        let mut over = mechanisms;
        over.push(Mechanism {
            probability: 0.25,
            effect: 1,
        });
        assert!(matches!(
            admit_pair_transitions(1 << 21, &over),
            Err(ExactMlCompileError::PairTransitionLimit {
                actual,
                limit: MAX_PAIR_TRANSITIONS
            }) if actual == MAX_PAIR_TRANSITIONS + (1 << 20)
        ));
    }

    fn direct_distribution(state_count: usize, mechanisms: &[Mechanism]) -> Vec<f64> {
        let mut result = vec![0.0; state_count];
        let subset_count = 1_usize << mechanisms.len();
        for subset in 0..subset_count {
            let mut state = 0_usize;
            let mut probability = 1.0;
            for (index, mechanism) in mechanisms.iter().enumerate() {
                let occurs = subset & (1_usize << index) != 0;
                if occurs {
                    state ^= mechanism.effect;
                    probability *= mechanism.probability;
                } else {
                    probability *= 1.0 - mechanism.probability;
                }
            }
            result[state] += probability;
        }
        result
    }
}
