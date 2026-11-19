//! The public packed reference sample and the packed sweep record a reference run reads.

use std::sync::Arc;

use stab_bits::{BitIter, BitSlice, BitVec};
use stab_records::MeasurementWidth;

use super::{SamplingExecutionError, SamplingPlan};

/// A deterministic reference measurement record, packed one bit per measurement.
///
/// Bit `i` is the noiseless outcome of measurement `i` in circuit order. The value is a shared
/// handle: cloning it, or reading a plan's cached sample through
/// [`SamplingPlan::try_reference_sample`], does not copy the bits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReferenceSample {
    /// The shared bits, or `None` for the empty record, which owns no storage.
    bits: Option<Arc<BitVec>>,
}

impl ReferenceSample {
    /// Bytes the handle allocates besides the words: the shared block's two reference counts
    /// and the vector header.
    const HANDLE_BYTES: u128 = (2 * size_of::<usize>() + size_of::<BitVec>()) as u128;

    /// Bytes a sample of `bits` measurements owns: nothing when empty, otherwise the shared
    /// handle and `ceil(bits / 64)` words.
    pub(crate) const fn storage_bytes(bits: usize) -> u128 {
        if bits == 0 {
            0
        } else {
            Self::HANDLE_BYTES + (bits.div_ceil(u64::BITS as usize) * size_of::<u64>()) as u128
        }
    }

    /// The number of measurements in the record.
    pub fn len(&self) -> usize {
        self.bits.as_deref().map_or(0, BitVec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The reference outcome of measurement `index`, or `None` past the record's end.
    pub fn get(&self, index: usize) -> Option<bool> {
        self.bits.as_deref()?.get(index)
    }

    /// The reference outcomes in measurement order.
    pub fn iter(&self) -> BitIter<'_> {
        self.as_bitslice().iter()
    }

    /// The packed record as a borrowed bit view.
    pub fn as_bitslice(&self) -> BitSlice<'_> {
        self.bits
            .as_deref()
            .map_or_else(BitSlice::default, BitVec::as_bitslice)
    }

    /// The packed little-endian words; bits at and past [`Self::len`] are zero.
    pub fn words(&self) -> &[u64] {
        self.bits.as_deref().map_or(&[], BitVec::words)
    }
}

impl From<BitVec> for ReferenceSample {
    fn from(bits: BitVec) -> Self {
        Self {
            bits: (!bits.is_empty()).then(|| Arc::new(bits)),
        }
    }
}

impl<'a> IntoIterator for &'a ReferenceSample {
    type Item = bool;
    type IntoIter = BitIter<'a>;

    fn into_iter(self) -> BitIter<'a> {
        self.iter()
    }
}

/// One assignment of a sampling plan's sweep bits, packed one bit per sweep bit.
///
/// [`Self::try_for_plan`] checks the width against the plan, so a record of the wrong width
/// cannot reach a reference run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SweepRecord {
    bits: BitVec,
}

impl SweepRecord {
    /// Wraps `bits` as a sweep assignment of `plan`, whose width it must equal.
    ///
    /// # Errors
    ///
    /// Returns [`SamplingExecutionError::InvalidSweepRecordWidth`] when `bits` has a different
    /// width from the plan's [`SamplingPlan::sweep_width`].
    pub fn try_for_plan(plan: &SamplingPlan, bits: BitVec) -> Result<Self, SamplingExecutionError> {
        validate_sweep_width(plan.sweep_width(), bits.len())?;
        Ok(Self { bits })
    }

    /// The number of sweep bits.
    pub fn width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.bits.len())
    }

    /// Sweep bit `index`, or `None` past the record's end.
    pub fn get(&self, index: usize) -> Option<bool> {
        self.bits.get(index)
    }

    /// The packed record as a borrowed bit view.
    pub fn as_bitslice(&self) -> BitSlice<'_> {
        self.bits.as_bitslice()
    }

    /// Whether every sweep bit is false.
    pub fn is_all_false(&self) -> bool {
        !self.bits.not_zero()
    }
}

pub(super) fn validate_sweep_width(
    expected: MeasurementWidth,
    actual: usize,
) -> Result<(), SamplingExecutionError> {
    if expected.get() == actual {
        Ok(())
    } else {
        Err(SamplingExecutionError::InvalidSweepRecordWidth {
            expected: expected.get(),
            actual,
        })
    }
}
