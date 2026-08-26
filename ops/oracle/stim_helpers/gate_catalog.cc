#include <algorithm>
#include <cstdint>
#include <iostream>
#include <sstream>
#include <string>
#include <string_view>
#include <vector>

#include "stim/gates/gates.h"

int main() {
    constexpr uint16_t relevant_flags =
        stim::GATE_IS_UNITARY | stim::GATE_IS_NOISY | stim::GATE_ARGS_ARE_DISJOINT_PROBABILITIES |
        stim::GATE_PRODUCES_RESULTS | stim::GATE_IS_NOT_FUSABLE | stim::GATE_IS_BLOCK |
        stim::GATE_TARGETS_PAIRS | stim::GATE_TARGETS_PAULI_STRING |
        stim::GATE_ONLY_TARGETS_MEASUREMENT_RECORD | stim::GATE_CAN_TARGET_BITS |
        stim::GATE_TAKES_NO_TARGETS | stim::GATE_ARGS_ARE_UNSIGNED_INTEGERS |
        stim::GATE_TARGETS_COMBINERS | stim::GATE_IS_RESET | stim::GATE_IS_SINGLE_QUBIT_GATE;
    std::vector<std::string> rows;
    for (size_t gate_index = 1; gate_index < stim::NUM_DEFINED_GATES; gate_index++) {
        const auto &gate = stim::GATE_DATA.items[gate_index];
        const auto &inverse = stim::GATE_DATA[gate.best_candidate_inverse_id];
        std::vector<std::string_view> aliases;
        for (const auto &entry : stim::GATE_DATA.hashed_name_to_gate_type_table) {
            if (!entry.expected_name.empty() && entry.id == gate.id) {
                aliases.push_back(entry.expected_name);
            }
        }
        std::sort(aliases.begin(), aliases.end());

        std::ostringstream row;
        row << gate.name << '\t' << inverse.name << '\t' << gate.category << '\t'
            << static_cast<unsigned>(gate.arg_count) << '\t'
            << static_cast<unsigned>(static_cast<uint16_t>(gate.flags) & relevant_flags) << '\t'
            << (gate.is_symmetric() ? 1 : 0) << '\t';
        for (size_t alias_index = 0; alias_index < aliases.size(); alias_index++) {
            if (alias_index != 0) {
                row << ',';
            }
            row << aliases[alias_index];
        }
        row << '\n';
        rows.push_back(row.str());
    }
    std::sort(rows.begin(), rows.end());
    for (const auto &row : rows) {
        std::cout << row;
    }
}
