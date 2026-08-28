use std::collections::BTreeSet;
use std::ffi::OsString;
use std::sync::OnceLock;

use stab_analysis::{
    DetectingRegionTargetOptions, ErrorAnalyzerOptions, MissingDetectorOptions,
    all_detecting_region_targets, all_detecting_region_ticks,
    circuit_detecting_regions_for_targets, circuit_flow_generators, circuit_inverse_unitary,
    circuit_to_detector_error_model, decomposed_circuit, gate_has_h_s_cx_m_r_decomposition,
    gate_has_tableau, missing_detectors,
};
use stab_model::{
    Circuit, DetectorErrorModel, Gate, GateArgumentRule, GateCategory, GateTargetRule, Probability,
};

use super::support::PinnedStimProgram;

const MAX_CASES: usize = 512;
const MAX_CASE_BYTES: usize = 64 * 1024;
const MAX_MATRIX_BYTES: usize = 4 * 1024 * 1024;
const ANALYZER_QUBITS: &str = "0 1 2 3 4 5 6 7";
const REGION_QUBITS: &str = "0 1 2 3 4 5 6 7 8";

const PINNED_STIM_CATALOG_HELPER: &[u8] = br#"
#include <cstdint>
#include <iostream>
#include <limits>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>

#include "stim/circuit/circuit.h"
#include "stim/simulators/error_analyzer.h"
#include "stim/util_top/circuit_to_detecting_regions.h"
#include "stim/util_top/circuit_flow_generators.h"
#include "stim/util_top/missing_detectors.h"
#include "stim/util_top/simplified_circuit.h"

namespace {

constexpr uint64_t MAX_CASES = 512;
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

std::string flow_generators_text(const stim::Circuit &circuit) {
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

std::string detecting_regions_text(const stim::Circuit &circuit) {
    const auto stats = circuit.compute_stats();
    std::set<stim::DemTarget> targets;
    for (uint64_t k = 0; k < stats.num_detectors; k++) {
        targets.insert(stim::DemTarget::relative_detector_id(k));
    }
    for (uint64_t k = 0; k < stats.num_observables; k++) {
        targets.insert(stim::DemTarget::observable_id(k));
    }
    std::set<uint64_t> ticks;
    for (uint64_t k = 0; k < stats.num_ticks; k++) {
        ticks.insert(k);
    }

    std::ostringstream out;
    for (const auto &[target, snapshots] :
         stim::circuit_to_detecting_regions(circuit, std::move(targets), std::move(ticks), true)) {
        for (const auto &[tick, pauli] : snapshots) {
            out << target << '@' << tick << '=' << pauli << '\n';
        }
    }
    std::string result = out.str();
    while (!result.empty() && result.back() == '\n') {
        result.pop_back();
    }
    return result;
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
    if (behavior == "decompose") {
        return stim::simplified_circuit(circuit).str();
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
    if (behavior == "detecting-regions") {
        return detecting_regions_text(circuit);
    }
    if (behavior == "missing-known-input") {
        return stim::missing_detectors(circuit, false).str();
    }
    if (behavior == "missing-unknown-input") {
        return stim::missing_detectors(circuit, true).str();
    }
    if (behavior == "flows" || behavior == "decompose-flows") {
        return flow_generators_text(circuit);
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
    expectation: CatalogExpectation,
}

impl CatalogCase {
    fn canonical(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            expectation: CatalogExpectation::Canonical,
        }
    }

    fn rejected(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            expectation: CatalogExpectation::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogExpectation {
    Canonical,
    Rejected,
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
    let cases = catalog_gate_target_cases(false);
    let gate_count = Gate::all().len();
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
fn p5_catalog_flow_generators_match_pinned_stim() {
    let mut cases = catalog_gate_target_cases(true);
    cases.push(CatalogCase::canonical(
        "nested-compact-repeat",
        "REPEAT 3 {\n    REPEAT 2 {\n        H 0\n    }\n}\n",
    ));
    let gate_count = Gate::all().len();
    compare_catalog_matrix("flows", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        circuit_flow_generators(&circuit)
            .map(flow_text)
            .map_err(|error| error.to_string())
    });
    eprintln!(
        "[stab-oracle] flow-generator catalog evidence covered {gate_count} gates across {} legal target-shape cases",
        cases.len()
    );
}

#[test]
#[ignore = "builds and executes a metadata-driven differential against pinned libstim"]
fn p5_catalog_detecting_regions_match_pinned_stim() {
    let cases = detecting_region_catalog_cases();
    let gate_count = fixed_tableau_gates().len();
    compare_catalog_matrix("detecting-regions", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        detecting_regions_text(&circuit)
    });
    eprintln!(
        "[stab-oracle] detecting-region catalog evidence covered {gate_count} fixed-tableau gates across {} value-level target-shape probes",
        cases.len()
    );
}

#[test]
#[ignore = "builds and executes a metadata-driven differential against pinned libstim"]
fn p5_catalog_missing_detectors_match_pinned_stim() {
    let gate_cases = missing_detector_gate_cases();
    let repeat_pairs = missing_detector_repeat_pairs();
    let mut cases = gate_cases.clone();
    cases.extend(
        repeat_pairs
            .iter()
            .flat_map(|(compact, explicit)| [compact.clone(), explicit.clone()]),
    );
    for (behavior, ignore_non_deterministic_measurements) in [
        ("missing-known-input", false),
        ("missing-unknown-input", true),
    ] {
        compare_catalog_matrix_with_probe(catalog_probe(), behavior, &cases, |source| {
            missing_detector_text(source, ignore_non_deterministic_measurements)
        });
        for (compact, explicit) in &repeat_pairs {
            assert_eq!(
                missing_detector_text(&compact.source, ignore_non_deterministic_measurements),
                missing_detector_text(&explicit.source, ignore_non_deterministic_measurements),
                "{behavior}/{}: compact and explicit repeats diverged",
                compact.id
            );
        }
    }
    let gate_count = fixed_tableau_gates().len();
    eprintln!(
        "[stab-oracle] missing-detector catalog evidence covered {gate_count} fixed-tableau gates through {} probes and {} compact/explicit repeat pairs in both input-state modes",
        gate_cases.len(),
        repeat_pairs.len()
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
            Ok(CatalogCase::canonical(
                gate.canonical_name(),
                representative_quantum_instruction(gate)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()
        .expect("every decomposition-bearing gate has a quantum target shape");
    assert_eq!(cases.len(), gates.len());
    compare_catalog_matrix("decompose-flows", &cases, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        let decomposed = decomposed_circuit(&circuit).map_err(|error| error.to_string())?;
        base_gate_flow_text(&decomposed)
    });
    let cross_cutting = [CatalogCase::canonical(
        "tags-noise-constants-repeat",
        "QUBIT_COORDS[q](1, 2) 0 1 2\nREPEAT[loop] 2 {\n    X_ERROR[noise](0.125) 0\n    SPP[phase] X0*Y1\n    MPP[product] X0*Y1 !Z2*Z2\n    MXX[pair](0.25) 0 1\n    DETECTOR[det](3) rec[-1]\n    TICK[tick]\n}\n",
    )];
    compare_catalog_matrix("decompose", &cross_cutting, |source| {
        let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
        decomposed_circuit(&circuit)
            .map(|decomposed| without_terminal_newlines(decomposed.to_stim_string()))
            .map_err(|error| error.to_string())
    });
    eprintln!(
        "[stab-oracle] base-gate decomposition catalog evidence covered {} decomposition-bearing gates and one cross-cutting exact fixture",
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
            Ok(CatalogCase::canonical(
                gate.canonical_name(),
                representative_quantum_instruction(gate)?,
            ))
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
    evaluate_stab: impl FnMut(&str) -> Result<String, String>,
) {
    compare_catalog_matrix_with_probe(catalog_probe(), behavior, cases, evaluate_stab);
}

fn catalog_probe() -> &'static PinnedStimProgram {
    static PROBE: OnceLock<PinnedStimProgram> = OnceLock::new();
    PROBE
        .get_or_init(|| PinnedStimProgram::compile("catalog-semantics", PINNED_STIM_CATALOG_HELPER))
}

fn compare_catalog_matrix_with_probe(
    probe: &PinnedStimProgram,
    behavior: &str,
    cases: &[CatalogCase],
    mut evaluate_stab: impl FnMut(&str) -> Result<String, String>,
) {
    validate_matrix(cases);
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
        match (case.expectation, evaluate_stab(&case.source), pinned) {
            (
                CatalogExpectation::Canonical,
                Ok(stab),
                PinnedOutcome::Canonical(pinned),
            ) if !canonical_outputs_match(behavior, &stab, &pinned) => {
                failures.push(format!(
                    "{behavior}/{}: canonical output mismatch\nsource:\n{}\nStab:\n{}\npinned Stim:\n{}",
                    case.id, case.source, stab, pinned
                ));
            }
            (CatalogExpectation::Canonical, Ok(_), PinnedOutcome::Canonical(_)) => {}
            (CatalogExpectation::Canonical, Err(error), PinnedOutcome::Canonical(_)) => failures.push(format!(
                "{behavior}/{}: Stab rejected a legal case: {error}\nsource:\n{}",
                case.id, case.source
            )),
            (CatalogExpectation::Canonical, stab, PinnedOutcome::Rejected { class, detail }) => failures.push(format!(
                "{behavior}/{}: pinned Stim rejected a generated legal case as {class}: {detail}\nStab outcome: {stab:?}\nsource:\n{}",
                case.id, case.source
            )),
            (CatalogExpectation::Rejected, Err(_), PinnedOutcome::Rejected { .. }) => {}
            (CatalogExpectation::Rejected, Ok(stab), PinnedOutcome::Rejected { class, detail }) => failures.push(format!(
                "{behavior}/{}: Stab accepted a shape expected to reject while pinned Stim rejected it as {class}: {detail}\nStab output:\n{stab}\nsource:\n{}",
                case.id, case.source
            )),
            (CatalogExpectation::Rejected, stab, PinnedOutcome::Canonical(pinned)) => failures.push(format!(
                "{behavior}/{}: a shape marked as rejected was accepted by pinned Stim\nStab outcome: {stab:?}\npinned Stim:\n{pinned}\nsource:\n{}",
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

fn canonical_outputs_match(behavior: &str, stab: &str, pinned: &str) -> bool {
    if behavior != "dem" {
        return stab == pinned;
    }
    let Ok(stab) = DetectorErrorModel::from_dem_str(stab) else {
        return false;
    };
    let Ok(pinned) = DetectorErrorModel::from_dem_str(pinned) else {
        return false;
    };
    stab == pinned
}

#[test]
fn dem_catalog_comparison_is_independent_of_host_long_double_print_width() {
    let stable = "error(0.1874999999999999722444243843710865) D0";
    let x86_stim = "error(0.1874999999999999722) D0";
    assert!(canonical_outputs_match("dem", stable, x86_stim));
    assert!(!canonical_outputs_match(
        "dem",
        stable,
        "error(0.1874999999999999722) D1"
    ));
    assert!(!canonical_outputs_match("flows", stable, x86_stim));
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

fn detecting_region_catalog_cases() -> Vec<CatalogCase> {
    fixed_tableau_gates()
        .into_iter()
        .flat_map(|gate| {
            fixed_tableau_detecting_shapes(gate)
                .into_iter()
                .flat_map(move |(shape, probes)| {
                    probes.iter().map(move |&(probe_id, probe)| {
                        let id = format!("{}/{}/{probe_id}", gate.canonical_name(), shape.id);
                        let source = format!(
                            "R {REGION_QUBITS}\nM 7 8\nTICK\n{}TICK\nMPP {probe}\nDETECTOR rec[-1]\n",
                            instruction_text(gate, &shape, false)
                        );
                        catalog_case_for_shape(gate, &shape, false, id, source)
                    })
                })
        })
        .collect()
}

fn fixed_tableau_gates() -> Vec<Gate> {
    let gates = Gate::all()
        .filter(|gate| gate_has_tableau(*gate))
        .collect::<Vec<_>>();
    assert_eq!(gates.len(), 46, "pinned Stim fixed-tableau gate count");
    gates
}

fn fixed_tableau_detecting_shapes(
    gate: Gate,
) -> Vec<(TargetShape, &'static [(&'static str, &'static str)])> {
    const EMPTY: &[(&str, &str)] = &[("identity", "X0")];
    const SINGLE: &[(&str, &str)] = &[("x", "X0"), ("z", "Z0")];
    const SINGLE_MANY: &[(&str, &str)] = &[("xyz", "X0*Y2*Z5")];
    const PAIR: &[(&str, &str)] = &[
        ("x-left", "X0"),
        ("z-left", "Z0"),
        ("x-right", "X1"),
        ("z-right", "Z1"),
    ];
    const PAIR_MANY: &[(&str, &str)] = &[("four-generators", "X0*Z2*X5*Z7")];
    const FEEDBACK: &[(&str, &str)] = &[("x", "X1"), ("z", "Z1")];

    match gate.target_rule() {
        GateTargetRule::AnySingleQubit => vec![
            (shape("empty", "", 0), EMPTY),
            (shape("single", "0", 0), SINGLE),
            (shape("many", "0 2 5", 0), SINGLE_MANY),
        ],
        GateTargetRule::PlainPairs => vec![
            (shape("empty", "", 0), EMPTY),
            (shape("pair", "0 1", 0), PAIR),
            (shape("many-pairs", "0 1 2 3 4 5 6 7", 0), PAIR_MANY),
        ],
        target_rule => {
            assert_eq!(
                target_rule,
                GateTargetRule::ClassicalControlPairs,
                "fixed-tableau gate {} has unsupported target rule {target_rule:?}",
                gate.canonical_name()
            );
            let mut shapes = vec![
                (shape("empty", "", 0), EMPTY),
                (shape("pair", "0 1", 0), PAIR),
                (shape("many-pairs", "0 1 2 3 4 5 6 7", 0), PAIR_MANY),
            ];
            shapes.extend(
                classical_control_shapes(gate)
                    .into_iter()
                    .filter(|shape| shape.id.contains("record") || shape.id.contains("sweep"))
                    .map(|shape| (shape, FEEDBACK)),
            );
            shapes
        }
    }
}

fn detecting_regions_text(circuit: &Circuit) -> Result<String, String> {
    let targets = all_detecting_region_targets(circuit).map_err(|error| error.to_string())?;
    let ticks = all_detecting_region_ticks(circuit).map_err(|error| error.to_string())?;
    circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets,
            ticks,
            ignore_anticommutation_errors: true,
        },
    )
    .map(|regions| {
        regions
            .into_iter()
            .flat_map(|(target, snapshots)| {
                snapshots
                    .into_iter()
                    .map(move |(tick, pauli)| format!("{target}@{}={pauli}", tick.get()))
            })
            .collect::<Vec<_>>()
            .join("\n")
    })
    .map_err(|error| error.to_string())
}

fn missing_detector_gate_cases() -> Vec<CatalogCase> {
    fixed_tableau_gates()
        .into_iter()
        .map(|gate| {
            let target_rule = gate.target_rule();
            let source = if target_rule == GateTargetRule::AnySingleQubit {
                format!("{} 0 1 2\nMPP X0 Y1 Z2\n", gate.canonical_name())
            } else {
                assert!(
                    matches!(
                        target_rule,
                        GateTargetRule::PlainPairs | GateTargetRule::ClassicalControlPairs
                    ),
                    "fixed-tableau gate {} has unsupported target rule {target_rule:?}",
                    gate.canonical_name()
                );
                two_qubit_missing_detector_probe(gate)
            };
            CatalogCase::canonical(gate.canonical_name(), source)
        })
        .collect()
}

fn two_qubit_missing_detector_probe(gate: Gate) -> String {
    let bases = ['I', 'X', 'Y', 'Z'];
    let mut gate_targets = Vec::new();
    let mut measurements = Vec::new();
    let mut pair_index = 0_usize;
    for left in bases {
        for right in bases {
            if left == 'I' && right == 'I' {
                continue;
            }
            let left_qubit = pair_index * 2;
            let right_qubit = left_qubit + 1;
            gate_targets.extend([left_qubit.to_string(), right_qubit.to_string()]);
            let mut product = String::new();
            if left != 'I' {
                product.push(left);
                product.push_str(&left_qubit.to_string());
            }
            if right != 'I' {
                if !product.is_empty() {
                    product.push('*');
                }
                product.push(right);
                product.push_str(&right_qubit.to_string());
            }
            measurements.push(product);
            pair_index += 1;
        }
    }
    assert_eq!(pair_index, 15, "two-qubit nonidentity Pauli probes");
    format!(
        "{} {}\nMPP {}\n",
        gate.canonical_name(),
        gate_targets.join(" "),
        measurements.join(" ")
    )
}

fn missing_detector_repeat_pairs() -> Vec<(CatalogCase, CatalogCase)> {
    vec![
        repeat_pair("covered-row", "R 0\n", "M 0\nDETECTOR rec[-1]\n", 4),
        repeat_pair(
            "cross-iteration-row",
            "R 0\nM 0\n",
            "M 0\nDETECTOR rec[-1] rec[-2]\n",
            3,
        ),
        repeat_pair(
            "state-preserving-clifford",
            "RX 0\n",
            "H 0\nH 0\nMX 0\nDETECTOR rec[-1]\n",
            5,
        ),
        (
            CatalogCase::canonical(
                "nested-covered/compact",
                "R 0\nREPEAT 3 {\n    REPEAT 2 {\n        M 0\n        DETECTOR rec[-1]\n    }\n}\n",
            ),
            CatalogCase::canonical(
                "nested-covered/explicit",
                format!("R 0\n{}", "M 0\nDETECTOR rec[-1]\n".repeat(6)),
            ),
        ),
    ]
}

fn repeat_pair(
    id: &str,
    prefix: &str,
    body: &str,
    repetitions: usize,
) -> (CatalogCase, CatalogCase) {
    (
        CatalogCase::canonical(
            format!("{id}/compact"),
            format!("{prefix}REPEAT {repetitions} {{\n{body}}}\n"),
        ),
        CatalogCase::canonical(
            format!("{id}/explicit"),
            format!("{prefix}{}", body.repeat(repetitions)),
        ),
    )
}

fn missing_detector_text(
    source: &str,
    ignore_non_deterministic_measurements: bool,
) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(source).map_err(|error| error.to_string())?;
    missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements,
        },
    )
    .map(|missing| without_terminal_newlines(missing.to_stim_string()))
    .map_err(|error| error.to_string())
}

fn catalog_gate_target_cases(classical_only_noops: bool) -> Vec<CatalogCase> {
    let cases = Gate::all()
        .flat_map(|gate| {
            analyzer_target_shapes(gate)
                .into_iter()
                .map(move |shape| analyzer_case(gate, shape, classical_only_noops))
        })
        .collect::<Vec<_>>();
    assert!(
        cases.len() >= Gate::all().len(),
        "every canonical gate must own at least one catalog case"
    );
    cases
}

fn analyzer_case(gate: Gate, shape: TargetShape, classical_only_noops: bool) -> CatalogCase {
    let mut source = format!("R {ANALYZER_QUBITS}\nM 6 7\n");
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
    catalog_case_for_shape(
        gate,
        &shape,
        classical_only_noops,
        format!("{}/{}", gate.canonical_name(), shape.id),
        source,
    )
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
    shapes.extend([
        shape("record-record", "rec[-1] rec[-2]", 0),
        shape("record-sweep", "rec[-1] sweep[0]", 0),
        shape("sweep-record", "sweep[0] rec[-1]", 0),
        shape("sweep-sweep", "sweep[0] sweep[1]", 0),
    ]);
    shapes
}

fn catalog_case_for_shape(
    gate: Gate,
    shape: &TargetShape,
    classical_only_noops: bool,
    id: impl Into<String>,
    source: impl Into<String>,
) -> CatalogCase {
    if classical_only_shape(shape) && !classical_only_noops && !gate.is_symmetric_gate() {
        CatalogCase::rejected(id, source)
    } else {
        CatalogCase::canonical(id, source)
    }
}

fn classical_only_shape(shape: &TargetShape) -> bool {
    matches!(
        shape.id,
        "record-record" | "record-sweep" | "sweep-record" | "sweep-sweep"
    )
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
