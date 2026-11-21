//! Shared vocabulary of the Stab components: memory budgets and their refusals, cooperative
//! cancellation, resource estimates, and the byte spans and severities of diagnostics.
//!
//! Every item here is a small value type that more than one component crate puts in its public
//! signatures, so that `stab-model`, `stab-records`, and `stab-algebra`, which do not depend on
//! one another, can agree on one type instead of converting between look-alikes. Each of those
//! crates re-exports the items it uses, so downstream code keeps importing them from the
//! component it already depends on.
//!
//! What belongs here: domain-neutral resource accounting ([`MemoryBudget`],
//! [`MemoryBudgetExceeded`], [`ResourceAmount`], [`Estimate`]), cancellation ([`Cancellation`]),
//! and the location and severity of a diagnostic ([`ByteSpan`], [`DiagnosticSeverity`]).
//!
//! What does not: anything that knows about circuits, detector error models, gates, packed bits,
//! result formats, or which operation was refused. Each component describes what it was building
//! with its own typed error and embeds the shared value only for the fact itself, for example
//! that a memory budget refused a requirement. The crate has no dependencies.

mod cancellation;
mod diagnostics;
mod estimate;
mod memory_budget;
mod refusal;

pub use cancellation::{Cancellation, CancellationCountdown, Cancelled};
pub use diagnostics::{ByteSpan, DiagnosticSeverity};
pub use estimate::Estimate;
pub use memory_budget::{MemoryBudget, MemoryBudgetParseError};
pub use refusal::{MemoryBudgetExceeded, ResourceAmount};
