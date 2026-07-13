use std::fmt::{self, Display};
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};

use clap::ValueEnum;
use serde::de::{Error as _, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(super) const SCHEMA_VERSION: u32 = 3;

const MAX_CASE_ID_BYTES: usize = 128;
const MAX_API_PATH_BYTES: usize = 1_024;
const MAX_SOURCE_PATH_BYTES: usize = 1_024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct CaseId(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StableCaseDomain {
    ApiItem,
    EvidenceApi,
    EvidenceBlocker,
    EvidenceOracle,
    EvidenceResource,
    EvidenceUpstream,
    UpstreamBlocker,
    UpstreamCpp,
    UpstreamPython,
}

impl StableCaseDomain {
    const fn prefix(self) -> &'static str {
        match self {
            Self::ApiItem => "cq-api-item",
            Self::EvidenceApi => "cq-evidence-api",
            Self::EvidenceBlocker => "cq-evidence-blocker",
            Self::EvidenceOracle => "cq-evidence-oracle",
            Self::EvidenceResource => "cq-evidence-resource",
            Self::EvidenceUpstream => "cq-evidence-upstream",
            Self::UpstreamBlocker => "cq-upstream-blocker",
            Self::UpstreamCpp => "cq-upstream-cpp",
            Self::UpstreamPython => "cq-upstream-py",
        }
    }
}

impl CaseId {
    pub(super) fn try_new(value: String) -> Result<Self, &'static str> {
        let valid = !value.is_empty()
            && value.len() <= MAX_CASE_ID_BYTES
            && !value.starts_with('-')
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        valid.then_some(Self(value)).ok_or(
            "case id must be lowercase kebab-case without empty components and at most 128 bytes",
        )
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn from_stable_suffix(domain: StableCaseDomain, suffix: &str) -> Self {
        Self(format!("{}-{suffix}", domain.prefix()))
    }
}

impl Display for CaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for CaseId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct ApiPath(String);

impl ApiPath {
    pub(super) fn try_new(value: String) -> Result<Self, &'static str> {
        if value.is_empty()
            || value.len() > MAX_API_PATH_BYTES
            || value.chars().any(char::is_control)
        {
            Err("API path must be nonempty, control-free, and at most 1024 bytes")
        } else {
            Ok(Self(value))
        }
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ApiPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for ApiPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct RelativeSourcePath(PathBuf);

impl RelativeSourcePath {
    pub(super) fn try_new(value: PathBuf) -> Result<Self, &'static str> {
        let Some(text) = value.to_str() else {
            return Err("source path must be valid UTF-8");
        };
        if text.is_empty()
            || text.len() > MAX_SOURCE_PATH_BYTES
            || text.contains('\\')
            || text.chars().any(char::is_control)
            || value.is_absolute()
            || value
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            Err(
                "source path must be a bounded UTF-8 relative path with slash-separated normal components",
            )
        } else {
            Ok(Self(value))
        }
    }

    pub(super) fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Display for RelativeSourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for RelativeSourcePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(PathBuf::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SemanticDigest([u8; 32]);

impl SemanticDigest {
    pub(super) const ZERO: Self = Self([0; 32]);

    pub(super) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(super) fn parse(value: &str) -> Result<Self, &'static str> {
        if value.len() != 64 {
            return Err("semantic digest must contain exactly 64 lowercase hexadecimal digits");
        }
        let mut bytes = [0u8; 32];
        for (slot, pair) in bytes.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
            let [high, low] = pair else {
                return Err("semantic digest must contain complete byte pairs");
            };
            let high = decode_lower_hex(*high)?;
            let low = decode_lower_hex(*low)?;
            *slot = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

impl Display for SemanticDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for SemanticDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SemanticDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

fn decode_lower_hex(value: u8) -> Result<u8, &'static str> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("semantic digest must contain exactly 64 lowercase hexadecimal digits"),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
pub(super) enum FeatureId {
    #[serde(rename = "CQ-STIM-FORMAT")]
    StimFormat,
    #[serde(rename = "CQ-DEM-FORMAT")]
    DemFormat,
    #[serde(rename = "CQ-RESULT-FORMATS")]
    ResultFormats,
    #[serde(rename = "CQ-GATE-CONTRACT")]
    GateContract,
    #[serde(rename = "CQ-BIT-KERNELS")]
    BitKernels,
    #[serde(rename = "CQ-CIRCUIT-API")]
    CircuitApi,
    #[serde(rename = "CQ-GENERATION")]
    Generation,
    #[serde(rename = "CQ-ALGEBRA")]
    Algebra,
    #[serde(rename = "CQ-SAMPLING")]
    Sampling,
    #[serde(rename = "CQ-DETECTION")]
    Detection,
    #[serde(rename = "CQ-DEM-SAMPLING")]
    DemSampling,
    #[serde(rename = "CQ-ANALYZER")]
    Analyzer,
    #[serde(rename = "CQ-SEARCH")]
    Search,
    #[serde(rename = "CQ-FLOW-UTILS")]
    FlowUtils,
    #[serde(rename = "CQ-CLI")]
    Cli,
    #[serde(rename = "CQ-RESOURCE")]
    Resource,
}

impl FeatureId {
    pub(super) const ALL: [Self; 16] = [
        Self::StimFormat,
        Self::DemFormat,
        Self::ResultFormats,
        Self::GateContract,
        Self::BitKernels,
        Self::CircuitApi,
        Self::Generation,
        Self::Algebra,
        Self::Sampling,
        Self::Detection,
        Self::DemSampling,
        Self::Analyzer,
        Self::Search,
        Self::FlowUtils,
        Self::Cli,
        Self::Resource,
    ];

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::StimFormat => "CQ-STIM-FORMAT",
            Self::DemFormat => "CQ-DEM-FORMAT",
            Self::ResultFormats => "CQ-RESULT-FORMATS",
            Self::GateContract => "CQ-GATE-CONTRACT",
            Self::BitKernels => "CQ-BIT-KERNELS",
            Self::CircuitApi => "CQ-CIRCUIT-API",
            Self::Generation => "CQ-GENERATION",
            Self::Algebra => "CQ-ALGEBRA",
            Self::Sampling => "CQ-SAMPLING",
            Self::Detection => "CQ-DETECTION",
            Self::DemSampling => "CQ-DEM-SAMPLING",
            Self::Analyzer => "CQ-ANALYZER",
            Self::Search => "CQ-SEARCH",
            Self::FlowUtils => "CQ-FLOW-UTILS",
            Self::Cli => "CQ-CLI",
            Self::Resource => "CQ-RESOURCE",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.as_str() == value)
    }

    pub(super) const fn performance_groups(self) -> &'static [&'static str] {
        match self {
            Self::StimFormat => &["PERF-CIRCUIT-MODEL"],
            Self::DemFormat => &["PERF-DEM-MODEL"],
            Self::ResultFormats => &["PERF-RESULT-IO", "PERF-CONVERT-CLI"],
            Self::GateContract => &[
                "PERF-GATE-CONTRACT",
                "PERF-SAMPLING",
                "PERF-DETECTION",
                "PERF-ERROR-ANALYSIS",
            ],
            Self::BitKernels => &["PERF-BIT-KERNELS"],
            Self::CircuitApi => &["PERF-CIRCUIT-MODEL", "PERF-FLOWS-AND-DETECTOR-UTILITIES"],
            Self::Generation => &["PERF-GENERATION"],
            Self::Algebra => &["PERF-STABILIZER-ALGEBRA"],
            Self::Sampling => &["PERF-SAMPLING"],
            Self::Detection => &["PERF-DETECTION"],
            Self::DemSampling => &["PERF-DEM-SAMPLING"],
            Self::Analyzer => &["PERF-ERROR-ANALYSIS"],
            Self::Search => &["PERF-SEARCH-AND-MATCHING"],
            Self::FlowUtils => &["PERF-FLOWS-AND-DETECTOR-UTILITIES"],
            Self::Cli => &["PERF-CLI-STARTUP-AND-ERRORS"],
            Self::Resource => &["PERF-RESOURCE-BOUNDARIES"],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationManifest {
    pub(super) schema_version: u32,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) stim_version: String,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) stim_commit: String,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) rust_toolchain: String,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) python_ast_version: String,
    pub(super) semantic_digest: SemanticDigest,
    #[serde(deserialize_with = "deserialize_vec_16")]
    pub(super) features: Vec<FeatureRecord>,
    #[serde(deserialize_with = "deserialize_vec_8192")]
    pub(super) upstream_cases: Vec<UpstreamCase>,
    #[serde(deserialize_with = "deserialize_vec_8192")]
    pub(super) public_api_items: Vec<PublicApiItem>,
    #[serde(deserialize_with = "deserialize_vec_8192")]
    pub(super) evidence_cases: Vec<EvidenceCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FeatureRecord {
    pub(super) id: FeatureId,
    #[serde(deserialize_with = "deserialize_text_vec_16")]
    pub(super) performance_groups: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpstreamCase {
    pub(super) id: CaseId,
    pub(super) path: RelativeSourcePath,
    pub(super) provenance: UpstreamProvenance,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) symbol: String,
    #[serde(deserialize_with = "deserialize_optional_text")]
    pub(super) subcase: Option<String>,
    pub(super) parameterization: Parameterization,
    pub(super) line: u32,
    #[serde(deserialize_with = "deserialize_vec_16")]
    pub(super) domain_ids: Vec<FeatureId>,
    pub(super) disposition: UpstreamDisposition,
    pub(super) deferred_product: Option<DeferredProduct>,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) reason: String,
    #[serde(deserialize_with = "deserialize_vec_16")]
    pub(super) ownerships: Vec<UpstreamOwnership>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Parameterization {
    None,
    StaticSubcase,
    DynamicFamily,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpstreamOwnership {
    pub(super) feature_id: FeatureId,
    pub(super) comparator: Comparator,
    pub(super) owner_case_id: CaseId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum DeferredProduct {
    Crumble,
    DeprecatedDetectorHypergraph,
    Diagrams,
    ExplainErrors,
    InteractiveSimulators,
    PythonBindings,
    Qasm,
    Quirk,
    Sinter,
    Stimcirq,
    Stimflow,
    ZxAndLatticeSurgery,
}

impl DeferredProduct {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Crumble => "crumble",
            Self::DeprecatedDetectorHypergraph => "deprecated-detector-hypergraph",
            Self::Diagrams => "diagrams",
            Self::ExplainErrors => "explain-errors",
            Self::InteractiveSimulators => "interactive-simulators",
            Self::PythonBindings => "python-bindings",
            Self::Qasm => "qasm",
            Self::Quirk => "quirk",
            Self::Sinter => "sinter",
            Self::Stimcirq => "stimcirq",
            Self::Stimflow => "stimflow",
            Self::ZxAndLatticeSurgery => "zx-and-lattice-surgery",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpstreamProvenance {
    GtestCase,
    PytestCase,
    SourceSymbol,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpstreamDisposition {
    ExactOracle,
    PortedRust,
    SemanticMining,
    DeferredProduct,
    NotApplicable,
    Superseded,
}

impl UpstreamDisposition {
    pub(super) const fn is_executable_scope(self) -> bool {
        matches!(
            self,
            Self::ExactOracle | Self::PortedRust | Self::SemanticMining
        )
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ExactOracle => "exact-oracle",
            Self::PortedRust => "ported-rust",
            Self::SemanticMining => "semantic-mining",
            Self::DeferredProduct => "deferred-product",
            Self::NotApplicable => "not-applicable",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Comparator {
    ExactBytes,
    ExactValue,
    Canonical,
    ErrorClass,
    Structural,
    StateEquivalence,
    SemanticInvariant,
    Statistical,
    Property,
    Resource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicApiItem {
    pub(super) id: CaseId,
    pub(super) feature_id: FeatureId,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) crate_name: String,
    pub(super) path: ApiPath,
    pub(super) kind: PublicApiKind,
    pub(super) source_path: RelativeSourcePath,
    pub(super) source_line: u32,
    pub(super) owner_case_id: CaseId,
    #[serde(deserialize_with = "deserialize_text_vec_16")]
    pub(super) performance_groups: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PublicApiKind {
    Constant,
    Enum,
    Field,
    Function,
    Macro,
    Method,
    Module,
    Static,
    Struct,
    Trait,
    TraitImpl,
    TraitMethod,
    TypeAlias,
    Union,
    Variant,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceCase {
    pub(super) id: CaseId,
    pub(super) feature_id: FeatureId,
    pub(super) behavioral_surface: BehavioralSurface,
    pub(super) provenance: EvidenceProvenance,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) source_id: String,
    pub(super) comparator: Comparator,
    pub(super) execution: ExecutionContract,
    pub(super) statistical_plan: Option<StatisticalPlanRef>,
    pub(super) property_plan: Option<PropertyPlanRef>,
    pub(super) primary_selector: EvidenceSelector,
    #[serde(deserialize_with = "deserialize_vec_64")]
    pub(super) supporting_selectors: Vec<EvidenceSelector>,
    pub(super) resource_contract: ResourceContract,
    #[serde(deserialize_with = "deserialize_text_vec_64")]
    pub(super) negative_axes: Vec<String>,
    #[serde(deserialize_with = "deserialize_text_vec_16")]
    pub(super) performance_groups: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) deferred_product: Option<DeferredProduct>,
    pub(super) status: EvidenceStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionContract {
    #[serde(deserialize_with = "deserialize_vec_16")]
    pub(super) tiers: Vec<ExecutionTier>,
    pub(super) timeout_ms: u64,
    pub(super) stdout_limit_bytes: usize,
    pub(super) stderr_limit_bytes: usize,
    pub(super) artifact_limit_bytes: usize,
    pub(super) expected_skip: ExpectedSkip,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExecutionTier {
    Pr,
    Full,
    Soak,
}

impl ExecutionTier {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Pr => "pr",
            Self::Full => "full",
            Self::Soak => "soak",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ExpectedSkip {
    Never,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum EvidenceProvenance {
    UpstreamSemanticCase,
    PublicRustApi,
    OracleFixture,
    RustRegression,
    BlockerLedger,
    QualificationPlan,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BehavioralSurface {
    RustApi,
    Cli,
    FileFormat,
    Engine,
    ResourceBoundary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatisticalPlanRef {
    pub(super) state: EvidenceState,
    pub(super) source: StatisticalPlanSource,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum StatisticalPlanSource {
    QualificationCase,
    OracleFixture,
    BlockerLedger,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PropertyPlanRef {
    pub(super) state: EvidenceState,
    pub(super) source: PropertyPlanSource,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) id: String,
    pub(super) plan: Option<PropertyExecutionPlan>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PropertyPlanSource {
    QualificationCase,
    OracleFixture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PropertyExecutionPlan {
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) generator_domain: String,
    pub(super) maximum_generated_bytes: usize,
    #[serde(deserialize_with = "deserialize_vec_64")]
    pub(super) seeds: Vec<u64>,
    pub(super) case_count: u32,
    pub(super) corpus_path: Option<RelativeSourcePath>,
    pub(super) corpus_sha256: Option<SemanticDigest>,
    pub(super) persistence_policy: PropertyPersistencePolicy,
    pub(super) execution_mode: PropertyExecutionMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PropertyPersistencePolicy {
    ExistingFocusedRegression,
    PersistMinimizedRegression,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PropertyExecutionMode {
    CargoSubprocess,
    QualificationWorkerSubprocess,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceSelector {
    pub(super) state: EvidenceState,
    pub(super) kind: SelectorKind,
    #[serde(deserialize_with = "deserialize_text_vec_16")]
    pub(super) value: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum EvidenceState {
    Existing,
    Planned,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SelectorKind {
    CargoTest,
    OracleFixture,
    OpsCheck,
    PropertyTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourceContract {
    pub(super) kind: ResourceKind,
    #[serde(deserialize_with = "deserialize_text")]
    pub(super) detail: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ResourceKind {
    Streaming,
    BoundedMaterialized,
    BoundedSearch,
    ConstantScratch,
    NotApplicable,
}

impl ResourceKind {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Streaming => "streaming",
            Self::BoundedMaterialized => "bounded-materialized",
            Self::BoundedSearch => "bounded-search",
            Self::ConstantScratch => "constant-scratch",
            Self::NotApplicable => "not-applicable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum EvidenceStatus {
    Planned,
    Implemented,
    EvidenceClose,
    Deferred,
}

struct BoundedText(String);

impl<'de> Deserialize<'de> for BoundedText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoundedTextVisitor;

        impl Visitor<'_> for BoundedTextVisitor {
            type Value = BoundedText;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "text containing at most 2048 bytes")
            }

            fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                validate_deserialized_text(value).map_err(E::custom)?;
                Ok(BoundedText(value.to_string()))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_borrowed_str(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                validate_deserialized_text(&value).map_err(E::custom)?;
                Ok(BoundedText(value))
            }
        }

        deserializer.deserialize_string(BoundedTextVisitor)
    }
}

fn validate_deserialized_text(value: &str) -> Result<(), &'static str> {
    if value.len() > 2_048 {
        Err("text exceeds the 2048-byte qualification manifest limit")
    } else if value.chars().any(char::is_control) {
        Err("qualification manifest text contains control characters")
    } else {
        Ok(())
    }
}

fn deserialize_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    BoundedText::deserialize(deserializer).map(|value| value.0)
}

fn deserialize_optional_text<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoundedText>::deserialize(deserializer).map(|value| value.map(|value| value.0))
}

fn deserialize_text_vec_16<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BoundedText, 16>(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

fn deserialize_text_vec_64<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BoundedText, 64>(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

fn deserialize_vec_16<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserialize_bounded_vec::<D, T, 16>(deserializer)
}

fn deserialize_vec_64<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserialize_bounded_vec::<D, T, 64>(deserializer)
}

fn deserialize_vec_8192<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserialize_bounded_vec::<D, T, 8_192>(deserializer)
}

fn deserialize_bounded_vec<'de, D, T, const LIMIT: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T, const LIMIT: usize>(PhantomData<T>);

    impl<'de, T, const LIMIT: usize> Visitor<'de> for BoundedVecVisitor<T, LIMIT>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a sequence containing at most {LIMIT} entries")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|size| size > LIMIT) {
                return Err(A::Error::custom(format!(
                    "sequence contains more than {LIMIT} entries"
                )));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(LIMIT));
            while let Some(value) = sequence.next_element()? {
                if values.len() == LIMIT {
                    return Err(A::Error::custom(format!(
                        "sequence contains more than {LIMIT} entries"
                    )));
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor::<T, LIMIT>(PhantomData))
}
