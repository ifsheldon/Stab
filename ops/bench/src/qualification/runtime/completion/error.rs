use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(in crate::qualification::runtime) enum CompletionError {
    #[error(transparent)]
    Artifact(#[from] super::super::artifact::ArtifactError),
    #[error(transparent)]
    Rollup(#[from] super::super::rollup::RollupError),
    #[error(transparent)]
    Parity(#[from] super::super::parity::ParityError),
    #[error(transparent)]
    SelfRegression(#[from] super::super::self_regression::SelfRegressionError),
    #[error(transparent)]
    Git(#[from] super::super::git::GitError),
    #[error(transparent)]
    Group(#[from] super::super::group::GroupError),
    #[error(transparent)]
    Correctness(#[from] super::super::correctness::CorrectnessError),
    #[error(transparent)]
    Probe(#[from] super::super::probe::ProbeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Time(#[from] std::time::SystemTimeError),
    #[error("unknown completion scope {0:?}")]
    UnknownScope(String),
    #[error("completion scope {0:?} contains no release groups")]
    EmptyScope(String),
    #[error("completion scope omits source-owned group {0}")]
    MissingScopeGroup(String),
    #[error("completion scope contains non-promotable group {0}")]
    NonPromotableScopeGroup(String),
    #[error("completion scope size overflows platform limits")]
    ScopeSizeOverflow,
    #[error("completion scope has invalid rollup count {0}")]
    RollupCount(usize),
    #[error("completion output collides with source evidence at {0}")]
    OutputCollision(PathBuf),
    #[error("completion repeats source path {0}")]
    DuplicatePath(PathBuf),
    #[error("completion repeats rollup identity {0}")]
    DuplicateRollup(String),
    #[error("completion omits rollup identity {0}")]
    MissingRollup(String),
    #[error("completion contains rollup outside its source-owned scope: {0}")]
    UnknownRollup(String),
    #[error("completion mixes repository, host, worker, or timing identities")]
    MixedIdentity,
    #[error("completion correctness evidence is missing or mismatched for group {0}")]
    GroupCorrectness(String),
    #[error("completion correctness evidence exceeds or violates the shared prerequisite contract")]
    CorrectnessArtifactCount,
    #[error("completion source report count is {actual}, expected {expected}")]
    SourceReportCount { actual: usize, expected: usize },
    #[error("completion has {0} accepted-maximum memory receipts, expected two")]
    MemoryReceiptCount(usize),
    #[error("completion repeats accepted-maximum memory receipt for {0}")]
    DuplicateMemoryReceipt(String),
    #[error("completion omits accepted-maximum memory receipt for {0}")]
    MissingMemoryReceipt(String),
    #[error("completion accepted-maximum memory receipt identity is mismatched for {0}")]
    MemoryReceiptIdentity(String),
    #[error("completion requires soft RLIMIT_NOFILE {expected}, observed {actual:?}")]
    DescriptorLimit { expected: u64, actual: Option<u64> },
    #[error("completion source report failed explicit Stim parity: {0}")]
    FailedParity(PathBuf),
    #[error("completion producer repository is dirty")]
    DirtyRepository,
    #[error("completion producer repository changed during reconstruction")]
    RepositoryChanged,
    #[error("completion schema {0} is not supported")]
    SchemaVersion(u32),
    #[error("completion artifact violates its schema or source-owned scope")]
    Boundary,
    #[error("completion artifact is not canonical JSON")]
    NonCanonical,
    #[error("completion output path does not match its manifest")]
    OutputBinding,
    #[error("completion inventories do not match current source contracts")]
    InventoryIdentity,
    #[error("completion replay does not reconstruct the checked artifacts")]
    Reconstruction,
    #[error("completion source evidence changed during replay")]
    SourceMutation,
    #[error("completion path is not UTF-8: {0}")]
    PathEncoding(PathBuf),
}
