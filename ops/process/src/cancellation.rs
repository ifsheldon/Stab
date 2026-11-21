//! Sticky SIGINT/SIGTERM cancellation shared by every supervised run.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// A sticky cancellation flag.
///
/// The handle from [`Cancellation::for_signals`] is set by the first SIGINT or
/// SIGTERM the process receives and stays set: the handlers remain installed
/// for the life of the process, so a signal that arrives between supervised
/// runs is observed by [`Cancellation::is_cancelled`] and by every later run.
/// Callers that run several phases should check it at each phase boundary.
#[derive(Clone, Debug)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
}

impl Cancellation {
    /// The process-wide signal-driven flag, installing its SIGINT and SIGTERM
    /// handlers on first use.
    pub fn for_signals() -> Result<Self, String> {
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
        installed.clone().map(|()| Self { cancelled })
    }

    /// A flag no signal can set, cancelled only through [`Self::cancel`].
    #[must_use]
    pub fn manual() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Sets the flag for this handle and every clone of it.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
