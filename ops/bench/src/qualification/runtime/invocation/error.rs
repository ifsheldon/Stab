use thiserror::Error;

use super::super::protocol::Implementation;
use super::WorkerIdentityEvidence;

#[derive(Debug, Error)]
pub(crate) enum InvocationError {
    #[error(transparent)]
    Adapter(#[from] super::super::adapter::AdapterError),
    #[error(transparent)]
    StabBuild(#[from] super::super::stab_build::StabBuildError),
    #[error(transparent)]
    Process(#[from] super::super::process::ProcessError),
    #[error(transparent)]
    Protocol(#[from] super::super::protocol::ProtocolError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Group(#[from] super::super::group::GroupError),
    #[error(transparent)]
    Git(#[from] super::super::git::GitError),
    #[error(transparent)]
    Toolchain(#[from] super::super::toolchain::ToolchainError),
    #[error(
        "private worker reproducibility requires a clean checkout before and after both builds"
    )]
    DirtyReproducibilityRepository,
    #[error("private worker reproducibility checkout changed from {before} to {after}")]
    ReproducibilityRepositoryChanged { before: String, after: String },
    #[error(
        "private Stim or Stab worker builds produced different identities: first={first:?}, second={second:?}"
    )]
    NonReproducibleWorkers {
        first: Box<WorkerIdentityEvidence>,
        second: Box<WorkerIdentityEvidence>,
    },
    #[error("qualification runtime group is not implemented by both workers: {0}")]
    UnsupportedGroup(String),
    #[error("qualification runtime group {0} does not match the materialized comparator sources")]
    ComparatorSourceContract(String),
    #[error("qualification CPU {0} exceeds the shared worker protocol")]
    CpuRange(usize),
    #[error("qualification workers were invoked before selecting a host-policy CPU")]
    MissingCpu,
    #[error("qualification workers lack the mandatory canonical contract preflight")]
    MissingContractPreflight,
    #[error("the source-owned worker contract preflight digest is stale")]
    ContractPreflightDefinition,
    #[error("qualification parent semantic work count overflows u64")]
    WorkOverflow,
    #[error(
        "{implementation} qualification worker failed with status {status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    WorkerFailed {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("{implementation} qualification worker emitted unexpected stderr: {stderr}")]
    UnexpectedStderr {
        implementation: Implementation,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the first unsupported circuit-parse scale before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    CapRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject a partial gate-name-hash sweep before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    GatePartialSweepRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the first unsupported simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountCapRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject an unaligned simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountAlignmentRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject a below-minimum simd-word-popcount width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PopcountMinimumRejection {
        implementation: Implementation,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the {class} simd-bits-xor width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    DenseXorWidthRejection {
        implementation: Implementation,
        class: &'static str,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the {class} simd-bits-not-zero width before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    NotZeroWidthRejection {
        implementation: Implementation,
        class: &'static str,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the {class} sparse-XOR work count before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    SparseXorWorkRejection {
        implementation: Implementation,
        class: &'static str,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the {class} bit-matrix transpose work count before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    BitMatrixTransposeWorkRejection {
        implementation: Implementation,
        class: &'static str,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "{implementation} did not reject the {class} Pauli multiplication invocation before the start barrier; status={status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    PauliWorkRejection {
        implementation: Implementation,
        class: &'static str,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("qualification invocation returned no measurement")]
    MissingMeasurement,
    #[error("qualification worker measured invalid duration {0}")]
    InvalidMeasuredDuration(f64),
    #[error("qualification process recorded invalid wall duration {0}")]
    InvalidWallDuration(f64),
}
