use std::thread;
use std::time::{Duration, Instant};

use crate::ReleaseError;

const SLEEP_SLICE: Duration = Duration::from_millis(50);

/// The release tool's view of the shared sticky SIGINT/SIGTERM flag.
#[derive(Clone, Debug)]
pub(crate) struct ReleaseCancellation {
    inner: stab_process::Cancellation,
}

impl ReleaseCancellation {
    pub(crate) fn for_signals() -> Result<Self, ReleaseError> {
        stab_process::Cancellation::for_signals()
            .map(|inner| Self { inner })
            .map_err(ReleaseError::CommandSignalHandlers)
    }

    pub(crate) fn check(&self, operation: &str) -> Result<(), ReleaseError> {
        if self.is_cancelled() {
            Err(ReleaseError::OperationInterrupted {
                operation: operation.to_string(),
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    pub(crate) fn sleep(&self, duration: Duration, operation: &str) -> Result<(), ReleaseError> {
        let started = Instant::now();
        while started.elapsed() < duration {
            self.check(operation)?;
            thread::sleep(SLEEP_SLICE.min(duration.saturating_sub(started.elapsed())));
        }
        self.check(operation)
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            inner: stab_process::Cancellation::manual(),
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.inner.cancel();
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[test]
    fn cancellation_interrupts_waits_immediately() {
        let cancellation = ReleaseCancellation::for_test();
        cancellation.cancel();
        let started = Instant::now();
        assert_matches!(
            cancellation.sleep(Duration::from_secs(5), "test wait"),
            Err(ReleaseError::OperationInterrupted { .. })
        );
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}
