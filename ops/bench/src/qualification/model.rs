use serde::{Deserialize, Serialize};

pub(super) const SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationSuite {
    pub(super) schema_version: u32,
    pub(super) stim_version: String,
    pub(super) stim_commit: String,
    pub(super) correctness_digest: String,
    pub(super) semantic_digest: String,
    pub(super) performance_features: Vec<PerformanceFeature>,
    pub(super) checklist_items: Vec<ChecklistItem>,
    pub(super) public_api_items: Vec<ApiDisposition>,
    pub(super) qualification_groups: Vec<QualificationGroup>,
    pub(super) manifest_rows: Vec<ManifestRowDisposition>,
    pub(super) upstream_perf_sources: Vec<UpstreamPerfSource>,
    pub(super) waiver_rows: Vec<WaiverDisposition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PerformanceFeature {
    pub(super) id: String,
    pub(super) correctness_features: Vec<String>,
    pub(super) disposition: PerformanceDisposition,
    pub(super) group_ids: Vec<String>,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChecklistItem {
    pub(super) id: String,
    pub(super) source_line: u32,
    pub(super) anchor_digest: String,
    pub(super) section: String,
    pub(super) feature: String,
    pub(super) raw_status: String,
    pub(super) scope: ChecklistScope,
    pub(super) deferred_remainder: bool,
    pub(super) selected_child: Option<String>,
    pub(super) deferred_child: Option<String>,
    pub(super) selected_child_ids: Vec<String>,
    pub(super) deferred_child_ids: Vec<String>,
    pub(super) selected_child_ownership: Vec<ChecklistChildOwnership>,
    pub(super) performance_features: Vec<String>,
    pub(super) disposition: PerformanceDisposition,
    pub(super) parent_group_ids: Vec<String>,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChecklistChildOwnership {
    pub(super) child_id: String,
    pub(super) performance_features: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ChecklistScope {
    Selected,
    Deferred,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ApiDisposition {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) performance_feature: String,
    pub(super) supporting_performance_features: Vec<String>,
    pub(super) correctness_case_id: String,
    pub(super) disposition: PerformanceDisposition,
    pub(super) parent_group_ids: Vec<String>,
    pub(super) reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PerformanceDisposition {
    Measured,
    CoveredByParent,
    FutureCandidate,
    NotPerformanceRelevant,
    NoFaithfulStimComparator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationGroup {
    pub(super) id: String,
    pub(super) manifest_row: String,
    pub(super) row_origin: RowOrigin,
    pub(super) performance_feature: String,
    pub(super) checklist_anchors: Vec<String>,
    pub(super) checklist_child_ids: Vec<String>,
    pub(super) public_api_items: Vec<String>,
    pub(super) disposition: PerformanceDisposition,
    pub(super) phase: Phase,
    pub(super) runner_fidelity: RunnerFidelity,
    pub(super) correctness_cases: Vec<String>,
    pub(super) correctness_binding: CorrectnessBinding,
    pub(super) planned_correctness_case_id: Option<String>,
    pub(super) workload_family: WorkloadFamily,
    pub(super) work_unit: String,
    pub(super) output_contract: OutputContract,
    pub(super) timing_policy: TimingPolicy,
    pub(super) memory_policy: MemoryPolicy,
    pub(super) threshold_policy: ThresholdPolicy,
    pub(super) reason: String,
    pub(super) owner: String,
    pub(super) status: QualificationStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RowOrigin {
    Inherited,
    Planned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Phase {
    Startup,
    Parse,
    Compile,
    Execute,
    Convert,
    Serialize,
    Search,
    Transform,
    EndToEnd,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RunnerFidelity {
    StimPerf,
    AdapterLibrary,
    ProcessCli,
    StabReportOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CorrectnessBinding {
    ExactApiOwners,
    ExactCases,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkloadFamily {
    pub(super) fixture: FixtureLocator,
    pub(super) source: String,
    pub(super) deterministic_seed: String,
    pub(super) scales: Vec<ScalePoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum FixtureLocator {
    RepositoryFile { path: String, sha256: String },
    Generated { id: String },
    Inline { id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScalePoint {
    pub(super) id: String,
    pub(super) family_id: String,
    pub(super) size_class: SizeClass,
    pub(super) parameters: String,
    pub(super) input_bytes: InputByteCount,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) semantic_work: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) input_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SizeClass {
    Small,
    Medium,
    Large,
}

impl SizeClass {
    pub(super) fn from_scale_id(id: &str) -> Self {
        match id {
            "large" => Self::Large,
            "medium" => Self::Medium,
            _ => Self::Small,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum InputByteCount {
    Exact { bytes: u64 },
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputContract {
    pub(super) expected_shape: String,
    pub(super) digest_state: EvidenceState,
    pub(super) sink_policy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) comparator_sources: Vec<ComparatorSource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ComparatorSource {
    pub(super) path: String,
    pub(super) sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum EvidenceState {
    Existing,
    Planned,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimingPolicy {
    pub(super) batch_policy: TimingBatchPolicy,
    pub(super) calibration_min_ms: u32,
    pub(super) calibration_max_ms: u32,
    pub(super) common_wide_ratio_max_ms: u32,
    pub(super) warmup_batches: u8,
    pub(super) full_pairs: u8,
    pub(super) timeout_seconds: u32,
    pub(super) gate_statistic: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TimingBatchPolicy {
    CommonIterations,
    IndependentThroughput,
}

impl TimingBatchPolicy {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::CommonIterations => "common-iterations",
            Self::IndependentThroughput => "independent-throughput",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemoryPolicy {
    pub(super) method: MemoryMethod,
    pub(super) scale_ids: Vec<String>,
    pub(super) expected_growth: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MemoryMethod {
    ProcessRss,
    StabAllocations,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ThresholdPolicy {
    #[serde(rename = "primary-1.25")]
    Primary1_25,
    RegressionOnly,
    ReportOnly,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum QualificationStatus {
    Planned,
    Implemented,
    Qualified,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestRowDisposition {
    pub(super) id: String,
    pub(super) primary_group_id: String,
    pub(super) supporting_performance_features: Vec<String>,
    pub(super) decision: RowDecision,
    pub(super) classifications: Vec<RowClassification>,
    pub(super) stim_mapping: StimMapping,
    pub(super) threshold_refs: Vec<String>,
    pub(super) threshold_max_relative_ratio: Option<String>,
    pub(super) threshold_measurement_pairs: Vec<MeasurementPair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) replacement_contracts: Vec<ReplacementContract>,
    pub(super) waiver_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasurementPair {
    pub(super) stim_name: String,
    pub(super) stab_name: String,
    pub(super) max_relative_ratio: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReplacementContract {
    pub(super) legacy_stim_name: String,
    pub(super) legacy_stab_name: String,
    pub(super) runtime_group_id: String,
    pub(super) runtime_measurement_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) runtime_scale_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RowDecision {
    Retained,
    Reworked,
    Diagnostic,
    Superseded,
    Removed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RowClassification {
    Faithful,
    Diagnostic,
    Proxy,
    Stale,
    Duplicate,
    MissingScale,
    MissingCorrectnessPreflight,
    MissingOutputDigest,
    MissingComparator,
    InProcessProcessMismatch,
    HeterogeneousMeasurements,
    UnmatchedSubmeasurement,
    AdapterCandidate,
    NoFaithfulStimComparator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum StimMapping {
    StimPerf { source: String, filter: String },
    ProcessCli { argv: String, stdin_path: String },
    PlannedAdapter { symbol: String, source: String },
    None { reason: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpstreamPerfSource {
    pub(super) path: String,
    pub(super) symbols: Vec<String>,
    pub(super) manifest_rows: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaiverDisposition {
    pub(super) id: String,
    pub(super) waiver_files: Vec<String>,
    pub(super) reasons: Vec<WaiverReason>,
    pub(super) qualification_disposition: PerformanceDisposition,
    pub(super) retirement_mapping: String,
    pub(super) follow_up: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaiverReason {
    pub(super) waiver_file: String,
    pub(super) reason: String,
}
