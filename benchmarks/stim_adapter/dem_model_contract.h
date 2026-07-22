// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#ifndef STAB_BENCHMARKS_STIM_ADAPTER_DEM_MODEL_CONTRACT_H
#define STAB_BENCHMARKS_STIM_ADAPTER_DEM_MODEL_CONTRACT_H

#include <atomic>
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <string_view>

#include "stim/dem/detector_error_model.h"

namespace stab_qualification {

constexpr uint64_t DEM_CYCLE_ITEMS = 8;
constexpr uint64_t DEM_MAX_ITEMS = 524288;
constexpr uint64_t DEM_CYCLE_BYTES = 222;
constexpr std::string_view DEM_CYCLE_TEXT =
    "error(0.125) D0\n"
    "error[edge](0.25) D1 D2 L0 ^ D3\n"
    "detector(0.5, 1) D4\n"
    "logical_observable L1\n"
    "shift_detectors(1.5, 3) 5\n"
    "detector[tagged] D2\n"
    "repeat[loop] 3 {\n"
    "    error(0.375) D0 D1\n"
    "    shift_detectors 2\n"
    "}\n"
    "error(0.0625) D5 ^ L2\n";

inline bool is_dem_model_workload(std::string_view workload) {
    return workload == "dem-parse" || workload == "dem-canonical-print";
}

inline std::string dem_model_fixture(uint64_t top_level_items) {
    if (top_level_items > DEM_MAX_ITEMS) {
        throw std::invalid_argument("DEM model work count exceeds the source-owned limit");
    }
    if (top_level_items == 0 || top_level_items % DEM_CYCLE_ITEMS != 0) {
        throw std::invalid_argument(
            "DEM model work count is not a positive complete fixture cycle");
    }
    const uint64_t cycles = top_level_items / DEM_CYCLE_ITEMS;
    if (cycles > std::numeric_limits<size_t>::max() / DEM_CYCLE_BYTES) {
        throw std::overflow_error("DEM model fixture capacity overflows size_t");
    }
    const size_t capacity = static_cast<size_t>(cycles * DEM_CYCLE_BYTES);
    std::string fixture;
    fixture.reserve(capacity);
    for (uint64_t index = 0; index < cycles; ++index) {
        fixture.append(DEM_CYCLE_TEXT);
    }
    if (fixture.size() != capacity) {
        throw std::runtime_error("DEM model fixture size differs from the source contract");
    }
    return fixture;
}

template <typename T>
inline void dem_optimizer_consume(const T &value) {
#if defined(__GNUC__) || defined(__clang__)
    asm volatile("" : : "g"(&value) : "memory");
#else
    (void)value;
    std::atomic_signal_fence(std::memory_order_seq_cst);
#endif
}

inline stim::DetectorErrorModel dem_model_parse(
    uint64_t iterations,
    const std::string &fixture) {
    stim::DetectorErrorModel parsed;
    for (uint64_t index = 0; index < iterations; ++index) {
        parsed = stim::DetectorErrorModel(fixture);
        dem_optimizer_consume(parsed);
    }
    return parsed;
}

inline std::string dem_model_serialize(
    uint64_t iterations,
    const stim::DetectorErrorModel &model) {
    std::string canonical;
    for (uint64_t index = 0; index < iterations; ++index) {
        canonical = model.str();
        dem_optimizer_consume(canonical);
    }
    return canonical;
}

inline std::string_view normalize_dem_canonical(std::string_view canonical) {
    if (!canonical.empty() && canonical.back() == '\n') {
        canonical.remove_suffix(1);
    }
    return canonical;
}

inline std::string_view validate_dem_canonical(
    std::string_view canonical,
    std::string_view fixture) {
    const auto actual = normalize_dem_canonical(canonical);
    const auto expected = normalize_dem_canonical(fixture);
    if (actual != expected) {
        throw std::runtime_error("DEM canonical output differs from the source-owned fixture");
    }
    return actual;
}

}  // namespace stab_qualification

#endif
