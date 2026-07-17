use super::super::invocation;

const MAIN: &str = "benchmarks/stim_adapter/main.cc";
pub(super) const SIMD_WORD_POPCOUNT: [&str; 2] = [
    MAIN,
    "benchmarks/stim_adapter/simd_word_popcount_contract.h",
];
pub(super) const SIMD_BITS_XOR: [&str; 2] =
    [MAIN, "benchmarks/stim_adapter/simd_bits_xor_contract.h"];
pub(super) const SIMD_BITS_NOT_ZERO: [&str; 2] = [
    MAIN,
    "benchmarks/stim_adapter/simd_bits_not_zero_contract.h",
];
pub(super) const SPARSE_XOR: [&str; 2] = [MAIN, "benchmarks/stim_adapter/sparse_xor_contract.h"];
pub(super) const BIT_MATRIX_TRANSPOSE: [&str; 2] = [
    MAIN,
    "benchmarks/stim_adapter/bit_matrix_transpose_contract.h",
];
pub(super) const PAULI_STRING_MULTIPLY: [&str; 2] = [
    MAIN,
    "benchmarks/stim_adapter/pauli_string_multiply_contract.h",
];
pub(super) const PAULI_STRING_ITER: [&str; 2] =
    [MAIN, "benchmarks/stim_adapter/pauli_string_iter_contract.h"];

pub(super) fn expected_paths(group_id: &str) -> &'static [&'static str] {
    match group_id {
        "PERFQ-M5-SIMD-WORD" => &SIMD_WORD_POPCOUNT,
        "PERFQ-M5-SIMD-BITS" => &SIMD_BITS_XOR,
        invocation::SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID
        | invocation::SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID
        | invocation::SIMD_BITS_NOT_ZERO_LATE_GROUP_ID => &SIMD_BITS_NOT_ZERO,
        invocation::SPARSE_XOR_ROW_GROUP_ID | invocation::SPARSE_XOR_ITEM_GROUP_ID => &SPARSE_XOR,
        invocation::BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID
        | invocation::BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID => &BIT_MATRIX_TRANSPOSE,
        invocation::PAULI_STRING_MULTIPLY_GROUP_ID => &PAULI_STRING_MULTIPLY,
        invocation::PAULI_STRING_ITER_RANGE_GROUP_ID
        | invocation::PAULI_STRING_ITER_SINGLETON_GROUP_ID => &PAULI_STRING_ITER,
        _ => &[],
    }
}
