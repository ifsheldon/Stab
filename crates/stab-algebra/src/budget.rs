use std::fmt::{Display, Formatter};

use stab_bits::{BitError, BitVec};
use stab_foundation::MemoryBudget;

use super::{StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;
const WORD_BYTES: u128 = size_of::<u64>() as u128;

/// Admits `requested` bytes of `resource` storage against `budget`, or reports the typed refusal.
///
/// Constructors that turn a small request, such as a qubit count, into storage admit the
/// conservatively counted bytes of that storage before allocating or consuming randomness, then
/// reserve the storage fallibly. Operations whose storage is a fixed multiple of operands the
/// caller already holds, such as composition and inversion, reserve fallibly without a budget
/// because their operands were admitted when they were built.
pub(crate) fn admit(
    budget: MemoryBudget,
    resource: StabilizerResource,
    requested: u128,
) -> StabilizerResult<()> {
    budget
        .admit(requested)
        .map_err(|exceeded| StabilizerError::MemoryBudgetExceeded { resource, exceeded })
}

/// The stabilizer-algebra storage that a refusal or allocation failure refers to.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StabilizerResource {
    /// Bit planes of an owned Pauli string or Pauli iterator state.
    PauliString,
    /// Bit planes of an owned Clifford string.
    CliffordString,
    /// Packed generator rows of a Tableau.
    Tableau,
    /// A sampled random Tableau together with its sampling matrices.
    RandomTableau,
    /// The stabilizer solver's transposed working rows and its output Tableau.
    StabilizerSolve,
    /// Measurement and observable terms of a stabilizer flow.
    FlowTerms,
}

impl Display for StabilizerResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PauliString => "Pauli string",
            Self::CliffordString => "Clifford string",
            Self::Tableau => "Tableau",
            Self::RandomTableau => "random Tableau",
            Self::StabilizerSolve => "stabilizer solve",
            Self::FlowTerms => "flow terms",
        })
    }
}

/// Words needed for `bits` packed bits.
pub(crate) const fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(WORD_BITS)
}

/// Bytes of `planes` packed planes of `bits` bits each.
pub(crate) fn plane_bytes(planes: u128, bits: usize) -> u128 {
    planes * (bits as u128).div_ceil(WORD_BITS as u128) * WORD_BYTES
}

/// Bytes of a Pauli string over `num_qubits` qubits: two packed planes.
pub(crate) fn pauli_bytes(num_qubits: usize) -> u128 {
    plane_bytes(2, num_qubits)
}

/// Bytes of a Clifford string over `num_qubits` qubits: six packed planes.
pub(crate) fn clifford_bytes(num_qubits: usize) -> u128 {
    plane_bytes(6, num_qubits)
}

/// Bytes of packed signed Pauli rows: `rows` rows of two planes plus one packed sign plane.
pub(crate) fn pauli_rows_bytes(rows: u128, num_qubits: usize) -> u128 {
    rows * pauli_bytes(num_qubits) + rows.div_ceil(WORD_BITS as u128) * WORD_BYTES
}

/// Bytes of an `num_qubits`-qubit Tableau: `2n` packed signed rows.
pub(crate) fn tableau_bytes(num_qubits: usize) -> u128 {
    pauli_rows_bytes(2 * num_qubits as u128, num_qubits)
}

/// Builds a zeroed `bits`-bit vector with fallibly reserved words, reporting allocation failure
/// against `resource`.
pub(crate) fn try_zeroed_bits(
    resource: StabilizerResource,
    bits: usize,
) -> StabilizerResult<BitVec> {
    BitVec::try_zeros(bits).map_err(|error| bit_allocation_failed(resource, error))
}

/// Resizes `bits` to `len` bits, zero-filling any new words, which are reserved fallibly.
pub(crate) fn try_resize_zeroed_bits(
    resource: StabilizerResource,
    bits: &mut BitVec,
    len: usize,
) -> StabilizerResult<()> {
    bits.try_resize_zeros(len)
        .map_err(|error| bit_allocation_failed(resource, error))
}

/// Reports a `stab-bits` storage failure as an allocation failure of `resource`, keeping any other
/// bit error.
fn bit_allocation_failed(resource: StabilizerResource, error: BitError) -> StabilizerError {
    match error {
        BitError::StorageAllocationFailed { words } => {
            allocation_failed(resource, words.saturating_mul(size_of::<u64>()))
        }
        other => StabilizerError::Bit(other),
    }
}

/// Reserves exactly `len` zeroed words, reporting allocation failure as a typed error.
pub(crate) fn try_zeroed_words(
    resource: StabilizerResource,
    len: usize,
) -> StabilizerResult<Vec<u64>> {
    try_filled(resource, len, 0_u64)
}

/// Reserves exactly `len` copies of `value`, reporting allocation failure as a typed error.
pub(crate) fn try_filled<T: Clone>(
    resource: StabilizerResource,
    len: usize,
    value: T,
) -> StabilizerResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| allocation_failed(resource, len.saturating_mul(size_of::<T>())))?;
    values.resize(len, value);
    Ok(values)
}

/// Reserves capacity for exactly `len` values, reporting allocation failure as a typed error.
pub(crate) fn try_with_capacity<T>(
    resource: StabilizerResource,
    len: usize,
) -> StabilizerResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| allocation_failed(resource, len.saturating_mul(size_of::<T>())))?;
    Ok(values)
}

/// Grows `values` by one slot fallibly before a push.
pub(crate) fn try_reserve_one<T>(
    resource: StabilizerResource,
    values: &mut Vec<T>,
) -> StabilizerResult<()> {
    values.try_reserve(1).map_err(|_| {
        allocation_failed(
            resource,
            values
                .len()
                .saturating_add(1)
                .saturating_mul(size_of::<T>()),
        )
    })
}

pub(crate) const fn allocation_failed(
    resource: StabilizerResource,
    bytes: usize,
) -> StabilizerError {
    StabilizerError::AllocationFailed { resource, bytes }
}
