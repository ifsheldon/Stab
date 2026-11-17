//! Cancellation polled by estimated work instead of by step count.
//!
//! [`stab_model::advanced::CancellationCountdown`] polls once per fixed number of steps, which
//! bounds the time between polls only while every step is cheap: one instruction with millions
//! of targets is one step. Frame batches, detector conversion, and the reference simulator
//! instead charge every step with the work it is about to do, in units of roughly one
//! nanosecond of lane-word work on the reference host, and walk wide target lists in chunks
//! charged one at a time. A poll is due once [`UNITS_PER_POLL`] units were charged, so the time
//! between two polls is bounded for any instruction shape, without reading a clock.

use stab_model::{Cancellation, Cancelled};

/// Work units between two polls of the signal: about one millisecond on the reference host.
const UNITS_PER_POLL: u32 = 1 << 20;

/// Most units one chunk of a wide loop charges, so a chunk overshoots a due poll by at most a
/// quarter of the poll interval.
const CHUNK_UNITS: usize = (UNITS_PER_POLL / 4) as usize;

/// Units of one frame or record operation on one 64-lane word: a Clifford target, a reset, a
/// recorded measurement plane, a Pauli-product or detector term.
pub(crate) const WORD_OP_UNITS: usize = 4;

/// Units of sampling one dense Bernoulli or categorical noise word.
pub(crate) const NOISE_WORD_UNITS: usize = 256;

/// Units of one event of a sparse noise channel: a geometric gap, a category, and its frame
/// update.
pub(crate) const NOISE_EVENT_UNITS: usize = 48;

/// Units of one instruction or repeat iteration before its targets are charged.
pub(crate) const STEP_UNITS: usize = 16;

/// Remaining work before the next poll; `Copy` so a hot loop can keep it in a register.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkCountdown {
    remaining: u32,
}

impl WorkCountdown {
    pub(crate) fn new() -> Self {
        Self {
            remaining: units_per_poll(),
        }
    }

    /// Charges `units` of work about to be done, polling `cancellation` when a poll is due.
    #[inline]
    pub(crate) fn charge(
        &mut self,
        units: usize,
        cancellation: &Cancellation,
    ) -> Result<(), Cancelled> {
        match u32::try_from(units) {
            Ok(units) if units < self.remaining => {
                self.remaining -= units;
                Ok(())
            }
            _ => {
                self.remaining = units_per_poll();
                poll(cancellation)
            }
        }
    }
}

impl Default for WorkCountdown {
    fn default() -> Self {
        Self::new()
    }
}

/// A [`WorkCountdown`] bound to the signal it polls.
#[derive(Debug)]
pub(crate) struct WorkPoll<'a> {
    cancellation: &'a Cancellation,
    countdown: WorkCountdown,
}

impl<'a> WorkPoll<'a> {
    pub(crate) fn new(cancellation: &'a Cancellation) -> Self {
        Self {
            cancellation,
            countdown: WorkCountdown::new(),
        }
    }

    /// Charges `units` of work about to be done, polling when a poll is due.
    #[inline]
    pub(crate) fn charge(&mut self, units: usize) -> Result<(), Cancelled> {
        self.countdown.charge(units, self.cancellation)
    }

    /// Runs `body` over consecutive chunks of `items`, charging each chunk before it runs.
    ///
    /// A chunk holds a multiple of `granule` items (pairs stay whole) and charges at most about
    /// a quarter of a poll interval at `units_per_item`, so a list of any length polls between
    /// chunks; a list that fits one chunk costs a single charge.
    #[inline]
    pub(crate) fn for_chunks<T, E: From<Cancelled>>(
        &mut self,
        items: &[T],
        units_per_item: usize,
        granule: usize,
        mut body: impl FnMut(usize, &[T]) -> Result<(), E>,
    ) -> Result<(), E> {
        let granule = granule.max(1);
        let per_chunk = CHUNK_UNITS / units_per_item.max(1);
        let chunk = (per_chunk - per_chunk % granule).max(granule);
        let mut start = 0;
        for part in items.chunks(chunk) {
            self.charge(part.len().saturating_mul(units_per_item))?;
            body(start, part)?;
            start += part.len();
        }
        Ok(())
    }
}

/// The poll interval; this crate's unit tests can shorten it on their thread.
#[inline(always)]
fn units_per_poll() -> u32 {
    #[cfg(test)]
    if let Some(units) = test_hook::units_per_poll() {
        return units;
    }
    UNITS_PER_POLL
}

/// Maximum work units per inner chunk, scaling with [`units_per_poll`] in unit tests.
#[inline(always)]
pub(crate) fn chunk_units() -> usize {
    #[cfg(test)]
    if let Some(units) = test_hook::units_per_poll() {
        return (units as usize / 4).max(1);
    }
    CHUNK_UNITS
}

/// Polls `cancellation`; in this crate's unit tests a hook can stand in for the signal.
#[inline]
fn poll(cancellation: &Cancellation) -> Result<(), Cancelled> {
    #[cfg(test)]
    if test_hook::poll_fires() {
        return Err(Cancelled);
    }
    cancellation.check()
}

/// Deterministic in-batch cancellation for this crate's unit tests.
///
/// The hook counts the work polls of the current thread and, when armed, reports cancellation
/// at exactly one of them, so a test can land a cancellation inside a chosen batch or inside the
/// reference simulation without racing a thread.
#[cfg(test)]
pub(crate) mod test_hook {
    use std::cell::Cell;

    thread_local! {
        static POLLS: Cell<u64> = const { Cell::new(0) };
        static FIRE_AT: Cell<Option<u64>> = const { Cell::new(None) };
        static UNITS_PER_POLL: Cell<Option<u32>> = const { Cell::new(None) };
    }

    pub(super) fn units_per_poll() -> Option<u32> {
        UNITS_PER_POLL.get()
    }

    /// Runs `body` with a poll due every `units` work units, so small fixtures poll many times
    /// per batch.
    pub(crate) fn with_units_per_poll<T>(units: u32, body: impl FnOnce() -> T) -> T {
        UNITS_PER_POLL.set(Some(units));
        let result = body();
        UNITS_PER_POLL.set(None);
        result
    }

    pub(super) fn poll_fires() -> bool {
        let polls = POLLS.get() + 1;
        POLLS.set(polls);
        if FIRE_AT.get() == Some(polls) {
            FIRE_AT.set(None);
            return true;
        }
        false
    }

    /// Runs `body` and returns its result with the number of work polls it made.
    pub(crate) fn count_polls<T>(body: impl FnOnce() -> T) -> (T, u64) {
        let before = POLLS.get();
        let result = body();
        (result, POLLS.get() - before)
    }

    /// Runs `body` with the `poll`-th work poll it makes (counting from one) reporting
    /// cancellation; returns whether that poll happened.
    pub(crate) fn cancel_at_poll<T>(poll: u64, body: impl FnOnce() -> T) -> (T, bool) {
        FIRE_AT.set(Some(POLLS.get() + poll));
        let result = body();
        let fired = FIRE_AT.get().is_none();
        FIRE_AT.set(None);
        (result, fired)
    }
}
