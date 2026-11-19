//! Iteration over the logical bits of packed storage.

use std::iter::FusedIterator;

use crate::{BitSlice, BitVec, WORD_BITS};

/// The logical bits of a [`BitSlice`] in index order, ignoring inactive tail bits.
#[derive(Clone, Debug)]
pub struct BitIter<'a> {
    words: &'a [u64],
    next: usize,
    end: usize,
}

impl<'a> BitIter<'a> {
    fn new(slice: BitSlice<'a>) -> Self {
        Self {
            words: slice.words(),
            next: 0,
            end: slice.len(),
        }
    }

    fn bit(&self, index: usize) -> bool {
        self.words
            .get(index / WORD_BITS)
            .is_some_and(|word| (word >> (index % WORD_BITS)) & 1 == 1)
    }
}

impl Iterator for BitIter<'_> {
    type Item = bool;

    #[inline]
    fn next(&mut self) -> Option<bool> {
        if self.next >= self.end {
            return None;
        }
        let bit = self.bit(self.next);
        self.next += 1;
        Some(bit)
    }

    fn nth(&mut self, n: usize) -> Option<bool> {
        self.next = self.next.saturating_add(n).min(self.end);
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.next;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for BitIter<'_> {
    fn next_back(&mut self) -> Option<bool> {
        if self.next >= self.end {
            return None;
        }
        self.end -= 1;
        Some(self.bit(self.end))
    }
}

impl ExactSizeIterator for BitIter<'_> {}

impl FusedIterator for BitIter<'_> {}

impl<'a> BitSlice<'a> {
    /// The slice's logical bits in index order.
    pub fn iter(&self) -> BitIter<'a> {
        BitIter::new(*self)
    }
}

impl<'a> IntoIterator for BitSlice<'a> {
    type Item = bool;
    type IntoIter = BitIter<'a>;

    fn into_iter(self) -> BitIter<'a> {
        BitIter::new(self)
    }
}

impl BitVec {
    /// The vector's logical bits in index order.
    pub fn iter(&self) -> BitIter<'_> {
        BitIter::new(self.as_bitslice())
    }
}

impl<'a> IntoIterator for &'a BitVec {
    type Item = bool;
    type IntoIter = BitIter<'a>;

    fn into_iter(self) -> BitIter<'a> {
        self.iter()
    }
}
