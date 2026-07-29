use super::{
    BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID, BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
    CLIFFORD_STRING_IDENTITY_GROUP_ID, CLIFFORD_STRING_NON_IDENTITY_GROUP_ID, apply,
};
use crate::error::BenchError;
use crate::qualification::discovery::api::{
    A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID, A2_SAMPLER_COMPILE_GROUP_ID,
    A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID, A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
};
use crate::qualification::model::{
    CorrectnessBinding, EvidenceState, MemoryMethod, MemoryPolicy, OutputContract,
    PRODUCT_DIAGNOSTIC_GATE_STATISTIC, PerformanceDisposition, Phase, QualificationGroup,
    QualificationStatus, RowOrigin, RunnerFidelity, ThresholdPolicy,
};
use crate::root::RepoRoot;

const CIRCUIT_PARSE_GROUP_ID: &str = "PERFQ-M4-CIRCUIT-PARSE";

pub(in crate::qualification::discovery) fn groups(
    root: &RepoRoot,
    existing: &[QualificationGroup],
) -> Result<Vec<QualificationGroup>, BenchError> {
    let bit_matrix_source = existing
        .iter()
        .find(|group| group.id == "PERFQ-M5-SIMD-BIT-TABLE")
        .ok_or_else(|| {
            BenchError::Qualification(
                "curated transpose groups require the inherited bit-matrix workload".to_string(),
            )
        })?;
    let clifford_source = existing
        .iter()
        .find(|group| group.id == CLIFFORD_STRING_IDENTITY_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "curated non-identity Clifford group requires the identity workload".to_string(),
            )
        })?;
    [
        (
            bit_matrix_source,
            BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
            "perfq-m5-bit-matrix-transpose-in-place",
        ),
        (
            bit_matrix_source,
            BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID,
            "perfq-m5-bit-matrix-transpose-allocating",
        ),
        (
            clifford_source,
            CLIFFORD_STRING_NON_IDENTITY_GROUP_ID,
            "perfq-m6-clifford-string-non-identity",
        ),
    ]
    .into_iter()
    .map(|(source, id, manifest_row)| {
        let mut group = source.clone();
        group.id = id.to_string();
        group.manifest_row = manifest_row.to_string();
        group.row_origin = RowOrigin::Planned;
        group.disposition = PerformanceDisposition::Measured;
        group.public_api_items.clear();
        apply(root, &mut group)?;
        Ok(group)
    })
    .collect()
}

pub(in crate::qualification::discovery) fn agent_diagnostic_groups(
    existing: &[QualificationGroup],
) -> Result<Vec<QualificationGroup>, BenchError> {
    let circuit = existing
        .iter()
        .find(|group| group.id == CIRCUIT_PARSE_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "A2 product diagnostics require the executable circuit fixture family".to_string(),
            )
        })?;
    [
        AgentDiagnosticSpec {
            id: A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID,
            manifest_row: "perfq-a2-circuit-model-fingerprint",
            performance_feature: "PERF-CIRCUIT-MODEL",
            phase: Phase::Transform,
            correctness_cases: &["cq-evidence-qualification-e16abe30d8c7992c"],
            owner: "stab-core/model-fingerprint",
            reason: "Measures only Circuit::fingerprint over a pre-parsed deterministic circuit; the result is compared with an untimed typed fingerprint witness.",
        },
        AgentDiagnosticSpec {
            id: A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
            manifest_row: "perfq-a2-sampling-request-fingerprint",
            performance_feature: "PERF-SAMPLING",
            phase: Phase::Compile,
            correctness_cases: &["cq-evidence-qualification-d63aa8cd2dc62e63"],
            owner: "stab-engine/compilation-request",
            reason: "Measures the inclusive CompilationRequestFingerprint::for_sampling call, including its model fingerprint, over a pre-parsed deterministic circuit.",
        },
        AgentDiagnosticSpec {
            id: A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID,
            manifest_row: "perfq-a2-sampling-request-estimate",
            performance_feature: "PERF-RESOURCE-BOUNDARIES",
            phase: Phase::Compile,
            correctness_cases: &["cq-evidence-qualification-d65b079751fe7119"],
            owner: "stab-core/resource-estimation",
            reason: "Measures only estimate_sampling_request over a pre-parsed deterministic circuit and compares the complete ResourceEstimate with an untimed typed witness.",
        },
        AgentDiagnosticSpec {
            id: A2_SAMPLER_COMPILE_GROUP_ID,
            manifest_row: "perfq-a2-sampler-compile",
            performance_feature: "PERF-SAMPLING",
            phase: Phase::Compile,
            correctness_cases: &["cq-evidence-qualification-7bcf8fcdbbfa6d68"],
            owner: "stab-engine/sampling-compiler",
            reason: "Measures the complete SamplingCompiler compile-and-release lifecycle over a pre-parsed deterministic circuit. Untimed recompilation compares the exact PlanFingerprint with a typed witness, the legacy CompiledSampler adapter is covered by the same correctness parent, and no sampling method is called.",
        },
    ]
    .into_iter()
    .map(|spec| agent_diagnostic_group(circuit, spec))
    .collect()
}

struct AgentDiagnosticSpec {
    id: &'static str,
    manifest_row: &'static str,
    performance_feature: &'static str,
    phase: Phase,
    correctness_cases: &'static [&'static str],
    owner: &'static str,
    reason: &'static str,
}

fn agent_diagnostic_group(
    circuit: &QualificationGroup,
    spec: AgentDiagnosticSpec,
) -> Result<QualificationGroup, BenchError> {
    let mut group = circuit.clone();
    group.id = spec.id.to_string();
    group.manifest_row = spec.manifest_row.to_string();
    group.row_origin = RowOrigin::Planned;
    group.performance_feature = spec.performance_feature.to_string();
    group.checklist_anchors.clear();
    group.checklist_child_ids.clear();
    group.public_api_items.clear();
    group.disposition = PerformanceDisposition::Measured;
    group.phase = spec.phase;
    group.runner_fidelity = RunnerFidelity::StabReportOnly;
    group.correctness_cases = spec
        .correctness_cases
        .iter()
        .map(|case| (*case).to_string())
        .collect();
    group.correctness_binding = CorrectnessBinding::ExactCases;
    group.planned_correctness_case_id = None;
    group.workload_family.source =
        "ops/bench/src/qualification/runtime/worker/agent_diagnostic.rs".to_string();
    group.work_unit = "circuit items".to_string();
    group.output_contract = OutputContract {
        expected_shape: "One exact typed operation result per invocation, retained through raw-work-v2 and compared with an untimed expected value before constructing the semantic digest."
            .to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: "Circuit text generation and parsing occur before timing. The worker times only the named Stab API, retains the final typed result, validates it outside timing, and emits one bounded protocol row."
            .to_string(),
        comparator_sources: Vec::new(),
    };
    group.timing_policy.gate_statistic = PRODUCT_DIAGNOSTIC_GATE_STATISTIC.to_string();
    group.memory_policy = MemoryPolicy {
        method: MemoryMethod::NotApplicable,
        scale_ids: Vec::new(),
        expected_growth:
            "No release memory claim; bounded worker RSS remains present only in raw diagnostic receipts."
                .to_string(),
    };
    group.threshold_policy = ThresholdPolicy::ReportOnly;
    group.reason = spec.reason.to_string();
    group.owner = spec.owner.to_string();
    group.status = QualificationStatus::Implemented;
    Ok(group)
}
