//! Validation for the source-owned non-deferred blocker closure ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::RepoRoot;
use digest::{computed_semantic_digest, digest_hex};
use evidence::{
    open_regular_file, read_benchmark_manifest, read_oracle_manifest, read_tracked_stim_paths,
};
use gate_contract::{
    GateContractFamily, GateContractSurface, validate_gate_contract_case,
    validate_gate_family_coverage, validate_gate_schema,
};
use oracle::validate_oracle_reference;
use provenance::validate_upstream_source;
use selector::CargoTestSelector;
use statistical::validate_statistical_plan;
use support::{validate_supporting_benchmarks, validate_supporting_oracles};

mod digest;
mod evidence;
mod gate_contract;
mod oracle;
mod provenance;
mod selector;
mod statistical;
mod support;

const SCHEMA_VERSION: u32 = 2;
const STIM_VERSION: &str = "v1.16.0";
const EXPECTED_LEDGER_DIGEST: [u8; 32] = [
    0x98, 0x18, 0x4b, 0x8e, 0x4a, 0x05, 0x3f, 0x80, 0x37, 0x08, 0x61, 0x15, 0x40, 0xe5, 0x46, 0x2e,
    0xe0, 0xff, 0x9f, 0x3c, 0xec, 0x2d, 0xdb, 0xd4, 0x75, 0x9e, 0xac, 0xee, 0x57, 0xb2, 0x42, 0xb0,
];
const MAX_LEDGER_BYTES: u64 = 1 << 20;
const MAX_MANIFEST_BYTES: u64 = 16 << 20;
const MAX_MANIFEST_ROWS: usize = 16_384;
const MAX_BLOCKERS: usize = 64;
const MAX_CASES: usize = 4_096;
const MAX_DISPLAY_BYTES: usize = 1_024;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TRACKED_PATH_BYTES: usize = crate::process::OUTPUT_LIMIT_BYTES;

#[derive(Debug, Error)]
pub(crate) enum BlockerLedgerError {
    #[error("failed to read blocker closure ledger {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("blocker closure ledger {path} is {actual} bytes; limit is {limit} bytes")]
    LedgerTooLarge {
        path: PathBuf,
        actual: u64,
        limit: u64,
    },

    #[error("failed to parse blocker closure ledger {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to read manifest {path}: {source}")]
    ReadManifest { path: PathBuf, source: csv::Error },

    #[error("failed to inspect {path}: {source}")]
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("manifest {path} is {actual} bytes; limit is {limit} bytes")]
    ManifestTooLarge {
        path: PathBuf,
        actual: u64,
        limit: u64,
    },

    #[error("manifest {path} exceeds the {limit}-row limit")]
    TooManyManifestRows { path: PathBuf, limit: usize },

    #[error("failed to list tracked Stim source files: {reason}")]
    ListTrackedStimFiles { reason: Box<str> },

    #[error("tracked Stim path list is {actual} bytes; limit is {limit} bytes")]
    TrackedStimFilesTooLarge { actual: usize, limit: usize },

    #[error("tracked Stim path list is not valid UTF-8")]
    NonUtf8TrackedStimPath,

    #[error("failed to run blocker test selector {selector}: {reason}")]
    SelectorProcess { selector: String, reason: Box<str> },

    #[error(
        "blocker test selector {selector} exited with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    SelectorFailed {
        selector: String,
        status: String,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error("evidence input is not a stable regular file: {path:?}")]
    EvidenceNotRegular { path: PathBuf },

    #[cfg(not(unix))]
    #[error("evidence file identity validation is unsupported on this target: {path:?}")]
    UnsupportedEvidenceIdentity { path: PathBuf },

    #[error("blocker test selector {selector} matched no tests for ledger cases {cases}")]
    SelectorMatchedNoTests { selector: String, cases: String },

    #[error(
        "exact blocker test selector {selector} matched {actual} tests instead of one for ledger cases {cases}"
    )]
    ExactSelectorMatchedUnexpectedCount {
        selector: String,
        cases: String,
        actual: usize,
    },

    #[error("blocker closure ledger validation failed:\n{0}")]
    Validation(Box<str>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockerLedger {
    schema_version: u32,
    stim_version: String,
    blockers: Vec<BlockerRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockerRecord {
    id: String,
    milestone: BlockerMilestone,
    disposition: BlockerDisposition,
    title: String,
    supporting_oracles: Vec<SupportingOracleReference>,
    supporting_benchmarks: Vec<SupportingBenchmarkReference>,
    cases: Vec<BlockerCase>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum BlockerMilestone {
    #[serde(rename = "PFM-B1")]
    B1,
    #[serde(rename = "PFM-B2")]
    B2,
    #[serde(rename = "PFM-B3")]
    B3,
    #[serde(rename = "PFM-B4")]
    B4,
    #[serde(rename = "PFM-B5")]
    B5,
}

impl BlockerMilestone {
    fn as_str(self) -> &'static str {
        match self {
            Self::B1 => "PFM-B1",
            Self::B2 => "PFM-B2",
            Self::B3 => "PFM-B3",
            Self::B4 => "PFM-B4",
            Self::B5 => "PFM-B5",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum BlockerDisposition {
    Implement,
    EvidenceClose,
}

impl BlockerDisposition {
    fn as_str(self) -> &'static str {
        match self {
            Self::Implement => "implement",
            Self::EvidenceClose => "evidence-close",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockerCase {
    id: String,
    surface: String,
    #[serde(default)]
    gate_surfaces: Vec<GateContractSurface>,
    #[serde(default)]
    gate_families: Vec<GateContractFamily>,
    upstream: UpstreamSource,
    comparator: ComparatorKind,
    #[serde(default)]
    statistical_plan: Option<StatisticalPlan>,
    status: CaseStatus,
    test: TestReference,
    oracle: OracleReference,
    benchmark: BenchmarkReference,
    resource_contract: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatisticalPlan {
    shots: u64,
    seed: u64,
    sigma_multiplier: f64,
    absolute_probability_floor: f64,
    familywise_false_positive_budget: f64,
    buckets: Vec<StatisticalBucket>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatisticalBucket {
    name: String,
    expected_probability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamSource {
    path: StimSourcePath,
    kind: UpstreamProvenance,
    test: String,
    subcase: String,
    #[serde(default)]
    anchors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum UpstreamProvenance {
    GtestCase,
    PytestCase,
    TestFamily,
    SourceSymbol,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
struct StimSourcePath(PathBuf);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ComparatorKind {
    Exact,
    Structural,
    Statistical,
    ErrorClass,
    SemanticInvariant,
    StateEquivalence,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum CaseStatus {
    Planned,
    Implemented,
    EvidenceClose,
}

impl CaseStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Implemented => "implemented",
            Self::EvidenceClose => "evidence-close",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestReference {
    state: EvidenceState,
    selector: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum EvidenceState {
    Existing,
    Planned,
    NotApplicable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OracleReference {
    state: EvidenceState,
    classification: OracleEvidenceClass,
    value: FixtureId,
    #[serde(default)]
    signature: Option<OracleEvidenceSignature>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupportingOracleReference {
    classification: OracleEvidenceClass,
    value: FixtureId,
    signature: OracleEvidenceSignature,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct OracleEvidenceSignature {
    parity_mode: OracleManifestParityMode,
    comparator: OracleManifestComparator,
    argv: String,
    upstream_source: StimSourcePath,
    #[serde(default)]
    stdin_path: Option<FixtureRelativeEvidencePath>,
    #[serde(default)]
    expected_stdout_path: Option<FixtureRelativeEvidencePath>,
    #[serde(default)]
    stdin_sha256: Option<Sha256Hex>,
    #[serde(default)]
    expected_stdout_sha256: Option<Sha256Hex>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct FixtureRelativeEvidencePath(PathBuf);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Sha256Hex(String);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupportingBenchmarkReference {
    classification: BenchmarkClass,
    value: BenchmarkId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum OracleEvidenceClass {
    Direct,
    PinnedGolden,
    RustTestProxy,
    Planned,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkReference {
    state: EvidenceState,
    value: String,
    #[serde(default)]
    classification: Option<BenchmarkClass>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
struct FixtureId(String);

impl FixtureId {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
struct BenchmarkId(String);

impl BenchmarkId {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum OracleManifestParityMode {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "exact-output-and-statistical")]
    ExactOutputAndStatistical,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum OracleManifestComparator {
    #[serde(rename = "exact-output")]
    ExactOutput,
    #[serde(rename = "help-health")]
    HelpHealth,
    #[serde(rename = "property")]
    Property,
    #[serde(rename = "statistical")]
    Statistical,
    #[serde(rename = "structural")]
    Structural,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum OracleManifestStatus {
    #[serde(rename = "implemented")]
    Implemented,
    #[serde(rename = "ignored")]
    Ignored,
    #[serde(rename = "manifest-only")]
    ManifestOnly,
    #[serde(rename = "red")]
    Red,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OracleRunner {
    StimCli,
    CargoTest,
    CoreFixture,
    ManifestOnly,
}

impl OracleRunner {
    fn from_argv(argv: &str) -> Option<Self> {
        let first = argv.split('|').next()?;
        match first {
            "cargo-test" => Some(Self::CargoTest),
            "core-parse-print"
            | "core-circuit-parse-print"
            | "core-dem-parse-print"
            | "core-time-reverse-flows" => Some(Self::CoreFixture),
            "manifest-only" => Some(Self::ManifestOnly),
            "--help" | "analyze_errors" | "convert" | "detect" | "gen" | "m2d" | "sample"
            | "sample_dem" => Some(Self::StimCli),
            value if value.starts_with("--gen=") || value.starts_with("--sample=") => {
                Some(Self::StimCli)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct OracleCommand {
    argv: String,
    runner: OracleRunner,
}

fn deserialize_oracle_command<'de, D>(deserializer: D) -> Result<OracleCommand, D::Error>
where
    D: Deserializer<'de>,
{
    let argv = String::deserialize(deserializer)?;
    let runner = OracleRunner::from_argv(&argv)
        .ok_or_else(|| serde::de::Error::custom(format!("unknown oracle argv runner {argv:?}")))?;
    Ok(OracleCommand { argv, runner })
}

#[derive(Debug, Deserialize)]
struct OracleManifestRow {
    id: FixtureId,
    upstream_source: StimSourcePath,
    parity_mode: OracleManifestParityMode,
    comparator: OracleManifestComparator,
    #[serde(rename = "argv", deserialize_with = "deserialize_oracle_command")]
    command: OracleCommand,
    #[serde(default)]
    stdin_path: Option<FixtureRelativeEvidencePath>,
    #[serde(default)]
    expected_stdout_path: Option<FixtureRelativeEvidencePath>,
    status: OracleManifestStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum BenchmarkRunner {
    #[serde(rename = "contract-only")]
    ContractOnly,
    #[serde(rename = "stim-cli")]
    StimCli,
    #[serde(rename = "stim-perf")]
    StimPerf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum BenchmarkThresholdClass {
    #[serde(rename = "baseline-metadata")]
    BaselineMetadata,
    #[serde(rename = "non-primary-report-only")]
    NonPrimaryReportOnly,
    #[serde(rename = "performance-gate")]
    PerformanceGate,
    #[serde(rename = "report-only")]
    ReportOnly,
}

#[derive(Debug, Deserialize)]
struct BenchmarkManifestRow {
    id: BenchmarkId,
    threshold_class: BenchmarkThresholdClass,
    runner: BenchmarkRunner,
    comparability: BenchmarkClass,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum BenchmarkClass {
    DirectMatch,
    CliBaseline,
    ContractRepresentative,
    ContractProxy,
    ContractSmoke,
    PartialMatch,
    ReportOnly,
    ContractOnly,
}

#[derive(Clone, Copy)]
struct ExpectedBlocker {
    id: &'static str,
    milestone: BlockerMilestone,
    disposition: BlockerDisposition,
    minimum_cases: usize,
}

const EXPECTED_BLOCKERS: [ExpectedBlocker; 8] = [
    ExpectedBlocker {
        id: "pfm2-qec-transforms",
        milestone: BlockerMilestone::B1,
        disposition: BlockerDisposition::Implement,
        minimum_cases: 19,
    },
    ExpectedBlocker {
        id: "pfm3-analyzer-sweep",
        milestone: BlockerMilestone::B2,
        disposition: BlockerDisposition::EvidenceClose,
        minimum_cases: 1,
    },
    ExpectedBlocker {
        id: "pfm3-gate-execution",
        milestone: BlockerMilestone::B2,
        disposition: BlockerDisposition::Implement,
        minimum_cases: 18,
    },
    ExpectedBlocker {
        id: "pfm4-dem-traversal",
        milestone: BlockerMilestone::B3,
        disposition: BlockerDisposition::Implement,
        minimum_cases: 7,
    },
    ExpectedBlocker {
        id: "pfm5-detecting-regions",
        milestone: BlockerMilestone::B4,
        disposition: BlockerDisposition::EvidenceClose,
        minimum_cases: 2,
    },
    ExpectedBlocker {
        id: "pfm5-missing-detectors",
        milestone: BlockerMilestone::B4,
        disposition: BlockerDisposition::EvidenceClose,
        minimum_cases: 14,
    },
    ExpectedBlocker {
        id: "pfm5-flow-engine",
        milestone: BlockerMilestone::B4,
        disposition: BlockerDisposition::Implement,
        minimum_cases: 33,
    },
    ExpectedBlocker {
        id: "pfm6-analyzer-search",
        milestone: BlockerMilestone::B5,
        disposition: BlockerDisposition::Implement,
        minimum_cases: 52,
    },
];

pub(crate) fn validate_and_print(
    root: &RepoRoot,
    list: bool,
    check_selectors: bool,
) -> Result<(), BlockerLedgerError> {
    let path = root.blocker_ledger();
    let ledger = BlockerLedger::read_from_path(&path)?;
    ledger.check(root)?;
    if check_selectors {
        ledger.check_existing_test_selectors(root)?;
    }
    ledger.print_summary(list);
    Ok(())
}

impl BlockerLedger {
    fn read_from_path(path: &Path) -> Result<Self, BlockerLedgerError> {
        let file = open_regular_file(path)?;
        let mut content = String::new();
        file.take(MAX_LEDGER_BYTES + 1)
            .read_to_string(&mut content)
            .map_err(|source| BlockerLedgerError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        let actual = u64::try_from(content.len()).unwrap_or(u64::MAX);
        if actual > MAX_LEDGER_BYTES {
            return Err(BlockerLedgerError::LedgerTooLarge {
                path: path.to_path_buf(),
                actual,
                limit: MAX_LEDGER_BYTES,
            });
        }
        Self::from_json(path, &content)
    }

    fn from_json(path: &Path, content: &str) -> Result<Self, BlockerLedgerError> {
        serde_json::from_str(content).map_err(|source| BlockerLedgerError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    fn check(&self, root: &RepoRoot) -> Result<(), BlockerLedgerError> {
        let oracle_rows = read_oracle_manifest(&root.fixture_manifest())?;
        let benchmark_rows = read_benchmark_manifest(&root.benchmark_manifest())?;
        let tracked_stim_paths = read_tracked_stim_paths(root)?;
        let mut violations = Vec::new();

        if self.schema_version != SCHEMA_VERSION {
            violations.push(format!(
                "schema_version is {}, expected {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.stim_version != STIM_VERSION {
            violations.push(format!(
                "stim_version is {:?}, expected {STIM_VERSION:?}",
                self.stim_version
            ));
        }
        let computed_digest = computed_semantic_digest(self);
        if computed_digest.as_slice() != EXPECTED_LEDGER_DIGEST {
            violations.push(format!(
                "blocker ledger semantic SHA-256 digest {} does not match the frozen inventory {}",
                digest_hex(computed_digest.as_slice()),
                digest_hex(&EXPECTED_LEDGER_DIGEST)
            ));
        }

        if self.blockers.len() > MAX_BLOCKERS {
            violations.push(format!(
                "ledger has {} blockers; limit is {MAX_BLOCKERS}",
                self.blockers.len()
            ));
        }
        let case_count = self
            .blockers
            .iter()
            .map(|blocker| blocker.cases.len())
            .sum::<usize>();
        if case_count > MAX_CASES {
            violations.push(format!(
                "ledger has {case_count} cases; limit is {MAX_CASES}"
            ));
        }

        let mut blocker_ids = BTreeSet::new();
        let mut case_ids = BTreeSet::new();
        validate_gate_schema(&mut violations);
        for blocker in &self.blockers {
            if !blocker_ids.insert(blocker.id.as_str()) {
                violations.push(format!("duplicate blocker id {:?}", blocker.id));
            }
            validate_identifier("blocker", &blocker.id, &mut violations);
            validate_display_text("blocker title", &blocker.title, &mut violations);
            validate_supporting_oracles(blocker, &oracle_rows, &mut violations);
            validate_supporting_benchmarks(blocker, &benchmark_rows, &mut violations);
            if blocker.cases.is_empty() {
                violations.push(format!("blocker {:?} has no owned cases", blocker.id));
            }
            if blocker.disposition == BlockerDisposition::EvidenceClose
                && blocker
                    .cases
                    .iter()
                    .any(|case| case.status != CaseStatus::EvidenceClose)
            {
                violations.push(format!(
                    "evidence-close blocker {:?} contains a non-evidence-close case",
                    blocker.id
                ));
            }
            for case in &blocker.cases {
                if !case_ids.insert(case.id.as_str()) {
                    violations.push(format!("duplicate case id {:?}", case.id));
                }
                validate_case(
                    root,
                    blocker,
                    case,
                    &oracle_rows,
                    &benchmark_rows,
                    &tracked_stim_paths,
                    &mut violations,
                );
            }
            validate_gate_family_coverage(blocker, &mut violations);
        }

        validate_expected_blockers(&self.blockers, &mut violations);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(BlockerLedgerError::Validation(
                violations.join("\n").into_boxed_str(),
            ))
        }
    }

    fn print_summary(&self, list: bool) {
        let mut selector_counts = BTreeMap::<Vec<String>, usize>::new();
        for case in self.blockers.iter().flat_map(|blocker| &blocker.cases) {
            *selector_counts
                .entry(case.test.selector.clone())
                .or_default() += 1;
        }
        let case_count = self
            .blockers
            .iter()
            .map(|blocker| blocker.cases.len())
            .sum::<usize>();
        println!(
            "[stab-oracle] blocker ledger schema={} stim={} blockers={} cases={case_count}",
            self.schema_version,
            self.stim_version,
            self.blockers.len()
        );
        for blocker in &self.blockers {
            let mut counts = BTreeMap::<CaseStatus, usize>::new();
            for case in &blocker.cases {
                *counts.entry(case.status).or_default() += 1;
            }
            let shared_selectors = blocker
                .cases
                .iter()
                .filter(|case| {
                    selector_counts
                        .get(&case.test.selector)
                        .is_some_and(|count| *count > 1)
                })
                .count();
            println!(
                "{} {} {} cases={} planned={} implemented={} evidence-close={} shared-selectors={} supporting-oracles={} supporting-benchmarks={}",
                blocker.id,
                blocker.milestone.as_str(),
                blocker.disposition.as_str(),
                blocker.cases.len(),
                counts.get(&CaseStatus::Planned).copied().unwrap_or(0),
                counts.get(&CaseStatus::Implemented).copied().unwrap_or(0),
                counts.get(&CaseStatus::EvidenceClose).copied().unwrap_or(0),
                shared_selectors,
                blocker.supporting_oracles.len(),
                blocker.supporting_benchmarks.len()
            );
            if list {
                for case in &blocker.cases {
                    let selector_is_shared = selector_counts
                        .get(&case.test.selector)
                        .is_some_and(|count| *count > 1);
                    let selector_is_exact = CargoTestSelector::parse(&case.test.selector)
                        .is_ok_and(CargoTestSelector::is_exact);
                    let selector_scope = if selector_is_shared {
                        "shared-selector"
                    } else if selector_is_exact {
                        "exact-selector"
                    } else {
                        "unique-filter"
                    };
                    println!(
                        "  {} {} {} {}",
                        case.id,
                        case.status.as_str(),
                        selector_scope,
                        case.surface
                    );
                }
            }
        }
        println!("Status: OK");
    }

    fn check_existing_test_selectors(&self, root: &RepoRoot) -> Result<(), BlockerLedgerError> {
        let mut selectors = BTreeMap::<Vec<String>, Vec<&str>>::new();
        for case in self.blockers.iter().flat_map(|blocker| &blocker.cases) {
            if case.test.state == EvidenceState::Existing {
                selectors
                    .entry(case.test.selector.clone())
                    .or_default()
                    .push(&case.id);
            }
        }

        for (selector, cases) in selectors {
            let parsed = CargoTestSelector::parse(&selector).map_err(|reason| {
                BlockerLedgerError::Validation(
                    format!("test selector {} {reason}", selector.join(" ")).into_boxed_str(),
                )
            })?;
            let display = parsed.display();
            let output =
                crate::run_process(Path::new("cargo"), parsed.args(), &[], Some(&root.path))
                    .map_err(|source| BlockerLedgerError::SelectorProcess {
                        selector: display.clone(),
                        reason: source.to_string().into_boxed_str(),
                    })?;
            if !output.success() {
                return Err(BlockerLedgerError::SelectorFailed {
                    selector: display,
                    status: crate::process::display_status(output.status),
                    stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
                    stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
                });
            }
            let stdout = String::from_utf8_lossy(&output.stdout.bytes);
            let match_count = selector::test_listing_match_count(&stdout);
            if match_count == 0 {
                return Err(BlockerLedgerError::SelectorMatchedNoTests {
                    selector: display,
                    cases: cases.join(", "),
                });
            }
            if parsed.is_exact() && match_count != 1 {
                return Err(BlockerLedgerError::ExactSelectorMatchedUnexpectedCount {
                    selector: display,
                    cases: cases.join(", "),
                    actual: match_count,
                });
            }
        }
        Ok(())
    }
}

fn validate_expected_blockers(blockers: &[BlockerRecord], violations: &mut Vec<String>) {
    for expected in EXPECTED_BLOCKERS {
        match blockers.iter().find(|blocker| blocker.id == expected.id) {
            Some(blocker) => {
                if blocker.milestone != expected.milestone {
                    violations.push(format!(
                        "blocker {:?} uses milestone {}, expected {}",
                        expected.id,
                        blocker.milestone.as_str(),
                        expected.milestone.as_str()
                    ));
                }
                if blocker.disposition != expected.disposition {
                    violations.push(format!(
                        "blocker {:?} uses disposition {}, expected {}",
                        expected.id,
                        blocker.disposition.as_str(),
                        expected.disposition.as_str()
                    ));
                }
                if blocker.cases.len() < expected.minimum_cases {
                    violations.push(format!(
                        "blocker {:?} has {} cases, expected at least {}",
                        expected.id,
                        blocker.cases.len(),
                        expected.minimum_cases
                    ));
                }
            }
            None => violations.push(format!("missing required blocker {:?}", expected.id)),
        }
    }
    if blockers.len() != EXPECTED_BLOCKERS.len() {
        violations.push(format!(
            "ledger has {} blockers, expected {}",
            blockers.len(),
            EXPECTED_BLOCKERS.len()
        ));
    }
}

fn validate_case(
    root: &RepoRoot,
    blocker: &BlockerRecord,
    case: &BlockerCase,
    oracle_rows: &BTreeMap<FixtureId, OracleManifestRow>,
    benchmark_rows: &BTreeMap<BenchmarkId, BenchmarkManifestRow>,
    tracked_stim_paths: &BTreeSet<StimSourcePath>,
    violations: &mut Vec<String>,
) {
    validate_identifier("case", &case.id, violations);
    validate_display_text("case surface", &case.surface, violations);
    validate_gate_contract_case(blocker, case, violations);
    validate_display_text("upstream test", &case.upstream.test, violations);
    validate_display_text("upstream subcase", &case.upstream.subcase, violations);
    validate_resource_contract(case, violations);
    validate_statistical_plan(case, violations);
    validate_upstream_source(root, case, tracked_stim_paths, violations);
    validate_test_reference(blocker, case, violations);
    validate_oracle_reference(root, case, oracle_rows, violations);
    validate_benchmark_reference(case, benchmark_rows, violations);

    if blocker.disposition == BlockerDisposition::Implement
        && case.status == CaseStatus::EvidenceClose
    {
        violations.push(format!(
            "implementation blocker {:?} case {:?} cannot use evidence-close status",
            blocker.id, case.id
        ));
    }
}

fn validate_identifier(kind: &str, value: &str, violations: &mut Vec<String>) {
    let valid = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        violations.push(format!("{kind} id {value:?} is not lowercase kebab-case"));
    }
}

fn validate_non_empty(label: &str, value: &str, violations: &mut Vec<String>) {
    if value.trim().is_empty() {
        violations.push(format!("{label} must not be empty"));
    }
}

fn validate_display_text(label: &str, value: &str, violations: &mut Vec<String>) {
    validate_non_empty(label, value, violations);
    if value.len() > MAX_DISPLAY_BYTES {
        violations.push(format!(
            "{label} is {} bytes; limit is {MAX_DISPLAY_BYTES}",
            value.len()
        ));
    }
    if value.chars().any(char::is_control) {
        violations.push(format!("{label} contains control characters"));
    }
}

fn validate_resource_contract(case: &BlockerCase, violations: &mut Vec<String>) {
    validate_display_text("resource contract", &case.resource_contract, violations);
    if case.resource_contract.trim().len() < 20 {
        violations.push(format!(
            "case {:?} resource_contract must describe a concrete boundary",
            case.id
        ));
    }
}

fn validate_gate_statistical_plan(
    case: &BlockerCase,
    plan: &StatisticalPlan,
    violations: &mut Vec<String>,
) {
    let Some(expected) = stab_core::__gate_contract_statistical_plans()
        .iter()
        .find(|expected| expected.case_id == case.id)
    else {
        return;
    };
    let scalar_fields_match = plan.shots == expected.shots
        && plan.seed == expected.seed
        && plan.sigma_multiplier == expected.sigma_multiplier
        && plan.absolute_probability_floor == expected.absolute_probability_floor
        && plan.familywise_false_positive_budget == expected.familywise_false_positive_budget;
    let buckets_match = plan.buckets.len() == expected.buckets.len()
        && plan
            .buckets
            .iter()
            .zip(expected.buckets)
            .all(|(actual, expected)| {
                actual.name == expected.name
                    && actual.expected_probability == expected.expected_probability
            });
    if !scalar_fields_match || !buckets_match {
        violations.push(format!(
            "case {:?} statistical plan differs from the canonical core gate contract",
            case.id
        ));
    }
}

fn validate_test_reference(
    blocker: &BlockerRecord,
    case: &BlockerCase,
    violations: &mut Vec<String>,
) {
    let expected_state = if case.status == CaseStatus::Planned {
        EvidenceState::Planned
    } else {
        EvidenceState::Existing
    };
    if case.test.state != expected_state {
        violations.push(format!(
            "case {:?} status {} requires a {:?} test selector",
            case.id,
            case.status.as_str(),
            expected_state
        ));
    }
    match CargoTestSelector::parse(&case.test.selector) {
        Ok(selector)
            if (matches!(
                blocker.milestone,
                BlockerMilestone::B1 | BlockerMilestone::B4 | BlockerMilestone::B5
            ) || blocker.id == "pfm3-gate-execution")
                && case.test.state == EvidenceState::Existing
                && !selector.is_exact() =>
        {
            violations.push(format!(
                "{} case {:?} must use an exact executable test selector",
                blocker.milestone.as_str(),
                case.id
            ));
        }
        Ok(_) => {}
        Err(reason) => violations.push(format!("case {:?} test selector {}", case.id, reason)),
    }
}

fn validate_benchmark_reference(
    case: &BlockerCase,
    benchmark_rows: &BTreeMap<BenchmarkId, BenchmarkManifestRow>,
    violations: &mut Vec<String>,
) {
    match case.benchmark.state {
        EvidenceState::Existing | EvidenceState::Planned => {
            validate_identifier("benchmark", &case.benchmark.value, violations);
            let Some(classification) = case.benchmark.classification else {
                violations.push(format!(
                    "case {:?} benchmark row lacks a comparability classification",
                    case.id
                ));
                return;
            };
            if case.benchmark.state == EvidenceState::Existing {
                let benchmark_id = BenchmarkId(case.benchmark.value.clone());
                match benchmark_rows.get(&benchmark_id) {
                    Some(row) if benchmark_class_matches_row(classification, row) => {}
                    Some(row) => violations.push(format!(
                        "case {:?} benchmark classification {:?} is incompatible with row {:?} signature ({:?}/{:?}/{:?})",
                        case.id,
                        classification,
                        row.id.as_str(),
                        row.runner,
                        row.threshold_class,
                        row.comparability
                    )),
                    None => violations.push(format!(
                        "case {:?} references missing benchmark row {:?}",
                        case.id, case.benchmark.value
                    )),
                }
            }
        }
        EvidenceState::NotApplicable => {
            if case.benchmark.classification.is_some() {
                violations.push(format!(
                    "case {:?} benchmark rationale must not have a classification",
                    case.id
                ));
            }
            if case.benchmark.value.trim().len() < 20 {
                violations.push(format!(
                    "case {:?} benchmark rationale is not specific enough",
                    case.id
                ));
            }
            validate_display_text("benchmark rationale", &case.benchmark.value, violations);
        }
    }
}

fn benchmark_class_matches_row(classification: BenchmarkClass, row: &BenchmarkManifestRow) -> bool {
    classification == row.comparability
}

#[cfg(test)]
mod tests;
