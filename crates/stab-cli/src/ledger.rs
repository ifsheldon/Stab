//! Command-level memory ledger.
//!
//! One `--memory-budget` covers a whole command. Each stage admits its storage against the
//! budget less the bytes earlier stages still hold, and a stage releases its hold as soon as its
//! storage is dropped, so later stages draw on that budget again.

use stab_model::{Circuit, DetectorErrorModel, MemoryBudget, ParseLimits};

use crate::diagnostics::is_memory_budget_refusal;
use crate::{CliError, MemoryBudgetArg};

/// The memory one command may hold across all of its stages.
#[derive(Debug)]
pub(crate) struct MemoryLedger {
    total: MemoryBudget,
    held: u64,
}

/// Bytes one stage holds against its command's ledger until it is released.
#[must_use = "a hold must be released when its storage is dropped, or it keeps charging the ledger"]
#[derive(Debug)]
pub(crate) struct Hold {
    bytes: u64,
}

/// The budget one stage admits against: the command's budget less what earlier stages hold.
///
/// A library keeps the budget it was given, so a refusal it reports later (for example a session
/// checked against its plan's budget) is attributed with the bytes held when that budget was set.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StageBudget {
    total: MemoryBudget,
    held: u64,
}

impl MemoryLedger {
    pub(crate) const fn new(memory: MemoryBudgetArg) -> Self {
        Self {
            total: memory.get(),
            held: 0,
        }
    }

    /// The budget the next stage may admit.
    pub(crate) const fn stage(&self) -> StageBudget {
        StageBudget {
            total: self.total,
            held: self.held,
        }
    }

    /// Records bytes a stage admitted against [`Self::stage`] and still holds.
    ///
    /// The ledger saturates rather than wraps, so an over-reported hold leaves every later stage
    /// a smaller budget instead of a larger one.
    pub(crate) const fn hold(&mut self, bytes: u64) -> Hold {
        self.held = self.held.saturating_add(bytes);
        Hold { bytes }
    }

    /// Returns a stage's bytes to the budget once its storage is dropped.
    pub(crate) const fn release(&mut self, hold: Hold) {
        self.held = self.held.saturating_sub(hold.bytes);
    }

    /// Parses circuit text against the stage budget, drops the text, and holds the parsed model.
    ///
    /// The parser charges the source bytes and the model's retained bytes together, so the text
    /// needs no hold of its own; taking it by value releases it before the next stage.
    pub(crate) fn parse_circuit(&mut self, input: Vec<u8>) -> Result<(Circuit, Hold), CliError> {
        let stage = self.stage();
        let (circuit, model_bytes) =
            Circuit::from_stim_bytes_admitted(&input, stage.parse_limits())
                .map_err(|error| stage.refusal(error))?;
        drop(input);
        Ok((circuit, self.hold(model_bytes)))
    }

    /// Parses detector-error-model text like [`Self::parse_circuit`].
    pub(crate) fn parse_dem(
        &mut self,
        input: Vec<u8>,
    ) -> Result<(DetectorErrorModel, Hold), CliError> {
        let stage = self.stage();
        let (model, model_bytes) =
            DetectorErrorModel::from_dem_bytes_admitted(&input, stage.parse_limits())
                .map_err(|error| stage.refusal(error))?;
        drop(input);
        Ok((model, self.hold(model_bytes)))
    }
}

impl StageBudget {
    /// The bytes this stage may admit.
    pub(crate) const fn get(self) -> MemoryBudget {
        MemoryBudget::from_bytes(self.total.bytes().saturating_sub(self.held))
    }

    pub(crate) fn parse_limits(self) -> ParseLimits {
        ParseLimits::default().with_memory_budget(self.get())
    }

    /// The input byte cap: one past this is refused before the input is retained.
    pub(crate) const fn input_bytes(self) -> u64 {
        self.get().bytes()
    }

    /// The per-record text cap of streamed record inputs. A record's buffer doubles while it is
    /// framed, so half the stage budget keeps the buffer itself within it.
    pub(crate) fn record_bytes(self) -> usize {
        self.get().bytes_usize() / 2
    }

    /// Attributes a budget refusal reported against this stage's budget to the whole command.
    ///
    /// Refusals of a stage that ran with the full budget, and every other error, pass through
    /// unchanged.
    pub(crate) fn refusal(self, error: impl Into<CliError>) -> CliError {
        let error = error.into();
        if self.held == 0 || !is_memory_budget_refusal(&error) {
            return error;
        }
        CliError::CommandMemoryBudget {
            source: Box::new(error),
            held_bytes: self.held,
            budget_bytes: self.total.bytes(),
        }
    }
}
