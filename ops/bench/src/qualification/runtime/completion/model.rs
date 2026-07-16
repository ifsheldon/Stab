use serde::{Deserialize, Serialize};

use super::super::correctness::CorrectnessPreflightEvidence;
use super::super::invocation::WorkerIdentityEvidence;
use super::super::probe::AdapterProbeReceipt;
use super::super::run::{QualificationTier, RepositoryEvidence};
use super::super::statistics::GateOutcome;

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
