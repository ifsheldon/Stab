use serde::{Deserialize, Serialize};

pub(super) const SCHEMA_VERSION: u32 = 5;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationSuite {
    pub(super) schema_version: u32,
    pub(super) stim_version: String,
    pub(super) stim_commit: String,
    pub(super) correctness_digest: String,
    pub(super) semantic_digest: String,
    pub(super) performance_features: Vec<PerformanceFeature>,
    pub(super) manifest_rows: Vec<ManifestRowDisposition>,
    pub(super) upstream_perf_sources: Vec<UpstreamPerfSource>,
    pub(super) waiver_rows: Vec<WaiverDisposition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PerformanceFeature {
    pub(super) id: String,
    pub(super) correctness_features: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PerformanceDisposition {
    CoveredByParent,
    FutureCandidate,
    NotPerformanceRelevant,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RowOrigin {
    Inherited,
    Planned,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SizeClass {
    Small,
    Medium,
    Large,
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
pub(super) struct ManifestRowDisposition {
    pub(super) id: String,
    pub(super) performance_feature: String,
    pub(super) supporting_performance_features: Vec<String>,
    pub(super) disposition: PerformanceDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) runtime_group_id: Option<String>,
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
    pub(super) policies: Vec<WaiverSourcePolicy>,
    pub(super) qualification_disposition: PerformanceDisposition,
    pub(super) retirement_mapping: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaiverSourcePolicy {
    pub(super) waiver_file: String,
    pub(super) kind: WaiverKind,
    pub(super) reason: String,
    pub(super) follow_up: String,
    pub(super) measurement_pairs: Vec<WaiverPair>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WaiverKind {
    NoComparableBaseline,
    UnstableFaithfulPairs,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WaiverPair {
    pub(super) stim_name: String,
    pub(super) stab_name: String,
}
