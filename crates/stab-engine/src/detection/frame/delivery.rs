//! The one delivery loop shared by every session that samples frame batches.
//!
//! A frame batch always samples every lane of the session's lane width. A run delivers lanes as
//! prefixes of the current batch and keeps any lanes it did not need for the next run, so
//! partitioning a seeded run across calls never changes its output stream.
//!
//! A batch reaches the sink only after it is complete, so cancellation observed while a batch is
//! sampled discards nothing that was delivered. The batch's only lasting effect is the random
//! stream it advanced, which [`sample_or_rewind`] restores, so the cancelled run ends like one
//! cancelled before that batch and a later run resamples the same batch.

use crate::detection::{DetectionError, DetectionResult};

/// A session's frame batch as seen by [`PendingLanes::deliver`].
pub(crate) trait FrameBatchSource {
    type Error;

    /// Lanes in every batch [`Self::refill`] samples: the lane count of the session's frame word.
    const BATCH_SHOTS: usize;

    /// Whether the caller asked the run to stop before its next batch.
    fn is_cancelled(&self) -> bool;

    /// Samples a fresh batch of [`Self::BATCH_SHOTS`] lanes, or reports that cancellation was
    /// observed inside the batch after rewinding the random stream to where the batch began.
    fn refill(&mut self) -> Result<Refill, Self::Error>;

    /// Hands the first `lanes` pending lanes to the sink.
    fn deliver(&mut self, lanes: usize) -> Result<(), Self::Error>;

    /// Drops the first `lanes` lanes and moves the rest of the batch down to lane zero, or
    /// returns [`Refill::Cancelled`] when cancellation was observed first. An interrupted
    /// discard finishes when it is called again with the same `lanes`.
    fn discard(&mut self, lanes: usize) -> Refill;
}

/// The lanes of the current frame batch not yet delivered.
///
/// Delivered lanes are discarded lazily, just before the next prefix is delivered, so a session
/// that is dropped mid-batch never moves lanes it will not deliver.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PendingLanes {
    /// Lanes at the bottom of the batch that were delivered but not yet discarded.
    delivered: usize,
    count: usize,
}

/// What one [`FrameBatchSource::refill`] or [`FrameBatchSource::discard`] did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refill {
    /// A full batch, or the rest of the current one, is ready to deliver.
    Sampled,
    /// Cancellation was observed inside the batch; the source is as it was before the call.
    Cancelled,
}

/// Runs one batch of `sample` on `rng`, rewinding `rng` to its state before the call when the
/// batch observed cancellation.
///
/// The snapshot is a copy of the generator's few words of state, so it costs nothing next to a
/// batch. Every other piece of state a batch writes is reset when the next batch starts.
#[inline]
pub(crate) fn sample_or_rewind<R: Clone, T>(
    rng: &mut R,
    sample: impl FnOnce(&mut R) -> DetectionResult<T>,
) -> DetectionResult<Option<T>> {
    let snapshot = rng.clone();
    match sample(rng) {
        Ok(value) => Ok(Some(value)),
        Err(DetectionError::Cancelled(_)) => {
            *rng = snapshot;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// How a delivery loop ended without an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeliveryOutcome {
    pub(crate) committed: u64,
    pub(crate) completed: bool,
}

/// A failed delivery loop with the progress it made before the failure.
#[derive(Debug)]
pub(crate) struct DeliveryFailure<E> {
    pub(crate) error: E,
    pub(crate) committed: u64,
    pub(crate) attempted: u64,
}

impl PendingLanes {
    /// Delivers `shots` lanes, sampling new batches whenever the pending lanes run out.
    ///
    /// Cancellation is checked before every delivered prefix and inside every sampled batch; a
    /// batch that observes it is discarded with the random stream rewound, so a cancelled loop
    /// can always be resumed. A failure leaves the pending lanes unspecified, so the caller must
    /// not resume the session after one.
    pub(crate) fn deliver<S: FrameBatchSource>(
        &mut self,
        shots: u64,
        source: &mut S,
    ) -> Result<DeliveryOutcome, DeliveryFailure<S::Error>> {
        let mut remaining = shots;
        let mut committed = 0_u64;
        while remaining > 0 {
            if source.is_cancelled() {
                break;
            }
            let wanted = usize::try_from(remaining)
                .map_or(S::BATCH_SHOTS, |remaining| remaining.min(S::BATCH_SHOTS));
            if self.count == 0 {
                let refill = source.refill().map_err(|error| DeliveryFailure {
                    error,
                    committed,
                    attempted: wanted as u64,
                })?;
                if refill == Refill::Cancelled {
                    break;
                }
                self.count = S::BATCH_SHOTS;
                self.delivered = 0;
            } else if self.delivered != 0 {
                // The shift is polled; a cancelled one keeps `delivered` so the next run
                // finishes it before delivering anything.
                if source.discard(self.delivered) == Refill::Cancelled {
                    break;
                }
                self.delivered = 0;
            }
            let lanes = wanted.min(self.count);
            source.deliver(lanes).map_err(|error| DeliveryFailure {
                error,
                committed,
                attempted: lanes as u64,
            })?;
            committed += lanes as u64;
            remaining -= lanes as u64;
            self.count -= lanes;
            self.delivered = lanes;
        }
        Ok(DeliveryOutcome {
            committed,
            completed: remaining == 0,
        })
    }
}
