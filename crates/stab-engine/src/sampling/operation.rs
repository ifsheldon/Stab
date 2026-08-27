use stab_algebra::PauliBasis;
use stab_model::RepeatNestingLimit;
use stab_model::advanced::ClassicalControl;

use arrayvec::ArrayVec;

use super::stabilizer_frame::LocalTableauTransform;

pub(super) const MIN_REFERENCE_FOLD_WORK: u128 = 64;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SampleOperation {
    ApplyHadamard {
        qubit: usize,
    },
    ApplyControlledX {
        control: usize,
        target: usize,
    },
    ApplyTableau {
        targets: Vec<usize>,
        transform: LocalTableauTransform,
    },
    Reset {
        qubit: usize,
        basis: PauliBasis,
    },
    Measure {
        qubit: usize,
        basis: PauliBasis,
        inverted: bool,
        flip_probability: f64,
        reset: bool,
    },
    MeasureProduct {
        terms: Vec<(usize, PauliBasis)>,
        inverted: bool,
        flip_probability: f64,
    },
    Pad {
        value: bool,
        flip_probability: f64,
    },
    SingleQubitPauliChannel {
        qubit: usize,
        probabilities: [f64; 3],
        total_probability: f64,
    },
    TwoQubitPauliChannel {
        left: usize,
        right: usize,
        probabilities: [f64; 15],
        total_probability: f64,
    },
    CorrelatedError {
        else_branch: bool,
        probability: f64,
        terms: Vec<(usize, PauliBasis)>,
    },
    HeraldedPauliChannel {
        qubit: usize,
        probabilities: [f64; 4],
    },
    ClassicallyControlledPauli {
        control: ClassicalControl,
        qubit: usize,
        basis: PauliBasis,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SampleProgramEntry {
    Execute(SampleOperation),
    Repeat { count: u64, body_end: usize },
    EndRepeat,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SampleProgram {
    entries: Vec<SampleProgramEntry>,
}

impl SampleProgram {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn push(
        &mut self,
        operation: SampleOperation,
    ) -> Result<(), super::SamplingCompileError> {
        self.try_push_entry(SampleProgramEntry::Execute(operation))
    }

    pub(super) fn begin_repeat(
        &mut self,
        count: u64,
    ) -> Result<usize, super::SamplingCompileError> {
        let marker = self.entries.len();
        self.try_push_entry(SampleProgramEntry::Repeat {
            count,
            body_end: usize::MAX,
        })?;
        Ok(marker)
    }

    pub(super) fn finish_repeat(
        &mut self,
        marker: usize,
    ) -> Result<(), super::SamplingCompileError> {
        if self.entries.len() == marker.saturating_add(1) {
            self.entries.pop();
            return Ok(());
        }
        let body_end = self.entries.len();
        self.try_push_entry(SampleProgramEntry::EndRepeat)?;
        let Some(SampleProgramEntry::Repeat {
            body_end: stored_end,
            ..
        }) = self.entries.get_mut(marker)
        else {
            return Err(super::SamplingCompileError::invalid_circuit(
                "sampler repeat marker was not retained during compilation",
            ));
        };
        *stored_end = body_end;
        Ok(())
    }

    pub(super) fn elide_leading_z_resets(&mut self) {
        let leading_z_resets = self
            .entries
            .iter()
            .take_while(|entry| {
                matches!(
                    entry,
                    SampleProgramEntry::Execute(SampleOperation::Reset {
                        basis: PauliBasis::Z,
                        ..
                    })
                )
            })
            .count();
        if leading_z_resets > 0 {
            self.entries.drain(..leading_z_resets);
            for entry in &mut self.entries {
                if let SampleProgramEntry::Repeat { body_end, .. } = entry {
                    *body_end -= leading_z_resets;
                }
            }
        }
    }

    pub(super) fn entries(&self) -> &[SampleProgramEntry] {
        &self.entries
    }

    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn executable_operations(&self) -> impl Iterator<Item = &SampleOperation> {
        self.entries.iter().filter_map(|entry| match entry {
            SampleProgramEntry::Execute(operation) => Some(operation),
            SampleProgramEntry::Repeat { .. } | SampleProgramEntry::EndRepeat => None,
        })
    }

    pub(super) fn cursor(&self) -> SampleProgramCursor<'_> {
        SampleProgramCursor::new(&self.entries, 0, self.entries.len())
    }

    pub(super) fn cursor_range(&self, start: usize, end: usize) -> SampleProgramCursor<'_> {
        SampleProgramCursor::new(&self.entries, start, end)
    }

    pub(super) fn compact_operation_count(&self, start: usize, end: usize) -> usize {
        self.entries
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .filter(|entry| matches!(entry, SampleProgramEntry::Execute(_)))
            .count()
    }

    pub(super) fn range_has_record_or_sweep_controls(&self, start: usize, end: usize) -> bool {
        self.entries
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .any(|entry| {
                matches!(
                    entry,
                    SampleProgramEntry::Execute(SampleOperation::ClassicallyControlledPauli { .. })
                )
            })
    }

    pub(super) fn has_reference_fold_candidate(&self, qubit_count: usize) -> bool {
        self.entries
            .iter()
            .enumerate()
            .any(|(index, entry)| match entry {
                SampleProgramEntry::Repeat { count, body_end }
                    if *body_end > index
                        && matches!(
                            self.entries.get(*body_end),
                            Some(SampleProgramEntry::EndRepeat)
                        ) =>
                {
                    self.reference_fold_is_profitable(index + 1, *body_end, *count, qubit_count)
                }
                _ => false,
            })
    }

    pub(super) fn reference_fold_is_profitable(
        &self,
        start: usize,
        end: usize,
        repeat_count: u64,
        qubit_count: usize,
    ) -> bool {
        if self.range_has_record_or_sweep_controls(start, end) {
            return false;
        }
        let repeated_work = self
            .reference_work_units(start, end, qubit_count)
            .saturating_mul(u128::from(repeat_count.saturating_sub(1)));
        let snapshot_work = (qubit_count as u128)
            .saturating_mul(qubit_count as u128)
            .saturating_mul(2);
        repeated_work >= MIN_REFERENCE_FOLD_WORK.max(snapshot_work)
    }

    fn reference_work_units(&self, start: usize, end: usize, qubit_count: usize) -> u128 {
        let qubits = (qubit_count as u128).max(1);
        self.entries
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| match entry {
                SampleProgramEntry::Execute(operation) => Some(operation),
                SampleProgramEntry::Repeat { .. } | SampleProgramEntry::EndRepeat => None,
            })
            .fold(0_u128, |work, operation| {
                let operation_work = match operation {
                    SampleOperation::ApplyHadamard { .. }
                    | SampleOperation::ApplyControlledX { .. } => qubits,
                    SampleOperation::ApplyTableau { targets, .. } => {
                        qubits.saturating_mul((targets.len() as u128).max(1))
                    }
                    SampleOperation::Reset { .. }
                    | SampleOperation::Measure { .. }
                    | SampleOperation::MeasureProduct { .. } => qubits.saturating_mul(qubits),
                    SampleOperation::Pad { .. }
                    | SampleOperation::SingleQubitPauliChannel { .. }
                    | SampleOperation::TwoQubitPauliChannel { .. }
                    | SampleOperation::CorrelatedError { .. }
                    | SampleOperation::HeraldedPauliChannel { .. }
                    | SampleOperation::ClassicallyControlledPauli { .. } => 1,
                };
                work.saturating_add(operation_work)
            })
    }

    fn try_push_entry(
        &mut self,
        entry: SampleProgramEntry,
    ) -> Result<(), super::SamplingCompileError> {
        if self.entries.len() == self.entries.capacity() {
            self.entries.try_reserve(1).map_err(|error| {
                super::SamplingCompileError::invalid_circuit(format!(
                    "unable to grow the compact sampling program: {error}"
                ))
            })?;
        }
        self.entries.push(entry);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct RepeatExecutionFrame {
    body_start: usize,
    body_end: usize,
    remaining: u64,
}

pub(super) struct SampleProgramCursor<'a> {
    entries: &'a [SampleProgramEntry],
    index: usize,
    end: usize,
    repeats: ArrayVec<RepeatExecutionFrame, { RepeatNestingLimit::HARD_MAX }>,
}

impl<'a> SampleProgramCursor<'a> {
    fn new(entries: &'a [SampleProgramEntry], start: usize, end: usize) -> Self {
        Self {
            entries,
            index: start,
            end,
            repeats: ArrayVec::new(),
        }
    }

    pub(super) fn next_operation(
        &mut self,
    ) -> Result<Option<&'a SampleOperation>, super::SamplingExecutionError> {
        loop {
            if self.index >= self.end {
                if self.repeats.is_empty() {
                    return Ok(None);
                }
                return Err(cursor_invariant(
                    "sampling program range ended inside a repeat",
                ));
            }
            let entry = self.entries.get(self.index).ok_or_else(|| {
                cursor_invariant("sampling program cursor moved beyond retained operations")
            })?;
            match entry {
                SampleProgramEntry::Execute(operation) => {
                    self.index = self.index.checked_add(1).ok_or_else(|| {
                        cursor_invariant("sampling program cursor index overflowed")
                    })?;
                    return Ok(Some(operation));
                }
                SampleProgramEntry::Repeat { count, body_end } => {
                    if *count == 0
                        || *body_end <= self.index
                        || *body_end >= self.end
                        || !matches!(
                            self.entries.get(*body_end),
                            Some(SampleProgramEntry::EndRepeat)
                        )
                    {
                        return Err(cursor_invariant(
                            "sampling program contains an invalid repeat marker",
                        ));
                    }
                    let frame = RepeatExecutionFrame {
                        body_start: self.index.checked_add(1).ok_or_else(|| {
                            cursor_invariant("sampling repeat body index overflowed")
                        })?,
                        body_end: *body_end,
                        remaining: *count,
                    };
                    self.repeats.try_push(frame).map_err(|_| {
                        cursor_invariant("sampling program exceeded the admitted repeat nesting")
                    })?;
                    self.index = frame.body_start;
                }
                SampleProgramEntry::EndRepeat => {
                    let frame = self.repeats.last_mut().ok_or_else(|| {
                        cursor_invariant("sampling program ended an absent repeat")
                    })?;
                    if frame.body_end != self.index || frame.remaining == 0 {
                        return Err(cursor_invariant(
                            "sampling program repeat end disagrees with its marker",
                        ));
                    }
                    if frame.remaining > 1 {
                        frame.remaining -= 1;
                        self.index = frame.body_start;
                    } else {
                        self.repeats.pop();
                        self.index = self.index.checked_add(1).ok_or_else(|| {
                            cursor_invariant("sampling repeat end index overflowed")
                        })?;
                    }
                }
            }
        }
    }
}

fn cursor_invariant(message: &str) -> super::SamplingExecutionError {
    super::SamplingExecutionError::InternalInvariant {
        message: message.to_owned(),
    }
}

pub(super) const SINGLE_QUBIT_PAULI_CHANNEL_BASES: [PauliBasis; 3] =
    [PauliBasis::X, PauliBasis::Y, PauliBasis::Z];

pub(super) const TWO_QUBIT_PAULI_CHANNEL_BASES: [(Option<PauliBasis>, Option<PauliBasis>); 15] = [
    (None, Some(PauliBasis::X)),
    (None, Some(PauliBasis::Y)),
    (None, Some(PauliBasis::Z)),
    (Some(PauliBasis::X), None),
    (Some(PauliBasis::X), Some(PauliBasis::X)),
    (Some(PauliBasis::X), Some(PauliBasis::Y)),
    (Some(PauliBasis::X), Some(PauliBasis::Z)),
    (Some(PauliBasis::Y), None),
    (Some(PauliBasis::Y), Some(PauliBasis::X)),
    (Some(PauliBasis::Y), Some(PauliBasis::Y)),
    (Some(PauliBasis::Y), Some(PauliBasis::Z)),
    (Some(PauliBasis::Z), None),
    (Some(PauliBasis::Z), Some(PauliBasis::X)),
    (Some(PauliBasis::Z), Some(PauliBasis::Y)),
    (Some(PauliBasis::Z), Some(PauliBasis::Z)),
];
