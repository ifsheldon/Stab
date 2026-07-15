// Copyright 2026 Stab contributors.
// SPDX-License-Identifier: MIT

#include <algorithm>
#include <array>
#include <bit>
#include <charconv>
#include <chrono>
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

#include "stim/circuit/circuit.h"

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
};

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
        } else {
            throw std::invalid_argument("unknown adapter option " + std::string(name));
        }
    }
    const bool protocol_smoke = result.workload == "protocol-smoke" && result.measurement_id == "main";
    const bool circuit_parse = result.workload == "circuit-parse" && result.measurement_id == "parse";
    const bool circuit_canonical_print =
        result.workload == "circuit-canonical-print" && result.measurement_id == "serialize";
    if (!protocol_smoke && !circuit_parse && !circuit_canonical_print) {
        throw std::invalid_argument("adapter workload and measurement are not a registered pair");
    }
    if (result.iterations == 0 || result.work_items == 0) {
        throw std::invalid_argument("adapter requires --iterations and --work-items");
    }
    if (result.evidence_mode != "timing" && result.evidence_mode != "memory") {
        throw std::invalid_argument("evidence-mode must be timing or memory");
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

}  // namespace

int main(int argc, const char **argv) {
    try {
        const Arguments arguments = parse_arguments(argc, argv);

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
        const auto input_digest = byte_digest(circuit_fixture);
        const std::optional<stim::Circuit> canonical_print_circuit =
            arguments.workload == "circuit-canonical-print"
                ? std::optional<stim::Circuit>(stim::Circuit(circuit_fixture))
                : std::nullopt;

        if (arguments.start_barrier) {
            wait_for_start_barrier();
        }
        verify_affinity(arguments.expected_cpu);

        const uint64_t setup_rss = status_kib("VmRSS:");
        if (arguments.iterations > std::numeric_limits<uint64_t>::max() / arguments.work_items) {
            throw std::overflow_error("adapter semantic work count overflows u64");
        }
        const auto started = std::chrono::steady_clock::now();
        std::array<uint64_t, 4> digest_state{};
        stim::Circuit parsed;
        std::string canonical;
        if (arguments.workload == "protocol-smoke") {
            digest_state = protocol_smoke(arguments.iterations, arguments.work_items);
        } else if (arguments.workload == "circuit-parse") {
            parsed = circuit_parse(arguments.iterations, circuit_fixture);
        } else {
            canonical = circuit_canonical_print(arguments.iterations, canonical_print_circuit.value());
        }
        const auto finished = std::chrono::steady_clock::now();
        if (arguments.workload == "circuit-parse") {
            digest_state = byte_digest(parsed.str());
        } else if (arguments.workload == "circuit-canonical-print") {
            digest_state = byte_digest(canonical);
        }
        const std::chrono::duration<double> elapsed = finished - started;
        if (!(elapsed.count() > 0)) {
            throw std::runtime_error("adapter measured a non-positive duration");
        }
        const uint64_t peak_rss = std::max(setup_rss, status_kib("VmHWM:"));

        std::cout << std::setprecision(17)
                  << "{\"schema_version\":3,\"implementation\":\"stim\","
                  << "\"evidence_mode\":\"" << arguments.evidence_mode << "\","
                  << "\"workload_id\":\"" << arguments.workload << "\","
                  << "\"measurement_id\":\"" << arguments.measurement_id << "\","
                  << "\"iteration_count\":" << arguments.iterations << ','
                  << "\"elapsed_seconds\":" << elapsed.count() << ','
                  << "\"work_count\":" << arguments.iterations * arguments.work_items << ','
                  << "\"input_bytes\":" << circuit_fixture.size() << ','
                  << "\"input_digest\":\"" << semantic_digest(input_digest) << "\","
                  << "\"output_digest\":\"" << semantic_digest(digest_state) << "\","
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
