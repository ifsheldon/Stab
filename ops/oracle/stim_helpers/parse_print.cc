// Prints pinned Stim's canonical text for a circuit or detector error model.
//
// Usage: stab_stim_parse_print circuit|dem < input
// Reads the whole of stdin, parses it with stim::Circuit or
// stim::DetectorErrorModel, and writes its str() followed by a newline when
// the text is non-empty and does not already end with one, the same
// convention the reverse-flow helper uses. Parse failures exit 1 with the
// Stim error on stderr.

#include <cstdint>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>

#include "stim/circuit/circuit.h"
#include "stim/dem/detector_error_model.h"

namespace {

constexpr uint64_t MAX_INPUT_BYTES = 64ULL << 20;

std::string read_stdin() {
    std::string input;
    char buffer[1 << 16];
    while (std::cin.read(buffer, sizeof(buffer)) || std::cin.gcount() > 0) {
        input.append(buffer, static_cast<size_t>(std::cin.gcount()));
        if (input.size() > MAX_INPUT_BYTES) {
            throw std::invalid_argument("input exceeds the helper's byte limit");
        }
    }
    return input;
}

void print_canonical(const std::string &text) {
    std::cout << text;
    if (!text.empty() && text.back() != '\n') {
        std::cout << '\n';
    }
}

}  // namespace

int main(int argc, char **argv) {
    try {
        if (argc != 2) {
            throw std::invalid_argument("usage: stab_stim_parse_print circuit|dem");
        }
        std::string mode = argv[1];
        std::string input = read_stdin();
        if (mode == "circuit") {
            print_canonical(stim::Circuit(input).str());
        } else if (mode == "dem") {
            print_canonical(stim::DetectorErrorModel(input).str());
        } else {
            throw std::invalid_argument("mode must be circuit or dem");
        }
        return 0;
    } catch (const std::exception &ex) {
        std::cerr << ex.what() << '\n';
        return 1;
    }
}
