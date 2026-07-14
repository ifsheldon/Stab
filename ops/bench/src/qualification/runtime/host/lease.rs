use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use thiserror::Error;

const LEASE_DIRECTORY: &str = "/tmp";

#[derive(Debug)]
pub(super) struct RunLease {
    _file: std::fs::File,
    #[cfg(test)]
    path: PathBuf,
}

impl RunLease {
    pub(super) fn acquire(profile_id: &str, cpu: usize) -> Result<Self, LeaseError> {
        Self::acquire_with_namespace("qualification", profile_id, cpu)
    }

    fn acquire_with_namespace(
        namespace: &str,
        profile_id: &str,
        cpu: usize,
    ) -> Result<Self, LeaseError> {
        let uid = rustix::process::getuid().as_raw();
        let profile_digest = super::sha256_hex(profile_id.as_bytes());
        let path = Path::new(LEASE_DIRECTORY).join(format!(
            "stab-pq1-{namespace}-{uid}-{profile_digest}-cpu{cpu}.lock"
        ));
        let descriptor = rustix::fs::open(
            &path,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .map_err(|source| LeaseError::Open {
            path: path.clone(),
            source,
        })?;
        let file = std::fs::File::from(descriptor);
        validate_file(&file, &path, uid)?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Self {
                _file: file,
                #[cfg(test)]
                path,
            }),
            Err(source) if source == rustix::io::Errno::AGAIN => Err(LeaseError::Busy(path)),
            Err(source) => Err(LeaseError::Lock { path, source }),
        }
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

fn validate_file(file: &std::fs::File, path: &Path, uid: u32) -> Result<(), LeaseError> {
    let metadata = file.metadata().map_err(|source| LeaseError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.nlink() != 1
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(LeaseError::Unsafe {
            path: path.to_path_buf(),
            uid: metadata.uid(),
            links: metadata.nlink(),
            mode: metadata.mode() & 0o777,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum LeaseError {
    #[error("failed to open qualification run lease {path}: {source}")]
    Open {
        path: PathBuf,
        source: rustix::io::Errno,
    },
    #[error("failed to inspect qualification run lease {path}: {source}")]
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("qualification run lease {path} is unsafe: uid={uid}, links={links}, mode={mode:o}")]
    Unsafe {
        path: PathBuf,
        uid: u32,
        links: u64,
        mode: u32,
    },
    #[error("qualification run lease is already held: {0}")]
    Busy(PathBuf),
    #[error("failed to lock qualification run lease {path}: {source}")]
    Lock {
        path: PathBuf,
        source: rustix::io::Errno,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn namespace() -> String {
        format!(
            "test-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn lease_excludes_a_second_holder_until_drop() {
        let namespace = namespace();
        let first =
            RunLease::acquire_with_namespace(&namespace, "test-profile", 7).expect("first lease");
        assert!(matches!(
            RunLease::acquire_with_namespace(&namespace, "test-profile", 7),
            Err(LeaseError::Busy(_))
        ));
        let path = first.path().to_path_buf();
        drop(first);
        let second = RunLease::acquire_with_namespace(&namespace, "test-profile", 7)
            .expect("lease after drop");
        drop(second);
        std::fs::remove_file(path).expect("remove test lease");
    }
}
