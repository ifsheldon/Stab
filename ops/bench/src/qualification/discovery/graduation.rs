use std::path::Path;

use super::super::model::{
    ComparatorSource, CorrectnessBinding, EvidenceState, FixtureLocator, InputByteCount,
    MemoryMethod, MemoryPolicy, OutputContract, QualificationGroup, QualificationStatus,
    RunnerFidelity, ScalePoint, ThresholdPolicy, WorkloadFamily,
};
use crate::error::BenchError;
use crate::root::RepoRoot;

const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
const CIRCUIT_CANONICAL_PRINT_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-CANONICAL-PRINT";
const GATE_NAME_HASH_GROUP_ID: &str = "PERFQ-M4-GATE-LOOKUP";
const SIMD_WORD_POPCOUNT_GROUP_ID: &str = "PERFQ-M5-SIMD-WORD";
const STIM_ADAPTER_SOURCE: &str = "benchmarks/stim_adapter/main.cc";
const SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/simd_word_popcount_contract.h";
const CIRCUIT_PARSE_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-633fa529edf5f549",
    "cq-evidence-qualification-e660819ae9a223c6",
];
const CIRCUIT_CANONICAL_PRINT_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-e660819ae9a223c6",
    "cq-evidence-qualification-ef933925fb901877",
];
const GATE_NAME_HASH_CORRECTNESS_CASE: &str = "cq-evidence-qualification-bd20a013e903a05f";
const SIMD_WORD_POPCOUNT_CORRECTNESS_CASES: [&str; 3] = [
    "cq-evidence-qualification-5118006702599a45",
    "cq-evidence-qualification-b1530dc4e48e942d",
    "cq-evidence-qualification-ba252d42660a41ce",
];
const EMPTY_INPUT_DIGEST: &str = "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";

pub(super) fn apply(root: &RepoRoot, group: &mut QualificationGroup) -> Result<(), BenchError> {
    match group.id.as_str() {
        CIRCUIT_PARSE_GROUP_ID => apply_circuit_parse(group),
        CIRCUIT_CANONICAL_PRINT_GROUP_ID => apply_circuit_canonical_print(group),
        GATE_NAME_HASH_GROUP_ID => apply_gate_name_hash(group),
        SIMD_WORD_POPCOUNT_GROUP_ID => apply_simd_word_popcount(root, group)?,
        _ => {}
    }
    Ok(())
}

fn apply_simd_word_popcount(
    root: &RepoRoot,
    group: &mut QualificationGroup,
) -> Result<(), BenchError> {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = SIMD_WORD_POPCOUNT_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = simd_word_popcount_workload_family();
    group.output_contract = OutputContract {
        expected_shape: "Exact deterministic fixture bytes plus a canonical digest over eight little-endian u64 fields in this order: accumulated popcount checksum, iteration count, bit width, all four input-fingerprint lanes, and final toggle-bit state."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare identical little-endian SplitMix64 words and initial toggle state outside timing, toggle bit 300 in the timed body, accumulate exact whole-vector popcounts using Stim ptr_simd[k].popcount() and Stab BitVec::popcount(), then read final state and construct the canonical output digest outside timing."
            .to_string(),
        comparator_sources: [STIM_ADAPTER_SOURCE, SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE]
            .into_iter()
            .map(|path| comparator_source(root, path))
            .collect::<Result<_, _>>()?,
    };
    group.memory_policy = circuit_memory_policy(
        "The aligned bit vector is prepared before timing and setup and peak process RSS are report-only observations at every scale. This slice makes no linear-growth acceptance claim; PQ6 owns explicit cross-scale RSS and allocation slack.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/bits".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust toggle-plus-popcount work with exact CQ2, deterministic input, semantic output, scale, timing, and bounded-worker contracts."
        .to_string();
    group.status = QualificationStatus::Implemented;
    Ok(())
}

fn comparator_source(root: &RepoRoot, path: &str) -> Result<ComparatorSource, BenchError> {
    let source = super::read_repo_text_bounded(root, &root.path.join(Path::new(path)))?;
    Ok(ComparatorSource {
        path: path.to_string(),
        sha256: super::sha256_hex(source.as_bytes()),
    })
}

fn apply_gate_name_hash(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = vec![GATE_NAME_HASH_CORRECTNESS_CASE.to_string()];
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = gate_name_hash_workload_family();
    group.output_contract = OutputContract {
        expected_shape: "Exact complete-table hash count plus matching final checksum, ordered table fingerprint, and semantic digest."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers prepare the 82 Stim gate-table entries, including NOT_A_GATE, outside timing, hash only complete table sweeps in the timed body, and consume the final checksum outside timing."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "The immutable 82-name registry is prepared before timing; setup and peak process RSS are report-only observations at every scale. This slice makes no bounded-growth claim; PQ6 owns an explicit cross-scale RSS and allocation-growth rule.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/gates".to_string();
    group.reason = "Implemented paired pinned-Stim and Rust all-gate-name hashing with exact CQ2, complete-sweep, output-digest, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn apply_circuit_parse(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CIRCUIT_PARSE_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = circuit_workload_family();
    group.output_contract = OutputContract {
        expected_shape:
            "Exact fixture byte count and digest plus canonical final-circuit semantic digest."
                .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers construct the source-owned fixture outside timing, bind its exact bytes, and digest the final parsed circuit outside timing."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "Process setup and peak RSS are reported separately at every timing scale; maximum accepted materialization and first rejection remain PQ6 resource evidence.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/circuit-parser".to_string();
    group.reason = "Implemented paired adapter and Rust parser workload with exact CQ2, input, output, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn apply_circuit_canonical_print(group: &mut QualificationGroup) {
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CIRCUIT_CANONICAL_PRINT_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = circuit_workload_family();
    group.output_contract = OutputContract {
        expected_shape:
            "Exact fixture byte count and digest plus final canonical circuit-text digest."
                .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers parse the source-owned fixture once outside timing, repeatedly serialize the resulting circuit while consuming each produced string, and compare the final canonical digest outside timing after normalizing only Stab's terminal newline."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.memory_policy = circuit_memory_policy(
        "Process setup RSS includes the parsed circuit and peak RSS includes canonical output allocation at every timing scale; maximum accepted materialization and first rejection remain PQ6 resource evidence.",
    );
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/circuit-printer".to_string();
    group.reason = "Implemented paired adapter and Rust canonical circuit serialization workload with exact CQ2, input, output, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}

fn circuit_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "circuit-parse-cycle-v1".to_string(),
        },
        source: "benchmarks/stim_adapter/main.cc".to_string(),
        deterministic_seed: "circuit-parse-cycle-v1".to_string(),
        scales: [
            (
                "small",
                64,
                429,
                "c3c0855f4f04402cd1768dee1ca0606d7d1ff8907d6a3a4e3b386fd78ff6c3b6",
            ),
            (
                "medium",
                4_096,
                27_981,
                "7c0a60d24fde2f776143003b987c30cd682d77fee5fd9f17bd9e9b5377a8ad04",
            ),
            (
                "large",
                65_536,
                447_821,
                "397e8db6accb8e66a826015e2d5db453271851fa2c49d40a0d98f91748219b60",
            ),
        ]
        .into_iter()
        .map(|(id, instructions, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
            parameters: format!("generator=circuit-parse-cycle-v1; instructions={instructions}"),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(instructions),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}

fn circuit_memory_policy(expected_growth: &str) -> MemoryPolicy {
    MemoryPolicy {
        method: MemoryMethod::ProcessRss,
        scale_ids: ["small", "medium", "large"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        expected_growth: expected_growth.to_string(),
    }
}

fn gate_name_hash_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "stim-v1.16.0-gate-name-table".to_string(),
        },
        source: "src/stim/gates/gates.perf.cc".to_string(),
        deterministic_seed: "not-applicable-static-gate-table".to_string(),
        scales: [("small", 1_u64), ("medium", 64), ("large", 4_096)]
            .into_iter()
            .map(|(id, sweeps)| {
                let gate_hashes = sweeps * 82;
                ScalePoint {
                    id: id.to_string(),
                    parameters: format!(
                        "generator=stim-v1.16.0-gate-name-table; names=82; complete_sweeps={sweeps}"
                    ),
                    input_bytes: InputByteCount::Exact { bytes: 0 },
                    semantic_work: Some(gate_hashes),
                    input_digest: Some(EMPTY_INPUT_DIGEST.to_string()),
                }
            })
            .collect(),
    }
}

fn simd_word_popcount_workload_family() -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated {
            id: "splitmix64-word-v1".to_string(),
        },
        source: "src/stim/mem/simd_word.perf.cc".to_string(),
        deterministic_seed: "splitmix64-word-v1".to_string(),
        scales: [
            (
                "small",
                4_096,
                512,
                "101e05fc22ce0676c277e9b16363a38750079d12e0b93f3c687ed95457b79d1c",
            ),
            (
                "medium",
                262_144,
                32_768,
                "b33ad442a544ef4b367ab3b2e9a47d65676791ed7661ad7fa2529b5249bfea77",
            ),
            (
                "large",
                16_777_216,
                2_097_152,
                "b1e7afd7d73691441ea033a9eb9496d02fa12bc4d3bcf059856c089112dae368",
            ),
        ]
        .into_iter()
        .map(|(id, bits, input_bytes, input_digest)| ScalePoint {
            id: id.to_string(),
            parameters: format!(
                "generator=splitmix64-word-v1; bits={bits}; alignment_bits=256; toggle_bit=300"
            ),
            input_bytes: InputByteCount::Exact { bytes: input_bytes },
            semantic_work: Some(bits),
            input_digest: Some(input_digest.to_string()),
        })
        .collect(),
    }
}
