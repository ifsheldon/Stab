//! Output storage of the record-writing commands, admitted before it is allocated.
//!
//! `sample`, `detect`, and `m2d` encode every record through [`RecordEncoder`]s and write
//! named output files through a [`crate::io_plan::OUTPUT_BUFFER_BYTES`] buffer. Both are
//! allocated after the engine is compiled but before the run's session, so each command admits
//! them against the stage budget first and holds them until it ends; the session then admits
//! its own storage against what is left.
//!
//! A run that the session alone would refuse reports that session refusal exactly as it did
//! before output storage was admitted, so only runs whose session fits but not beside the
//! output storage report the output stage.

use stab_records::{RecordEncoder, RecordFormat};

use stab_model::MemoryBudget;

use crate::CliError;
use crate::io_plan::OUTPUT_BUFFER_BYTES;
use crate::ledger::{Hold, MemoryLedger, StageBudget};

/// Whether a run hands the encoders bit-plane batches, which each encoder transposes 64 shots
/// at a time, or shot-major records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncodedBatches {
    ShotMajor,
    BitPlanes,
}

/// The bytes one command's record outputs retain, summed before any of them is allocated.
#[derive(Debug)]
#[must_use = "output storage must be admitted before its encoders and buffers are allocated"]
pub(crate) struct OutputStorage {
    command: &'static str,
    batches: EncodedBatches,
    /// Records each encoder writes, when the command knows them up front.
    records: Option<u64>,
    bytes: u128,
}

impl OutputStorage {
    /// Starts the storage of a command whose encoders each write at most `records` records, or
    /// an unknown number when `None`.
    pub(crate) const fn new(
        command: &'static str,
        batches: EncodedBatches,
        records: Option<u64>,
    ) -> Self {
        Self {
            command,
            batches,
            records,
            bytes: 0,
        }
    }

    /// Adds one encoder of `format` whose records join parts of `part_widths` bits.
    pub(crate) fn encoder(mut self, format: RecordFormat, part_widths: &[usize]) -> Self {
        let width = part_widths
            .iter()
            .fold(0_usize, |total, width| total.saturating_add(*width));
        let mut bytes = match self.records {
            Some(records) => {
                RecordEncoder::shot_major_scratch_bytes_for_records(format, width, records)
            }
            None => RecordEncoder::shot_major_scratch_bytes(format, width),
        };
        if self.batches == EncodedBatches::BitPlanes {
            bytes =
                bytes.saturating_add(RecordEncoder::bit_plane_scratch_bytes(format, part_widths));
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self
    }

    /// Adds the write buffer of an output file when the command names one.
    ///
    /// Standard output's buffer is created with the process, before any budget is known, so it
    /// is not charged here.
    pub(crate) fn file_buffer(mut self, named: bool) -> Self {
        if named {
            self.bytes = self.bytes.saturating_add(OUTPUT_BUFFER_BYTES as u128);
        }
        self
    }

    /// Adds other per-command output scratch of `bytes`.
    pub(crate) fn scratch(mut self, bytes: u128) -> Self {
        self.bytes = self.bytes.saturating_add(bytes);
        self
    }

    /// Admits the storage against the next stage's budget and holds it, or refuses before
    /// anything is allocated.
    ///
    /// A requirement beyond `u64` is reported as the largest byte count, still a true lower bound
    /// for the refused storage.
    pub(crate) fn admit(self, ledger: &mut MemoryLedger) -> Result<Hold, CliError> {
        let stage = ledger.stage();
        let budget = stage.get();
        match u64::try_from(self.bytes) {
            Ok(bytes) if budget.admits(self.bytes) => Ok(ledger.hold(bytes)),
            _ => Err(stage.refusal(CliError::OutputStorageBudget {
                command: self.command,
                exceeded: stab_model::MemoryBudgetExceeded::new(
                    stab_model::ResourceAmount::saturating_from_u128(self.bytes),
                    budget,
                ),
            })),
        }
    }
}

/// Admits `output` and then builds the session with `build` against the budget left beside it.
///
/// When either is refused, the session is also built against the budget it had before the
/// output storage was held: a session that is refused there too reports that refusal unchanged,
/// so a run the session alone does not fit is diagnosed as it was before, and otherwise the
/// output or session refusal beside the held output storage is reported. The fallback session
/// is dropped before returning.
pub(crate) fn admit_output_then_session<T>(
    output: OutputStorage,
    ledger: &mut MemoryLedger,
    mut build: impl FnMut(MemoryBudget) -> Result<T, CliError>,
) -> Result<(Hold, T), CliError> {
    let before = ledger.stage();
    let refused = match output.admit(ledger) {
        Ok(hold) => {
            let stage = ledger.stage();
            match build(stage.get()) {
                Ok(session) => return Ok((hold, session)),
                Err(error) => {
                    ledger.release(hold);
                    stage.refusal(error)
                }
            }
        }
        Err(error) => error,
    };
    session_alone(before, &mut build)?;
    Err(refused)
}

/// Builds and drops the session against the budget it had without the output storage.
fn session_alone<T>(
    before: StageBudget,
    build: &mut impl FnMut(MemoryBudget) -> Result<T, CliError>,
) -> Result<(), CliError> {
    build(before.get())
        .map(drop)
        .map_err(|error| before.refusal(error))
}
