// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <atomic>
#include <cstdint>

#include "stim/mem/simd_bits.h"

namespace stab_qualification {

inline void simd_bits_xor_contract(
    uint64_t iterations,
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> &destination,
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> &source) {
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        destination ^= source;
    }
}

}  // namespace stab_qualification
