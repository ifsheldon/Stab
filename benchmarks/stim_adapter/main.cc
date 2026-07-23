// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#include <algorithm>
#include <array>
#include <atomic>
#include <bit>
#include <charconv>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <optional>
#include <sched.h>
#include <stdexcept>
#include <string>
#include <string_view>
#include <sstream>
#include <vector>

#include "stim/circuit/circuit.h"
#include "stim/gates/gates.h"
#include "stim/mem/simd_bits.h"

#include "bit_matrix_transpose_contract.h"
#include "clifford_string_contract.h"
#include "dem_model_contract.h"
#include "pauli_string_iter_contract.h"
#include "pauli_string_multiply_contract.h"
#include "simd_bits_not_zero_contract.h"
#include "simd_bits_xor_contract.h"
#include "simd_word_popcount_contract.h"
#include "sparse_xor_contract.h"

#ifndef STAB_STIM_COMMIT
#error "STAB_STIM_COMMIT must identify the pinned Stim source"
#endif
#ifndef STAB_ADAPTER_SOURCE_DIGEST
#error "STAB_ADAPTER_SOURCE_DIGEST must identify the adapter source"
#endif
#ifndef STAB_ADAPTER_BUILD_FINGERPRINT
#error "STAB_ADAPTER_BUILD_FINGERPRINT must identify the adapter build"
#endif

namespace {

struct Arguments {
    std::string workload;
    std::string measurement_id;
    uint64_t iterations = 0;
    uint64_t work_items = 0;
    std::string evidence_mode;
    bool start_barrier = false;
    std::optional<uint32_t> expected_cpu;
    std::optional<std::string> input_descriptor_hex;
};

enum class NotZeroPattern {
    EARLY,
    ZERO,
    LATE,
};

enum class SparseXorKind {
    ROW,
    ITEM,
};

std::optional<NotZeroPattern> not_zero_pattern(std::string_view workload) {
    if (workload == "simd-bits-not-zero-early") {
        return NotZeroPattern::EARLY;
    }
    if (workload == "simd-bits-not-zero-zero") {
        return NotZeroPattern::ZERO;
    }
    if (workload == "simd-bits-not-zero-late") {
        return NotZeroPattern::LATE;
    }
    return std::nullopt;
}

std::optional<SparseXorKind> sparse_xor_kind(std::string_view workload) {
    if (workload == "sparse-xor-row") {
        return SparseXorKind::ROW;
    }
    if (workload == "sparse-xor-item") {
        return SparseXorKind::ITEM;
    }
    return std::nullopt;
}

uint64_t parse_u64(std::string_view text, std::string_view name) {
    uint64_t value = 0;
    const auto result = std::from_chars(text.data(), text.data() + text.size(), value);
    if (result.ec != std::errc{} || result.ptr != text.data() + text.size()) {
        throw std::invalid_argument(std::string(name) + " must be a decimal u64");
    }
    return value;
}

uint64_t parse_positive_u64(std::string_view text, std::string_view name) {
    const uint64_t value = parse_u64(text, name);
    if (value == 0) {
        throw std::invalid_argument(std::string(name) + " must be positive");
    }
    return value;
}

Arguments parse_arguments(int argc, const char **argv) {
    Arguments result;
    for (int index = 1; index < argc; index += 2) {
        if (index + 1 >= argc) {
            throw std::invalid_argument("adapter options require values");
        }
        const std::string_view name(argv[index]);
        const std::string_view value(argv[index + 1]);
        if (name == "--workload") {
            result.workload = value;
        } else if (name == "--measurement-id") {
            result.measurement_id = value;
        } else if (name == "--iterations") {
            result.iterations = parse_positive_u64(value, "iterations");
        } else if (name == "--work-items") {
            result.work_items = parse_positive_u64(value, "work-items");
        } else if (name == "--evidence-mode") {
            result.evidence_mode = value;
        } else if (name == "--start-barrier") {
            if (value != "true" && value != "false") {
                throw std::invalid_argument("start-barrier must be true or false");
            }
            result.start_barrier = value == "true";
        } else if (name == "--expected-cpu") {
            const uint64_t cpu = parse_u64(value, "expected-cpu");
            if (cpu >= CPU_SETSIZE || cpu > std::numeric_limits<uint32_t>::max()) {
                throw std::invalid_argument("expected-cpu exceeds the supported affinity mask");
            }
            result.expected_cpu = static_cast<uint32_t>(cpu);
        } else if (name == "--input-descriptor-hex") {
            result.input_descriptor_hex = std::string(value);
        } else {
            throw std::invalid_argument("unknown adapter option " + std::string(name));
        }
    }
    const bool protocol_smoke = result.workload == "protocol-smoke" && result.measurement_id == "main";
    const bool circuit_parse = result.workload == "circuit-parse" && result.measurement_id == "parse";
    const bool circuit_canonical_print =
        result.workload == "circuit-canonical-print" && result.measurement_id == "serialize";
    const bool dem_parse = result.workload == "dem-parse" && result.measurement_id == "parse";
    const bool dem_canonical_print =
        result.workload == "dem-canonical-print" && result.measurement_id == "serialize";
    const bool gate_name_hash =
        result.workload == "gate-name-hash" && result.measurement_id == "hash-all-names";
    const bool simd_word_popcount =
        result.workload == "simd-word-popcount" && result.measurement_id == "toggle-popcount";
    const bool simd_bits_xor =
        result.workload == "simd-bits-xor" && result.measurement_id == "xor-complete-vector";
    const bool simd_bits_not_zero =
        not_zero_pattern(result.workload).has_value() && result.measurement_id == "not-zero";
    const bool sparse_xor =
        (sparse_xor_kind(result.workload) == SparseXorKind::ROW &&
         result.measurement_id == "row-xor") ||
        (sparse_xor_kind(result.workload) == SparseXorKind::ITEM &&
         result.measurement_id == "xor-item");
    const auto transpose_kind = stab_qualification::bit_matrix_transpose_kind(result.workload);
    const bool bit_matrix_transpose =
        (transpose_kind == stab_qualification::BitMatrixTransposeKind::IN_PLACE &&
         result.measurement_id == "in-place-transpose") ||
        (transpose_kind == stab_qualification::BitMatrixTransposeKind::ALLOCATING &&
         result.measurement_id == "allocating-transpose");
    const bool pauli_string_multiply =
        stab_qualification::is_pauli_string_multiply_workload(result.workload) &&
        result.measurement_id == "right-multiply-in-place";
    const bool pauli_string_iter =
        stab_qualification::pauli_iter_kind(result.workload).has_value() &&
        result.measurement_id == "construct-and-iterate-borrowed";
    const auto clifford_kind = stab_qualification::clifford_workload_kind(result.workload);
    const bool clifford_string =
        clifford_kind.has_value() &&
        result.measurement_id == stab_qualification::clifford_measurement(clifford_kind.value());
    if (!protocol_smoke && !circuit_parse && !circuit_canonical_print && !dem_parse &&
        !dem_canonical_print && !gate_name_hash && !simd_word_popcount && !simd_bits_xor &&
        !simd_bits_not_zero && !sparse_xor && !bit_matrix_transpose &&
        !pauli_string_multiply && !pauli_string_iter && !clifford_string) {
        throw std::invalid_argument("adapter workload and measurement are not a registered pair");
    }
    if (result.iterations == 0 || result.work_items == 0) {
        throw std::invalid_argument("adapter requires --iterations and --work-items");
    }
    if (result.evidence_mode != "contract" && result.evidence_mode != "timing" &&
        result.evidence_mode != "memory") {
        throw std::invalid_argument("evidence-mode must be contract, timing, or memory");
    }
    return result;
}

void wait_for_start_barrier() {
    char value = 0;
    if (!std::cin.get(value) || value != '\n') {
        throw std::runtime_error("start barrier must contain exactly one newline");
    }
    if (std::cin.get(value)) {
        throw std::runtime_error("start barrier contains trailing bytes");
    }
}

void verify_affinity(const std::optional<uint32_t> expected_cpu) {
    if (!expected_cpu.has_value()) {
        return;
    }
    cpu_set_t set;
    CPU_ZERO(&set);
    if (sched_getaffinity(0, sizeof(set), &set) != 0) {
        throw std::runtime_error("failed to read worker CPU affinity");
    }
    for (uint32_t cpu = 0; cpu < CPU_SETSIZE; ++cpu) {
        const bool expected = cpu == expected_cpu.value();
        if (static_cast<bool>(CPU_ISSET(cpu, &set)) != expected) {
            throw std::runtime_error("worker CPU affinity differs from the requested singleton");
        }
    }
}

uint64_t status_kib(std::string_view field) {
    std::ifstream input("/proc/self/status");
    if (!input) {
        throw std::runtime_error("failed to read /proc/self/status");
    }
    std::string line;
    while (std::getline(input, line)) {
        std::istringstream fields(line);
        std::string label;
        if (!(fields >> label) || label != field) {
            continue;
        }
        uint64_t kib = 0;
        std::string unit;
        std::string extra;
        if (!(fields >> kib >> unit) || unit != "kB" || fields >> extra ||
            kib > std::numeric_limits<uint64_t>::max() / 1024) {
            throw std::runtime_error("malformed resident-memory field");
        }
        return kib * 1024;
    }
    throw std::runtime_error("resident-memory field is missing");
}

std::array<uint64_t, 4> protocol_smoke(uint64_t iterations, uint64_t work_items) {
    std::array<uint64_t, 4> state{
        0x243f6a8885a308d3ULL,
        0x13198a2e03707344ULL,
        0xa4093822299f31d0ULL,
        0x082efa98ec4e6c89ULL,
    };
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        for (uint64_t item = 0; item < work_items; ++item) {
            const uint64_t value = item * 0x9e3779b97f4a7c15ULL + std::rotl(iteration, 17);
            for (uint32_t lane = 0; lane < state.size(); ++lane) {
                state[lane] ^= std::rotl(value, static_cast<int>(lane * 11));
                state[lane] = std::rotl(
                    state[lane] * (0x100000001b3ULL + static_cast<uint64_t>(lane) * 2),
                    static_cast<int>(7 + lane));
            }
        }
    }
    return state;
}

constexpr uint64_t MAX_CIRCUIT_PARSE_INSTRUCTIONS = 1000000;
constexpr std::array<std::string_view, 6> CIRCUIT_INSTRUCTION_CYCLE{
    "H 0\n",
    "S 1\n",
    "CX 0 1\n",
    "M 0\n",
    "DETECTOR rec[-1]\n",
    "TICK\n",
};

std::string circuit_parse_fixture(uint64_t work_items) {
    if (work_items > MAX_CIRCUIT_PARSE_INSTRUCTIONS) {
        throw std::invalid_argument("circuit-parse instruction count exceeds the source-owned limit");
    }
    std::string fixture;
    if (work_items > std::numeric_limits<size_t>::max() / 12) {
        throw std::overflow_error("circuit-parse fixture capacity overflows size_t");
    }
    fixture.reserve(static_cast<size_t>(work_items) * 12);
    for (uint64_t index = 0; index < work_items; ++index) {
        fixture.append(CIRCUIT_INSTRUCTION_CYCLE[index % CIRCUIT_INSTRUCTION_CYCLE.size()]);
    }
    return fixture;
}

stim::Circuit circuit_parse(uint64_t iterations, const std::string &fixture) {
    stim::Circuit parsed;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        parsed = stim::Circuit(fixture);
    }
    return parsed;
}

std::string circuit_canonical_print(uint64_t iterations, const stim::Circuit &circuit) {
    std::string canonical;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        canonical = circuit.str();
    }
    return canonical;
}

std::vector<std::string> gate_hash_names() {
    std::vector<std::string> names;
    names.reserve(stim::GATE_DATA.items.size());
    for (const auto &gate : stim::GATE_DATA.items) {
        names.emplace_back(gate.name);
    }
    return names;
}

uint64_t gate_table_digest(const std::vector<std::string> &names) {
    uint64_t digest = 0xcbf29ce484222325ULL;
    for (const auto &name : names) {
        for (const auto byte : name) {
            digest ^= static_cast<uint8_t>(byte);
            digest *= 0x100000001b3ULL;
        }
        digest *= 0x100000001b3ULL;
        const uint16_t hash = stim::gate_name_to_hash(name);
        digest ^= static_cast<uint8_t>(hash);
        digest *= 0x100000001b3ULL;
        digest ^= static_cast<uint8_t>(hash >> 8);
        digest *= 0x100000001b3ULL;
    }
    return digest;
}

std::array<uint64_t, 4> gate_name_hash(
    uint64_t iterations,
    uint64_t work_items,
    uint64_t sweeps,
    const std::vector<std::string> &names,
    uint64_t table_digest) {
    uint64_t checksum = 0;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        for (uint64_t sweep = 0; sweep < sweeps; ++sweep) {
            std::atomic_signal_fence(std::memory_order_seq_cst);
            for (const auto &name : names) {
                checksum += stim::gate_name_to_hash(name);
            }
        }
    }
    return {checksum, iterations, work_items, table_digest};
}

std::array<uint64_t, 4> byte_digest(std::string_view bytes) {
    std::array<uint64_t, 4> state{
        0x6a09e667f3bcc908ULL,
        0xbb67ae8584caa73bULL,
        0x3c6ef372fe94f82bULL,
        0xa54ff53a5f1d36f1ULL,
    };
    for (uint64_t index = 0; index < bytes.size(); ++index) {
        const uint64_t value = static_cast<uint8_t>(bytes[index]) + index * 0x9e3779b97f4a7c15ULL;
        for (uint32_t lane = 0; lane < state.size(); ++lane) {
            state[lane] ^= std::rotl(value, static_cast<int>(lane * 13));
            state[lane] = std::rotl(
                state[lane] * (0x100000001b3ULL + static_cast<uint64_t>(lane) * 2),
                static_cast<int>(9 + lane));
        }
    }
    return state;
}

std::string semantic_digest(const std::array<uint64_t, 4> &state) {
    std::ostringstream output;
    output << std::hex << std::setfill('0');
    for (const auto value : state) {
        output << std::setw(16) << value;
    }
    return output.str();
}

constexpr uint64_t POPCOUNT_ALIGNMENT_BITS = 256;
constexpr uint64_t POPCOUNT_MIN_BITS = 512;
constexpr uint64_t POPCOUNT_MAX_BITS = 268435456;
constexpr size_t POPCOUNT_TOGGLE_BIT = 300;
constexpr uint64_t DENSE_XOR_ALIGNMENT_BITS = 256;
constexpr uint64_t DENSE_XOR_MIN_BITS = 256;
constexpr uint64_t DENSE_XOR_MAX_BITS = 268435456;
constexpr uint64_t NOT_ZERO_MIN_BITS = 64;
constexpr uint64_t NOT_ZERO_MAX_BITS = 268435456;

uint64_t splitmix64_word(uint64_t index) {
    uint64_t value = index + 0x9e3779b97f4a7c15ULL;
    value = (value ^ (value >> 30)) * 0xbf58476d1ce4e5b9ULL;
    value = (value ^ (value >> 27)) * 0x94d049bb133111ebULL;
    return value ^ (value >> 31);
}

void mix_digest_byte(std::array<uint64_t, 4> &state, uint64_t index, uint8_t byte) {
    const uint64_t value = byte + index * 0x9e3779b97f4a7c15ULL;
    for (uint32_t lane = 0; lane < state.size(); ++lane) {
        state[lane] ^= std::rotl(value, static_cast<int>(lane * 13));
        state[lane] = std::rotl(
            state[lane] * (0x100000001b3ULL + static_cast<uint64_t>(lane) * 2),
            static_cast<int>(9 + lane));
    }
}

void mix_digest_words(
    std::array<uint64_t, 4> &state,
    uint64_t &byte_index,
    const uint64_t *words,
    size_t word_count) {
    for (size_t word_index = 0; word_index < word_count; ++word_index) {
        const uint64_t word = words[word_index];
        for (uint32_t word_byte = 0; word_byte < 8; ++word_byte) {
            mix_digest_byte(state, byte_index, static_cast<uint8_t>(word >> (word_byte * 8)));
            ++byte_index;
        }
    }
}

std::array<uint64_t, 4> byte_digest_words(const uint64_t *words, size_t word_count) {
    std::array<uint64_t, 4> state{
        0x6a09e667f3bcc908ULL,
        0xbb67ae8584caa73bULL,
        0x3c6ef372fe94f82bULL,
        0xa54ff53a5f1d36f1ULL,
    };
    uint64_t byte_index = 0;
    mix_digest_words(state, byte_index, words, word_count);
    return state;
}

std::array<uint64_t, 4> byte_digest_word_pair(
    const uint64_t *first,
    const uint64_t *second,
    size_t word_count) {
    std::array<uint64_t, 4> state{
        0x6a09e667f3bcc908ULL,
        0xbb67ae8584caa73bULL,
        0x3c6ef372fe94f82bULL,
        0xa54ff53a5f1d36f1ULL,
    };
    uint64_t byte_index = 0;
    mix_digest_words(state, byte_index, first, word_count);
    mix_digest_words(state, byte_index, second, word_count);
    return state;
}

constexpr uint64_t SPARSE_ROW_BASE_WORK_ITEMS = 1997;
constexpr uint64_t SPARSE_ROW_MAX_WORK_ITEMS = SPARSE_ROW_BASE_WORK_ITEMS * 4096;
constexpr uint64_t SPARSE_ITEM_BASE_WORK_ITEMS = 7;
constexpr uint64_t SPARSE_ITEM_MAX_WORK_ITEMS = SPARSE_ITEM_BASE_WORK_ITEMS * 4096;
constexpr uint64_t SPARSE_ROW_MARKER = 1;
constexpr uint64_t SPARSE_ITEM_MARKER = 2;
constexpr std::array<uint32_t, SPARSE_ITEM_BASE_WORK_ITEMS> SPARSE_ITEM_SEQUENCE{
    2, 5, 9, 5, 3, 6, 10};

std::string_view sparse_xor_workload(SparseXorKind kind) {
    return kind == SparseXorKind::ROW ? "sparse-xor-row" : "sparse-xor-item";
}

uint64_t sparse_xor_base_work_items(SparseXorKind kind) {
    return kind == SparseXorKind::ROW ? SPARSE_ROW_BASE_WORK_ITEMS : SPARSE_ITEM_BASE_WORK_ITEMS;
}

uint64_t sparse_xor_max_work_items(SparseXorKind kind) {
    return kind == SparseXorKind::ROW ? SPARSE_ROW_MAX_WORK_ITEMS : SPARSE_ITEM_MAX_WORK_ITEMS;
}

uint64_t sparse_xor_complete_sweeps(SparseXorKind kind, uint64_t work_items) {
    const uint64_t maximum = sparse_xor_max_work_items(kind);
    if (work_items > maximum) {
        throw std::invalid_argument(
            std::string(sparse_xor_workload(kind)) + " work count " +
            std::to_string(work_items) + " exceeds maximum " + std::to_string(maximum));
    }
    const uint64_t base = sparse_xor_base_work_items(kind);
    if (work_items < base || work_items % base != 0) {
        throw std::invalid_argument(
            std::string(sparse_xor_workload(kind)) + " work count " +
            std::to_string(work_items) + " is not a positive multiple of " +
            std::to_string(base));
    }
    return work_items / base;
}

void append_le_u64(std::vector<uint8_t> &output, uint64_t value) {
    for (uint32_t byte = 0; byte < 8; ++byte) {
        output.push_back(static_cast<uint8_t>(value >> (byte * 8)));
    }
}

void append_le_u32(std::vector<uint8_t> &output, uint32_t value) {
    for (uint32_t byte = 0; byte < 4; ++byte) {
        output.push_back(static_cast<uint8_t>(value >> (byte * 8)));
    }
}

std::vector<uint8_t> canonical_sparse_items(const uint32_t *items, size_t size) {
    if (size > (std::numeric_limits<size_t>::max() - 8) / 4) {
        throw std::overflow_error("sparse XOR item encoding size overflows size_t");
    }
    std::vector<uint8_t> output;
    output.reserve(8 + size * 4);
    append_le_u64(output, static_cast<uint64_t>(size));
    for (size_t index = 0; index < size; ++index) {
        append_le_u32(output, items[index]);
    }
    return output;
}

std::vector<uint8_t> canonical_sparse_table(
    const std::vector<stim::SparseXorVec<uint32_t>> &table) {
    size_t item_count = 0;
    for (const auto &row : table) {
        if (row.sorted_items.size() > std::numeric_limits<size_t>::max() - item_count) {
            throw std::overflow_error("sparse XOR table item count overflows size_t");
        }
        item_count += row.sorted_items.size();
    }
    if (table.size() > (std::numeric_limits<size_t>::max() - 8) / 8) {
        throw std::overflow_error("sparse XOR table header size overflows size_t");
    }
    const size_t header_bytes = 8 + table.size() * 8;
    if (item_count > (std::numeric_limits<size_t>::max() - header_bytes) / 4) {
        throw std::overflow_error("sparse XOR table encoding size overflows size_t");
    }
    std::vector<uint8_t> output;
    output.reserve(header_bytes + item_count * 4);
    append_le_u64(output, static_cast<uint64_t>(table.size()));
    for (const auto &row : table) {
        append_le_u64(output, static_cast<uint64_t>(row.sorted_items.size()));
        for (const auto item : row.sorted_items) {
            append_le_u32(output, item);
        }
    }
    return output;
}

std::array<uint64_t, 4> byte_digest_bytes(const std::vector<uint8_t> &bytes) {
    return byte_digest(std::string_view(
        reinterpret_cast<const char *>(bytes.data()), bytes.size()));
}

struct SparseXorFixture {
    SparseXorKind kind;
    uint64_t sweeps;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
    std::vector<stim::SparseXorVec<uint32_t>> table;
    stim::SparseXorVec<uint32_t> buffer;
};

std::vector<uint8_t> canonical_sparse_state(const SparseXorFixture &fixture) {
    if (fixture.kind == SparseXorKind::ROW) {
        return canonical_sparse_table(fixture.table);
    }
    return canonical_sparse_items(
        fixture.buffer.sorted_items.data(), fixture.buffer.sorted_items.size());
}

SparseXorFixture sparse_xor_fixture(SparseXorKind kind, uint64_t work_items) {
    const uint64_t sweeps = sparse_xor_complete_sweeps(kind, work_items);
    SparseXorFixture fixture{kind, sweeps, 0, {}, {}, {}};
    std::vector<uint8_t> canonical_input;
    std::vector<uint8_t> canonical_initial_state;
    if (kind == SparseXorKind::ROW) {
        fixture.table.resize(1000);
        for (uint32_t row = 0; row < fixture.table.size(); ++row) {
            fixture.table[row].xor_item(row);
            fixture.table[row].xor_item(row + 1);
            fixture.table[row].xor_item(row + 4);
            fixture.table[row].xor_item(row + 8);
            fixture.table[row].xor_item(row + 15);
        }
        canonical_input = canonical_sparse_table(fixture.table);
        canonical_initial_state = canonical_input;
        stab_qualification::sparse_xor_row_callback(fixture.table);
        stab_qualification::sparse_xor_row_callback(fixture.table);
    } else {
        canonical_input = canonical_sparse_items(
            SPARSE_ITEM_SEQUENCE.data(), SPARSE_ITEM_SEQUENCE.size());
        canonical_initial_state = canonical_sparse_state(fixture);
        stab_qualification::sparse_xor_item_callback(fixture.buffer);
        stab_qualification::sparse_xor_item_callback(fixture.buffer);
    }
    if (canonical_sparse_state(fixture) != canonical_initial_state) {
        throw std::runtime_error(
            std::string(sparse_xor_workload(kind)) +
            " capacity priming did not restore the canonical sparse XOR state");
    }
    fixture.input_bytes = static_cast<uint64_t>(canonical_input.size());
    fixture.input_digest = byte_digest_bytes(canonical_input);
    return fixture;
}

std::array<uint64_t, 4> sparse_xor_output_digest(
    const SparseXorFixture &fixture,
    uint64_t iterations,
    uint64_t work_items) {
    const auto final_state_digest = byte_digest_bytes(canonical_sparse_state(fixture));
    const std::array<uint64_t, 12> fields{
        iterations,
        work_items,
        fixture.kind == SparseXorKind::ROW ? SPARSE_ROW_MARKER : SPARSE_ITEM_MARKER,
        sparse_xor_base_work_items(fixture.kind),
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        final_state_digest[0],
        final_state_digest[1],
        final_state_digest[2],
        final_state_digest[3],
    };
    return byte_digest_words(fields.data(), fields.size());
}

struct PopcountFixture {
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> bits;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
};

PopcountFixture popcount_fixture(uint64_t bit_count) {
    if (bit_count < POPCOUNT_MIN_BITS) {
        throw std::invalid_argument("simd-word-popcount bit width is below the source-owned minimum");
    }
    if (bit_count > POPCOUNT_MAX_BITS) {
        throw std::invalid_argument("simd-word-popcount bit width exceeds the source-owned limit");
    }
    if (bit_count % POPCOUNT_ALIGNMENT_BITS != 0) {
        throw std::invalid_argument("simd-word-popcount bit width is not a multiple of 256");
    }
    if (bit_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-word-popcount bit width exceeds size_t");
    }
    const uint64_t word_count = bit_count / 64;
    if (word_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-word-popcount word count exceeds size_t");
    }
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> bits(static_cast<size_t>(bit_count));
    if (bits.num_u64_padded() != word_count) {
        throw std::runtime_error("simd-word-popcount padded width differs from the fixture");
    }
    for (uint64_t index = 0; index < word_count; ++index) {
        bits.u64[index] = splitmix64_word(index);
    }
    const auto input_digest = byte_digest_words(bits.u64, static_cast<size_t>(word_count));
    return PopcountFixture{std::move(bits), word_count * 8, input_digest};
}

std::array<uint64_t, 4> popcount_output_digest(
    uint64_t checksum,
    uint64_t iterations,
    uint64_t work_items,
    const std::array<uint64_t, 4> &input_digest,
    bool final_bit) {
    const std::array<uint64_t, 8> fields{
        checksum,
        iterations,
        work_items,
        input_digest[0],
        input_digest[1],
        input_digest[2],
        input_digest[3],
        static_cast<uint64_t>(final_bit),
    };
    return byte_digest_words(fields.data(), fields.size());
}

struct DenseXorFixture {
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> destination;
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> source;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
};

DenseXorFixture dense_xor_fixture(uint64_t bit_count) {
    if (bit_count < DENSE_XOR_MIN_BITS) {
        throw std::invalid_argument("simd-bits-xor bit width is below the source-owned minimum");
    }
    if (bit_count > DENSE_XOR_MAX_BITS) {
        throw std::invalid_argument("simd-bits-xor bit width exceeds the source-owned limit");
    }
    if (bit_count % DENSE_XOR_ALIGNMENT_BITS != 0) {
        throw std::invalid_argument("simd-bits-xor bit width is not a multiple of 256");
    }
    if (bit_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-bits-xor bit width exceeds size_t");
    }
    const uint64_t word_count = bit_count / 64;
    if (word_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-bits-xor word count exceeds size_t");
    }
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> destination(static_cast<size_t>(bit_count));
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> source(static_cast<size_t>(bit_count));
    if (destination.num_u64_padded() != word_count || source.num_u64_padded() != word_count) {
        throw std::runtime_error("simd-bits-xor padded width differs from the fixture");
    }
    for (uint64_t index = 0; index < word_count; ++index) {
        if (index > (std::numeric_limits<uint64_t>::max() - 1) / 2) {
            throw std::overflow_error("simd-bits-xor fixture index overflows u64");
        }
        destination.u64[index] = splitmix64_word(index * 2);
        source.u64[index] = splitmix64_word(index * 2 + 1);
    }
    const auto input_digest = byte_digest_word_pair(
        destination.u64, source.u64, static_cast<size_t>(word_count));
    return DenseXorFixture{
        std::move(destination), std::move(source), word_count * 16, input_digest};
}

std::array<uint64_t, 4> dense_xor_output_digest(
    const DenseXorFixture &fixture,
    uint64_t iterations,
    uint64_t work_items) {
    const auto destination_digest =
        byte_digest_words(fixture.destination.u64, fixture.destination.num_u64_padded());
    const auto source_digest = byte_digest_words(fixture.source.u64, fixture.source.num_u64_padded());
    const std::array<uint64_t, 14> fields{
        iterations,
        work_items,
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
        destination_digest[0],
        destination_digest[1],
        destination_digest[2],
        destination_digest[3],
        source_digest[0],
        source_digest[1],
        source_digest[2],
        source_digest[3],
    };
    return byte_digest_words(fields.data(), fields.size());
}

struct NotZeroFixture {
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> bits;
    uint64_t input_bytes;
    std::array<uint64_t, 4> input_digest;
    NotZeroPattern pattern;
};

uint64_t not_zero_hit_marker(NotZeroPattern pattern, uint64_t bit_count) {
    switch (pattern) {
        case NotZeroPattern::EARLY:
            return bit_count * 3 / 50;
        case NotZeroPattern::ZERO:
            return std::numeric_limits<uint64_t>::max();
        case NotZeroPattern::LATE:
            return bit_count - 1;
    }
    throw std::invalid_argument("unknown simd-bits-not-zero pattern");
}

NotZeroFixture not_zero_fixture(uint64_t bit_count, NotZeroPattern pattern) {
    if (bit_count < NOT_ZERO_MIN_BITS) {
        throw std::invalid_argument("simd-bits-not-zero bit width is below the source-owned minimum");
    }
    if (bit_count > NOT_ZERO_MAX_BITS) {
        throw std::invalid_argument("simd-bits-not-zero bit width exceeds the source-owned limit");
    }
    if (bit_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-bits-not-zero bit width exceeds size_t");
    }
    const uint64_t word_count = (bit_count + 63) / 64;
    if (word_count > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("simd-bits-not-zero word count exceeds size_t");
    }
    stim::simd_bits<stim::MAX_BITWORD_WIDTH> bits(static_cast<size_t>(bit_count));
    std::fill(bits.u64, bits.u64 + bits.num_u64_padded(), 0);
    if (pattern != NotZeroPattern::ZERO) {
        const uint64_t hit_index = not_zero_hit_marker(pattern, bit_count);
        bits.u64[hit_index / 64] |= uint64_t{1} << (hit_index % 64);
    }
    const auto input_digest = byte_digest_words(bits.u64, static_cast<size_t>(word_count));
    return NotZeroFixture{
        std::move(bits), word_count * 8, input_digest, pattern};
}

std::array<uint64_t, 4> not_zero_output_digest(
    uint64_t checksum,
    uint64_t iterations,
    uint64_t work_items,
    const NotZeroFixture &fixture) {
    const std::array<uint64_t, 8> fields{
        checksum,
        iterations,
        work_items,
        not_zero_hit_marker(fixture.pattern, work_items),
        fixture.input_digest[0],
        fixture.input_digest[1],
        fixture.input_digest[2],
        fixture.input_digest[3],
    };
    return byte_digest_words(fields.data(), fields.size());
}

template <typename CALLBACK>
double measure_workload(CALLBACK callback) {
    const auto started = std::chrono::steady_clock::now();
    callback();
    const auto finished = std::chrono::steady_clock::now();
    return std::chrono::duration<double>(finished - started).count();
}

}  // namespace

int main(int argc, const char **argv) {
    try {
        const Arguments arguments = parse_arguments(argc, argv);
        if (arguments.iterations > std::numeric_limits<uint64_t>::max() / arguments.work_items) {
            throw std::overflow_error("adapter semantic work count overflows u64");
        }
        const uint64_t work_count = arguments.iterations * arguments.work_items;

        const auto prepared_clifford_kind =
            stab_qualification::clifford_workload_kind(arguments.workload);
        std::optional<stab_qualification::CliffordStringFixture> clifford_string;
        if (prepared_clifford_kind.has_value()) {
            if (!arguments.input_descriptor_hex.has_value()) {
                throw std::invalid_argument(
                    "Clifford-string workload requires --input-descriptor-hex");
            }
            const auto descriptor = stab_qualification::parse_clifford_descriptor(
                arguments.input_descriptor_hex.value());
            clifford_string.emplace(stab_qualification::clifford_string_fixture(
                prepared_clifford_kind.value(),
                descriptor,
                arguments.work_items,
                arguments.iterations));
        } else if (arguments.input_descriptor_hex.has_value()) {
            throw std::invalid_argument(
                "--input-descriptor-hex is only valid for Clifford-string workloads");
        }

        // Linking and constructing a pinned Stim type ensures this is an adapter build, not a
        // free-standing synthetic comparator.
        const stim::Circuit linked_stim("H 0\nM 0\n");
        if (linked_stim.count_qubits() != 1) {
            throw std::runtime_error("pinned Stim circuit smoke check failed");
        }
        const bool circuit_workload = arguments.workload == "circuit-parse" ||
                                      arguments.workload == "circuit-canonical-print";
        const std::string circuit_fixture = circuit_workload
                                                ? circuit_parse_fixture(arguments.work_items)
                                                : std::string{};
        const std::optional<stim::Circuit> canonical_print_circuit =
            arguments.workload == "circuit-canonical-print"
                ? std::optional<stim::Circuit>(stim::Circuit(circuit_fixture))
                : std::nullopt;
        const bool dem_model_workload =
            stab_qualification::is_dem_model_workload(arguments.workload);
        const std::string dem_fixture = dem_model_workload
                                            ? stab_qualification::dem_model_fixture(
                                                  arguments.work_items)
                                            : std::string{};
        const std::optional<stim::DetectorErrorModel> dem_print_model =
            arguments.workload == "dem-canonical-print"
                ? std::optional<stim::DetectorErrorModel>(
                      stim::DetectorErrorModel(dem_fixture))
                : std::nullopt;
        const std::optional<std::vector<std::string>> gate_names =
            arguments.workload == "gate-name-hash"
                ? std::optional<std::vector<std::string>>(gate_hash_names())
                : std::nullopt;
        std::optional<uint64_t> gate_sweeps;
        std::optional<uint64_t> gate_digest;
        if (gate_names.has_value()) {
            if (gate_names->empty() || arguments.work_items % gate_names->size() != 0) {
                throw std::invalid_argument(
                    "gate-name-hash work count is not a complete gate-table sweep");
            }
            gate_sweeps = arguments.work_items / gate_names->size();
            gate_digest = gate_table_digest(gate_names.value());
        }
        std::optional<PopcountFixture> popcount;
        if (arguments.workload == "simd-word-popcount") {
            popcount.emplace(popcount_fixture(arguments.work_items));
        }
        std::optional<DenseXorFixture> dense_xor;
        if (arguments.workload == "simd-bits-xor") {
            dense_xor.emplace(dense_xor_fixture(arguments.work_items));
        }
        const auto prepared_not_zero_pattern = not_zero_pattern(arguments.workload);
        std::optional<NotZeroFixture> not_zero;
        if (prepared_not_zero_pattern.has_value()) {
            not_zero.emplace(
                not_zero_fixture(arguments.work_items, prepared_not_zero_pattern.value()));
        }
        const auto prepared_sparse_xor_kind = sparse_xor_kind(arguments.workload);
        std::optional<SparseXorFixture> sparse_xor;
        if (prepared_sparse_xor_kind.has_value()) {
            sparse_xor.emplace(
                sparse_xor_fixture(prepared_sparse_xor_kind.value(), arguments.work_items));
        }
        const auto prepared_transpose_kind =
            stab_qualification::bit_matrix_transpose_kind(arguments.workload);
        std::optional<stab_qualification::BitMatrixTransposeFixture> transpose;
        if (prepared_transpose_kind.has_value()) {
            transpose.emplace(stab_qualification::bit_matrix_transpose_fixture(
                prepared_transpose_kind.value(), arguments.work_items));
        }
        std::optional<stab_qualification::PauliMultiplyFixture> pauli_multiply;
        if (stab_qualification::is_pauli_string_multiply_workload(arguments.workload)) {
            pauli_multiply.emplace(
                stab_qualification::pauli_multiply_fixture(arguments.work_items));
        }
        const auto prepared_pauli_iter_kind =
            stab_qualification::pauli_iter_kind(arguments.workload);
        std::optional<stab_qualification::PauliIterFixture> pauli_iter;
        if (prepared_pauli_iter_kind.has_value()) {
            pauli_iter.emplace(stab_qualification::pauli_iter_fixture(
                prepared_pauli_iter_kind.value(), arguments.work_items, work_count));
        }
        const uint64_t input_bytes = popcount.has_value()
                                         ? popcount->input_bytes
                                     : dense_xor.has_value()
                                         ? dense_xor->input_bytes
                                     : not_zero.has_value()
                                         ? not_zero->input_bytes
                                     : sparse_xor.has_value()
                                         ? sparse_xor->input_bytes
                                     : transpose.has_value()
                                         ? transpose->input_bytes
                                     : pauli_multiply.has_value()
                                         ? pauli_multiply->input_bytes
                                     : pauli_iter.has_value()
                                         ? pauli_iter->input_bytes
                                     : clifford_string.has_value()
                                         ? stab_qualification::CLIFFORD_DESCRIPTOR_BYTES
                                     : dem_model_workload
                                         ? static_cast<uint64_t>(dem_fixture.size())
                                         : static_cast<uint64_t>(circuit_fixture.size());
        const auto input_digest = popcount.has_value()
                                      ? popcount->input_digest
                                  : dense_xor.has_value()
                                      ? dense_xor->input_digest
                                  : not_zero.has_value()
                                      ? not_zero->input_digest
                                  : sparse_xor.has_value()
                                      ? sparse_xor->input_digest
                                  : transpose.has_value()
                                      ? transpose->input_digest
                                  : pauli_multiply.has_value()
                                      ? pauli_multiply->input_digest
                                  : pauli_iter.has_value()
                                      ? pauli_iter->input_digest
                                      : dem_model_workload
                                          ? byte_digest(dem_fixture)
                                          : byte_digest(circuit_fixture);
        const std::string input_digest_text = clifford_string.has_value()
                                                  ? clifford_string->input_digest
                                                  : semantic_digest(input_digest);

        if (clifford_string.has_value()) {
            stab_qualification::clifford_reset_execution_state(clifford_string.value());
        }
        if (arguments.start_barrier) {
            wait_for_start_barrier();
        }
        verify_affinity(arguments.expected_cpu);

        const uint64_t setup_rss = status_kib("VmRSS:");
        std::array<uint64_t, 4> digest_state{};
        stim::Circuit parsed;
        std::string canonical;
        stim::DetectorErrorModel parsed_dem;
        std::string canonical_dem;
        uint64_t popcount_checksum = 0;
        uint64_t not_zero_checksum = 0;
        double elapsed_seconds = 0;
        if (arguments.workload == "protocol-smoke") {
            elapsed_seconds = measure_workload(
                [&]() { digest_state = protocol_smoke(arguments.iterations, arguments.work_items); });
        } else if (arguments.workload == "circuit-parse") {
            elapsed_seconds = measure_workload(
                [&]() { parsed = circuit_parse(arguments.iterations, circuit_fixture); });
        } else if (arguments.workload == "circuit-canonical-print") {
            const auto &prepared_circuit = canonical_print_circuit.value();
            elapsed_seconds = measure_workload([&]() {
                canonical = circuit_canonical_print(arguments.iterations, prepared_circuit);
            });
        } else if (arguments.workload == "dem-parse") {
            elapsed_seconds = measure_workload([&]() {
                parsed_dem = stab_qualification::dem_model_parse(
                    arguments.iterations, dem_fixture);
            });
        } else if (arguments.workload == "dem-canonical-print") {
            const auto &prepared_model = dem_print_model.value();
            elapsed_seconds = measure_workload([&]() {
                canonical_dem = stab_qualification::dem_model_serialize(
                    arguments.iterations, prepared_model);
            });
        } else if (arguments.workload == "gate-name-hash") {
            const uint64_t prepared_sweeps = gate_sweeps.value();
            const auto &prepared_names = gate_names.value();
            const uint64_t prepared_digest = gate_digest.value();
            elapsed_seconds = measure_workload([&]() {
                digest_state = gate_name_hash(
                    arguments.iterations,
                    arguments.work_items,
                    prepared_sweeps,
                    prepared_names,
                    prepared_digest);
            });
        } else if (arguments.workload == "simd-word-popcount") {
            auto &prepared_popcount = popcount.value();
            elapsed_seconds = measure_workload([&]() {
                popcount_checksum = stab_qualification::simd_word_popcount_contract(
                    arguments.iterations, prepared_popcount.bits);
            });
        } else if (arguments.workload == "simd-bits-xor") {
            auto &prepared_xor = dense_xor.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::simd_bits_xor_contract(
                    arguments.iterations, prepared_xor.destination, prepared_xor.source);
            });
        } else if (prepared_not_zero_pattern.has_value()) {
            const auto &prepared_not_zero = not_zero.value();
            elapsed_seconds = measure_workload([&]() {
                not_zero_checksum = stab_qualification::simd_bits_not_zero_contract(
                    arguments.iterations, prepared_not_zero.bits);
            });
        } else if (prepared_sparse_xor_kind == SparseXorKind::ROW) {
            auto &prepared_sparse_xor = sparse_xor.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::sparse_xor_row_contract(
                    arguments.iterations,
                    prepared_sparse_xor.sweeps,
                    prepared_sparse_xor.table);
            });
        } else if (prepared_sparse_xor_kind == SparseXorKind::ITEM) {
            auto &prepared_sparse_xor = sparse_xor.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::sparse_xor_item_contract(
                    arguments.iterations,
                    prepared_sparse_xor.sweeps,
                    prepared_sparse_xor.buffer);
            });
        } else if (prepared_transpose_kind.has_value()) {
            auto &prepared_transpose = transpose.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::bit_matrix_transpose_contract(
                    arguments.iterations, prepared_transpose);
            });
        } else if (pauli_multiply.has_value()) {
            auto &prepared_pauli = pauli_multiply.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::pauli_multiply_contract(
                    arguments.iterations, prepared_pauli);
            });
        } else if (pauli_iter.has_value()) {
            auto &prepared_pauli_iter = pauli_iter.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::pauli_iter_contract(
                    arguments.iterations, prepared_pauli_iter);
            });
        } else if (clifford_string.has_value()) {
            auto &prepared_clifford = clifford_string.value();
            elapsed_seconds = measure_workload([&]() {
                stab_qualification::clifford_string_contract(
                    arguments.iterations, prepared_clifford);
            });
        } else {
            throw std::invalid_argument("unreachable registered adapter workload");
        }
        if (arguments.workload == "circuit-parse") {
            digest_state = byte_digest(parsed.str());
        } else if (arguments.workload == "circuit-canonical-print") {
            digest_state = byte_digest(canonical);
        } else if (arguments.workload == "dem-parse") {
            canonical_dem = parsed_dem.str();
            digest_state = byte_digest(
                stab_qualification::validate_dem_canonical(canonical_dem, dem_fixture));
        } else if (arguments.workload == "dem-canonical-print") {
            digest_state = byte_digest(
                stab_qualification::validate_dem_canonical(canonical_dem, dem_fixture));
        } else if (arguments.workload == "simd-word-popcount") {
            const bool final_bit = popcount->bits[POPCOUNT_TOGGLE_BIT];
            digest_state = popcount_output_digest(
                popcount_checksum,
                arguments.iterations,
                arguments.work_items,
                popcount->input_digest,
                final_bit);
        } else if (arguments.workload == "simd-bits-xor") {
            digest_state = dense_xor_output_digest(
                dense_xor.value(), arguments.iterations, arguments.work_items);
        } else if (prepared_not_zero_pattern.has_value()) {
            digest_state = not_zero_output_digest(
                not_zero_checksum,
                arguments.iterations,
                arguments.work_items,
                not_zero.value());
        } else if (prepared_sparse_xor_kind.has_value()) {
            digest_state = sparse_xor_output_digest(
                sparse_xor.value(), arguments.iterations, arguments.work_items);
        } else if (prepared_transpose_kind.has_value()) {
            digest_state = stab_qualification::bit_matrix_transpose_output_digest(
                transpose.value(), arguments.iterations, arguments.work_items);
        } else if (pauli_multiply.has_value()) {
            digest_state = stab_qualification::pauli_multiply_output_digest(
                pauli_multiply.value(), arguments.iterations, work_count);
        } else if (pauli_iter.has_value()) {
            digest_state = stab_qualification::pauli_iter_output_digest(
                pauli_iter.value(), arguments.iterations, work_count);
        }
        const std::string output_digest_text = clifford_string.has_value()
                                                   ? stab_qualification::clifford_output_digest(
                                                         clifford_string.value(),
                                                         arguments.iterations,
                                                         work_count)
                                                   : semantic_digest(digest_state);
        const bool elapsed_is_valid =
            std::isfinite(elapsed_seconds) &&
            (arguments.evidence_mode == "contract" ? elapsed_seconds >= 0 : elapsed_seconds > 0);
        if (!elapsed_is_valid) {
            throw std::runtime_error("adapter measured an invalid duration for the evidence mode");
        }
        const uint64_t peak_rss = std::max(setup_rss, status_kib("VmHWM:"));

        std::cout << std::setprecision(17)
                  << "{\"schema_version\":4,\"implementation\":\"stim\","
                  << "\"evidence_mode\":\"" << arguments.evidence_mode << "\","
                  << "\"workload_id\":\"" << arguments.workload << "\","
                  << "\"measurement_id\":\"" << arguments.measurement_id << "\","
                  << "\"iteration_count\":" << arguments.iterations << ','
                  << "\"elapsed_seconds\":" << elapsed_seconds << ','
                  << "\"work_count\":" << work_count << ','
                  << "\"input_bytes\":" << input_bytes << ','
                  << "\"input_digest\":\"" << input_digest_text << "\","
                  << "\"output_digest\":\"" << output_digest_text << "\","
                  << "\"setup_rss_bytes\":" << setup_rss << ','
                  << "\"peak_rss_bytes\":" << peak_rss << ','
                  << "\"affinity_cpu\":";
        if (arguments.expected_cpu.has_value()) {
            std::cout << arguments.expected_cpu.value();
        } else {
            std::cout << "null";
        }
        std::cout << ','
                  << "\"stim_commit\":\"" << STAB_STIM_COMMIT << "\","
                  << "\"source_digest\":\"" << STAB_ADAPTER_SOURCE_DIGEST << "\","
                  << "\"build_fingerprint\":\"" << STAB_ADAPTER_BUILD_FINGERPRINT << "\"}\n";
        return 0;
    } catch (const std::exception &error) {
        std::cerr << "stim qualification adapter: " << error.what() << '\n';
        return 2;
    }
}
