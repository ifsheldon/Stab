use std::io::Read as _;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use thiserror::Error;

const MAX_EXECUTABLE_BYTES: u64 = 1 << 30;

#[derive(Debug)]
pub(super) struct SealedExecutable {
    role: &'static str,
    descriptor: std::fs::File,
    bytes: u64,
    sha256: String,
}

impl SealedExecutable {
    pub(super) fn open(role: &'static str, path: &Path) -> Result<Self, ExecutableError> {
        let mut source =
            crate::source_file::open_regular_file_bounded_descriptor(path, MAX_EXECUTABLE_BYTES)
                .map_err(|error| ExecutableError::UnsafeSource {
                    role,
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                })?;
        let metadata = source.metadata().map_err(|source| ExecutableError::Io {
            role,
            path: path.to_path_buf(),
            source,
        })?;
        if metadata.len() == 0 || metadata.permissions().mode() & 0o111 == 0 {
            return Err(ExecutableError::UnsafeSource {
                role,
                path: path.to_path_buf(),
                reason: "file must be nonempty and executable".to_string(),
            });
        }
        let descriptor = rustix::fs::memfd_create(
            format!("stab-pq1-{role}"),
            rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|source| ExecutableError::Seal {
            role,
            source: source.into(),
        })?;
        let mut descriptor = std::fs::File::from(descriptor);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 << 10];
        let mut copied = 0_u64;
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|source| ExecutableError::Io {
                    role,
                    path: path.to_path_buf(),
                    source,
                })?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(u64::try_from(count).map_err(|_| ExecutableError::SizeOverflow)?)
                .ok_or(ExecutableError::SizeOverflow)?;
            if copied > metadata.len() || copied > MAX_EXECUTABLE_BYTES {
                return Err(ExecutableError::ChangedWhileCopying { role });
            }
            let chunk = buffer.get(..count).ok_or(ExecutableError::SizeOverflow)?;
            std::io::Write::write_all(&mut descriptor, chunk).map_err(|source| {
                ExecutableError::Io {
                    role,
                    path: path.to_path_buf(),
                    source,
                }
            })?;
            hasher.update(chunk);
        }
        if copied != metadata.len() {
            return Err(ExecutableError::ChangedWhileCopying { role });
        }
        std::io::Write::flush(&mut descriptor).map_err(|source| ExecutableError::Io {
            role,
            path: path.to_path_buf(),
            source,
        })?;
        descriptor
            .set_permissions(std::fs::Permissions::from_mode(0o500))
            .map_err(|source| ExecutableError::Io {
                role,
                path: path.to_path_buf(),
                source,
            })?;
        rustix::fs::fcntl_add_seals(
            &descriptor,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK
                | rustix::fs::SealFlags::SEAL,
        )
        .map_err(|source| ExecutableError::Seal {
            role,
            source: source.into(),
        })?;
        rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::empty()).map_err(|source| {
            ExecutableError::Seal {
                role,
                source: source.into(),
            }
        })?;
        Ok(Self {
            role,
            descriptor,
            bytes: copied,
            sha256: hex_digest(&hasher.finalize()),
        })
    }

    pub(super) fn program(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.descriptor.as_raw_fd()))
    }

    pub(super) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(super) fn verify(&self) -> Result<(), ExecutableError> {
        let seals = rustix::fs::fcntl_get_seals(&self.descriptor).map_err(|source| {
            ExecutableError::Seal {
                role: self.role,
                source: source.into(),
            }
        })?;
        let required = rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::SEAL;
        if !seals.contains(required) {
            return Err(ExecutableError::MissingSeals { role: self.role });
        }
        let (bytes, sha256) = digest_descriptor(&self.descriptor)?;
        if bytes != self.bytes || sha256 != self.sha256 {
            return Err(ExecutableError::SealedIdentityChanged { role: self.role });
        }
        Ok(())
    }
}

fn digest_descriptor(file: &std::fs::File) -> Result<(u64, String), ExecutableError> {
    use std::os::unix::fs::FileExt as _;

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 << 10];
    let mut offset = 0_u64;
    loop {
        let count = file
            .read_at(&mut buffer, offset)
            .map_err(ExecutableError::Descriptor)?;
        if count == 0 {
            break;
        }
        offset = offset
            .checked_add(u64::try_from(count).map_err(|_| ExecutableError::SizeOverflow)?)
            .ok_or(ExecutableError::SizeOverflow)?;
        let chunk = buffer.get(..count).ok_or(ExecutableError::SizeOverflow)?;
        hasher.update(chunk);
    }
    Ok((offset, hex_digest(&hasher.finalize())))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}

#[derive(Debug, Error)]
pub(super) enum ExecutableError {
    #[error("qualification executable {role} at {path} is unsafe: {reason}")]
    UnsafeSource {
        role: &'static str,
        path: PathBuf,
        reason: String,
    },
    #[error("qualification executable {role} at {path} could not be copied: {source}")]
    Io {
        role: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("qualification executable {role} changed while being copied")]
    ChangedWhileCopying { role: &'static str },
    #[error("qualification executable {role} could not be sealed: {source}")]
    Seal {
        role: &'static str,
        source: std::io::Error,
    },
    #[error("qualification executable {role} is missing required seals")]
    MissingSeals { role: &'static str },
    #[error("sealed qualification executable {role} changed identity")]
    SealedIdentityChanged { role: &'static str },
    #[error("failed to read a sealed qualification executable: {0}")]
    Descriptor(std::io::Error),
    #[error("qualification executable size accounting overflowed")]
    SizeOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_copy_retains_identity_after_source_changes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("worker");
        std::fs::write(&source, b"first executable bytes").expect("write source");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o700))
            .expect("make executable");
        let sealed = SealedExecutable::open("test", &source).expect("seal executable");
        let digest = sealed.sha256().to_string();
        std::fs::write(&source, b"changed executable bytes").expect("change source");
        sealed.verify().expect("sealed identity remains valid");
        assert_eq!(sealed.sha256(), digest);
        assert!(sealed.program().starts_with("/proc/self/fd"));
    }
}
