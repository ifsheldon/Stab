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
#include <vector>

#include "stim/gates/gates.h"
#include "stim/stabilizers/clifford_string.h"
#include "stim/stabilizers/tableau.h"

namespace stab_qualification {

constexpr uint64_t CLIFFORD_DESCRIPTOR_BYTES = 64;
constexpr uint64_t CLIFFORD_FIXTURE_SCHEMA = 1;
constexpr uint64_t CLIFFORD_GATE_COUNT = 24;
constexpr uint64_t CLIFFORD_NON_IDENTITY_CYCLE = 23;
constexpr uint64_t CLIFFORD_COMPLETE_SPAN = 552;
constexpr uint64_t CLIFFORD_PUBLIC_CAP = 1048576;
constexpr uint64_t CLIFFORD_IDENTITY_MARKER = 3550043079824723011ULL;
constexpr uint64_t CLIFFORD_NON_IDENTITY_MARKER = 3551455952266415171ULL;
constexpr uint64_t CLIFFORD_WITNESS_INCREMENT = 0x9e3779b97f4a7c15ULL;
constexpr std::string_view CLIFFORD_GATE_DIGEST_DOMAIN =
    "stab.clifford-string.gates.v1";

enum class CliffordWorkloadKind {
    IDENTITY,
    NON_IDENTITY,
};

inline std::optional<CliffordWorkloadKind> clifford_workload_kind(
    std::string_view workload) {
    if (workload == "clifford-string-right-multiply-identity") {
        return CliffordWorkloadKind::IDENTITY;
    }
    if (workload == "clifford-string-right-multiply-non-identity") {
        return CliffordWorkloadKind::NON_IDENTITY;
    }
    return std::nullopt;
}

inline std::string_view clifford_measurement(CliffordWorkloadKind kind) {
    return kind == CliffordWorkloadKind::IDENTITY ? "right-multiply-identity"
                                                   : "right-multiply-non-identity";
}

inline uint64_t clifford_marker(CliffordWorkloadKind kind) {
    return kind == CliffordWorkloadKind::IDENTITY ? CLIFFORD_IDENTITY_MARKER
                                                   : CLIFFORD_NON_IDENTITY_MARKER;
}

inline uint64_t clifford_cycle_count(CliffordWorkloadKind kind) {
    return kind == CliffordWorkloadKind::IDENTITY ? 0 : CLIFFORD_NON_IDENTITY_CYCLE;
}

inline uint64_t clifford_complete_span(CliffordWorkloadKind kind) {
    return kind == CliffordWorkloadKind::IDENTITY ? 0 : CLIFFORD_COMPLETE_SPAN;
}

constexpr std::array<stim::GateType, 24> CLIFFORD_GATE_ORDER{
    stim::GateType::I,
    stim::GateType::X,
    stim::GateType::Y,
    stim::GateType::Z,
    stim::GateType::H_XY,
    stim::GateType::S,
    stim::GateType::S_DAG,
    stim::GateType::H_NXY,
    stim::GateType::H,
    stim::GateType::SQRT_Y_DAG,
    stim::GateType::H_NXZ,
    stim::GateType::SQRT_Y,
    stim::GateType::H_YZ,
    stim::GateType::H_NYZ,
    stim::GateType::SQRT_X,
    stim::GateType::SQRT_X_DAG,
    stim::GateType::C_XYZ,
    stim::GateType::C_XYNZ,
    stim::GateType::C_NXYZ,
    stim::GateType::C_XNYZ,
    stim::GateType::C_ZYX,
    stim::GateType::C_ZNYX,
    stim::GateType::C_NZYX,
    stim::GateType::C_ZYNX,
};

class CliffordSha256 {
   public:
    void update(const uint8_t *bytes, size_t size) {
        for (size_t index = 0; index < size; ++index) {
            update_byte(bytes[index]);
        }
    }

    void update(std::string_view bytes) {
        update(reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size());
    }

    void update_u64_le(uint64_t value) {
        for (uint32_t byte = 0; byte < 8; ++byte) {
            update_byte(static_cast<uint8_t>(value >> (byte * 8)));
        }
    }

    std::array<uint8_t, 32> finish() const {
        CliffordSha256 copy = *this;
        if (copy.total_bytes_ > std::numeric_limits<uint64_t>::max() / 8) {
            throw std::overflow_error("Clifford SHA-256 input length overflows u64 bits");
        }
        const uint64_t bit_length = copy.total_bytes_ * 8;
        copy.update_byte(0x80);
        while (copy.buffer_size_ != 56) {
            copy.update_byte(0);
        }
        for (int shift = 56; shift >= 0; shift -= 8) {
            copy.update_byte(static_cast<uint8_t>(bit_length >> shift));
        }
        std::array<uint8_t, 32> output{};
        for (size_t word = 0; word < copy.state_.size(); ++word) {
            for (size_t byte = 0; byte < 4; ++byte) {
                output[word * 4 + byte] = static_cast<uint8_t>(
                    copy.state_[word] >> static_cast<uint32_t>((3 - byte) * 8));
            }
        }
        return output;
    }

   private:
    static constexpr std::array<uint32_t, 64> K{
        0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U, 0x3956c25bU,
        0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U, 0xd807aa98U, 0x12835b01U,
        0x243185beU, 0x550c7dc3U, 0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U,
        0xc19bf174U, 0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU,
        0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU, 0x983e5152U,
        0xa831c66dU, 0xb00327c8U, 0xbf597fc7U, 0xc6e00bf3U, 0xd5a79147U,
        0x06ca6351U, 0x14292967U, 0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU,
        0x53380d13U, 0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
        0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U, 0xd192e819U,
        0xd6990624U, 0xf40e3585U, 0x106aa070U, 0x19a4c116U, 0x1e376c08U,
        0x2748774cU, 0x34b0bcb5U, 0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU,
        0x682e6ff3U, 0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U,
        0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U,
    };

    void update_byte(uint8_t byte) {
        buffer_[buffer_size_++] = byte;
        ++total_bytes_;
        if (buffer_size_ == buffer_.size()) {
            process_block();
            buffer_size_ = 0;
        }
    }

    void process_block() {
        std::array<uint32_t, 64> words{};
        for (size_t index = 0; index < 16; ++index) {
            words[index] =
                static_cast<uint32_t>(buffer_[index * 4]) << 24 |
                static_cast<uint32_t>(buffer_[index * 4 + 1]) << 16 |
                static_cast<uint32_t>(buffer_[index * 4 + 2]) << 8 |
                static_cast<uint32_t>(buffer_[index * 4 + 3]);
        }
        for (size_t index = 16; index < words.size(); ++index) {
            const uint32_t s0 = std::rotr(words[index - 15], 7) ^
                                std::rotr(words[index - 15], 18) ^
                                (words[index - 15] >> 3);
            const uint32_t s1 = std::rotr(words[index - 2], 17) ^
                                std::rotr(words[index - 2], 19) ^
                                (words[index - 2] >> 10);
            words[index] = words[index - 16] + s0 + words[index - 7] + s1;
        }
        uint32_t a = state_[0];
        uint32_t b = state_[1];
        uint32_t c = state_[2];
        uint32_t d = state_[3];
        uint32_t e = state_[4];
        uint32_t f = state_[5];
        uint32_t g = state_[6];
        uint32_t h = state_[7];
        for (size_t index = 0; index < words.size(); ++index) {
            const uint32_t s1 = std::rotr(e, 6) ^ std::rotr(e, 11) ^ std::rotr(e, 25);
            const uint32_t choice = (e & f) ^ (~e & g);
            const uint32_t temp1 = h + s1 + choice + K[index] + words[index];
            const uint32_t s0 = std::rotr(a, 2) ^ std::rotr(a, 13) ^ std::rotr(a, 22);
            const uint32_t majority = (a & b) ^ (a & c) ^ (b & c);
            const uint32_t temp2 = s0 + majority;
            h = g;
            g = f;
            f = e;
            e = d + temp1;
            d = c;
            c = b;
            b = a;
            a = temp1 + temp2;
        }
        state_[0] += a;
        state_[1] += b;
        state_[2] += c;
        state_[3] += d;
        state_[4] += e;
        state_[5] += f;
        state_[6] += g;
        state_[7] += h;
    }

    std::array<uint32_t, 8> state_{
        0x6a09e667U,
        0xbb67ae85U,
        0x3c6ef372U,
        0xa54ff53aU,
        0x510e527fU,
        0x9b05688cU,
        0x1f83d9abU,
        0x5be0cd19U,
    };
    std::array<uint8_t, 64> buffer_{};
    size_t buffer_size_ = 0;
    uint64_t total_bytes_ = 0;
};

inline std::string clifford_hex(const std::array<uint8_t, 32> &bytes) {
    static constexpr char HEX[] = "0123456789abcdef";
    std::string output;
    output.reserve(bytes.size() * 2);
    for (const uint8_t byte : bytes) {
        output.push_back(HEX[byte >> 4]);
        output.push_back(HEX[byte & 15]);
    }
    return output;
}

inline uint8_t clifford_hex_nibble(char value) {
    if (value >= '0' && value <= '9') {
        return static_cast<uint8_t>(value - '0');
    }
    if (value >= 'a' && value <= 'f') {
        return static_cast<uint8_t>(value - 'a' + 10);
    }
    if (value >= 'A' && value <= 'F') {
        return static_cast<uint8_t>(value - 'A' + 10);
    }
    throw std::invalid_argument("Clifford descriptor contains a non-hexadecimal character");
}

struct CliffordDescriptor {
    std::array<uint64_t, 8> fields;
    std::array<uint8_t, 64> bytes;
};

inline CliffordDescriptor parse_clifford_descriptor(std::string_view text) {
    if (text.size() != 128) {
        throw std::invalid_argument(
            "Clifford descriptor must contain exactly 128 hexadecimal characters");
    }
    CliffordDescriptor descriptor{};
    for (size_t index = 0; index < descriptor.bytes.size(); ++index) {
        descriptor.bytes[index] = static_cast<uint8_t>(
            clifford_hex_nibble(text[index * 2]) << 4 |
            clifford_hex_nibble(text[index * 2 + 1]));
    }
    for (size_t field = 0; field < descriptor.fields.size(); ++field) {
        uint64_t value = 0;
        for (size_t byte = 0; byte < 8; ++byte) {
            value |= static_cast<uint64_t>(descriptor.bytes[field * 8 + byte]) << (byte * 8);
        }
        descriptor.fields[field] = value;
    }
    return descriptor;
}

inline void validate_clifford_field(
    std::string_view name,
    uint64_t actual,
    uint64_t expected) {
    if (actual != expected) {
        throw std::invalid_argument(
            "Clifford-string descriptor " + std::string(name) + " is " +
            std::to_string(actual) + ", expected " + std::to_string(expected));
    }
}

inline size_t validate_clifford_descriptor(
    const CliffordDescriptor &descriptor,
    CliffordWorkloadKind requested_kind,
    uint64_t work_items) {
    const uint64_t width = descriptor.fields[0];
    if (width == 0) {
        throw std::invalid_argument("Clifford-string width must be positive");
    }
    if (width > CLIFFORD_PUBLIC_CAP) {
        throw std::invalid_argument(
            "Clifford-string width " + std::to_string(width) + " exceeds maximum " +
            std::to_string(CLIFFORD_PUBLIC_CAP));
    }
    CliffordWorkloadKind actual_kind;
    if (descriptor.fields[1] == CLIFFORD_IDENTITY_MARKER) {
        actual_kind = CliffordWorkloadKind::IDENTITY;
    } else if (descriptor.fields[1] == CLIFFORD_NON_IDENTITY_MARKER) {
        actual_kind = CliffordWorkloadKind::NON_IDENTITY;
    } else {
        throw std::invalid_argument(
            "Clifford-string descriptor has unknown workload marker " +
            std::to_string(descriptor.fields[1]));
    }
    if (actual_kind != requested_kind) {
        throw std::invalid_argument("Clifford-string descriptor workload marker mismatch");
    }
    validate_clifford_field("fixture schema", descriptor.fields[2], CLIFFORD_FIXTURE_SCHEMA);
    validate_clifford_field("canonical gate count", descriptor.fields[3], CLIFFORD_GATE_COUNT);
    validate_clifford_field(
        "right-cycle count", descriptor.fields[4], clifford_cycle_count(requested_kind));
    validate_clifford_field(
        "complete cross-product span",
        descriptor.fields[5],
        clifford_complete_span(requested_kind));
    validate_clifford_field(
        "public Clifford-qubit cap", descriptor.fields[6], CLIFFORD_PUBLIC_CAP);
    validate_clifford_field("reserved field", descriptor.fields[7], 0);
    if (width != work_items) {
        throw std::invalid_argument(
            "Clifford-string descriptor width " + std::to_string(width) +
            " differs from work-items " + std::to_string(work_items));
    }
    if (width > std::numeric_limits<size_t>::max()) {
        throw std::overflow_error("Clifford-string width exceeds size_t");
    }
    return static_cast<size_t>(width);
}

inline uint8_t clifford_gate_code(stim::GateType gate) {
    for (size_t code = 0; code < CLIFFORD_GATE_ORDER.size(); ++code) {
        if (CLIFFORD_GATE_ORDER[code] == gate) {
            return static_cast<uint8_t>(code);
        }
    }
    throw std::runtime_error("Clifford gate is absent from the canonical 24-gate table");
}

inline std::array<std::array<uint8_t, 24>, 24> clifford_scalar_table() {
    std::vector<stim::Tableau<stim::MAX_BITWORD_WIDTH>> tableaus;
    tableaus.reserve(CLIFFORD_GATE_ORDER.size());
    for (const auto gate : CLIFFORD_GATE_ORDER) {
        tableaus.push_back(stim::GATE_DATA[gate].tableau<stim::MAX_BITWORD_WIDTH>());
    }
    std::array<std::array<uint8_t, 24>, 24> products{};
    for (size_t left = 0; left < tableaus.size(); ++left) {
        for (size_t right = 0; right < tableaus.size(); ++right) {
            const auto product = tableaus[right].then(tableaus[left]);
            const auto found = std::find(tableaus.begin(), tableaus.end(), product);
            if (found == tableaus.end()) {
                throw std::runtime_error(
                    "independent Clifford product was absent from the canonical table");
            }
            products[left][right] = static_cast<uint8_t>(found - tableaus.begin());
        }
    }
    return products;
}

inline std::vector<uint8_t> clifford_initial_left_codes(
    CliffordWorkloadKind kind,
    size_t width) {
    std::vector<uint8_t> codes(width);
    if (kind == CliffordWorkloadKind::NON_IDENTITY) {
        for (size_t index = 0; index < width; ++index) {
            codes[index] = static_cast<uint8_t>(index % 24);
        }
    }
    return codes;
}

inline std::vector<uint8_t> clifford_right_codes(
    CliffordWorkloadKind kind,
    size_t width) {
    std::vector<uint8_t> codes(width);
    if (kind == CliffordWorkloadKind::NON_IDENTITY) {
        for (size_t index = 0; index < width; ++index) {
            codes[index] = static_cast<uint8_t>(1 + (index / 24) % 23);
        }
    }
    return codes;
}

struct CliffordScalarExpected {
    std::vector<uint8_t> final_left_codes;
    std::vector<uint8_t> right_codes;
    uint64_t execution_witness;
};

inline CliffordScalarExpected clifford_scalar_expected(
    CliffordWorkloadKind kind,
    size_t width,
    uint64_t iterations) {
    auto left = clifford_initial_left_codes(kind, width);
    auto right = clifford_right_codes(kind, width);
    const auto table = clifford_scalar_table();
    uint64_t callback_count = 0;
    uint64_t witness = 0;
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        for (size_t index = 0; index < width; ++index) {
            left[index] = table[left[index]][right[index]];
        }
        ++callback_count;
        const uint8_t code = left[static_cast<size_t>((callback_count - 1) % width)];
        witness = std::rotl(witness ^ code, 13) + CLIFFORD_WITNESS_INCREMENT + callback_count;
    }
    return CliffordScalarExpected{std::move(left), std::move(right), witness};
}

using CliffordContractString = stim::CliffordString<stim::MAX_BITWORD_WIDTH>;

template <typename T>
inline T &clifford_optimizer_opaque_mutable(T &value) {
    T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

template <typename T>
inline const T &clifford_optimizer_opaque_const(const T &value) {
    const T *pointer = &value;
    asm volatile("" : "+r"(pointer) : : "memory");
    return *pointer;
}

struct CliffordStringFixture {
    CliffordWorkloadKind kind;
    CliffordDescriptor descriptor;
    CliffordContractString left;
    CliffordContractString right;
    CliffordScalarExpected expected;
    std::string input_digest;
    uint64_t callback_count;
    uint64_t execution_witness;
};

inline CliffordStringFixture clifford_string_fixture(
    CliffordWorkloadKind kind,
    const CliffordDescriptor &descriptor,
    uint64_t work_items,
    uint64_t iterations) {
    const size_t width = validate_clifford_descriptor(descriptor, kind, work_items);
    const auto initial_left = clifford_initial_left_codes(kind, width);
    const auto expected = clifford_scalar_expected(kind, width, iterations);
    CliffordContractString left(width);
    CliffordContractString right(width);
    for (size_t index = 0; index < width; ++index) {
        left.set_gate_at(index, CLIFFORD_GATE_ORDER[initial_left[index]]);
        right.set_gate_at(index, CLIFFORD_GATE_ORDER[expected.right_codes[index]]);
    }
    CliffordSha256 input_digest;
    input_digest.update(descriptor.bytes.data(), descriptor.bytes.size());
    return CliffordStringFixture{
        kind,
        descriptor,
        std::move(left),
        std::move(right),
        expected,
        clifford_hex(input_digest.finish()),
        0,
        0,
    };
}

inline void clifford_reset_execution_state(CliffordStringFixture &fixture) {
    fixture.callback_count = 0;
    fixture.execution_witness = 0;
}

inline void clifford_string_contract(uint64_t iterations, CliffordStringFixture &fixture) {
    const uint64_t width = fixture.descriptor.fields[0];
    for (uint64_t iteration = 0; iteration < iterations; ++iteration) {
        std::atomic_signal_fence(std::memory_order_seq_cst);
        clifford_optimizer_opaque_mutable(fixture.left) *=
            clifford_optimizer_opaque_const(fixture.right);
        std::atomic_signal_fence(std::memory_order_seq_cst);
        ++fixture.callback_count;
        const size_t index = static_cast<size_t>((fixture.callback_count - 1) % width);
        const uint8_t code = clifford_gate_code(fixture.left.gate_at(index));
        fixture.execution_witness =
            std::rotl(fixture.execution_witness ^ code, 13) + CLIFFORD_WITNESS_INCREMENT +
            fixture.callback_count;
        clifford_optimizer_opaque_const(code);
        clifford_optimizer_opaque_const(fixture.execution_witness);
    }
    clifford_optimizer_opaque_const(fixture.left);
}

inline std::vector<uint8_t> clifford_codes(const CliffordContractString &value) {
    std::vector<uint8_t> codes(value.num_qubits);
    for (size_t index = 0; index < value.num_qubits; ++index) {
        codes[index] = clifford_gate_code(value.gate_at(index));
    }
    return codes;
}

inline void validate_clifford_codes(
    std::string_view name,
    const std::vector<uint8_t> &actual,
    const std::vector<uint8_t> &expected) {
    if (actual.size() != expected.size()) {
        throw std::runtime_error(
            "Clifford-string " + std::string(name) + " sequence length differs");
    }
    for (size_t index = 0; index < actual.size(); ++index) {
        if (actual[index] != expected[index]) {
            throw std::runtime_error(
                "Clifford-string " + std::string(name) + " sequence differs at index " +
                std::to_string(index));
        }
    }
}

inline std::array<uint64_t, 4> clifford_gate_digest_lanes(
    const std::vector<uint8_t> &codes) {
    CliffordSha256 digest;
    digest.update(CLIFFORD_GATE_DIGEST_DOMAIN);
    const uint8_t zero = 0;
    digest.update(&zero, 1);
    digest.update_u64_le(static_cast<uint64_t>(codes.size()));
    digest.update(codes.data(), codes.size());
    const auto bytes = digest.finish();
    std::array<uint64_t, 4> lanes{};
    for (size_t lane = 0; lane < lanes.size(); ++lane) {
        for (size_t byte = 0; byte < 8; ++byte) {
            lanes[lane] |= static_cast<uint64_t>(bytes[lane * 8 + byte]) << (byte * 8);
        }
    }
    return lanes;
}

inline uint64_t clifford_non_identity_count(const std::vector<uint8_t> &codes) {
    return static_cast<uint64_t>(std::count_if(
        codes.begin(), codes.end(), [](uint8_t code) { return code != 0; }));
}

inline std::array<uint64_t, 16> clifford_output_fields(
    const CliffordStringFixture &fixture,
    uint64_t iterations,
    uint64_t semantic_work) {
    if (fixture.callback_count != iterations) {
        throw std::runtime_error("Clifford-string callback count differs from iterations");
    }
    if (fixture.execution_witness != fixture.expected.execution_witness) {
        throw std::runtime_error("Clifford-string execution witness differs from scalar oracle");
    }
    const auto left = clifford_codes(fixture.left);
    const auto right = clifford_codes(fixture.right);
    validate_clifford_codes("left", left, fixture.expected.final_left_codes);
    validate_clifford_codes("right", right, fixture.expected.right_codes);
    const auto left_digest = clifford_gate_digest_lanes(left);
    const auto right_digest = clifford_gate_digest_lanes(right);
    return {
        iterations,
        semantic_work,
        fixture.descriptor.fields[0],
        clifford_marker(fixture.kind),
        clifford_non_identity_count(left),
        clifford_non_identity_count(right),
        fixture.callback_count,
        fixture.execution_witness,
        left_digest[0],
        left_digest[1],
        left_digest[2],
        left_digest[3],
        right_digest[0],
        right_digest[1],
        right_digest[2],
        right_digest[3],
    };
}

inline std::string clifford_output_digest(
    const CliffordStringFixture &fixture,
    uint64_t iterations,
    uint64_t semantic_work) {
    const auto fields = clifford_output_fields(fixture, iterations, semantic_work);
    CliffordSha256 digest;
    for (const uint64_t field : fields) {
        digest.update_u64_le(field);
    }
    return clifford_hex(digest.finish());
}

}  // namespace stab_qualification
