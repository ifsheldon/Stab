use super::super::model::{
    CorrectnessBinding, EvidenceState, FixtureLocator, InputByteCount, MemoryMethod, MemoryPolicy,
    OutputContract, QualificationGroup, QualificationStatus, RunnerFidelity, ScalePoint,
    ThresholdPolicy, WorkloadFamily,
};

const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";
const CIRCUIT_PARSE_CORRECTNESS_CASES: [&str; 2] = [
    "cq-evidence-qualification-633fa529edf5f549",
    "cq-evidence-qualification-e660819ae9a223c6",
];

pub(super) fn apply(group: &mut QualificationGroup) {
    if group.id != CIRCUIT_PARSE_GROUP_ID {
        return;
    }
    group.runner_fidelity = RunnerFidelity::AdapterLibrary;
    group.correctness_cases = CIRCUIT_PARSE_CORRECTNESS_CASES
        .into_iter()
        .map(str::to_string)
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family = WorkloadFamily {
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
    };
    group.output_contract = OutputContract {
        expected_shape:
            "Exact fixture byte count and digest plus canonical final-circuit semantic digest."
                .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Both workers construct the source-owned fixture outside timing, bind its exact bytes, and digest the final parsed circuit outside timing."
            .to_string(),
    };
    group.memory_policy = MemoryPolicy {
        method: MemoryMethod::ProcessRss,
        scale_ids: ["small", "medium", "large"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        expected_growth: "Process setup and peak RSS are reported separately at every timing scale; maximum accepted materialization and first rejection remain PQ6 resource evidence."
            .to_string(),
    };
    group.threshold_policy = ThresholdPolicy::Primary1_25;
    group.owner = "stab-core/circuit-parser".to_string();
    group.reason = "Implemented paired adapter and Rust parser workload with exact CQ2, input, output, scale, timing, and memory bindings."
        .to_string();
    group.status = QualificationStatus::Implemented;
}
