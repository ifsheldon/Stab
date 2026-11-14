//! The one delivery loop shared by every session that samples frame batches.
//!
//! A frame batch always samples [`FRAME_BATCH_SHOTS`] lanes. A run delivers lanes as prefixes
//! of the current batch and keeps any lanes it did not need for the next run, so partitioning a
//! seeded run across calls never changes its output stream.

use super::FRAME_BATCH_SHOTS;

/// A session's frame batch as seen by [`PendingLanes::deliver`].
pub(crate) trait FrameBatchSource {
    type Error;

    /// Whether the caller asked the run to stop before its next batch.
    fn is_cancelled(&self) -> bool;

    /// Samples a fresh batch of [`FRAME_BATCH_SHOTS`] lanes.
    fn refill(&mut self) -> Result<(), Self::Error>;

    /// Hands the first `lanes` pending lanes to the sink.
    fn deliver(&mut self, lanes: usize) -> Result<(), Self::Error>;

    /// Drops the first `lanes` lanes and moves the rest of the batch down to lane zero.
    fn discard(&mut self, lanes: usize);
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
    /// Cancellation is checked before every delivered prefix. A failure leaves the pending
    /// lanes unspecified, so the caller must not resume the session after one.
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
            let wanted = usize::try_from(remaining).map_or(FRAME_BATCH_SHOTS, |remaining| {
                remaining.min(FRAME_BATCH_SHOTS)
            });
            if self.count == 0 {
                source.refill().map_err(|error| DeliveryFailure {
                    error,
                    committed,
                    attempted: wanted as u64,
                })?;
                self.count = FRAME_BATCH_SHOTS;
                self.delivered = 0;
            } else if self.delivered != 0 {
                source.discard(self.delivered);
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
