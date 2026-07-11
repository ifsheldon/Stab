#include <cstdint>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

#include "stim/util_top/circuit_inverse_qec.h"

namespace {

constexpr uint64_t MAX_BLOB_BYTES = 64ULL << 20;
constexpr uint64_t MAX_FLOWS = 4096;

uint64_t read_u64_line(const char *label, uint64_t limit) {
    std::string line;
    if (!std::getline(std::cin, line)) {
        throw std::invalid_argument(std::string("missing ") + label);
    }
    size_t consumed = 0;
    uint64_t value = std::stoull(line, &consumed);
    if (consumed != line.size() || value > limit) {
        throw std::invalid_argument(std::string("invalid ") + label);
    }
    return value;
}

std::string read_blob(const char *label) {
    uint64_t size = read_u64_line(label, MAX_BLOB_BYTES);
    if (size > static_cast<uint64_t>(std::numeric_limits<std::streamsize>::max())) {
        throw std::invalid_argument(std::string(label) + " is too large for this platform");
    }
    std::string result(static_cast<size_t>(size), '\0');
    std::cin.read(result.data(), static_cast<std::streamsize>(size));
    if (std::cin.gcount() != static_cast<std::streamsize>(size) || std::cin.get() != '\n') {
        throw std::invalid_argument(std::string("truncated ") + label);
    }
    return result;
}

}  // namespace

int main() {
    try {
        bool dont_turn_measurements_into_resets = read_u64_line("options", 1) != 0;
        uint64_t flow_count = read_u64_line("flow count", MAX_FLOWS);
        stim::Circuit circuit(read_blob("circuit"));
        std::vector<stim::Flow<64>> flows;
        flows.reserve(static_cast<size_t>(flow_count));
        for (uint64_t k = 0; k < flow_count; k++) {
            flows.push_back(stim::Flow<64>::from_str(read_blob("flow")));
        }

        auto [inverse, inverse_flows] =
            stim::circuit_inverse_qec<64>(circuit, flows, dont_turn_measurements_into_resets);
        std::cout << "circuit:\n";
        std::string inverse_text = inverse.str();
        std::cout << inverse_text;
        if (!inverse_text.empty() && inverse_text.back() != '\n') {
            std::cout << '\n';
        }
        std::cout << "flows:\n";
        for (const auto &flow : inverse_flows) {
            std::cout << flow << '\n';
        }
        return 0;
    } catch (const std::exception &ex) {
        std::cerr << ex.what() << '\n';
        return 1;
    }
}
