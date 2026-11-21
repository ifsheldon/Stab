use std::collections::BTreeMap;
use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub(super) const SUITE_SCHEMA_VERSION: u16 = 1;
pub(super) const MAX_RELEASE_FAMILIES: usize = 12;
pub(super) const MAX_RELEASE_CASES: usize = 30;
pub(super) const PARITY_MAX_RATIO: f64 = 1.25;
pub(super) const SELF_REGRESSION_MAX_RATIO: f64 = 1.15;
pub(super) const TIMING_BOUNDARY: &str = "e2e-user-workflow-v3";
pub(super) const STIM_VERSION: &str = "v1.16.0";
pub(super) const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
pub(super) const BOOTSTRAP_SEED: u64 = 6_004_497_137_074_745_393;
pub(super) const BOOTSTRAP_RESAMPLES: usize = 10_000;
pub(super) const CONFIDENCE_LEVEL: f64 = 0.95;
pub(super) const MAXIMUM_TEMPERATURE_MILLIDEGREES: u64 = 100_000;
pub(super) const MAXIMUM_OUTPUT_BYTES: usize = 64 << 20;
pub(super) const COMMAND_TIMEOUT_SECONDS: u64 = 600;
pub(super) const MAXIMUM_CASE_RSS_BYTES: u64 = 512 << 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Suite {
    pub(super) schema_version: u16,
    pub(super) stim: StimIdentity,
    pub(super) policy: Policy,
    pub(super) tiers: BTreeMap<Tier, TierPolicy>,
    pub(super) expected_release_families: usize,
    pub(super) expected_release_cases: usize,
    #[serde(default)]
    pub(super) self_baselines: Vec<SelfBaseline>,
    pub(super) families: Vec<Family>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StimIdentity {
    pub(super) version: String,
    pub(super) commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub(super) timing_boundary: String,
    pub(super) parity_max_ratio: f64,
    pub(super) self_regression_max_ratio: f64,
    pub(super) bootstrap_seed: u64,
    pub(super) bootstrap_resamples: usize,
    pub(super) confidence_level: f64,
    pub(super) maximum_temperature_millidegrees: u64,
    pub(super) maximum_output_bytes: usize,
    pub(super) command_timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Tier {
    Smoke,
    Full,
    Soak,
}

impl fmt::Display for Tier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Smoke => "smoke",
            Self::Full => "full",
            Self::Soak => "soak",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TierPolicy {
    pub(super) warmups: usize,
    pub(super) samples: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Family {
    pub(super) id: String,
    pub(super) description: String,
    pub(super) runner: RunnerKind,
    pub(super) comparator: Comparator,
    pub(super) prerequisites: Vec<String>,
    pub(super) cases: Vec<Case>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RunnerKind {
    Cli,
    Rust,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Comparator {
    StimCli,
    SelfOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) id: String,
    pub(super) size_class: SizeClass,
    pub(super) batch: u32,
    pub(super) work: SemanticWork,
    pub(super) maximum_stab_peak_rss_bytes: u64,
    pub(super) workload: Workload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SizeClass {
    Small,
    Medium,
    Large,
    Narrow,
    Wide,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SemanticWork {
    pub(super) amount: u64,
    pub(super) unit: WorkUnit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WorkUnit {
    GeneratedBytes,
    InputBytes,
    Records,
    Shots,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Workload {
    Cli {
        args: Vec<String>,
        stdin: DataRecipe,
        #[serde(default)]
        files: Vec<FileSpec>,
        stdout: OutputContract,
    },
    CliPipeline {
        steps: Vec<PipelineStep>,
        stdout: OutputContract,
    },
    RustPipeline {
        shots: u64,
        minimum_logical_failures: u64,
        maximum_logical_failures: u64,
        seed: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PipelineStep {
    pub(super) args: Vec<String>,
    pub(super) stdin: PipelineInput,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PipelineInput {
    Empty,
    Previous,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileSpec {
    pub(super) name: String,
    pub(super) role: FileRole,
    #[serde(default)]
    pub(super) data: Option<DataRecipe>,
    #[serde(default)]
    pub(super) output: Option<OutputContract>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FileRole {
    Input,
    Output,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum DataRecipe {
    Empty,
    GeneratedCircuit {
        args: Vec<String>,
    },
    /// A generated circuit that both binaries analyze into byte-identical DEM text.
    GeneratedDem {
        circuit: Vec<String>,
        analyze: Vec<String>,
    },
    /// Records sampled once by pinned Stim from a generated circuit, so both
    /// binaries consume identical stochastic input.
    SampledRecords {
        circuit: Vec<String>,
        sample: Vec<String>,
        records: usize,
    },
    FoldedCircuit {
        qubits: usize,
        repeat_blocks: usize,
        repeat_count: u64,
        error_probability: f64,
    },
    Records {
        format: RecordFormat,
        records: usize,
        bits: usize,
        pattern: RecordPattern,
    },
    TypedDets {
        records: usize,
        detectors: usize,
        observables: usize,
        detector_hits: usize,
        observable_hits: usize,
    },
    M2dCircuit {
        bits: usize,
    },
    Dem {
        detectors: usize,
        mechanisms: usize,
        repeat_count: u64,
        error_probability: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum RecordFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RecordPattern {
    Alternating,
    Sparse,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum OutputContract {
    Exact {
        minimum_bytes: usize,
    },
    Records {
        format: RecordFormat,
        records: usize,
        bits: usize,
        minimum_one_bits: u64,
        maximum_one_fraction: f64,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelfBaseline {
    pub(super) architecture: String,
    pub(super) cpu_model: String,
    pub(super) rustc: String,
    pub(super) target: String,
    pub(super) timing_boundary: String,
    pub(super) case_id: CaseId,
    pub(super) case_digest: String,
    pub(super) median_seconds_per_work: f64,
    pub(super) upper_seconds_per_work: f64,
}

/// The `family.case` identity of one release case.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct CaseId(String);

impl CaseId {
    pub(super) fn new(family_id: &str, case_id: &str) -> Self {
        Self(format!("{family_id}.{case_id}"))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CaseId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::borrow::Borrow<str> for CaseId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Suite {
    pub(super) fn case_count(&self) -> usize {
        self.families.iter().map(|family| family.cases.len()).sum()
    }

    /// Every case with its family and identity, in source order.
    pub(super) fn cases(&self) -> impl Iterator<Item = (&Family, &Case, CaseId)> {
        self.families.iter().flat_map(|family| {
            family
                .cases
                .iter()
                .map(move |case| (family, case, CaseId::new(&family.id, &case.id)))
        })
    }

    /// Digest of the workload contract, excluding the reviewed self-regression baselines.
    pub(super) fn contract_digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(
            self.schema_version,
            &self.stim,
            &self.policy,
            &self.tiers,
            self.expected_release_families,
            self.expected_release_cases,
            &self.families,
        ))
        .map_err(|source| source.to_string())?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

impl Case {
    /// Digest binding this case's full definition to its family id.
    pub(super) fn digest(&self, family_id: &str) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(family_id, self)).map_err(|source| source.to_string())?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

mod validate;

#[cfg(test)]
mod tests;
