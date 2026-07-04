use std::ops::{Bound, Range, RangeBounds};

use crate::{CircuitError, CircuitResult};

use super::{Circuit, CircuitInstruction, CircuitItem};

#[derive(Clone, Debug)]
pub struct CircuitFlattenedInstructionIter<'a> {
    stack: Vec<ForwardInstructionFrame<'a>>,
}

impl<'a> CircuitFlattenedInstructionIter<'a> {
    pub(super) fn new(circuit: &'a Circuit) -> Self {
        Self {
            stack: vec![ForwardInstructionFrame::new(circuit.items())],
        }
    }
}

impl<'a> Iterator for CircuitFlattenedInstructionIter<'a> {
    type Item = &'a CircuitInstruction;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.stack.last_mut() {
            if frame.index == frame.items.len() {
                if frame.start_next_repetition() {
                    continue;
                }
                self.stack.pop();
                continue;
            }

            let item = frame.items.get(frame.index)?;
            frame.index += 1;
            match item {
                CircuitItem::Instruction(instruction) => return Some(instruction),
                CircuitItem::RepeatBlock(repeat) => {
                    if !repeat.body().items().is_empty() {
                        self.stack.push(ForwardInstructionFrame::new_repeated(
                            repeat.body().items(),
                            repeat.repeat_count().get(),
                        ));
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
struct ForwardInstructionFrame<'a> {
    items: &'a [CircuitItem],
    index: usize,
    remaining_repetitions: u64,
}

impl<'a> ForwardInstructionFrame<'a> {
    fn new(items: &'a [CircuitItem]) -> Self {
        Self::new_repeated(items, 1)
    }

    fn new_repeated(items: &'a [CircuitItem], repetitions: u64) -> Self {
        Self {
            items,
            index: 0,
            remaining_repetitions: repetitions,
        }
    }

    fn start_next_repetition(&mut self) -> bool {
        if self.remaining_repetitions > 1 {
            self.remaining_repetitions -= 1;
            self.index = 0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
pub struct CircuitFlattenedInstructionRevIter<'a> {
    stack: Vec<ReverseInstructionFrame<'a>>,
}

impl<'a> CircuitFlattenedInstructionRevIter<'a> {
    pub(super) fn new(circuit: &'a Circuit) -> Self {
        Self {
            stack: vec![ReverseInstructionFrame::new(circuit.items())],
        }
    }
}

impl<'a> Iterator for CircuitFlattenedInstructionRevIter<'a> {
    type Item = &'a CircuitInstruction;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.stack.last_mut() {
            if frame.index == 0 {
                if frame.start_previous_repetition() {
                    continue;
                }
                self.stack.pop();
                continue;
            }

            frame.index -= 1;
            let item = frame.items.get(frame.index)?;
            match item {
                CircuitItem::Instruction(instruction) => return Some(instruction),
                CircuitItem::RepeatBlock(repeat) => {
                    if !repeat.body().items().is_empty() {
                        self.stack.push(ReverseInstructionFrame::new_repeated(
                            repeat.body().items(),
                            repeat.repeat_count().get(),
                        ));
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug)]
struct ReverseInstructionFrame<'a> {
    items: &'a [CircuitItem],
    index: usize,
    remaining_repetitions: u64,
}

impl<'a> ReverseInstructionFrame<'a> {
    fn new(items: &'a [CircuitItem]) -> Self {
        Self::new_repeated(items, 1)
    }

    fn new_repeated(items: &'a [CircuitItem], repetitions: u64) -> Self {
        Self {
            items,
            index: items.len(),
            remaining_repetitions: repetitions,
        }
    }

    fn start_previous_repetition(&mut self) -> bool {
        if self.remaining_repetitions > 1 {
            self.remaining_repetitions -= 1;
            self.index = self.items.len();
            true
        } else {
            false
        }
    }
}

pub(super) fn checked_item_range(
    range: impl RangeBounds<usize>,
    len: usize,
) -> CircuitResult<Range<usize>> {
    let start = match range.start_bound() {
        Bound::Included(start) => *start,
        Bound::Excluded(start) => start
            .checked_add(1)
            .ok_or_else(|| circuit_item_range_error("excluded start index overflowed"))?,
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(end) => end
            .checked_add(1)
            .ok_or_else(|| circuit_item_range_error("included end index overflowed"))?,
        Bound::Excluded(end) => *end,
        Bound::Unbounded => len,
    };

    if start > end || end > len {
        return Err(circuit_item_range_error(format!(
            "{start}..{end} outside top-level item length {len}",
        )));
    }
    Ok(start..end)
}

pub(super) fn circuit_item_range_error(value: impl ToString) -> CircuitError {
    CircuitError::invalid_domain_value("circuit item range", value)
}
