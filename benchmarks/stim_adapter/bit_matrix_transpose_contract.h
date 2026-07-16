// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <array>
#include <atomic>
#include <bit>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>

#include "stim/mem/simd_bit_table.h"

namespace stab_qualification {

constexpr uint64_t BIT_MATRIX_TRANSPOSE_MIN_DIMENSION = 256;
constexpr uint64_t BIT_MATRIX_TRANSPOSE_MAX_DIMENSION = 16384;
constexpr uint64_t BIT_MATRIX_TRANSPOSE_DIMENSION_ALIGNMENT = 256;

enum class BitMatrixTransposeKind {
    IN_PLACE,
    ALLOCATING,
};

inline std::optional<BitMatrixTransposeKind> bit_matrix_transpose_kind(
    std::string_view workload) {
    if (workload == "bit-matrix-transpose-in-place") {
        return BitMatrixTransposeKind::IN_PLACE;
    }
    if (workload == "bit-matrix-transpose-allocating") {
        return BitMatrixTransposeKind::ALLOCATING;
    }
    return std::nullopt;
}

inline uint64_t bit_matrix_integer_square_root(uint64_t value) {
    uint64_t remainder = value;
    uint64_t result = 0;
    uint64_t bit = uint64_t{1} << 62;
    while (bit > remainder) {
        bit >>= 2;
    }
    while (bit != 0) {
        if (remainder >= result + bit) {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    return result;
}

inline uint64_t checked_bit_matrix_transpose_dimension(uint64_t work_items) {
    const uint64_t dimension = bit_matrix_integer_square_root(work_items);
    if (dimension == 0 || dimension > std::numeric_limits<uint64_t>::max() / dimension ||
        dimension * dimension != work_items) {
        throw std::invalid_argument(
            "bit-matrix transpose work count " + std::to_string(work_items) +
            " is not a perfect square");
    }
    if (dimension < BIT_MATRIX_TRANSPOSE_MIN_DIMENSION) {
        throw std::invalid_argument(
            "bit-matrix transpose dimension " + std::to_string(dimension) +
            " is below the minimum " +
            std::to_string(BIT_MATRIX_TRANSPOSE_MIN_DIMENSION));
    }
    if (dimension % BIT_MATRIX_TRANSPOSE_DIMENSION_ALIGNMENT != 0) {
        throw std::invalid_argument(
            "bit-matrix transpose dimension " + std::to_string(dimension) +
            " is not a multiple of " +
            std::to_string(BIT_MATRIX_TRANSPOSE_DIMENSION_ALIGNMENT));
    }
    if (dimension > BIT_MATRIX_TRANSPOSE_MAX_DIMENSION) {
        throw std::invalid_argument(
            "bit-matrix transpose dimension " + std::to_string(dimension) +
            " exceeds maximum " + std::to_string(BIT_MATRIX_TRANSPOSE_MAX_DIMENSION));
    }
    if (work_items / 8 > std::numeric_limits<uint64_t>::max() - 16) {
        throw std::overflow_error("bit-matrix transpose canonical byte count overflows u64");
    }
    return dimension;
}

inline uint64_t bit_matrix_transpose_splitmix64(uint64_t input) {
    uint64_t value = input + 0x9e3779b97f4a7c15ULL;
    value = (value ^ (value >> 30)) * 0xbf58476d1ce4e5b9ULL;
    value = (value ^ (value >> 27)) * 0x94d049bb133111ebULL;
    return value ^ (value >> 31);
}

inline uint64_t bit_matrix_transpose_fixture_column(
    uint64_t row,
    uint64_t lane,
    uint64_t dimension) {
    constexpr uint64_t SEED = 0xd1b54a32d192ed03ULL;
    constexpr uint64_t ROW_AFFINE = 0x00000001000001b3ULL;
    constexpr uint64_t LANE_AFFINE = 0x000000009e3779b9ULL;
    if (row > (std::numeric_limits<uint64_t>::max() - lane * LANE_AFFINE) / ROW_AFFINE) {
        throw std::overflow_error("bit-matrix transpose fixture affine calculation overflows u64");
    }
    const uint64_t affine = row * ROW_AFFINE + lane * LANE_AFFINE;
    if (row > (std::numeric_limits<uint64_t>::max() - lane * 31) / 17) {
        throw std::overflow_error("bit-matrix transpose fixture offset calculation overflows u64");
    }
    const uint64_t offset = row * 17 + lane * 31;
    return (bit_matrix_transpose_splitmix64(
                SEED ^ affine ^ std::rotl(dimension, 29)) +
            offset) %
           dimension;
}

class BitMatrixTransposeDigest {
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

using BitMatrixTransposeTable = stim::simd_bit_table<stim::MAX_BITWORD_WIDTH>;

inline std::array<uint64_t, 4> bit_matrix_transpose_matrix_digest(
    const BitMatrixTransposeTable &matrix,
    uint64_t dimension) {
    if (matrix.num_major_bits_padded() != dimension ||
        matrix.num_minor_bits_padded() != dimension) {
        throw std::runtime_error(
            "bit-matrix transpose padded shape differs from the logical fixture");
    }
    BitMatrixTransposeDigest digest;
    digest.update_u64(dimension);
    digest.update_u64(dimension);
    const size_t row_words = static_cast<size_t>(dimension / 64);
    for (size_t row = 0; row < static_cast<size_t>(dimension); ++row) {
        const auto row_bits = matrix[row];
        for (size_t word = 0; word < row_words; ++word) {
            digest.update_u64(row_bits.u64[word]);
        }
    }
    return digest.finish();
}

inline std::array<uint64_t, 4> bit_matrix_transpose_digest_words(
    const uint64_t *words,
    size_t word_count) {
    BitMatrixTransposeDigest digest;
    for (size_t index = 0; index < word_count; ++index) {
        digest.update_u64(words[index]);
    }
    return digest.finish();
}

template <typename T>
inline T &bit_matrix_transpose_optimizer_opaque_mutable(T &value) {
    T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

template <typename T>
inline const T &bit_matrix_transpose_optimizer_opaque_const(const T &value) {
    const T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

struct BitMatrixTransposeFixture {
    BitMatrixTransposeKind kind;
    uint64_t dimension;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
    BitMatrixTransposeTable matrix;
    std::optional<BitMatrixTransposeTable> result;
};

inline BitMatrixTransposeFixture bit_matrix_transpose_fixture(
    BitMatrixTransposeKind kind,
    uint64_t work_items) {
    const uint64_t dimension = checked_bit_matrix_transpose_dimension(work_items);
    if (dimension > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("bit-matrix transpose dimension exceeds size_t");
    }
    BitMatrixTransposeTable matrix(
        static_cast<size_t>(dimension), static_cast<size_t>(dimension));
    for (uint64_t row = 0; row < dimension; ++row) {
        for (uint64_t lane = 0; lane < 8; ++lane) {
            matrix[static_cast<size_t>(row)][bit_matrix_transpose_fixture_column(
                row, lane, dimension)] = true;
        }
    }
    const auto input_digest = bit_matrix_transpose_matrix_digest(matrix, dimension);
    if (kind == BitMatrixTransposeKind::IN_PLACE) {
        matrix.do_square_transpose();
        matrix.do_square_transpose();
        if (bit_matrix_transpose_matrix_digest(matrix, dimension) != input_digest) {
            throw std::runtime_error(
                "bit-matrix-transpose-in-place warmup did not restore the canonical bit-matrix state");
        }
    } else {
        for (size_t warmup = 0; warmup < 2; ++warmup) {
            auto warmed = matrix.transposed();
            bit_matrix_transpose_optimizer_opaque_mutable(warmed);
        }
    }
    return BitMatrixTransposeFixture{
        kind,
        dimension,
        work_items / 8 + 16,
        input_digest,
        std::move(matrix),
        std::nullopt,
    };
}

inline void bit_matrix_transpose_contract(
    uint64_t iterations,
    BitMatrixTransposeFixture &fixture) {
    if (fixture.kind == BitMatrixTransposeKind::IN_PLACE) {
        for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
            std::atomic_signal_fence(std::memory_order_seq_cst);
            bit_matrix_transpose_optimizer_opaque_mutable(fixture.matrix)
                .do_square_transpose();
        }
        return;
    }
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        auto next = bit_matrix_transpose_optimizer_opaque_const(fixture.matrix).transposed();
        bit_matrix_transpose_optimizer_opaque_mutable(next);
        fixture.result.emplace(std::move(next));
    }
}

inline std::array<uint64_t, 4> bit_matrix_transpose_output_digest(
    const BitMatrixTransposeFixture &fixture,
    uint64_t iterations,
    uint64_t work_items) {
    if (fixture.kind == BitMatrixTransposeKind::IN_PLACE) {
        const auto final_digest =
            bit_matrix_transpose_matrix_digest(fixture.matrix, fixture.dimension);
        const std::array<uint64_t, 12> fields{
            iterations,
            work_items,
            fixture.dimension,
            3,
            fixture.input_digest[0],
            fixture.input_digest[1],
            fixture.input_digest[2],
            fixture.input_digest[3],
            final_digest[0],
            final_digest[1],
            final_digest[2],
            final_digest[3],
        };
        return bit_matrix_transpose_digest_words(fields.data(), fields.size());
    }
    if (!fixture.result.has_value()) {
        throw std::runtime_error("allocating bit-matrix transpose produced no retained result");
    }
    const auto result_digest =
        bit_matrix_transpose_matrix_digest(fixture.result.value(), fixture.dimension);
    const auto source_digest =
        bit_matrix_transpose_matrix_digest(fixture.matrix, fixture.dimension);
    if (source_digest != fixture.input_digest) {
        throw std::runtime_error("allocating bit-matrix transpose modified its source matrix");
    }
    const std::array<uint64_t, 16> fields{
        iterations,
        work_items,
        fixture.dimension,
        4,
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        result_digest[0],
        result_digest[1],
        result_digest[2],
        result_digest[3],
        source_digest[0],
        source_digest[1],
        source_digest[2],
        source_digest[3],
    };
    return bit_matrix_transpose_digest_words(fields.data(), fields.size());
}

}  // namespace stab_qualification
