//! Program-wide storage for the variable-length payloads of compiled instructions.
//!
//! Every instruction of a [`super::CircuitProgram`] refers to its targets, flags, Pauli terms,
//! controlled pairs, and probability tables through an [`ArenaSpan`] into one [`ProgramArena`]
//! instead of owning a heap allocation, so lowering a circuit of millions of short instructions
//! reserves a handful of vectors once, and dropping it frees them at once. Admission counts the
//! elements of every pool before materialization reserves them exactly, and a pool never grows
//! past that reservation.

use std::marker::PhantomData;

use stab_algebra::PauliBasis;

use super::{ControlledPair, admission_drift, byte_product, checked_add_bytes, storage_overflow};
use crate::detection::error::{DetectionError, DetectionResult};

/// A run of consecutive elements of one [`ProgramArena`] pool.
///
/// Spans are created only by [`ProgramArena::span_since`] and every pool holds at most
/// `u32::MAX` elements, so `start + len` never exceeds the pool it was cut from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArenaSpan<T> {
    start: u32,
    len: u32,
    element: PhantomData<fn() -> T>,
}

impl<T> ArenaSpan<T> {
    /// The span of no elements, used for absent optional payloads such as inversion flags.
    pub(crate) const EMPTY: Self = Self {
        start: 0,
        len: 0,
        element: PhantomData,
    };

    pub(crate) const fn len(self) -> usize {
        self.len as usize
    }
}

/// One product of an `MPP` instruction: its term count and its static sign.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PauliProduct {
    term_count: u32,
    inverted: bool,
}

impl PauliProduct {
    pub(super) fn new(term_count: usize, inverted: bool) -> DetectionResult<Self> {
        Ok(Self {
            term_count: u32::try_from(term_count).map_err(|_| storage_overflow())?,
            inverted,
        })
    }
}

/// A resolved Pauli term: a frame qubit and the basis applied to it.
///
/// Qubit ids are `u32` in the circuit model, so the term packs into 8 bytes.
pub(crate) type PauliTerm = (u32, PauliBasis);

/// The element types a [`ProgramArena`] stores, each in its own pool.
pub(crate) trait ArenaElement: Copy + Sized {
    fn pool(arena: &ProgramArena) -> &Vec<Self>;
    fn pool_mut(arena: &mut ProgramArena) -> &mut Vec<Self>;
}

macro_rules! arena_element {
    ($element:ty, $pool:ident) => {
        impl ArenaElement for $element {
            #[inline(always)]
            fn pool(arena: &ProgramArena) -> &Vec<Self> {
                &arena.$pool
            }

            fn pool_mut(arena: &mut ProgramArena) -> &mut Vec<Self> {
                &mut arena.$pool
            }
        }
    };
}

arena_element!(u32, qubits);
arena_element!(bool, flags);
arena_element!(PauliTerm, terms);
arena_element!(ControlledPair, pairs);
arena_element!(f64, probabilities);
arena_element!(PauliProduct, products);

/// Element counts of every [`ProgramArena`] pool, as admission derives them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ArenaCounts {
    /// Qubit targets.
    pub(super) qubits: usize,
    /// Result inversion flags and measurement pad values.
    pub(super) flags: usize,
    /// Pauli terms of correlated errors, observables, and Pauli products.
    pub(super) terms: usize,
    /// Controlled-Pauli target pairs.
    pub(super) pairs: usize,
    /// Probabilities of multi-outcome noise channels.
    pub(super) probabilities: usize,
    /// Products of `MPP` instructions.
    pub(super) products: usize,
}

impl ArenaCounts {
    /// Adds `other`, refusing totals that a [`ArenaSpan`] cannot index.
    pub(super) fn add(&mut self, other: Self) -> DetectionResult<()> {
        let sum = |left: usize, right: usize| {
            left.checked_add(right)
                .filter(|total| u32::try_from(*total).is_ok())
                .ok_or_else(arena_index_limit)
        };
        *self = Self {
            qubits: sum(self.qubits, other.qubits)?,
            flags: sum(self.flags, other.flags)?,
            terms: sum(self.terms, other.terms)?,
            pairs: sum(self.pairs, other.pairs)?,
            probabilities: sum(self.probabilities, other.probabilities)?,
            products: sum(self.products, other.products)?,
        };
        Ok(())
    }

    /// Adds `qubits` qubit targets, which is [`Self::add`] of a payload holding only qubit
    /// targets, since every other total already fits an [`ArenaSpan`] index.
    #[inline(always)]
    pub(super) fn add_qubits(&mut self, qubits: usize) -> DetectionResult<()> {
        self.qubits = self
            .qubits
            .checked_add(qubits)
            .filter(|total| u32::try_from(*total).is_ok())
            .ok_or_else(arena_index_limit)?;
        Ok(())
    }

    /// The bytes these elements occupy.
    pub(super) fn bytes(self) -> DetectionResult<u64> {
        [
            byte_product(self.qubits, size_of::<u32>())?,
            byte_product(self.flags, size_of::<bool>())?,
            byte_product(self.terms, size_of::<PauliTerm>())?,
            byte_product(self.pairs, size_of::<ControlledPair>())?,
            byte_product(self.probabilities, size_of::<f64>())?,
            byte_product(self.products, size_of::<PauliProduct>())?,
        ]
        .into_iter()
        .try_fold(0, checked_add_bytes)
    }
}

/// The payload pools of one compiled program.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProgramArena {
    qubits: Vec<u32>,
    flags: Vec<bool>,
    terms: Vec<PauliTerm>,
    pairs: Vec<ControlledPair>,
    probabilities: Vec<f64>,
    products: Vec<PauliProduct>,
}

impl ProgramArena {
    /// Reserves exactly the admitted element count of every pool.
    pub(super) fn try_with_capacity(counts: ArenaCounts) -> DetectionResult<Self> {
        let mut arena = Self::default();
        reserve(&mut arena.qubits, counts.qubits, "resolved qubit targets")?;
        reserve(&mut arena.flags, counts.flags, "result flags")?;
        reserve(&mut arena.terms, counts.terms, "resolved Pauli terms")?;
        reserve(&mut arena.pairs, counts.pairs, "controlled Pauli pairs")?;
        reserve(
            &mut arena.probabilities,
            counts.probabilities,
            "noise channel probabilities",
        )?;
        reserve(&mut arena.products, counts.products, "Pauli products")?;
        Ok(arena)
    }

    /// The elements of `span`.
    #[inline(always)]
    pub(crate) fn get<T: ArenaElement>(&self, span: ArenaSpan<T>) -> DetectionResult<&[T]> {
        let start = span.start as usize;
        T::pool(self)
            .get(start..start + span.len())
            .ok_or_else(span_escaped)
    }

    /// The `N` probabilities of `span`.
    #[inline(always)]
    pub(crate) fn probabilities<const N: usize>(
        &self,
        span: ArenaSpan<f64>,
    ) -> DetectionResult<[f64; N]> {
        self.get(span)?.try_into().map_err(|_| span_escaped())
    }

    /// Appends `value` within the admitted reservation.
    pub(super) fn push<T: ArenaElement>(&mut self, value: T) -> DetectionResult<()> {
        let pool = T::pool_mut(self);
        if pool.len() == pool.capacity() {
            return Err(admission_drift());
        }
        pool.push(value);
        Ok(())
    }

    /// Appends `values` within the admitted reservation and returns their span.
    pub(super) fn extend<T: ArenaElement>(
        &mut self,
        values: impl IntoIterator<Item = T>,
    ) -> DetectionResult<ArenaSpan<T>> {
        let start = self.mark::<T>();
        for value in values {
            self.push(value)?;
        }
        self.span_since(start)
    }

    /// The current length of `T`'s pool, the start of the next span.
    pub(super) fn mark<T: ArenaElement>(&self) -> usize {
        T::pool(self).len()
    }

    /// The span of the elements pushed onto `T`'s pool since `start`.
    pub(super) fn span_since<T: ArenaElement>(
        &self,
        start: usize,
    ) -> DetectionResult<ArenaSpan<T>> {
        let end = T::pool(self).len();
        let len = end.checked_sub(start).ok_or_else(admission_drift)?;
        Ok(ArenaSpan {
            start: u32::try_from(start).map_err(|_| arena_index_limit())?,
            len: u32::try_from(len).map_err(|_| arena_index_limit())?,
            element: PhantomData,
        })
    }

    /// The bytes the pools retain.
    pub(super) fn retained_bytes(&self) -> DetectionResult<u64> {
        ArenaCounts {
            qubits: self.qubits.capacity(),
            flags: self.flags.capacity(),
            terms: self.terms.capacity(),
            pairs: self.pairs.capacity(),
            probabilities: self.probabilities.capacity(),
            products: self.products.capacity(),
        }
        .bytes()
    }

    /// Yields each product of `MPP` instruction storage as its terms and static inversion.
    pub(crate) fn pauli_products(
        &self,
        terms: ArenaSpan<PauliTerm>,
        products: ArenaSpan<PauliProduct>,
    ) -> DetectionResult<impl Iterator<Item = DetectionResult<(&[PauliTerm], bool)>> + '_> {
        let mut rest = self.get(terms)?;
        Ok(self.get(products)?.iter().map(move |product| {
            let (terms, tail) = rest
                .split_at_checked(product.term_count as usize)
                .ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled Pauli product escaped its term arena",
                    )
                })?;
            rest = tail;
            Ok((terms, product.inverted))
        }))
    }
}

fn reserve<T>(pool: &mut Vec<T>, capacity: usize, context: &str) -> DetectionResult<()> {
    pool.try_reserve_exact(capacity).map_err(|error| {
        DetectionError::storage_allocation(format!(
            "unable to reserve {capacity} {context}: {error}"
        ))
    })
}

fn span_escaped() -> DetectionError {
    DetectionError::invalid_sampler_compilation("compiled instruction payload escaped its arena")
}

fn arena_index_limit() -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "the circuit lowers to more than {} payload elements of one kind, more than one compiled program can index",
        u32::MAX
    ))
}
