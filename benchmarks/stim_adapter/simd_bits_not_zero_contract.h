// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <atomic>
#include <cstdint>

#include "stim/mem/simd_bits.h"

namespace stab_qualification {

inline uint64_t simd_bits_not_zero_contract(
    uint64_t iterations,
    const stim::simd_bits<stim::MAX_BITWORD_WIDTH> &bits) {
    uint64_t checksum = 0;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        checksum += static_cast<uint64_t>(bits.not_zero());
    }
    return checksum;
}

}  // namespace stab_qualification
