// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <atomic>
#include <cstddef>
#include <cstdint>

#include "stim/mem/simd_bits.h"

namespace stab_qualification {

inline uint64_t simd_word_popcount_contract(
    uint64_t iterations,
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> &bits) {
    uint64_t checksum = 0;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        bits[300] ^= true;
        for (size_t word_index = 0; word_index < bits.num_simd_words; ++word_index) {
            checksum += bits.ptr_simd[word_index].popcount();
        }
    }
    return checksum;
}

}  // namespace stab_qualification
