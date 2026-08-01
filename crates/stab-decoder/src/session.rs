use std::error::Error;

use stab_records::ObservablePredictionBatch;

use crate::{
    DecodeBatchError, DecodeCancellation, DecodeContractError, DecodePreflightError,
    DecodeSessionFailure, DecoderInputBatchView, DecoderLayout, ValidatedDecodeBatch,
};

/// Completion state of one decoder call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DecodeBatchStatus {
    Completed,
    Cancelled,
}

/// Exact requested and completed record counts from one decoder call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeBatchSummary {
    status: DecodeBatchStatus,
    requested_shots: usize,
    completed_shots: usize,
}

impl DecodeBatchSummary {
    pub const fn completed(shot_count: usize) -> Self {
        Self {
            status: DecodeBatchStatus::Completed,
            requested_shots: shot_count,
            completed_shots: shot_count,
        }
    }

    pub const fn cancelled(requested_shots: usize, completed_shots: usize) -> Self {
        Self {
            status: DecodeBatchStatus::Cancelled,
            requested_shots,
            completed_shots,
        }
    }

    pub const fn status(self) -> DecodeBatchStatus {
        self.status
    }

    pub const fn requested_shots(self) -> usize {
        self.requested_shots
    }

    pub const fn completed_shots(self) -> usize {
        self.completed_shots
    }
}

/// Reusable mutable decoder state with statically dispatched batch execution.
///
/// Implementations must write predictions in record order, check cancellation between records,
/// and report the exact completed prefix on cancellation or failure. Compilation remains specific
/// to each decoder implementation and is intentionally absent from this common trait.
pub trait DecoderSession: Sized {
    type Error: Error + Send + Sync + 'static;

    fn layout(&self) -> DecoderLayout;

    fn decode_validated_batch(
        &mut self,
        batch: ValidatedDecodeBatch<'_, '_>,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodeBatchSummary, DecodeSessionFailure<Self::Error>>;
}

/// Validates and executes one batch through a statically dispatched decoder session.
///
/// Detector width, correction width, and prediction capacity are checked before implementation
/// code receives mutable output. A token cancelled before the call returns a zero-progress summary
/// without dispatching to the implementation.
pub fn decode_batch<S: DecoderSession>(
    session: &mut S,
    input: DecoderInputBatchView<'_>,
    predictions: &mut ObservablePredictionBatch,
    cancellation: &DecodeCancellation,
) -> Result<DecodeBatchSummary, DecodeBatchError<S::Error>> {
    let layout = session.layout();
    validate_preflight(layout, input, predictions)?;
    let requested_shots = input.shot_count();
    if cancellation.is_cancelled() {
        return Ok(DecodeBatchSummary::cancelled(requested_shots, 0));
    }

    let prediction_view = predictions
        .view_prefix_mut(requested_shots)
        .map_err(|error| DecodeContractError::PredictionStorage {
            message: error.to_string(),
        })?;
    let batch = ValidatedDecodeBatch::new(input, prediction_view, layout);
    let result = session.decode_validated_batch(batch, cancellation);

    let actual_layout = session.layout();
    if actual_layout != layout {
        return Err(DecodeContractError::SessionLayoutChanged {
            expected: layout,
            actual: actual_layout,
        }
        .into());
    }

    match result {
        Ok(summary) => {
            validate_summary(summary, requested_shots)?;
            Ok(summary)
        }
        Err(failure) => {
            if failure.completed_shots() > requested_shots {
                return Err(DecodeContractError::FailureProgress {
                    requested: requested_shots,
                    actual: failure.completed_shots(),
                }
                .into());
            }
            Err(failure.into())
        }
    }
}

fn validate_preflight(
    layout: DecoderLayout,
    input: DecoderInputBatchView<'_>,
    predictions: &ObservablePredictionBatch,
) -> Result<(), DecodePreflightError> {
    if input.detector_width() != layout.detector_width() {
        return Err(DecodePreflightError::DetectorWidth {
            expected: layout.detector_width(),
            actual: input.detector_width(),
        });
    }
    if predictions.correction_width() != layout.correction_width() {
        return Err(DecodePreflightError::CorrectionWidth {
            expected: layout.correction_width(),
            actual: predictions.correction_width(),
        });
    }
    let available = predictions.records().shot_count();
    let required = input.shot_count();
    if available < required {
        return Err(DecodePreflightError::PredictionShotCapacity {
            required,
            available,
        });
    }
    Ok(())
}

fn validate_summary(
    summary: DecodeBatchSummary,
    requested_shots: usize,
) -> Result<(), DecodeContractError> {
    if summary.requested_shots() != requested_shots {
        return Err(DecodeContractError::RequestedShotCount {
            expected: requested_shots,
            actual: summary.requested_shots(),
        });
    }
    if summary.completed_shots() > requested_shots {
        return Err(DecodeContractError::CompletedShotCount {
            requested: requested_shots,
            actual: summary.completed_shots(),
        });
    }
    Ok(())
}
