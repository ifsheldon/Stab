// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#pragma once

#include <algorithm>
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

#include "stim/stabilizers/pauli_string_iter.h"

namespace stab_qualification {

constexpr uint64_t PAULI_ITER_RANGE_OUTPUT_CAP = 1000000;
constexpr uint64_t PAULI_ITER_PUBLIC_QUBIT_CAP = 1048576;
constexpr uint64_t PAULI_ITER_SINGLETON_OUTPUT_CAP = PAULI_ITER_PUBLIC_QUBIT_CAP * 3;
constexpr uint64_t PAULI_ITER_RANGE_MARKER = 6;
constexpr uint64_t PAULI_ITER_SINGLETON_MARKER = 7;
constexpr uint64_t PAULI_ITER_X_MASK = 1;
constexpr uint64_t PAULI_ITER_Y_MASK = 2;
constexpr uint64_t PAULI_ITER_Z_MASK = 4;
constexpr uint64_t PAULI_ITER_INPUT_BYTES = 8 * 8;

enum class PauliIterKind {
    RANGE,
    SINGLETON,
};

inline std::optional<PauliIterKind> pauli_iter_kind(std::string_view workload) {
    if (workload == "pauli-string-iter-range") {
        return PauliIterKind::RANGE;
    }
    if (workload == "pauli-string-iter-singleton") {
        return PauliIterKind::SINGLETON;
    }
    return std::nullopt;
}

inline std::string_view pauli_iter_workload(PauliIterKind kind) {
    return kind == PauliIterKind::RANGE ? "pauli-string-iter-range"
                                        : "pauli-string-iter-singleton";
}

inline uint64_t pauli_iter_marker(PauliIterKind kind) {
    return kind == PauliIterKind::RANGE ? PAULI_ITER_RANGE_MARKER
                                        : PAULI_ITER_SINGLETON_MARKER;
}

inline uint64_t checked_pauli_iter_choose(uint64_t n, uint64_t k) {
    k = std::min(k, n - k);
    uint64_t result = 1;
    for (uint64_t index = 1; index <= k; ++index) {
        const uint64_t factor = n - k + index;
        if (result > std::numeric_limits<uint64_t>::max() / factor) {
            throw std::overflow_error("Pauli iterator combinatorial output count overflows u64");
        }
        result = result * factor / index;
    }
    return result;
}

inline uint64_t checked_pauli_iter_range_output_count(uint64_t width) {
    uint64_t outputs = 0;
    for (uint64_t weight = 2; weight <= std::min<uint64_t>(5, width); ++weight) {
        const uint64_t combinations = checked_pauli_iter_choose(width, weight);
        const uint64_t basis_products = uint64_t{1} << weight;
        if (combinations > std::numeric_limits<uint64_t>::max() / basis_products) {
            throw std::overflow_error("Pauli iterator combinatorial output count overflows u64");
        }
        const uint64_t term = combinations * basis_products;
        if (outputs > std::numeric_limits<uint64_t>::max() - term) {
            throw std::overflow_error("Pauli iterator combinatorial output count overflows u64");
        }
        outputs += term;
    }
    return outputs;
}

struct PauliIterSpec {
    PauliIterKind kind;
    uint64_t width;
    uint64_t min_weight;
    uint64_t max_weight;
    uint64_t axis_mask;
    uint64_t outputs_per_iteration;
    uint64_t output_cap;
};

inline PauliIterSpec pauli_iter_spec(PauliIterKind kind, uint64_t work_items) {
    if (kind == PauliIterKind::RANGE) {
        for (uint64_t width = 2; width <= 23; ++width) {
            const uint64_t outputs = checked_pauli_iter_range_output_count(width);
            if (outputs == work_items) {
                if (outputs > PAULI_ITER_RANGE_OUTPUT_CAP) {
                    throw std::invalid_argument(
                        std::string(pauli_iter_workload(kind)) + " output count " +
                        std::to_string(outputs) + " exceeds maximum " +
                        std::to_string(PAULI_ITER_RANGE_OUTPUT_CAP));
                }
                return PauliIterSpec{
                    kind,
                    width,
                    2,
                    5,
                    PAULI_ITER_X_MASK | PAULI_ITER_Z_MASK,
                    outputs,
                    PAULI_ITER_RANGE_OUTPUT_CAP,
                };
            }
        }
        if (work_items > PAULI_ITER_RANGE_OUTPUT_CAP) {
            throw std::invalid_argument(
                std::string(pauli_iter_workload(kind)) + " output count " +
                std::to_string(work_items) + " exceeds maximum " +
                std::to_string(PAULI_ITER_RANGE_OUTPUT_CAP));
        }
        throw std::invalid_argument(
            std::string(pauli_iter_workload(kind)) + " work count " +
            std::to_string(work_items) +
            " is not a complete source-owned iterator traversal");
    }

    if (work_items % 3 != 0 || work_items == 0) {
        throw std::invalid_argument(
            std::string(pauli_iter_workload(kind)) + " work count " +
            std::to_string(work_items) +
            " is not a complete source-owned iterator traversal");
    }
    const uint64_t width = work_items / 3;
    if (width > PAULI_ITER_PUBLIC_QUBIT_CAP) {
        throw std::invalid_argument(
            std::string(pauli_iter_workload(kind)) + " width " +
            std::to_string(width) + " exceeds maximum " +
            std::to_string(PAULI_ITER_PUBLIC_QUBIT_CAP));
    }
    return PauliIterSpec{
        kind,
        width,
        1,
        1,
        PAULI_ITER_X_MASK | PAULI_ITER_Y_MASK | PAULI_ITER_Z_MASK,
        work_items,
        PAULI_ITER_SINGLETON_OUTPUT_CAP,
    };
}

class PauliIterDigest {
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

inline std::array<uint64_t, 4> pauli_iter_digest_words(
    const uint64_t *words,
    size_t word_count) {
    PauliIterDigest digest;
    for (size_t index = 0; index < word_count; ++index) {
        digest.update_u64(words[index]);
    }
    return digest.finish();
}

inline std::array<uint64_t, 4> pauli_iter_input_digest(const PauliIterSpec &spec) {
    const std::array<uint64_t, 8> fields{
        spec.width,
        spec.min_weight,
        spec.max_weight,
        spec.axis_mask,
        spec.outputs_per_iteration,
        pauli_iter_marker(spec.kind),
        spec.output_cap,
        PAULI_ITER_PUBLIC_QUBIT_CAP,
    };
    return pauli_iter_digest_words(fields.data(), fields.size());
}

using PauliIterator = stim::PauliStringIterator<stim::MAX_BITWORD_WIDTH>;

template <typename T>
inline T &pauli_iter_optimizer_opaque(T &value) {
    T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

inline PauliIterator pauli_iter_build(const PauliIterSpec &spec) {
    if (spec.width > std::numeric_limits<size_t>::max() ||
        spec.min_weight > std::numeric_limits<size_t>::max() ||
        spec.max_weight > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("Pauli iterator value cannot be represented as size_t");
    }
    return PauliIterator(
        static_cast<size_t>(spec.width),
        static_cast<size_t>(spec.min_weight),
        static_cast<size_t>(spec.max_weight),
        (spec.axis_mask & PAULI_ITER_X_MASK) != 0,
        (spec.axis_mask & PAULI_ITER_Y_MASK) != 0,
        (spec.axis_mask & PAULI_ITER_Z_MASK) != 0);
}

inline std::pair<uint64_t, uint64_t> pauli_iter_traverse(PauliIterator &iterator) {
    uint64_t outputs = 0;
    uint64_t width_checksum = 0;
    while (pauli_iter_optimizer_opaque(iterator).iter_next()) {
        if (outputs == std::numeric_limits<uint64_t>::max()) {
            throw std::overflow_error("Pauli iterator output count overflows u64");
        }
        ++outputs;
        const uint64_t width = iterator.result.num_qubits;
        if (width_checksum > std::numeric_limits<uint64_t>::max() - width) {
            throw std::overflow_error(
                "Pauli iterator output-count times result-width checksum overflows u64");
        }
        width_checksum += width;
    }
    return {outputs, width_checksum};
}

inline std::array<uint64_t, 4> pauli_iter_result_digest(
    const PauliIterator &iterator,
    uint64_t width) {
    const size_t words = static_cast<size_t>((width + 63) / 64);
    PauliIterDigest digest;
    for (size_t index = 0; index < words; ++index) {
        digest.update_u64(iterator.result.xs.u64[index]);
    }
    for (size_t index = 0; index < words; ++index) {
        digest.update_u64(iterator.result.zs.u64[index]);
    }
    return digest.finish();
}

struct PauliIterValidation {
    uint64_t outputs;
    uint64_t width_checksum;
    std::array<uint64_t, 4> final_result_digest;
};

inline PauliIterValidation pauli_iter_validate(
    PauliIterator &iterator,
    uint64_t expected_outputs,
    uint64_t width) {
    uint64_t outputs = 0;
    uint64_t width_checksum = 0;
    std::optional<std::array<uint64_t, 4>> final_result_digest;
    while (pauli_iter_optimizer_opaque(iterator).iter_next()) {
        if (outputs == std::numeric_limits<uint64_t>::max()) {
            throw std::overflow_error("Pauli iterator output count overflows u64");
        }
        ++outputs;
        const uint64_t result_width = iterator.result.num_qubits;
        if (width_checksum > std::numeric_limits<uint64_t>::max() - result_width) {
            throw std::overflow_error(
                "Pauli iterator output-count times result-width checksum overflows u64");
        }
        width_checksum += result_width;
        if (outputs == expected_outputs) {
            final_result_digest = pauli_iter_result_digest(iterator, width);
        }
    }
    if (!final_result_digest.has_value()) {
        throw std::runtime_error("Pauli iterator validation produced no final result");
    }
    return PauliIterValidation{outputs, width_checksum, final_result_digest.value()};
}

struct PauliIterFixture {
    PauliIterSpec spec;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
    std::array<uint64_t, 4> final_result_digest;
    uint64_t observed_outputs;
    uint64_t observed_width_checksum;
};

inline PauliIterFixture pauli_iter_fixture(
    PauliIterKind kind,
    uint64_t work_items,
    uint64_t semantic_work) {
    const auto spec = pauli_iter_spec(kind, work_items);
    if (spec.width != 0 &&
        semantic_work > std::numeric_limits<uint64_t>::max() / spec.width) {
        throw std::overflow_error(
            "Pauli iterator output-count times result-width checksum overflows u64");
    }
    auto validation = pauli_iter_build(spec);
    const auto validation_summary =
        pauli_iter_validate(validation, spec.outputs_per_iteration, spec.width);
    const uint64_t expected_width_checksum = spec.outputs_per_iteration * spec.width;
    if (validation_summary.outputs != spec.outputs_per_iteration ||
        validation_summary.width_checksum != expected_width_checksum) {
        throw std::runtime_error(
            std::string(pauli_iter_workload(kind)) +
            " validation produced an unexpected traversal summary");
    }
    return PauliIterFixture{
        spec,
        PAULI_ITER_INPUT_BYTES,
        pauli_iter_input_digest(spec),
        validation_summary.final_result_digest,
        0,
        0,
    };
}

inline void pauli_iter_contract(uint64_t iterations, PauliIterFixture &fixture) {
    uint64_t observed_outputs = 0;
    uint64_t observed_width_checksum = 0;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        auto iterator = pauli_iter_build(fixture.spec);
        const auto [outputs, width_checksum] = pauli_iter_traverse(iterator);
        std::atomic_signal_fence(std::memory_order_seq_cst);
        observed_outputs += outputs;
        observed_width_checksum += width_checksum;
    }
    fixture.observed_outputs = observed_outputs;
    fixture.observed_width_checksum = observed_width_checksum;
    pauli_iter_optimizer_opaque(fixture);
}

inline std::array<uint64_t, 4> pauli_iter_output_digest(
    const PauliIterFixture &fixture,
    uint64_t iterations,
    uint64_t semantic_work) {
    const uint64_t expected_width_checksum = semantic_work * fixture.spec.width;
    if (fixture.observed_outputs != semantic_work ||
        fixture.observed_width_checksum != expected_width_checksum) {
        throw std::runtime_error(
            std::string(pauli_iter_workload(fixture.spec.kind)) +
            " timing produced an unexpected traversal summary");
    }
    const std::array<uint64_t, 18> fields{
        iterations,
        semantic_work,
        fixture.spec.width,
        pauli_iter_marker(fixture.spec.kind),
        fixture.spec.min_weight,
        fixture.spec.max_weight,
        fixture.spec.axis_mask,
        fixture.spec.outputs_per_iteration,
        fixture.observed_outputs,
        fixture.observed_width_checksum,
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        fixture.final_result_digest[0],
        fixture.final_result_digest[1],
        fixture.final_result_digest[2],
        fixture.final_result_digest[3],
    };
    return pauli_iter_digest_words(fields.data(), fields.size());
}

}  // namespace stab_qualification
