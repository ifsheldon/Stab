use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

#[cfg(test)]
use crate::RELEASE_VERSION;
use crate::ReleaseError;

const MAX_TARGET_LABEL_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackagedBinary {
    pub(crate) binary: PathBuf,
    pub(crate) checksum: PathBuf,
}

pub(crate) fn package_binary(
    root: &Path,
    binary: &Path,
    target: &str,
    output: &Path,
) -> Result<PackagedBinary, ReleaseError> {
    validate_target(target)?;
    let source = resolve_relative(root, binary)?;
    if !binary.starts_with("target/release") {
        return Err(ReleaseError::InvalidPath(binary.to_path_buf()));
    }
    require_regular_file(&source)?;
    let output = create_output_directory(root, output, None)?;
    let asset_name = format!("stab-{target}");
    let asset = output.join(&asset_name);
    fs::copy(&source, &asset).map_err(|source| ReleaseError::io(&asset, source))?;
    File::open(&asset)
        .and_then(|file| file.sync_all())
        .map_err(|source| ReleaseError::io(&asset, source))?;

    let digest = sha256_file(&asset)?;
    let checksum = output.join(format!("{asset_name}.sha256"));
    let mut checksum_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&checksum)
        .map_err(|source| ReleaseError::io(&checksum, source))?;
    writeln!(checksum_file, "{digest}  {asset_name}")
        .map_err(|source| ReleaseError::io(&checksum, source))?;
    checksum_file
        .sync_all()
        .map_err(|source| ReleaseError::io(&checksum, source))?;
    sync_directory(&output)?;
    Ok(PackagedBinary {
        binary: asset,
        checksum,
    })
}

pub(crate) fn create_report_directory(root: &Path, output: &Path) -> Result<PathBuf, ReleaseError> {
    create_output_directory(root, output, Some(Path::new("target/releases")))
}

pub(crate) fn validate_report_output(root: &Path, output: &Path) -> Result<(), ReleaseError> {
    validate_relative(output)?;
    let relative = output
        .strip_prefix("target/releases")
        .map_err(|_| ReleaseError::InvalidPath(output.to_path_buf()))?;
    if relative.as_os_str().is_empty() {
        return Err(ReleaseError::InvalidPath(output.to_path_buf()));
    }
    require_no_symlink_ancestors(root, output)?;
    let absolute = root.join(output);
    if absolute.exists() {
        return Err(ReleaseError::OutputExists(absolute));
    }
    Ok(())
}

pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), ReleaseError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| ReleaseError::io(path, source))?;
    file.write_all(bytes)
        .map_err(|source| ReleaseError::io(path, source))?;
    file.sync_all()
        .map_err(|source| ReleaseError::io(path, source))?;
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, ReleaseError> {
    let mut file = File::open(path).map_err(|source| ReleaseError::io(path, source))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 << 10];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ReleaseError::io(path, source))?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or_else(|| {
            ReleaseError::PackageContract(format!(
                "reader returned {read} bytes for a {}-byte buffer",
                buffer.len()
            ))
        })?;
        hash.update(bytes);
    }
    Ok(hex::encode(hash.finalize()))
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), ReleaseError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| ReleaseError::io(path, source))
}

fn create_output_directory(
    root: &Path,
    output: &Path,
    required_prefix: Option<&Path>,
) -> Result<PathBuf, ReleaseError> {
    validate_relative(output)?;
    if required_prefix.is_some_and(|prefix| !output.starts_with(prefix)) {
        return Err(ReleaseError::InvalidPath(output.to_path_buf()));
    }
    let absolute = root.join(output);
    require_no_symlink_ancestors(root, output)?;
    if absolute.exists() {
        return Err(ReleaseError::OutputExists(absolute));
    }
    let parent = absolute
        .parent()
        .ok_or_else(|| ReleaseError::InvalidPath(output.to_path_buf()))?;
    fs::create_dir_all(parent).map_err(|source| ReleaseError::io(parent, source))?;
    require_no_symlink_ancestors(root, output)?;
    fs::create_dir(&absolute).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            ReleaseError::OutputExists(absolute.clone())
        } else {
            ReleaseError::io(&absolute, source)
        }
    })?;
    Ok(absolute)
}

fn resolve_relative(root: &Path, path: &Path) -> Result<PathBuf, ReleaseError> {
    validate_relative(path)?;
    require_no_symlink_ancestors(root, path)?;
    Ok(root.join(path))
}

fn validate_relative(path: &Path) -> Result<(), ReleaseError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ReleaseError::InvalidPath(path.to_path_buf()));
    }
    Ok(())
}

fn require_no_symlink_ancestors(root: &Path, relative: &Path) -> Result<(), ReleaseError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(ReleaseError::InvalidPath(relative.to_path_buf()));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ReleaseError::SymlinkPath(current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(ReleaseError::io(&current, source)),
        }
    }
    Ok(())
}

fn require_regular_file(path: &Path) -> Result<(), ReleaseError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ReleaseError::io(path, source))?;
    if !metadata.file_type().is_file() {
        return Err(ReleaseError::NotRegularFile(path.to_path_buf()));
    }
    Ok(())
}

fn validate_target(target: &str) -> Result<(), ReleaseError> {
    if target.is_empty()
        || target.len() > MAX_TARGET_LABEL_BYTES
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || target.starts_with('-')
        || target.ends_with('-')
    {
        return Err(ReleaseError::InvalidTarget(target.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_packaging_writes_stable_asset_and_checksum() {
        let root = tempfile::tempdir().expect("root");
        let binary_directory = root.path().join("target/release");
        fs::create_dir_all(&binary_directory).expect("binary directory");
        fs::write(binary_directory.join("stab"), b"stab-binary").expect("binary");

        let packaged = package_binary(
            root.path(),
            Path::new("target/release/stab"),
            "linux-aarch64",
            Path::new("dist"),
        )
        .expect("package binary");
        assert_eq!(
            fs::read(&packaged.binary).expect("packaged binary"),
            b"stab-binary"
        );
        assert_eq!(
            fs::read_to_string(&packaged.checksum).expect("checksum"),
            format!(
                "{}  stab-linux-aarch64\n",
                sha256_file(&packaged.binary).expect("digest")
            )
        );
    }

    #[test]
    fn release_paths_and_target_labels_fail_closed() {
        let root = tempfile::tempdir().expect("root");
        assert!(matches!(
            create_report_directory(root.path(), Path::new("../release")),
            Err(ReleaseError::InvalidPath(_))
        ));
        assert!(matches!(
            create_report_directory(root.path(), Path::new("dist/release")),
            Err(ReleaseError::InvalidPath(_))
        ));
        assert!(matches!(
            validate_report_output(root.path(), Path::new("target/releases/../release")),
            Err(ReleaseError::InvalidPath(_))
        ));
        for target in ["", "-linux", "linux-", "linux/aarch64", "linux_aarch64"] {
            assert!(matches!(
                validate_target(target),
                Err(ReleaseError::InvalidTarget(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn release_output_rejects_symlink_ancestors() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("outside")).expect("outside");
        fs::create_dir(root.path().join("target")).expect("target");
        symlink(
            root.path().join("outside"),
            root.path().join("target/releases"),
        )
        .expect("symlink");
        assert!(matches!(
            create_report_directory(root.path(), Path::new("target/releases/report")),
            Err(ReleaseError::SymlinkPath(_))
        ));
    }

    #[test]
    fn release_version_matches_binary_contract() {
        assert_eq!(RELEASE_VERSION, "0.2.0");
    }
}
