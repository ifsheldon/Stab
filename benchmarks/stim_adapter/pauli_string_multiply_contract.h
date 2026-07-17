// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <array>
#include <atomic>
#include <bit>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>

#include "stim/stabilizers/pauli_string.h"

namespace stab_qualification {

constexpr uint64_t PAULI_MULTIPLY_MIN_QUBITS = 1;
constexpr uint64_t PAULI_MULTIPLY_MAX_QUBITS = 1048576;
constexpr uint64_t PAULI_MULTIPLY_WORKLOAD_MARKER = 5;

inline bool is_pauli_string_multiply_workload(std::string_view workload) {
    return workload == "pauli-string-right-multiply";
}

inline size_t checked_pauli_multiply_width(uint64_t work_items) {
    if (work_items < PAULI_MULTIPLY_MIN_QUBITS) {
        throw std::invalid_argument(
            "Pauli multiplication width " + std::to_string(work_items) +
            " is below the minimum " + std::to_string(PAULI_MULTIPLY_MIN_QUBITS));
    }
    if (work_items > PAULI_MULTIPLY_MAX_QUBITS) {
        throw std::invalid_argument(
            "Pauli multiplication width " + std::to_string(work_items) +
            " exceeds maximum " + std::to_string(PAULI_MULTIPLY_MAX_QUBITS));
    }
    if (work_items > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("Pauli multiplication width exceeds size_t");
    }
    return static_cast<size_t>(work_items);
}

inline uint64_t pauli_multiply_splitmix64(uint64_t input) {
    uint64_t value = input + 0x9e3779b97f4a7c15ULL;
    value = (value ^ (value >> 30)) * 0xbf58476d1ce4e5b9ULL;
    value = (value ^ (value >> 27)) * 0x94d049bb133111ebULL;
    return value ^ (value >> 31);
}

class PauliMultiplyDigest {
   public:
    void update_u64(uint64_t word) {
        for (uint32_t word_byte = 0; word_byte < 8; ++word_byte) {
            const uint8_t byte = static_cast<uint8_t>(word >> (word_byte * 8));
            const uint64_t value = byte + byte_index_ * 0x9e3779b97f4a7c15ULL;
            for (uint32_t lane = 0; lane < state_.size(); ++lane) {
                state_[lane] ^= std::rotl(value, static_cast<int>(lane * 13));
                state_[lane] = std::rotl(
                    state_[lane] *
                        (0x100000001b3ULL + static_cast<uint64_t>(lane) * 2),
                    static_cast<int>(9 + lane));
            }
            ++byte_index_;
        }
    }

    std::array<uint64_t, 4> finish() const {
        return state_;
    }

   private:
    std::array<uint64_t, 4> state_{
        0x6a09e667f3bcc908ULL,
        0xbb67ae8584caa73bULL,
        0x3c6ef372fe94f82bULL,
        0xa54ff53a5f1d36f1ULL,
    };
    uint64_t byte_index_ = 0;
};

using PauliMultiplyString = stim::PauliString<stim::MAX_BITWORD_WIDTH>;

inline uint64_t pauli_multiply_tail_mask(uint64_t width) {
    const uint64_t tail = width % 64;
    return tail == 0 ? std::numeric_limits<uint64_t>::max()
                     : (uint64_t{1} << tail) - 1;
}

inline void pauli_multiply_update_plane(
    PauliMultiplyDigest &digest,
    const stim::simd_bits<stim::MAX_BITWORD_WIDTH> &plane,
    uint64_t width) {
    const size_t word_count = static_cast<size_t>((width + 63) / 64);
    for (size_t word = 0; word < word_count; ++word) {
        uint64_t value = plane.u64[word];
        if (word + 1 == word_count) {
            value &= pauli_multiply_tail_mask(width);
        }
        digest.update_u64(value);
    }
}

inline std::array<uint64_t, 4> pauli_multiply_planes_digest(
    const PauliMultiplyString &pauli,
    uint64_t width) {
    PauliMultiplyDigest digest;
    pauli_multiply_update_plane(digest, pauli.xs, width);
    pauli_multiply_update_plane(digest, pauli.zs, width);
    return digest.finish();
}

inline std::array<uint64_t, 4> pauli_multiply_input_digest(
    const PauliMultiplyString &left,
    const PauliMultiplyString &right,
    uint64_t width) {
    PauliMultiplyDigest digest;
    digest.update_u64(width);
    digest.update_u64(PAULI_MULTIPLY_WORKLOAD_MARKER);
    digest.update_u64(0);
    digest.update_u64(1);
    pauli_multiply_update_plane(digest, left.xs, width);
    pauli_multiply_update_plane(digest, left.zs, width);
    pauli_multiply_update_plane(digest, right.xs, width);
    pauli_multiply_update_plane(digest, right.zs, width);
    return digest.finish();
}

inline std::array<uint64_t, 4> pauli_multiply_digest_words(
    const uint64_t *words,
    size_t word_count) {
    PauliMultiplyDigest digest;
    for (size_t index = 0; index < word_count; ++index) {
        digest.update_u64(words[index]);
    }
    return digest.finish();
}

template <typename T>
inline T &pauli_multiply_optimizer_opaque_mutable(T &value) {
    T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

template <typename T>
inline const T &pauli_multiply_optimizer_opaque_const(const T &value) {
    const T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

struct PauliMultiplyFixture {
    uint64_t width;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
    std::array<uint64_t, 4> initial_left_digest;
    std::array<uint64_t, 4> initial_right_digest;
    PauliMultiplyString left;
    PauliMultiplyString right;
    uint64_t phase_checksum;
};

inline PauliMultiplyFixture pauli_multiply_fixture(uint64_t work_items) {
    const size_t width = checked_pauli_multiply_width(work_items);
    PauliMultiplyString left(width);
    PauliMultiplyString right(width);
    left.sign = false;
    right.sign = true;
    constexpr uint64_t LEFT_SEED = 0x243f6a8885a308d3ULL;
    constexpr uint64_t LEFT_STRIDE = 0x9e3779b97f4a7c15ULL;
    constexpr uint64_t RIGHT_SEED = 0x13198a2e03707344ULL;
    constexpr uint64_t RIGHT_STRIDE = 0xbf58476d1ce4e5b9ULL;
    for (uint64_t qubit = 0; qubit < work_items; ++qubit) {
        const uint64_t left_basis =
            pauli_multiply_splitmix64(LEFT_SEED + qubit * LEFT_STRIDE) & 3;
        const uint64_t right_basis =
            pauli_multiply_splitmix64(RIGHT_SEED + qubit * RIGHT_STRIDE) & 3;
        left.xs[static_cast<size_t>(qubit)] = (left_basis & 1) != 0;
        left.zs[static_cast<size_t>(qubit)] = (left_basis & 2) != 0;
        right.xs[static_cast<size_t>(qubit)] = (right_basis & 1) != 0;
        right.zs[static_cast<size_t>(qubit)] = (right_basis & 2) != 0;
    }
    const auto initial_left_digest = pauli_multiply_planes_digest(left, work_items);
    const auto initial_right_digest = pauli_multiply_planes_digest(right, work_items);
    const auto input_digest = pauli_multiply_input_digest(left, right, work_items);
    for (size_t warmup = 0; warmup < 2; ++warmup) {
        pauli_multiply_optimizer_opaque_mutable(left)
            .ref()
            .inplace_right_mul_returning_log_i_scalar(
                pauli_multiply_optimizer_opaque_const(right).ref());
    }
    if (pauli_multiply_planes_digest(left, work_items) != initial_left_digest) {
        throw std::runtime_error(
            "Pauli multiplication warmup did not restore the canonical left operand");
    }
    if (pauli_multiply_planes_digest(right, work_items) != initial_right_digest) {
        throw std::runtime_error("Pauli multiplication modified its right operand");
    }
    const uint64_t word_count = (work_items + 63) / 64;
    return PauliMultiplyFixture{
        work_items,
        32 + word_count * 32,
        input_digest,
        initial_left_digest,
        initial_right_digest,
        std::move(left),
        std::move(right),
        0,
    };
}

inline void pauli_multiply_contract(uint64_t iterations, PauliMultiplyFixture &fixture) {
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        const uint8_t phase = pauli_multiply_optimizer_opaque_mutable(fixture.left)
                                  .ref()
                                  .inplace_right_mul_returning_log_i_scalar(
                                      pauli_multiply_optimizer_opaque_const(fixture.right).ref());
        fixture.phase_checksum += phase;
    }
    pauli_multiply_optimizer_opaque_const(fixture.left);
}

inline std::array<uint64_t, 4> pauli_multiply_output_digest(
    const PauliMultiplyFixture &fixture,
    uint64_t iterations,
    uint64_t semantic_work) {
    const auto final_left_digest =
        pauli_multiply_planes_digest(fixture.left, fixture.width);
    const auto final_right_digest =
        pauli_multiply_planes_digest(fixture.right, fixture.width);
    if (final_right_digest != fixture.initial_right_digest) {
        throw std::runtime_error("Pauli multiplication modified its right operand");
    }
    const std::array<uint64_t, 17> fields{
        iterations,
        semantic_work,
        fixture.width,
        PAULI_MULTIPLY_WORKLOAD_MARKER,
        fixture.phase_checksum,
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        final_left_digest[0],
        final_left_digest[1],
        final_left_digest[2],
        final_left_digest[3],
        final_right_digest[0],
        final_right_digest[1],
        final_right_digest[2],
        final_right_digest[3],
    };
    return pauli_multiply_digest_words(fields.data(), fields.size());
}

}  // namespace stab_qualification
