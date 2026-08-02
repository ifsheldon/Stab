use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use object::{Architecture, BinaryFormat, Object as _};
use serde::{Deserialize, Serialize};

use crate::{RELEASE_VERSION, ReleaseError, archive, repository, safe_fs};

const MAX_BINARY_BYTES: u64 = 128 << 20;
const MAX_VERSION_OUTPUT: usize = 64 << 10;
const BUILD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const RELEASE_TARGETS: &[&str] = &["linux-aarch64", "macos-aarch64"];
const ASSET_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackagedBinary {
    pub(crate) binary: PathBuf,
    pub(crate) checksum: PathBuf,
    pub(crate) manifest: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseTarget {
    LinuxAarch64,
    MacosAarch64,
}

impl ReleaseTarget {
    fn parse(value: &str) -> Result<Self, ReleaseError> {
        match value {
            "linux-aarch64" => Ok(Self::LinuxAarch64),
            "macos-aarch64" => Ok(Self::MacosAarch64),
            _ => Err(ReleaseError::InvalidTarget(value.to_string())),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::LinuxAarch64 => "linux-aarch64",
            Self::MacosAarch64 => "macos-aarch64",
        }
    }

    fn binary_format(self) -> BinaryFormat {
        match self {
            Self::LinuxAarch64 => BinaryFormat::Elf,
            Self::MacosAarch64 => BinaryFormat::MachO,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AssetManifest {
    schema_version: u32,
    tag: String,
    commit: String,
    version: String,
    target: String,
    binary: String,
    bytes: u64,
    sha256: String,
    toolchain: repository::ToolchainIdentity,
}

pub(crate) fn build_binary(
    root: &Path,
    target: &str,
    output: &Path,
    tag: &str,
) -> Result<PackagedBinary, ReleaseError> {
    let target = ReleaseTarget::parse(target)?;
    let commit = repository::require_clean_tag(root, tag)?;
    let toolchain = repository::capture_toolchain(root)?;
    let output = safe_fs::RetainedDirectory::create_new_under(
        root,
        output,
        Some(Path::new("target/releases")),
    )?;
    let work = output.create_directory(OsStr::new("work"))?;
    let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
    let environment = vec![(
        OsString::from("CARGO_TARGET_DIR"),
        cargo_target.path().as_os_str().to_os_string(),
    )];
    repository::run_with_environment(
        root,
        &repository::cargo_program(),
        [
            OsStr::new("build"),
            OsStr::new("--release"),
            OsStr::new("--locked"),
            OsStr::new("--package"),
            OsStr::new("stab-cli"),
        ],
        &environment,
        BUILD_TIMEOUT,
        8 << 20,
    )?;
    repository::require_unchanged(root, &commit)?;
    repository::require_toolchain(root, &toolchain)?;

    cargo_target.revalidate()?;
    let release_directory = cargo_target.open_directory(OsStr::new("release"))?;
    let binary_path = release_directory.path().join("stab");
    let binary_file = release_directory.open_regular(OsStr::new("stab"))?;
    let identity = safe_fs::file_identity(&binary_file, &binary_path)?;
    let bytes = safe_fs::read_bounded_file(binary_file, &binary_path, MAX_BINARY_BYTES)?;
    validate_binary_bytes(&bytes, target)?;
    let version = repository::run_capture(
        root,
        binary_path.as_os_str(),
        [OsStr::new("--version")],
        VERSION_TIMEOUT,
        MAX_VERSION_OUTPUT,
    )?;
    validate_version_output(&version)?;
    safe_fs::require_same_path_identity(&binary_path, identity)?;

    let asset_name = format!("stab-{}", target.label());
    let binary = output.path().join(&asset_name);
    let asset_file = output.write_new(OsStr::new(&asset_name), &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        asset_file
            .set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|source| ReleaseError::io(&binary, source))?;
        asset_file
            .sync_all()
            .map_err(|source| ReleaseError::io(&binary, source))?;
    }
    let digest = archive::sha256_bytes(&bytes);
    let checksum_name = format!("{asset_name}.sha256");
    let checksum_bytes = format!("{digest}  {asset_name}\n");
    output.write_new(OsStr::new(&checksum_name), checksum_bytes.as_bytes())?;
    let manifest_name = format!("{asset_name}.json");
    let manifest = AssetManifest {
        schema_version: ASSET_MANIFEST_SCHEMA_VERSION,
        tag: tag.to_string(),
        commit,
        version: RELEASE_VERSION.to_string(),
        target: target.label().to_string(),
        binary: asset_name,
        bytes: u64::try_from(bytes.len()).map_err(|_| {
            ReleaseError::BinaryContract("binary size does not fit in u64".to_string())
        })?,
        sha256: digest,
        toolchain,
    };
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    manifest_bytes.push(b'\n');
    output.write_new(OsStr::new(&manifest_name), &manifest_bytes)?;
    work.revalidate()?;
    output.revalidate()?;
    fs::remove_dir_all(work.path()).map_err(|source| ReleaseError::io(work.path(), source))?;
    output.sync()?;
    Ok(PackagedBinary {
        binary,
        checksum: output.path().join(checksum_name),
        manifest: output.path().join(manifest_name),
    })
}

pub(crate) fn verify_assets(root: &Path, assets: &Path, tag: &str) -> Result<(), ReleaseError> {
    let commit = repository::require_clean_tag(root, tag)?;
    validate_relative(assets)?;
    let directory =
        safe_fs::RetainedDirectory::open_under(root, assets, Some(Path::new("target/releases")))?;
    let mut expected_files = BTreeSet::new();
    for label in RELEASE_TARGETS {
        let target = ReleaseTarget::parse(label)?;
        let asset_name = format!("stab-{label}");
        let checksum_name = format!("{asset_name}.sha256");
        let manifest_name = format!("{asset_name}.json");
        expected_files.extend([
            asset_name.clone(),
            checksum_name.clone(),
            manifest_name.clone(),
        ]);
        let bytes = directory.read_bounded(OsStr::new(&asset_name), MAX_BINARY_BYTES)?;
        validate_binary_bytes(&bytes, target)?;
        let digest = archive::sha256_bytes(&bytes);
        let checksum = directory.read_bounded(OsStr::new(&checksum_name), 4096)?;
        if checksum != format!("{digest}  {asset_name}\n").as_bytes() {
            return Err(ReleaseError::BinaryContract(format!(
                "checksum sidecar for {asset_name} is invalid"
            )));
        }
        let manifest_bytes = directory.read_bounded(OsStr::new(&manifest_name), 1 << 20)?;
        let manifest: AssetManifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.schema_version != ASSET_MANIFEST_SCHEMA_VERSION
            || manifest.tag != tag
            || manifest.commit != commit
            || manifest.version != RELEASE_VERSION
            || manifest.target != *label
            || manifest.binary != asset_name
            || manifest.sha256 != digest
            || u64::try_from(bytes.len()).ok() != Some(manifest.bytes)
        {
            return Err(ReleaseError::BinaryContract(format!(
                "asset manifest for {label} does not bind the reviewed tag, target, version, and bytes"
            )));
        }
    }
    let actual_files = fs::read_dir(directory.path())
        .map_err(|source| ReleaseError::io(directory.path(), source))?
        .map(|entry| {
            entry
                .map_err(|source| ReleaseError::io(directory.path(), source))
                .and_then(|entry| {
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| ReleaseError::InvalidPath(entry.path()))
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    directory.revalidate()?;
    if actual_files != expected_files {
        return Err(ReleaseError::BinaryContract(format!(
            "release asset set differs: expected {expected_files:?}, found {actual_files:?}"
        )));
    }
    Ok(())
}

fn validate_binary_bytes(bytes: &[u8], target: ReleaseTarget) -> Result<(), ReleaseError> {
    let binary = object::File::parse(bytes).map_err(|error| {
        ReleaseError::BinaryContract(format!("asset is not a supported executable: {error}"))
    })?;
    if binary.architecture() != Architecture::Aarch64 || binary.format() != target.binary_format() {
        return Err(ReleaseError::BinaryContract(format!(
            "asset has {:?}/{:?}, expected Aarch64/{:?}",
            binary.architecture(),
            binary.format(),
            target.binary_format()
        )));
    }
    Ok(())
}

fn validate_version_output(output: &str) -> Result<(), ReleaseError> {
    let expected = format!("stab {RELEASE_VERSION}");
    if output.trim() != expected {
        return Err(ReleaseError::BinaryContract(format!(
            "stab --version returned {:?}, expected {expected:?}",
            output.trim()
        )));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use object::Endianness;
    use object::write::Object as WriteObject;

    use super::*;

    fn executable(format: BinaryFormat, architecture: Architecture) -> Vec<u8> {
        WriteObject::new(format, architecture, Endianness::Little)
            .write()
            .expect("object bytes")
    }

    #[test]
    fn arbitrary_wrong_version_and_wrong_architecture_binaries_are_rejected() {
        assert!(validate_binary_bytes(b"arbitrary payload", ReleaseTarget::LinuxAarch64).is_err());
        assert!(validate_version_output("stab 9.9.9\n").is_err());
        let wrong_arch = executable(BinaryFormat::Elf, Architecture::X86_64);
        assert!(validate_binary_bytes(&wrong_arch, ReleaseTarget::LinuxAarch64).is_err());
    }

    #[test]
    fn target_format_and_version_contracts_accept_exact_values() {
        let linux = executable(BinaryFormat::Elf, Architecture::Aarch64);
        validate_binary_bytes(&linux, ReleaseTarget::LinuxAarch64).expect("Linux AArch64");
        let macos = executable(BinaryFormat::MachO, Architecture::Aarch64);
        validate_binary_bytes(&macos, ReleaseTarget::MacosAarch64).expect("macOS AArch64");
        validate_version_output("stab 0.2.0\n").expect("version");
    }

    #[test]
    fn release_asset_output_is_no_replace() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir_all(root.path().join("target/releases/dist")).expect("existing output");
        assert!(matches!(
            safe_fs::RetainedDirectory::create_new_under(
                root.path(),
                Path::new("target/releases/dist"),
                Some(Path::new("target/releases"))
            ),
            Err(ReleaseError::OutputExists(_))
        ));
    }
}
