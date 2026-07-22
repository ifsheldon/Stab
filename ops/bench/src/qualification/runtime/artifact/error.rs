use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ArtifactError {
    #[error("performance qualification artifact publication requires Linux")]
    UnsupportedHost,
    #[error(
        "qualification output must be a normal repository-relative directory below target/benchmarks/qualification: {0}"
    )]
    InvalidOutput(PathBuf),
    #[error("qualification artifact name is not source-owned: {0}")]
    InvalidArtifactName(&'static str),
    #[error("qualification artifact {name} is {actual} bytes, exceeding {maximum}")]
    ArtifactTooLarge {
        name: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("qualification output contains unexpected existing artifacts: {0:?}")]
    UnexpectedExistingArtifacts(BTreeSet<OsString>),
    #[error("qualification staging output contains unexpected artifacts: {0:?}")]
    UnexpectedStagedArtifacts(BTreeSet<OsString>),
    #[error("qualification staging output contains a duplicate artifact: {0}")]
    DuplicateStagedArtifact(&'static str),
    #[error("qualification staging output is no longer active")]
    InactiveStaging,
    #[error(
        "qualification bound directory artifact set changed: expected {expected:?}, actual {actual:?}"
    )]
    BoundArtifactSetChanged {
        expected: BTreeSet<OsString>,
        actual: BTreeSet<OsString>,
    },
    #[error("qualification output contains too many existing artifacts")]
    TooManyExistingArtifacts,
    #[error("qualification producer output already exists and cannot be replaced: {0}")]
    OutputAlreadyExists(PathBuf),
    #[error("failed to reserve a unique qualification staging directory")]
    NoStagingName,
    #[error("qualification artifact filesystem operation failed: {0}")]
    Io(rustix::io::Errno),
    #[error("qualification artifact directory identity check failed: {0}")]
    DirectoryIdentity(&'static str),
    #[error("qualification repository root is not the bound live absolute directory")]
    RepositoryIdentity,
    #[error("qualification artifact publication changed state and could not be rolled back")]
    PublicationRollback,
    #[error("qualification artifact write failed: {0}")]
    Write(std::io::Error),
    #[error("{write}; qualification staging cleanup also failed: {cleanup}")]
    WriteCleanup {
        write: Box<ArtifactError>,
        cleanup: Box<ArtifactError>,
    },
    #[error("qualification artifact is not a bounded regular file: {0}")]
    UnsafeArtifact(&'static str),
    #[error("qualification artifact {0} changed while its derived report was being validated")]
    ConcurrentReplacement(&'static str),
    #[error("qualification source changed while its derived report was being published: {0}")]
    ExternalSourceChanged(&'static str),
    #[error("qualification rollup source is not a direct sibling artifact: {0}")]
    NonSiblingArtifact(PathBuf),
    #[error("qualification artifact is not a direct child of its source-owned root: {0}")]
    NonDirectArtifact(PathBuf),
    #[error("qualification artifact size cannot be represented on this host")]
    SizeOverflow,
    #[error("qualification artifact read limit exceeds the source-owned maximum: {0}")]
    InvalidReadLimit(usize),
}
