//! Per-query cache of repeat-body detector counts and coordinate shifts.
//!
//! A coordinate query needs the detector count and coordinate shift of every repeat body it
//! enters. Computing them afresh at each body would revisit every deeper body once per enclosing
//! level, so query time would grow with the square of the nesting depth. Computing a body's value
//! here also records the value of every body nested inside it, and a recorded nested value is
//! combined in place of revisiting that body. Each value is accumulated body-first from zero, as
//! [`super::coordinate_shift_of`] and the circuit counters do, so the checked arithmetic, the
//! floating-point operations, and the first error are the same as without the cache.

use std::collections::HashMap;
use std::rc::Rc;

use crate::{Gate, ModelResult};

use super::super::{Circuit, CircuitItem};
use super::{add_coordinate_shift_mul, circuit_count_overflow};

/// Identifies a repeat body by address; the cache borrows the whole circuit for its lifetime,
/// so distinct bodies never share an address while it exists.
type BodyKey = *const Circuit;

/// Detector counts and coordinate shifts of the repeat bodies of one circuit.
///
/// The cache holds at most one count and one shift vector per repeat body, and a shift vector
/// is no longer than the widest `SHIFT_COORDS` nested in its body.
#[derive(Default)]
pub(super) struct BodySummaries<'a> {
    detector_counts: HashMap<BodyKey, u64>,
    shifts: HashMap<BodyKey, Rc<Vec<f64>>>,
    _circuit: std::marker::PhantomData<&'a Circuit>,
}

impl<'a> BodySummaries<'a> {
    /// Returns the number of detectors one execution of `body` declares.
    pub(super) fn detector_count(&mut self, body: &'a Circuit) -> ModelResult<u64> {
        if let Some(count) = self.detector_counts.get(&key(body)) {
            return Ok(*count);
        }
        // Each frame holds the enclosing block's iterator, its partial sum, the repeat count of
        // the body being summed, and that body's key.
        let mut enclosing: Vec<(std::slice::Iter<'a, CircuitItem>, u64, u64, BodyKey)> = Vec::new();
        let mut items = body.items().iter();
        let mut count = 0_u64;
        loop {
            match items.next() {
                Some(CircuitItem::Instruction(instruction)) => {
                    count = count
                        .checked_add(u64::from(instruction.gate() == Gate::DETECTOR))
                        .ok_or_else(circuit_count_overflow)?;
                }
                Some(CircuitItem::RepeatBlock(repeat)) => {
                    let repeat_count = repeat.repeat_count().get();
                    if let Some(body_count) = self.detector_counts.get(&key(repeat.body())) {
                        count = add_repeated_count(count, *body_count, repeat_count)?;
                        continue;
                    }
                    let parent = std::mem::replace(&mut items, repeat.body().items().iter());
                    enclosing.push((parent, count, repeat_count, key(repeat.body())));
                    count = 0;
                }
                None => {
                    let Some((parent, parent_count, repeat_count, body_key)) = enclosing.pop()
                    else {
                        self.detector_counts.insert(key(body), count);
                        return Ok(count);
                    };
                    self.detector_counts.insert(body_key, count);
                    count = add_repeated_count(parent_count, count, repeat_count)?;
                    items = parent;
                }
            }
        }
    }

    /// Returns the coordinate shift one execution of `body` applies.
    pub(super) fn shift(&mut self, body: &'a Circuit) -> ModelResult<Rc<Vec<f64>>> {
        if let Some(shift) = self.shifts.get(&key(body)) {
            return Ok(Rc::clone(shift));
        }
        let shift = Rc::new(self.accumulate_shift(body)?);
        self.shifts.insert(key(body), Rc::clone(&shift));
        Ok(shift)
    }

    /// Returns the coordinate shift of a body the caller enters only once, releasing its cached
    /// copy.
    pub(super) fn take_shift(&mut self, body: &'a Circuit) -> ModelResult<Rc<Vec<f64>>> {
        if let Some(shift) = self.shifts.remove(&key(body)) {
            return Ok(shift);
        }
        Ok(Rc::new(self.accumulate_shift(body)?))
    }

    /// Accumulates the shift of `body`, caching the shift of every body nested inside it.
    fn accumulate_shift(&mut self, body: &'a Circuit) -> ModelResult<Vec<f64>> {
        let mut enclosing: Vec<(std::slice::Iter<'a, CircuitItem>, Vec<f64>, u64, BodyKey)> =
            Vec::new();
        let mut items = body.items().iter();
        let mut shift = Vec::new();
        loop {
            match items.next() {
                Some(CircuitItem::Instruction(instruction)) => {
                    if instruction.gate() == Gate::SHIFT_COORDS
                        && let Some(args) = instruction.coordinate_arguments()
                    {
                        add_coordinate_shift_mul(&mut shift, args, 1.0)?;
                    }
                }
                Some(CircuitItem::RepeatBlock(repeat)) => {
                    let repeat_count = repeat.repeat_count().get();
                    if let Some(body_shift) = self.shifts.get(&key(repeat.body())) {
                        add_coordinate_shift_mul(&mut shift, body_shift, repeat_count as f64)?;
                        continue;
                    }
                    let parent = std::mem::replace(&mut items, repeat.body().items().iter());
                    let parent_shift = std::mem::take(&mut shift);
                    enclosing.push((parent, parent_shift, repeat_count, key(repeat.body())));
                }
                None => {
                    let Some((parent, parent_shift, repeat_count, body_key)) = enclosing.pop()
                    else {
                        return Ok(shift);
                    };
                    let body_shift = std::mem::replace(&mut shift, parent_shift);
                    add_coordinate_shift_mul(&mut shift, &body_shift, repeat_count as f64)?;
                    self.shifts.insert(body_key, Rc::new(body_shift));
                    items = parent;
                }
            }
        }
    }
}

fn key(body: &Circuit) -> BodyKey {
    std::ptr::from_ref(body)
}

fn add_repeated_count(count: u64, body_count: u64, repeat_count: u64) -> ModelResult<u64> {
    let repeated_count = body_count
        .checked_mul(repeat_count)
        .ok_or_else(circuit_count_overflow)?;
    count
        .checked_add(repeated_count)
        .ok_or_else(circuit_count_overflow)
}
