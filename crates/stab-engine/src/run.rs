//! Progress, status, summary, and error of one session run, shared by every execution family.

use std::fmt;

use stab_model::MemoryBudgetExceeded;

use crate::{
    DemSamplingExecutionError, DetectionExecutionError, SamplingExecutionError, ShotCount,
};

/// Sink operation that failed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SinkFailurePhase {
    WriteBatch,
    Finish,
}

impl SinkFailurePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WriteBatch => "write-batch",
            Self::Finish => "finish",
        }
    }
}

/// Exact progress at an execution failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunProgress {
    committed_shots: ShotCount,
    attempted_batch_shots: ShotCount,
}

impl RunProgress {
    pub(crate) const fn new(committed_shots: u64, attempted_batch_shots: u64) -> Self {
        Self {
            committed_shots: ShotCount::new(committed_shots),
            attempted_batch_shots: ShotCount::new(attempted_batch_shots),
        }
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn attempted_batch_shots(self) -> ShotCount {
        self.attempted_batch_shots
    }
}

/// Completion state of one run, or of one batch a transaction accepted.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RunStatus {
    Completed,
    Cancelled,
}

impl RunStatus {
    pub const fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }

    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Summary of one completed or cooperatively cancelled run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunSummary {
    status: RunStatus,
    requested_shots: ShotCount,
    committed_shots: ShotCount,
    total_committed_shots: ShotCount,
}

impl RunSummary {
    pub(crate) const fn new(
        status: RunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
        total_committed_shots: u64,
    ) -> Self {
        Self {
            status,
            requested_shots,
            committed_shots: ShotCount::new(committed_shots),
            total_committed_shots: ShotCount::new(total_committed_shots),
        }
    }

    pub const fn status(self) -> RunStatus {
        self.status
    }

    pub const fn requested_shots(self) -> ShotCount {
        self.requested_shots
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn total_committed_shots(self) -> ShotCount {
        self.total_committed_shots
    }
}

/// Non-lossy composition of a family's engine failure `E` and a sink failure.
///
/// `E` is [`SamplingExecutionError`], [`DetectionExecutionError`], or
/// [`DemSamplingExecutionError`], the error of the session that ran.
#[derive(Debug)]
pub enum RunError<E, SinkError> {
    Engine {
        source: E,
        progress: RunProgress,
    },
    Sink {
        phase: SinkFailurePhase,
        source: SinkError,
        progress: RunProgress,
    },
}

impl<E, SinkError> RunError<E, SinkError> {
    pub const fn progress(&self) -> RunProgress {
        match self {
            Self::Engine { progress, .. } | Self::Sink { progress, .. } => *progress,
        }
    }

    /// Writes the error, naming a sink failure as `sink_label`'s sink.
    fn fmt_with_sink_label(
        &self,
        sink_label: &str,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result
    where
        E: fmt::Display,
        SinkError: fmt::Display,
    {
        match self {
            Self::Engine { source, progress } => write!(
                formatter,
                "{source} after {} committed shots",
                progress.committed_shots().get()
            ),
            Self::Sink {
                phase,
                source,
                progress,
            } => write!(
                formatter,
                "{sink_label} sink {} failed after {} committed shots while attempting {} shots: {source}",
                phase.as_str(),
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get()
            ),
        }
    }
}

/// The family-specific parts of [`RunError`]: the sink's name in messages, and the memory-budget
/// refusal of the engine error, which each family reports from its own error type.
macro_rules! run_error_family {
    ($error:ty, $sink_label:literal) => {
        impl<SinkError> RunError<$error, SinkError> {
            /// The engine's memory-budget refusal; a sink failure is the caller's own type and is
            /// not inspected.
            pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
                match self {
                    Self::Engine { source, .. } => source.memory_budget_exceeded(),
                    Self::Sink { .. } => None,
                }
            }
        }

        impl<SinkError: fmt::Display> fmt::Display for RunError<$error, SinkError> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.fmt_with_sink_label($sink_label, formatter)
            }
        }
    };
}

run_error_family!(SamplingExecutionError, "sampling");
run_error_family!(DetectionExecutionError, "detection");
run_error_family!(DemSamplingExecutionError, "DEM sampling");

impl<E, SinkError> std::error::Error for RunError<E, SinkError>
where
    E: std::error::Error + 'static,
    SinkError: std::error::Error + 'static,
    Self: fmt::Display,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::Sink { source, .. } => Some(source),
        }
    }
}
