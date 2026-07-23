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
constexpr uint64_t DEM_FOLDED_MAX_ITEMS = 131072;
constexpr std::string_view DEM_FLAT_ERRORS_CYCLE_TEXT =
    "error(0.125) D0\n"
    "error(0.25) D1 D2\n"
    "error(0.375) D3 L0\n"
    "error(0.0625) D4 ^ D5\n"
    "error(0.5) D6 D7 D8\n"
    "error(0.03125) D9 L1 ^ D10\n"
    "error(0.75) D11 D12 L2\n"
    "error(0.875) D13 ^ D14 L3\n";
constexpr std::string_view DEM_COORDINATE_SPARSE_CYCLE_TEXT =
    "detector[tag-a](0.5, 1) D1000000\n"
    "logical_observable L100000\n"
    "shift_detectors(1.5, -2, 3) 1000001\n"
    "error[edge](0.25) D0 D1000000 L0 ^ D7\n"
    "detector(2, 3.5) D42\n"
    "error(0.125) D999999 L99999\n"
    "shift_detectors 17\n"
    "detector[tag-b] D1000017\n";
constexpr std::string_view DEM_FOLDED_REPEATS_CYCLE_TEXT =
    "repeat[outer] 1000000 {\n"
    "    repeat[inner] 1024 {\n"
    "        error(0.125) D0 D1000000 L100000\n"
    "        shift_detectors 1000001\n"
    "    }\n"
    "}\n";

enum class DemFamily {
    FLAT_ERRORS,
    COORDINATE_SPARSE,
    FOLDED_REPEATS,
};

inline DemFamily dem_family(std::string_view family) {
    if (family == "flat-errors") {
        return DemFamily::FLAT_ERRORS;
    }
    if (family == "coordinate-sparse") {
        return DemFamily::COORDINATE_SPARSE;
    }
    if (family == "folded-repeats") {
        return DemFamily::FOLDED_REPEATS;
    }
    throw std::invalid_argument("unknown DEM model input family");
}

inline std::string_view dem_cycle_text(DemFamily family) {
    switch (family) {
        case DemFamily::FLAT_ERRORS:
            return DEM_FLAT_ERRORS_CYCLE_TEXT;
        case DemFamily::COORDINATE_SPARSE:
            return DEM_COORDINATE_SPARSE_CYCLE_TEXT;
        case DemFamily::FOLDED_REPEATS:
            return DEM_FOLDED_REPEATS_CYCLE_TEXT;
    }
    throw std::invalid_argument("unknown DEM model input family");
}

inline uint64_t dem_cycle_items(DemFamily family) {
    return family == DemFamily::FOLDED_REPEATS ? 1 : DEM_CYCLE_ITEMS;
}

inline uint64_t dem_max_items(DemFamily family) {
    return family == DemFamily::FOLDED_REPEATS ? DEM_FOLDED_MAX_ITEMS : DEM_MAX_ITEMS;
}

inline bool is_dem_model_workload(std::string_view workload) {
    return workload == "dem-parse" || workload == "dem-canonical-print";
}

inline std::string dem_model_fixture(DemFamily family, uint64_t top_level_items) {
    const uint64_t cycle_items = dem_cycle_items(family);
    const uint64_t maximum_items = dem_max_items(family);
    const std::string_view cycle_text = dem_cycle_text(family);
    if (top_level_items > maximum_items) {
        throw std::invalid_argument("DEM model work count exceeds the source-owned limit");
    }
    if (top_level_items == 0 || top_level_items % cycle_items != 0) {
        throw std::invalid_argument(
            "DEM model work count is not a positive complete fixture cycle");
    }
    const uint64_t cycles = top_level_items / cycle_items;
    if (cycles > std::numeric_limits<size_t>::max() / cycle_text.size()) {
        throw std::overflow_error("DEM model fixture capacity overflows size_t");
    }
    const size_t capacity = static_cast<size_t>(cycles * cycle_text.size());
    std::string fixture;
    fixture.reserve(capacity);
    for (uint64_t index = 0; index < cycles; ++index) {
        fixture.append(cycle_text);
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
