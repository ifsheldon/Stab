use super::{
    BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID, BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
    CLIFFORD_STRING_IDENTITY_GROUP_ID, CLIFFORD_STRING_NON_IDENTITY_GROUP_ID, apply,
};
use crate::error::BenchError;
use crate::qualification::discovery::api::{
    A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID, A2_SAMPLER_COMPILE_GROUP_ID,
    A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID, A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
    A7_EXACT_ML_COMPILE_GROUP_ID, A7_EXACT_ML_REUSED_DECODE_GROUP_ID, A7_PIPELINE_GROUP_ID,
};
use crate::qualification::model::{
    CorrectnessBinding, EvidenceState, FixtureLocator, InputByteCount, MemoryMethod, MemoryPolicy,
    OutputContract, PRODUCT_DIAGNOSTIC_GATE_STATISTIC, PerformanceDisposition, Phase,
    QualificationGroup, QualificationStatus, RowOrigin, RunnerFidelity, ScalePoint, SizeClass,
    ThresholdPolicy, WorkloadFamily,
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
            owner: "stab-model/model-fingerprint",
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

pub(in crate::qualification::discovery) fn decoder_diagnostic_groups(
    existing: &[QualificationGroup],
) -> Result<Vec<QualificationGroup>, BenchError> {
    let circuit = existing
        .iter()
        .find(|group| group.id == CIRCUIT_PARSE_GROUP_ID)
        .ok_or_else(|| {
            BenchError::Qualification(
                "A7 decoder diagnostics require an executable timing policy source".to_string(),
            )
        })?;
    [
        DecoderDiagnosticSpec {
            id: A7_EXACT_ML_COMPILE_GROUP_ID,
            manifest_row: "perfq-a7-exact-ml-compile",
            phase: Phase::Compile,
            correctness_cases: &[
                "cq-evidence-qualification-160444d1041f2b2a",
                "cq-evidence-qualification-1686de935fd64494",
                "cq-evidence-qualification-278e629a855d3c41",
                "cq-evidence-qualification-3a9a1dd7ddabd7b1",
                "cq-evidence-qualification-55748da764c91bf8",
                "cq-evidence-qualification-7179fce5697cce0d",
                "cq-evidence-qualification-7b17d9e5cca84df5",
                "cq-evidence-qualification-8cae8440e6c8c601",
                "cq-evidence-qualification-93951fde964ca94d",
                "cq-evidence-qualification-b376b5e2d4be1223",
                "cq-evidence-qualification-b4fefe4518cc5c2d",
            ],
            owner: "stab-reference-decoder/exact-ml-compiler",
            work_unit: "joint-state mechanism loop visits",
            family: exact_ml_compile_family,
            expected_shape: "One completed compile-and-release count per invocation plus an untimed exact witness over model identity, dimensions, retained bytes, and every syndrome prediction.",
            sink_policy: "DEM generation occurs before timing. The worker times only complete ExactMlDecoderSession compilation and release, then recompiles and emits the exact typed witness outside raw-work-v2 for comparison with a frozen source-owned digest.",
            reason: "Measures the bounded exact-ML compiler at three admitted joint-state transition scales. This is Stab-only because pinned Stim v1.16.0 provides no faithful external-decoder compiler comparator.",
        },
        DecoderDiagnosticSpec {
            id: A7_EXACT_ML_REUSED_DECODE_GROUP_ID,
            manifest_row: "perfq-a7-exact-ml-reused-decode",
            phase: Phase::Execute,
            correctness_cases: &[
                "cq-evidence-qualification-0b8090f2ca9daf37",
                "cq-evidence-qualification-0e2885667877d158",
                "cq-evidence-qualification-278e629a855d3c41",
                "cq-evidence-qualification-36ae168d2b56fdbe",
                "cq-evidence-qualification-3add8f2f8632a7fb",
                "cq-evidence-qualification-63678c8f7a576971",
                "cq-evidence-qualification-7b17d9e5cca84df5",
                "cq-evidence-qualification-889f3fecd9d3e6da",
                "cq-evidence-qualification-93951fde964ca94d",
                "cq-evidence-qualification-9d8d2046dac8054b",
                "cq-evidence-qualification-d1877c09db8c3c35",
                "cq-evidence-qualification-f7e633d78cd2e6c6",
            ],
            owner: "stab-decoder/reused-batch-decode",
            work_unit: "decoded shots",
            family: exact_ml_decode_family,
            expected_shape: "Exact completed shot count and prediction digest from one precompiled session over a deterministic packed syndrome batch.",
            sink_policy: "Model compilation, detector input construction, prediction allocation, and cancellation-token construction occur before timing. Raw-work-v2 includes each decode_batch call and mandatory returned-progress validation; the complete caller-owned prediction buffer is digested afterward and compared with a frozen source-owned witness.",
            reason: "Measures allocation-free reuse of one exact-ML session and caller-owned output across deterministic small, medium, and accepted-maximum shot-count batches without inventing a Stim ratio. The shared 14-detector session is not the decoder-width admission maximum.",
        },
        DecoderDiagnosticSpec {
            id: A7_PIPELINE_GROUP_ID,
            manifest_row: "perfq-a7-sample-detect-decode-pipeline",
            phase: Phase::EndToEnd,
            correctness_cases: &[
                "cq-evidence-qualification-278e629a855d3c41",
                "cq-evidence-qualification-63678c8f7a576971",
                "cq-evidence-qualification-7b17d9e5cca84df5",
                "cq-evidence-qualification-7d5fce18cc43d73d",
                "cq-evidence-qualification-93951fde964ca94d",
                "cq-evidence-qualification-c61285e1ce0e88e8",
            ],
            owner: "stab-engine/sample-detect-decode-composition",
            work_unit: "pipeline shots",
            family: pipeline_family,
            expected_shape: "Exact seeded shot and logical-failure counts from the public sample-to-detection-to-decoder composition.",
            sink_policy: "Circuit generation, DEM lowering, plan compilation, and session construction occur before timing. Raw-work-v2 measures one complete reusable sampling, typed detection conversion, exact decoding, and logical-failure-counting pass; the complete report is validated afterward against a frozen source-owned witness.",
            reason: "Measures the complete public A7 experiment at bounded single-pass shot scales. It remains a Stab-only baseline candidate because Stim has no equivalent external-decoder composition contract.",
        },
    ]
    .into_iter()
    .map(|spec| decoder_diagnostic_group(circuit, spec))
    .collect()
}

struct DecoderDiagnosticSpec {
    id: &'static str,
    manifest_row: &'static str,
    phase: Phase,
    correctness_cases: &'static [&'static str],
    owner: &'static str,
    work_unit: &'static str,
    family: fn() -> WorkloadFamily,
    expected_shape: &'static str,
    sink_policy: &'static str,
    reason: &'static str,
}

fn decoder_diagnostic_group(
    circuit: &QualificationGroup,
    spec: DecoderDiagnosticSpec,
) -> Result<QualificationGroup, BenchError> {
    let mut group = circuit.clone();
    group.id = spec.id.to_string();
    group.manifest_row = spec.manifest_row.to_string();
    group.row_origin = RowOrigin::Planned;
    group.performance_feature = "PERF-DETECTION".to_string();
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
    group.workload_family = (spec.family)();
    group.work_unit = spec.work_unit.to_string();
    group.output_contract = OutputContract {
        expected_shape: spec.expected_shape.to_string(),
        digest_state: EvidenceState::Existing,
        sink_policy: spec.sink_policy.to_string(),
        comparator_sources: Vec::new(),
    };
    group.timing_policy.gate_statistic = PRODUCT_DIAGNOSTIC_GATE_STATISTIC.to_string();
    group.memory_policy = MemoryPolicy {
        method: MemoryMethod::ProcessRss,
        scale_ids: vec!["large".to_string()],
        expected_growth: "The largest source-owned scale owns one separately instrumented accepted-maximum worker peak-RSS check. The cap is frozen in the runtime diagnostic policy; retained prediction bytes and allocation-free reuse remain exact focused-test contracts."
            .to_string(),
    };
    group.threshold_policy = ThresholdPolicy::ReportOnly;
    group.reason = spec.reason.to_string();
    group.owner = spec.owner.to_string();
    group.status = QualificationStatus::Implemented;
    Ok(group)
}

fn exact_ml_compile_family() -> WorkloadFamily {
    generated_family(
        "a7-exact-ml-compile-v1",
        "ops/bench/src/qualification/runtime/worker/decoder_diagnostic.rs",
        [
            scale(
                "small",
                SizeClass::Small,
                "detectors=6; observables=1; mechanisms=12; joint_states=128",
                1_536,
                615,
                "d76af5f2acbbabce017a7a4c59ba005b444175d242d64369d189baca9ab3876c",
            ),
            scale(
                "medium",
                SizeClass::Medium,
                "detectors=10; observables=1; mechanisms=32; joint_states=2048",
                65_536,
                1_630,
                "8b6d49a6d56cfcda62d7a1147f5f79bf1b7c271a3497d7e43f6fc3c8272bc6b8",
            ),
            scale(
                "large",
                SizeClass::Large,
                "detectors=20; observables=1; represented_mechanisms=2; active_mechanisms=1; joint_states=2097152; passes=2; tie_fallback=true",
                4_194_304,
                50,
                "605f2f7256498d73a515a0ea07ffd04cb420fa76f9cb000417cf02116e097b0e",
            ),
        ],
    )
}

fn exact_ml_decode_family() -> WorkloadFamily {
    generated_family(
        "a7-exact-ml-reused-decode-v1",
        "ops/bench/src/qualification/runtime/worker/decoder_diagnostic.rs",
        [
            scale(
                "small",
                SizeClass::Small,
                "shots=1024; detectors=14; observables=1",
                1_024,
                11_526,
                "3e11e8b3ac52c11a09dfc97615ef5b0f059209fe3f1ace2599e96d40a2c81055",
            ),
            scale(
                "medium",
                SizeClass::Medium,
                "shots=65536; detectors=14; observables=1",
                65_536,
                527_622,
                "b2ddb9e0081df66c939959e66ab26cffa9b5672f5657769de6dd6435d01aecb1",
            ),
            scale(
                "large",
                SizeClass::Large,
                "shots=262144; detectors=14; observables=1",
                262_144,
                2_100_486,
                "0a541c73465194737db84d2051c834a678c0cacff1e58a5a8c5788fb693c9e30",
            ),
        ],
    )
}

fn pipeline_family() -> WorkloadFamily {
    generated_family(
        "a7-sample-detect-decode-v1",
        "ops/bench/src/qualification/runtime/worker/decoder_diagnostic.rs",
        [
            scale(
                "small",
                SizeClass::Small,
                "shots=1024; distance=3; rounds=3; seed=0xA7D3C0DE",
                1_024,
                776,
                "727aaea332dc3b6655f5233cec7ed4d5ba47fba1a8f70316e496a4165df7e518",
            ),
            scale(
                "medium",
                SizeClass::Medium,
                "shots=16384; distance=3; rounds=3; seed=0xA7D3C0DE",
                16_384,
                776,
                "d0e7b29774458576150c08bace932ebd9cadbc17ec3efd6a880e88b46831b044",
            ),
            scale(
                "large",
                SizeClass::Large,
                "shots=262144; distance=3; rounds=3; seed=0xA7D3C0DE",
                262_144,
                776,
                "6b247b780447aad7eb3df7800834aa0528776aba97d529878fdacc59872c76e2",
            ),
        ],
    )
}

fn generated_family(id: &str, source: &str, scales: [ScalePoint; 3]) -> WorkloadFamily {
    WorkloadFamily {
        fixture: FixtureLocator::Generated { id: id.to_string() },
        source: source.to_string(),
        deterministic_seed: id.to_string(),
        scales: scales.into(),
    }
}

fn scale(
    id: &str,
    size_class: SizeClass,
    parameters: &str,
    semantic_work: u64,
    input_bytes: u64,
    input_digest: &str,
) -> ScalePoint {
    ScalePoint {
        id: id.to_string(),
        family_id: "default".to_string(),
        size_class,
        parameters: parameters.to_string(),
        input_bytes: InputByteCount::Exact { bytes: input_bytes },
        semantic_work: Some(semantic_work),
        input_digest: Some(input_digest.to_string()),
    }
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
