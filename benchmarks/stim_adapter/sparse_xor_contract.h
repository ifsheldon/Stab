// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <vector>

#include "stim/mem/sparse_xor_vec.h"

namespace stab_qualification {

template <typename T>
inline T &sparse_xor_optimizer_opaque_mutable(T &value) {
    T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

inline void sparse_xor_row_callback(
    std::vector<stim::SparseXorVec<uint32_t>> &table) {
    const size_t row_count = table.size();
    for (size_t row = 1; row < row_count; ++row) {
        table[row - 1] ^= table[row];
    }
    for (size_t row = row_count; --row > 1;) {
        table[row - 1] ^= table[row];
    }
}

inline void sparse_xor_item_callback(stim::SparseXorVec<uint32_t> &buffer) {
    constexpr uint32_t ITEMS[]{2, 5, 9, 5, 3, 6, 10};
    for (const auto item : ITEMS) {
        buffer.xor_item(item);
    }
}

inline void sparse_xor_row_contract(
    uint64_t iterations,
    uint64_t sweeps,
    std::vector<stim::SparseXorVec<uint32_t>> &table) {
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        for (uint64_t sweep = 0; sweep < sweeps; ++sweep) {
            std::atomic_signal_fence(std::memory_order_seq_cst);
            sparse_xor_row_callback(sparse_xor_optimizer_opaque_mutable(table));
        }
    }
}

inline void sparse_xor_item_contract(
    uint64_t iterations,
    uint64_t sweeps,
    stim::SparseXorVec<uint32_t> &buffer) {
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        for (uint64_t sweep = 0; sweep < sweeps; ++sweep) {
            std::atomic_signal_fence(std::memory_order_seq_cst);
            sparse_xor_item_callback(sparse_xor_optimizer_opaque_mutable(buffer));
        }
    }
}

}  // namespace stab_qualification
