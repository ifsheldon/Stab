use std::collections::BTreeSet;
use std::ffi::OsString;

use stab_analysis::{
    ErrorAnalyzerOptions, circuit_flow_generators, circuit_inverse_unitary,
    circuit_to_detector_error_model, decomposed_circuit, gate_has_h_s_cx_m_r_decomposition,
};
use stab_model::{Circuit, Gate, GateArgumentRule, GateCategory, GateTargetRule, Probability};

use super::support::PinnedStimProgram;

const MAX_CASES: usize = 256;
const MAX_CASE_BYTES: usize = 64 * 1024;
const MAX_MATRIX_BYTES: usize = 4 * 1024 * 1024;
const ANALYZER_QUBITS: &str = "0 1 2 3 4 5 6 7";

const PINNED_STIM_CATALOG_HELPER: &[u8] = br#"
#include <cstdint>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>

#include "stim/circuit/circuit.h"
#include "stim/simulators/error_analyzer.h"
#include "stim/util_top/circuit_flow_generators.h"
#include "stim/util_top/simplified_circuit.h"

namespace {

constexpr uint64_t MAX_CASES = 256;
constexpr uint64_t MAX_CASE_BYTES = 64ULL << 10;
constexpr uint64_t MAX_MATRIX_BYTES = 4ULL << 20;

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

std::string read_blob(uint64_t &total) {
    uint64_t size = read_u64_line("case size", MAX_CASE_BYTES);
    if (size > MAX_MATRIX_BYTES - total ||
        size > static_cast<uint64_t>(std::numeric_limits<std::streamsize>::max())) {
        throw std::invalid_argument("catalog matrix input is too large");
    }
    total += size;
    std::string result(static_cast<size_t>(size), '\0');
    std::cin.read(result.data(), static_cast<std::streamsize>(size));
    if (std::cin.gcount() != static_cast<std::streamsize>(size) || std::cin.get() != '\n') {
        throw std::invalid_argument("truncated catalog matrix case");
    }
    return result;
}

void write_result(const char *outcome, const std::string &payload) {
    std::cout << outcome << '\n' << payload.size() << '\n';
    std::cout.write(payload.data(), static_cast<std::streamsize>(payload.size()));
    std::cout << '\n';
}

std::string evaluate(const std::string &behavior, const std::string &source) {
    stim::Circuit circuit(source);
    if (behavior == "dem") {
        return stim::ErrorAnalyzer::circuit_to_detector_error_model(
                   circuit,
                   false,
                   false,
                   true,
                   1,
                   false,
                   false)
            .str();
    }
    if (behavior == "decompose-flows") {
        circuit = stim::simplified_circuit(circuit);
        for (const auto &operation : circuit.operations) {
            switch (operation.gate_type) {
                case stim::GateType::H:
                case stim::GateType::S:
                case stim::GateType::CX:
                case stim::GateType::M:
                case stim::GateType::R:
                    break;
                default:
                    throw std::logic_error(
                        "decomposition escaped the H/S/CX/M/R base gate set");
            }
        }
    }
    if (behavior == "inverse") {
        return circuit.inverse(false).str();
    }
    if (behavior == "decompose-flows") {
        std::ostringstream out;
        for (const auto &flow : stim::circuit_flow_generators<64>(circuit)) {
            out << flow << '\n';
        }
        std::string result = out.str();
        while (!result.empty() && result.back() == '\n') {
            result.pop_back();
        }
        return result;
    }
    throw std::invalid_argument("unknown catalog behavior");
}

}  // namespace

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "expected one catalog behavior\n";
        return 2;
    }
    try {
        uint64_t count = read_u64_line("case count", MAX_CASES);
        uint64_t total = 0;
        for (uint64_t case_index = 0; case_index < count; case_index++) {
            std::string source = read_blob(total);
            try {
                write_result("canonical", evaluate(argv[1], source));
            } catch (const std::invalid_argument &ex) {
                write_result("invalid-argument", ex.what());
            } catch (const std::out_of_range &ex) {
                write_result("out-of-range", ex.what());
            } catch (const std::logic_error &ex) {
                write_result("logic-error", ex.what());
            } catch (const std::runtime_error &ex) {
                write_result("runtime-error", ex.what());
            } catch (const std::exception &ex) {
                write_result("exception", ex.what());
            }
        }
        return 0;
    } catch (const std::exception &ex) {
        std::cerr << ex.what() << '\n';
        return 1;
    }
}
"#;

#[derive(Clone, Debug)]
struct CatalogCase {
    id: String,
    source: String,
}

#[derive(Debug, Eq, PartialEq)]
enum PinnedOutcome {
    Canonical(String),
    Rejected { class: String, detail: String },
}

#[derive(Clone, Debug)]
struct TargetShape {
    id: &'static str,
    targets: &'static str,
    measurement_count: usize,
}

#[test]
#[ignore = "builds and executes a metadata-driven differential against pinned libstim"]
fn p5_catalog_circuit_to_dem_matches_pinned_stim() {
    let cases = Gate::all()
        .flat_map(|gate| {
            analyzer_target_shapes(gate)
                .into_iter()
                .map(move |shape| analyzer_case(gate, shape))
        })
        .collect::<Vec<_>>();
    let gate_count = Gate::all().len();
    assert!(
        cases.len() >= gate_count,
        "every canonical gate must own at least one analyzer case"
    );
    compare_catalog_matrix("dem", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        let options = ErrorAnalyzerOptions {
            allow_gauge_detectors: true,
            approximate_disjoint_errors_threshold: Some(
                Probability::try_new(1.0).expect("unit probability"),
            ),
            ..ErrorAnalyzerOptions::default()
        };
        circuit_to_detector_error_model(&circuit, options)
            .map(|model| without_terminal_newlines(model.to_dem_string()))
            .map_err(|error| error.to_string())
    });
    eprintln!(
        "[stab-oracle] circuit-to-DEM catalog evidence covered {gate_count} gates across {} legal target-shape cases",
        cases.len()
    );
}

#[test]
#[ignore = "builds and executes a metadata-driven differential against pinned libstim"]
fn p5_catalog_base_gate_decomposition_matches_pinned_stim() {
    let gates = Gate::all()
        .filter(|gate| gate_has_h_s_cx_m_r_decomposition(*gate))
        .collect::<Vec<_>>();
    let cases = gates
        .iter()
        .copied()
        .map(|gate| {
            Ok(CatalogCase {
                id: gate.canonical_name().to_string(),
                source: representative_quantum_instruction(gate)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .expect("every decomposition-bearing gate has a quantum target shape");
    assert_eq!(cases.len(), gates.len());
    compare_catalog_matrix("decompose-flows", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        let decomposed = decomposed_circuit(&circuit).map_err(|error| error.to_string())?;
        base_gate_flow_text(&decomposed)
    });
    eprintln!(
        "[stab-oracle] base-gate decomposition catalog evidence covered {} decomposition-bearing gates",
        cases.len()
    );
}

#[test]
#[ignore = "builds and executes a metadata-driven differential against pinned libstim"]
fn p5_catalog_strict_unitary_inverse_matches_pinned_stim() {
    let gates = Gate::all()
        .filter(|gate| gate.is_unitary())
        .collect::<Vec<_>>();
    let cases = gates
        .iter()
        .copied()
        .map(|gate| {
            Ok(CatalogCase {
                id: gate.canonical_name().to_string(),
                source: representative_quantum_instruction(gate)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()
        .expect("every unitary gate has a quantum target shape");
    assert_eq!(cases.len(), gates.len());
    compare_catalog_matrix("inverse", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        circuit_inverse_unitary(&circuit)
            .map(|inverse| without_terminal_newlines(inverse.to_stim_string()))
            .map_err(|error| error.to_string())
    });
    eprintln!(
        "[stab-oracle] strict inverse catalog evidence covered {} unitary gates",
        cases.len()
    );
}

fn compare_catalog_matrix(
    behavior: &str,
    cases: &[CatalogCase],
    mut evaluate_stab: impl FnMut(&str) -> Result<String, String>,
) {
    validate_matrix(cases);
    let probe = PinnedStimProgram::compile("catalog-semantics", PINNED_STIM_CATALOG_HELPER);
    let input = encode_matrix(cases);
    let output = probe.run([OsString::from(behavior)], &input);
    assert!(
        output.success(),
        "pinned Stim {behavior} matrix failed: {}",
        output.stderr.render_for_diagnostics()
    );
    let pinned = decode_matrix(&output.stdout.bytes, cases.len());

    let mut failures = Vec::new();
    for (case, pinned) in cases.iter().zip(pinned) {
        match (evaluate_stab(&case.source), pinned) {
            (Ok(stab), PinnedOutcome::Canonical(pinned)) if stab != pinned => {
                failures.push(format!(
                    "{behavior}/{}: canonical output mismatch\nsource:\n{}\nStab:\n{}\npinned Stim:\n{}",
                    case.id, case.source, stab, pinned
                ));
            }
            (Ok(_), PinnedOutcome::Canonical(_)) => {}
            (Err(error), PinnedOutcome::Canonical(_)) => failures.push(format!(
                "{behavior}/{}: Stab rejected a legal case: {error}\nsource:\n{}",
                case.id, case.source
            )),
            (stab, PinnedOutcome::Rejected { class, detail }) => failures.push(format!(
                "{behavior}/{}: pinned Stim rejected a generated legal case as {class}: {detail}\nStab outcome: {stab:?}\nsource:\n{}",
                case.id, case.source
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} catalog semantic mismatch(es):\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn validate_matrix(cases: &[CatalogCase]) {
    assert!(!cases.is_empty(), "catalog matrix must not be empty");
    assert!(cases.len() <= MAX_CASES, "catalog matrix exceeds case cap");
    let mut ids = BTreeSet::new();
    let mut total = 0_usize;
    for case in cases {
        assert!(
            ids.insert(&case.id),
            "duplicate catalog case id {}",
            case.id
        );
        assert!(
            case.source.len() <= MAX_CASE_BYTES,
            "catalog case {} exceeds byte cap",
            case.id
        );
        total = total
            .checked_add(case.source.len())
            .expect("catalog matrix byte count");
    }
    assert!(total <= MAX_MATRIX_BYTES, "catalog matrix exceeds byte cap");
}

fn encode_matrix(cases: &[CatalogCase]) -> Vec<u8> {
    let mut encoded = format!("{}\n", cases.len()).into_bytes();
    for case in cases {
        encoded.extend_from_slice(format!("{}\n", case.source.len()).as_bytes());
        encoded.extend_from_slice(case.source.as_bytes());
        encoded.push(b'\n');
    }
    encoded
}

fn decode_matrix(encoded: &[u8], expected_count: usize) -> Vec<PinnedOutcome> {
    let mut cursor = 0_usize;
    let mut outcomes = Vec::with_capacity(expected_count);
    for _ in 0..expected_count {
        let class = read_frame_line(encoded, &mut cursor);
        let size = read_frame_line(encoded, &mut cursor)
            .parse::<usize>()
            .expect("pinned Stim result size");
        let end = cursor.checked_add(size).expect("pinned Stim frame size");
        let payload = encoded
            .get(cursor..end)
            .expect("complete pinned Stim result frame");
        cursor = end;
        assert_eq!(encoded.get(cursor), Some(&b'\n'), "result frame delimiter");
        cursor += 1;
        let detail = std::str::from_utf8(payload)
            .expect("pinned Stim result UTF-8")
            .to_string();
        outcomes.push(if class == "canonical" {
            PinnedOutcome::Canonical(detail)
        } else {
            PinnedOutcome::Rejected {
                class: class.to_string(),
                detail,
            }
        });
    }
    assert_eq!(cursor, encoded.len(), "unexpected trailing helper output");
    outcomes
}

fn read_frame_line<'a>(encoded: &'a [u8], cursor: &mut usize) -> &'a str {
    let suffix = encoded.get(*cursor..).expect("frame cursor");
    let line_end = suffix
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("newline-terminated helper frame");
    let line = std::str::from_utf8(suffix.get(..line_end).expect("complete helper frame line"))
        .expect("helper frame UTF-8");
    *cursor += line_end + 1;
    line
}

fn analyzer_case(gate: Gate, shape: TargetShape) -> CatalogCase {
    let mut source = format!("R {ANALYZER_QUBITS}\nM 7\n");
    if gate.target_rule() == GateTargetRule::RecOrPauli && shape.id == "pauli-set" {
        source.push_str("RX 0\nRY 1\n");
    }
    source.push_str(&format!("X_ERROR(0.125) {ANALYZER_QUBITS}\n"));
    if gate.category() == GateCategory::ControlFlow {
        source.push_str("REPEAT 2 {\n    X_ERROR(0.125) 0 2\n}\n");
    } else {
        if gate.canonical_name() == "ELSE_CORRELATED_ERROR" {
            source.push_str("E(0.125) X7\n");
        }
        source.push_str(&instruction_text(gate, &shape, true));
        if gate.produces_measurements() {
            append_recent_result_detectors(&mut source, shape.measurement_count);
        }
    }
    source.push_str(&format!("M {ANALYZER_QUBITS}\n"));
    append_recent_result_detectors(&mut source, 8);
    CatalogCase {
        id: format!("{}/{}", gate.canonical_name(), shape.id),
        source,
    }
}

fn append_recent_result_detectors(source: &mut String, count: usize) {
    for offset in (1..=count).rev() {
        source.push_str(&format!("DETECTOR rec[-{offset}]\n"));
    }
}

fn analyzer_target_shapes(gate: Gate) -> Vec<TargetShape> {
    match gate.target_rule() {
        GateTargetRule::None => vec![shape("none", "", 0)],
        GateTargetRule::AnySingleQubit => vec![shape("empty", "", 0), shape("qubits", "0 2 5", 0)],
        GateTargetRule::MeasurementQubits => vec![
            shape("empty", "", 0),
            shape("plain-qubits", "0 2", 2),
            shape("inverted-qubits", "!0 2", 2),
        ],
        GateTargetRule::MeasurementPads => {
            vec![shape("empty", "", 0), shape("pad-values", "0 1 0", 3)]
        }
        GateTargetRule::PlainPairs => {
            vec![shape("empty", "", 0), shape("qubit-pairs", "0 1 2 3", 0)]
        }
        GateTargetRule::ClassicalControlPairs => classical_control_shapes(gate),
        GateTargetRule::MeasurementPairs => vec![
            shape("empty", "", 0),
            shape("plain-qubit-pairs", "0 1 2 3", 2),
            shape("inverted-qubit-pairs", "!0 1 2 !3", 2),
        ],
        GateTargetRule::RecOnly => vec![
            shape("empty-record-set", "", 0),
            shape("record-set", "rec[-1]", 0),
        ],
        GateTargetRule::RecOrPauli => vec![
            shape("empty-set", "", 0),
            shape("record-set", "rec[-1]", 0),
            shape("pauli-set", "X0 Y1 Z2", 0),
        ],
        GateTargetRule::QubitCoords => vec![shape("empty", "", 0), shape("qubits", "0 2 5", 0)],
        GateTargetRule::PauliProducts => vec![
            shape("empty", "", 0),
            shape("single-pauli", "X0", 1),
            shape("one-product", "!X0*Y1*Z2", 1),
            shape("multiple-products", "X0*Y1 Z2*!X3", 2),
        ],
        GateTargetRule::PauliList => vec![
            shape("empty", "", 0),
            shape("single-pauli", "X0", 0),
            shape("pauli-list", "X0 !Y1 Z2", 0),
        ],
    }
}

fn classical_control_shapes(gate: Gate) -> Vec<TargetShape> {
    let mut shapes = vec![shape("empty", "", 0), shape("qubit-pairs", "0 1 2 3", 0)];
    if gate.is_symmetric_gate() {
        shapes.extend([
            shape("record-first", "rec[-1] 1", 0),
            shape("record-second", "1 rec[-1]", 0),
            shape("sweep-first", "sweep[0] 1", 0),
            shape("sweep-second", "1 sweep[0]", 0),
        ]);
    } else if gate.canonical_name().ends_with('Z') {
        shapes.extend([
            shape("record-second", "1 rec[-1]", 0),
            shape("sweep-second", "1 sweep[0]", 0),
        ]);
    } else {
        shapes.extend([
            shape("record-first", "rec[-1] 1", 0),
            shape("sweep-first", "sweep[0] 1", 0),
        ]);
    }
    shapes
}

const fn shape(id: &'static str, targets: &'static str, measurement_count: usize) -> TargetShape {
    TargetShape {
        id,
        targets,
        measurement_count,
    }
}

fn representative_quantum_instruction(gate: Gate) -> Result<String, String> {
    let shape = match gate.target_rule() {
        GateTargetRule::AnySingleQubit => shape("qubit", "2", 0),
        GateTargetRule::MeasurementQubits => shape("qubit", "2", 1),
        GateTargetRule::PlainPairs
        | GateTargetRule::ClassicalControlPairs
        | GateTargetRule::MeasurementPairs => shape("qubit-pair", "2 5", 0),
        GateTargetRule::PauliProducts => shape("pauli-product", "X2*Y5", 0),
        other => {
            return Err(format!(
                "catalog gate {} declares unsupported decomposition/inverse target rule {other:?}",
                gate.canonical_name()
            ));
        }
    };
    Ok(instruction_text(gate, &shape, false))
}

fn instruction_text(gate: Gate, shape: &TargetShape, include_optional_arguments: bool) -> String {
    let arguments = representative_arguments(gate.argument_rule(), include_optional_arguments);
    if shape.targets.is_empty() {
        format!("{}{arguments}\n", gate.canonical_name())
    } else {
        format!("{}{arguments} {}\n", gate.canonical_name(), shape.targets)
    }
}

fn representative_arguments(rule: GateArgumentRule, include_optional: bool) -> String {
    match rule {
        GateArgumentRule::Exact(0) => String::new(),
        GateArgumentRule::Exact(count) => numeric_arguments(count, "0"),
        GateArgumentRule::Any => "(1,2.5)".to_string(),
        GateArgumentRule::OptionalProbability if include_optional => "(0.125)".to_string(),
        GateArgumentRule::OptionalProbability => String::new(),
        GateArgumentRule::ProbabilityList(count) => probability_arguments(count),
        GateArgumentRule::AnyProbabilityList => "(0.125,0.25)".to_string(),
        GateArgumentRule::UnsignedInteger => "(0)".to_string(),
    }
}

fn probability_arguments(count: usize) -> String {
    let mut values = vec!["0"; count];
    if let Some(first) = values.first_mut() {
        *first = "0.125";
    }
    format!("({})", values.join(","))
}

fn numeric_arguments(count: usize, value: &str) -> String {
    format!("({})", vec![value; count].join(","))
}

fn without_terminal_newlines(mut text: String) -> String {
    text.truncate(text.trim_end_matches('\n').len());
    text
}

fn flow_text<T: ToString>(flows: impl IntoIterator<Item = T>) -> String {
    flows
        .into_iter()
        .map(|flow| flow.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn base_gate_flow_text(circuit: &Circuit) -> Result<String, String> {
    for item in circuit.items() {
        let instruction = item
            .as_instruction()
            .ok_or_else(|| "decomposition unexpectedly retained a repeat block".to_string())?;
        if !matches!(
            instruction.gate().canonical_name(),
            "H" | "S" | "CX" | "M" | "R"
        ) {
            return Err(format!(
                "decomposition escaped the H/S/CX/M/R base gate set through {}",
                instruction.gate().canonical_name()
            ));
        }
    }
    circuit_flow_generators(circuit)
        .map(flow_text)
        .map_err(|error| error.to_string())
}
