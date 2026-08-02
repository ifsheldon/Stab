use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::ReleaseError;

const SLEEP_SLICE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug)]
pub(crate) struct ReleaseCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ReleaseCancellation {
    pub(crate) fn for_signals() -> Result<Self, ReleaseError> {
        static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
        static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
        let cancelled = Arc::clone(CANCELLED.get_or_init(|| Arc::new(AtomicBool::new(false))));
        let installed = INSTALLED.get_or_init(|| {
            for signal in [
                signal_hook::consts::signal::SIGINT,
                signal_hook::consts::signal::SIGTERM,
            ] {
                signal_hook::flag::register(signal, Arc::clone(&cancelled))
                    .map_err(|source| source.to_string())?;
            }
            Ok(())
        });
        if let Err(reason) = installed {
            return Err(ReleaseError::CommandSignalHandlers(reason.clone()));
        }
        Ok(Self { cancelled })
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
        self.cancelled.load(Ordering::Acquire)
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
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_interrupts_waits_immediately() {
        let cancellation = ReleaseCancellation::for_test();
        cancellation.cancel();
        let started = Instant::now();
        assert!(matches!(
            cancellation.sleep(Duration::from_secs(5), "test wait"),
            Err(ReleaseError::OperationInterrupted { .. })
        ));
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}
