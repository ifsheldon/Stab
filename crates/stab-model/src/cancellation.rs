use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;

/// Cloneable cooperative cancellation signal polled by long-running operations.
///
/// Operations that interpret repeat blocks, stream batches, or traverse expanded work poll the
/// signal at bounded intervals and stop with a typed [`Cancelled`] outcome once it is set. A
/// request is visible to every clone, so a caller keeps one clone and hands another to the
/// operation, possibly on a different thread.
///
/// [`Cancellation::never`], which is also the default, is a signal that cannot be set; it costs
/// no allocation, and operations given it behave exactly as before cancellation existed.
/// [`Cancellation::new`] creates a signal that can be set with [`Cancellation::cancel`].
#[derive(Clone, Default)]
pub struct Cancellation {
    requested: Option<Arc<AtomicBool>>,
}

impl Cancellation {
    /// A signal that is never set; [`Cancellation::cancel`] on it has no effect.
    pub const fn never() -> Self {
        Self { requested: None }
    }

    /// A new signal, not yet set, that [`Cancellation::cancel`] can set.
    pub fn new() -> Self {
        Self {
            requested: Some(Arc::new(AtomicBool::new(false))),
        }
    }

    /// Whether [`Cancellation::cancel`] can set this signal, that is, it is not
    /// [`Cancellation::never`].
    pub const fn is_cancellable(&self) -> bool {
        self.requested.is_some()
    }

    /// Requests cancellation of every operation polling this signal or one of its clones.
    pub fn cancel(&self) {
        if let Some(requested) = &self.requested {
            requested.store(true, Ordering::Relaxed);
        }
    }

    /// Withdraws an earlier request so that a resumable session can continue.
    ///
    /// An operation that already stopped stays stopped; only later polls observe the reset.
    pub fn reset(&self) {
        if let Some(requested) = &self.requested {
            requested.store(false, Ordering::Relaxed);
        }
    }

    /// Whether cancellation has been requested.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.requested
            .as_ref()
            .is_some_and(|requested| requested.load(Ordering::Relaxed))
    }

    /// Returns [`Cancelled`] when cancellation has been requested.
    #[inline]
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for Cancellation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cancellation")
            .field("cancellable", &self.is_cancellable())
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl PartialEq for Cancellation {
    /// Two values are equal when they are the same signal: both never, or clones of one signal.
    fn eq(&self, other: &Self) -> bool {
        match (&self.requested, &other.requested) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for Cancellation {}

/// An operation stopped because its [`Cancellation`] signal was set.
///
/// A cancelled operation returns no partial result; work it already delivered to a sink stays
/// delivered, and the owning operation documents whether it can be resumed.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, PartialEq)]
#[error("operation was cancelled")]
pub struct Cancelled;

/// Countdown that polls a [`Cancellation`] once every [`CancellationCountdown::INTERVAL`] ticks.
///
/// Long-running loops tick once per instruction, repeat iteration, or comparable bounded unit of
/// work, so the signal is polled at a bounded interval without an atomic load per unit.
#[derive(Clone, Copy, Debug)]
pub struct CancellationCountdown {
    remaining: u32,
}

impl CancellationCountdown {
    /// Ticks between polls.
    pub const INTERVAL: u32 = 1024;

    pub const fn new() -> Self {
        Self {
            remaining: Self::INTERVAL,
        }
    }

    /// Counts one unit of work, polling `cancellation` when the interval elapses.
    #[inline]
    pub fn tick(&mut self, cancellation: &Cancellation) -> Result<(), Cancelled> {
        // `remaining` is at least one between calls, so this never wraps.
        self.remaining = self.remaining.wrapping_sub(1);
        if self.remaining == 0 {
            self.remaining = Self::INTERVAL;
            cancellation.check()
        } else {
            Ok(())
        }
    }
}

impl Default for CancellationCountdown {
    fn default() -> Self {
        Self::new()
    }
}
