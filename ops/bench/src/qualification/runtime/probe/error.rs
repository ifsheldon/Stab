use thiserror::Error;

#[derive(Debug, Error)]
pub(in crate::qualification::runtime) enum ProbeError {
    #[error(transparent)]
    Artifact(#[from] super::super::artifact::ArtifactError),
    #[error(transparent)]
    Adapter(#[from] super::super::adapter::AdapterError),
    #[error(transparent)]
    Git(#[from] super::super::git::GitError),
    #[error(transparent)]
    Worker(#[from] super::super::worker::WorkerError),
    #[error(transparent)]
    Process(#[from] super::super::process::ProcessError),
    #[error(transparent)]
    Protocol(#[from] super::super::protocol::ProtocolError),
    #[error(transparent)]
    Statistics(#[from] super::super::statistics::StatisticsError),
    #[error(transparent)]
    Invocation(#[from] super::super::invocation::InvocationError),
    #[error(transparent)]
    StabBuild(#[from] super::super::stab_build::StabBuildError),
    #[error(transparent)]
    Toolchain(#[from] super::super::toolchain::ToolchainError),
    #[error(transparent)]
    Host(#[from] super::super::host::HostError),
    #[error("failed to resolve the current Stab qualification worker: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("Stab qualification worker identity changed during the probe")]
    WorkerIdentityChanged,
    #[error("qualification probe publication requires a clean repository")]
    DirtyRepository,
    #[error("qualification probe repository changed from {before} to {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("DEM accepted-maximum memory receipt is incomplete or malformed")]
    MemoryReceipt,
    #[error("qualification probe semantic work count overflows u64")]
    WorkOverflow,
    #[error("failed to serialize the qualification probe receipt: {0}")]
    Json(#[from] serde_json::Error),
    #[error("qualification probe contract failed: {0}")]
    Contract(String),
}
