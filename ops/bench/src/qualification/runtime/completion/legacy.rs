#![allow(
    dead_code,
    reason = "historical completion schema fields remain for read-only parsing"
)]

use serde::{Deserialize, Serialize};

use super::super::correctness::CorrectnessPreflightEvidence;
use super::super::invocation::WorkerIdentityEvidence;
use super::super::probe::AdapterProbeReceipt;
use super::super::run::{QualificationTier, RepositoryEvidence};
use super::super::statistics::GateOutcome;
use super::{
    CompletionCorrectness, CompletionMemory, CompletionRegression, CompletionRollup,
    CompletionSourceReport, MemoryScalingStatus,
};
use crate::qualification::runtime::protocol::TimingBoundary;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CompletionStepKind {
    WorkerReproducibility,
    AdapterProbe,
    ReportReplay,
    Regression,
    RollupReplay,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactReceipt {
    pub(super) path: String,
    pub(super) name: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceDirectoryReceipt {
    pub(super) tier: QualificationTier,
    pub(super) scale_id: Option<String>,
    pub(super) path: String,
    pub(super) artifacts: Vec<ArtifactReceipt>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum CompletionStepResult {
    WorkerReproducibility {
        workers: WorkerIdentityEvidence,
    },
    AdapterProbe {
        probe: AdapterProbeReceipt,
    },
    ReportReplay {
        tier: QualificationTier,
        scale_id: String,
    },
    Regression {
        group_id: String,
        checked_measurements: usize,
        report_only: bool,
    },
    RollupReplay {
        tier: QualificationTier,
        scale_count: usize,
        overall_outcome: GateOutcome,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionStep {
    pub(super) index: usize,
    pub(super) kind: CompletionStepKind,
    pub(super) repository_commit: String,
    pub(super) canonical_arguments: Vec<String>,
    pub(super) inputs: Vec<ArtifactReceipt>,
    pub(super) exit_status: i32,
    pub(super) outputs: Vec<ArtifactReceipt>,
    pub(super) result: CompletionStepResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionEnvironmentEvidence {
    pub(super) host_policy_sha256: String,
    pub(super) host_profile_id: String,
    pub(super) architecture: String,
    pub(super) cpu_identity: String,
    pub(super) target_triple: String,
    pub(super) toolchain_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionReceipt {
    pub(super) schema_version: u32,
    pub(super) output: String,
    pub(super) generated_unix_epoch_seconds: u64,
    pub(super) group_id: String,
    pub(super) group_contract_sha256: String,
    pub(super) performance_inventory_sha256: String,
    pub(super) correctness_inventory_sha256: String,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) repository: RepositoryEvidence,
    pub(super) environment: CompletionEnvironmentEvidence,
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) correctness_preflight: CorrectnessPreflightEvidence,
    pub(super) source_reports: Vec<EvidenceDirectoryReceipt>,
    pub(super) rollups: Vec<EvidenceDirectoryReceipt>,
    pub(super) steps: Vec<CompletionStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionPreflight {
    pub(super) schema_version: u32,
    pub(super) report_sha256: String,
    pub(super) output: String,
    pub(super) group_id: String,
    pub(super) performance_inventory_sha256: String,
    pub(super) correctness_inventory_sha256: String,
    pub(super) stab_commit: String,
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) source_reports: Vec<EvidenceDirectoryReceipt>,
    pub(super) rollups: Vec<EvidenceDirectoryReceipt>,
    pub(super) step_count: usize,
    pub(super) steps_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LegacyCompletionSummary {
    pub(super) output: String,
    pub(super) group_id: String,
    pub(super) performance_inventory_sha256: String,
    pub(super) correctness_inventory_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionManifestV2 {
    schema_version: u32,
    output: String,
    generated_unix_epoch_seconds: u64,
    scope_id: String,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    parity_policy_sha256: String,
    regression_policy_sha256: String,
    regression_baselines_sha256: String,
    stim_tag: String,
    stim_commit: String,
    repository: RepositoryEvidence,
    environment: CompletionEnvironmentV2,
    workers: WorkerIdentityEvidence,
    timing_boundary: TimingBoundary,
    correctness_preflight: CorrectnessPreflightEvidence,
    rollups: Vec<CompletionRollup>,
    source_reports: Vec<CompletionSourceReport>,
    memory: Vec<CompletionMemory>,
    parity_outcome: GateOutcome,
    regression_outcomes: Vec<CompletionRegression>,
    environment_valid: bool,
    memory_scaling_status: MemoryScalingStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionEnvironmentV2 {
    host_policy_sha256: String,
    host_profile_id: String,
    operating_system: String,
    architecture: String,
    cpu_identity: String,
    rust_toolchain: String,
    target_triple: String,
    toolchain_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompletionManifestV3 {
    schema_version: u32,
    output: String,
    generated_unix_epoch_seconds: u64,
    scope_id: String,
    performance_inventory_sha256: String,
    correctness_inventory_sha256: String,
    parity_policy_sha256: String,
    regression_policy_sha256: String,
    regression_baselines_sha256: String,
    stim_tag: String,
    stim_commit: String,
    repository: RepositoryEvidence,
    environment: CompletionEnvironmentV2,
    workers: WorkerIdentityEvidence,
    timing_boundary: TimingBoundary,
    correctness_preflights: Vec<CompletionCorrectness>,
    rollups: Vec<CompletionRollup>,
    source_reports: Vec<CompletionSourceReport>,
    memory: Vec<CompletionMemory>,
    parity_outcome: GateOutcome,
    regression_outcomes: Vec<CompletionRegression>,
    environment_valid: bool,
    memory_scaling_status: MemoryScalingStatus,
}

pub(super) fn parse_v1(bytes: &[u8]) -> Result<LegacyCompletionSummary, serde_json::Error> {
    let receipt: CompletionReceipt = serde_json::from_slice(bytes)?;
    Ok(LegacyCompletionSummary {
        output: receipt.output,
        group_id: receipt.group_id,
        performance_inventory_sha256: receipt.performance_inventory_sha256,
        correctness_inventory_sha256: receipt.correctness_inventory_sha256,
    })
}

pub(super) fn parse_v2(bytes: &[u8]) -> Result<LegacyCompletionSummary, serde_json::Error> {
    let receipt: CompletionManifestV2 = serde_json::from_slice(bytes)?;
    Ok(LegacyCompletionSummary {
        output: receipt.output,
        group_id: receipt.scope_id,
        performance_inventory_sha256: receipt.performance_inventory_sha256,
        correctness_inventory_sha256: receipt.correctness_inventory_sha256,
    })
}

pub(super) fn parse_v3(bytes: &[u8]) -> Result<LegacyCompletionSummary, serde_json::Error> {
    let receipt: CompletionManifestV3 = serde_json::from_slice(bytes)?;
    Ok(LegacyCompletionSummary {
        output: receipt.output,
        group_id: receipt.scope_id,
        performance_inventory_sha256: receipt.performance_inventory_sha256,
        correctness_inventory_sha256: receipt.correctness_inventory_sha256,
    })
}
