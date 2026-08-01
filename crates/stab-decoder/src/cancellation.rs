use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cloneable cooperative cancellation token checked at decoder record boundaries.
///
/// Cancellation is one-way for a token. Create a new token for an independent call instead of
/// resetting shared state while another thread may still observe it.
#[derive(Clone, Debug, Default)]
pub struct DecodeCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DecodeCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
