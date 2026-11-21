use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use object::{Architecture, BinaryFormat, Object as _, ObjectKind, ObjectSegment as _};
use serde::{Deserialize, Serialize};

use crate::{RELEASE_VERSION, ReleaseError, archive, cargo, repository, safe_fs};

const MAX_BINARY_BYTES: u64 = 128 << 20;
const MAX_VERSION_OUTPUT: usize = 64 << 10;
const BUILD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const RELEASE_TARGETS: &[&str] = &["linux-aarch64", "macos-aarch64"];
const ASSET_MANIFEST_SCHEMA_VERSION: u32 = 1;

pub(crate) struct ReviewedAsset {
    name: String,
    path: PathBuf,
    file: File,
    identity: safe_fs::FileIdentity,
    read_limit: u64,
    bytes: u64,
    sha256: String,
}

impl ReviewedAsset {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    /// A sealed in-memory copy of the reviewed bytes for upload. The retained
    /// descriptor is reread and checked first, so a later change to the file
    /// can neither alter the upload nor pass unnoticed before it.
    pub(crate) fn upload_file(&mut self) -> Result<File, ReleaseError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|source| ReleaseError::io(&self.path, source))?;
        let bytes = self.read_reviewed_bytes()?;
        safe_fs::sealed_copy(&self.name, &bytes)
    }

    fn read_reviewed_bytes(&self) -> Result<Vec<u8>, ReleaseError> {
        let reader = self
            .file
            .try_clone()
            .map_err(|source| ReleaseError::io(&self.path, source))?;
        let bytes = safe_fs::read_bounded_file(reader, &self.path, self.read_limit)?;
        if u64::try_from(bytes.len()).ok() != Some(self.bytes)
            || archive::sha256_bytes(&bytes) != self.sha256
        {
            return Err(ReleaseError::BinaryContract(format!(
                "reviewed release asset {} changed after validation",
                self.name
            )));
        }
        Ok(bytes)
    }

    fn revalidate(&self) -> Result<(), ReleaseError> {
        safe_fs::require_same_path_identity(&self.path, self.identity)?;
        self.read_reviewed_bytes()?;
        safe_fs::require_same_path_identity(&self.path, self.identity)
    }
}

pub(crate) struct ReviewedAssets {
    directory: safe_fs::RetainedDirectory,
    commit: String,
    assets: Vec<ReviewedAsset>,
}

impl ReviewedAssets {
    pub(crate) fn commit(&self) -> &str {
        &self.commit
    }

    pub(crate) fn assets_mut(&mut self) -> &mut [ReviewedAsset] {
        &mut self.assets
    }

    pub(crate) fn assets(&self) -> &[ReviewedAsset] {
        &self.assets
    }

    pub(crate) fn revalidate(&self) -> Result<(), ReleaseError> {
        self.directory.revalidate()?;
        for asset in &self.assets {
            asset.revalidate()?;
        }
        self.require_exact_entry_set()?;
        self.directory.revalidate()
    }

    fn require_exact_entry_set(&self) -> Result<(), ReleaseError> {
        let expected = self
            .assets
            .iter()
            .map(|asset| asset.name.clone())
            .collect::<BTreeSet<_>>();
        let actual = self
            .directory
            .entry_names(expected.len().saturating_add(1))?
            .into_iter()
            .map(|name| {
                name.into_string()
                    .map_err(|name| ReleaseError::InvalidPath(self.directory.path().join(name)))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if actual != expected {
            return Err(ReleaseError::BinaryContract(format!(
                "release asset set differs: expected {expected:?}, found {actual:?}"
            )));
        }
        Ok(())
    }
}

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

    fn rust_host(self) -> &'static str {
        match self {
            Self::LinuxAarch64 => "aarch64-unknown-linux-gnu",
            Self::MacosAarch64 => "aarch64-apple-darwin",
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
    let cargo = cargo::CargoSandbox::create(root, &work, &cargo_target)?;
    cargo.run(
        root,
        [
            OsStr::new("build"),
            OsStr::new("--release"),
            OsStr::new("--locked"),
            OsStr::new("--package"),
            OsStr::new("stab-cli"),
        ],
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
    let binary_reader = binary_file
        .try_clone()
        .map_err(|source| ReleaseError::io(&binary_path, source))?;
    let bytes = safe_fs::read_bounded_file(binary_reader, &binary_path, MAX_BINARY_BYTES)?;
    validate_binary_bytes(&bytes, target)?;
    let version = capture_version_from_descriptor(root, &binary_file, &binary_path)?;
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
    work.remove_tree()?;
    output.sync()?;
    Ok(PackagedBinary {
        binary,
        checksum: output.path().join(checksum_name),
        manifest: output.path().join(manifest_name),
    })
}

pub(crate) fn verify_assets(root: &Path, assets: &Path, tag: &str) -> Result<(), ReleaseError> {
    review_assets(root, assets, tag)?.revalidate()
}

pub(crate) fn review_assets(
    root: &Path,
    assets: &Path,
    tag: &str,
) -> Result<ReviewedAssets, ReleaseError> {
    let commit = repository::require_clean_tag(root, tag)?;
    let toolchain = repository::capture_toolchain(root)?;
    safe_fs::validate_relative(assets)?;
    let directory =
        safe_fs::RetainedDirectory::open_under(root, assets, Some(Path::new("target/releases")))?;
    let mut expected_files = BTreeSet::new();
    let mut reviewed_assets = Vec::with_capacity(RELEASE_TARGETS.len() * 3);
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
        let (binary, bytes) = retain_asset(&directory, &asset_name, MAX_BINARY_BYTES)?;
        validate_binary_bytes(&bytes, target)?;
        let digest = archive::sha256_bytes(&bytes);
        let (checksum_asset, checksum) = retain_asset(&directory, &checksum_name, 4096)?;
        if checksum != format!("{digest}  {asset_name}\n").as_bytes() {
            return Err(ReleaseError::BinaryContract(format!(
                "checksum sidecar for {asset_name} is invalid"
            )));
        }
        let (manifest_asset, manifest_bytes) = retain_asset(&directory, &manifest_name, 1 << 20)?;
        let manifest: AssetManifest = serde_json::from_slice(&manifest_bytes)?;
        repository::require_reviewed_asset_toolchain(
            &toolchain,
            &manifest.toolchain,
            target.rust_host(),
        )?;
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
        reviewed_assets.extend([binary, checksum_asset, manifest_asset]);
    }
    repository::require_unchanged(root, &commit)?;
    repository::require_toolchain(root, &toolchain)?;
    let actual_files = directory
        .entry_names(expected_files.len().saturating_add(1))?
        .into_iter()
        .map(|name| {
            name.into_string()
                .map_err(|name| ReleaseError::InvalidPath(directory.path().join(name)))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    directory.revalidate()?;
    if actual_files != expected_files {
        return Err(ReleaseError::BinaryContract(format!(
            "release asset set differs: expected {expected_files:?}, found {actual_files:?}"
        )));
    }
    let reviewed = ReviewedAssets {
        directory,
        commit,
        assets: reviewed_assets,
    };
    reviewed.revalidate()?;
    Ok(reviewed)
}

fn retain_asset(
    directory: &safe_fs::RetainedDirectory,
    name: &str,
    limit: u64,
) -> Result<(ReviewedAsset, Vec<u8>), ReleaseError> {
    let path = directory.path().join(name);
    let mut file = directory.open_regular(OsStr::new(name))?;
    let identity = safe_fs::file_identity(&file, &path)?;
    let reader = file
        .try_clone()
        .map_err(|source| ReleaseError::io(&path, source))?;
    let bytes = safe_fs::read_bounded_file(reader, &path, limit)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|source| ReleaseError::io(&path, source))?;
    let byte_count = u64::try_from(bytes.len()).map_err(|_| {
        ReleaseError::BinaryContract(format!("asset {name} size does not fit in u64"))
    })?;
    let sha256 = archive::sha256_bytes(&bytes);
    Ok((
        ReviewedAsset {
            name: name.to_string(),
            path,
            file,
            identity,
            read_limit: limit,
            bytes: byte_count,
            sha256,
        },
        bytes,
    ))
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
    let kind = binary.kind();
    let runnable_kind = match target {
        ReleaseTarget::LinuxAarch64 => {
            matches!(kind, ObjectKind::Executable | ObjectKind::Dynamic)
        }
        ReleaseTarget::MacosAarch64 => kind == ObjectKind::Executable,
    };
    if !runnable_kind {
        return Err(ReleaseError::BinaryContract(format!(
            "asset has {kind:?} object kind, expected an executable{}",
            if target == ReleaseTarget::LinuxAarch64 {
                " or position-independent executable"
            } else {
                ""
            }
        )));
    }
    let entry = binary.entry();
    let entry_is_executable = entry != 0
        && binary.segments().any(|segment| {
            let start = segment.address();
            let virtual_entry = start.checked_add(segment.size()).is_some_and(|end| {
                segment.permissions().executable() && (start..end).contains(&entry)
            });
            let (file_start, file_size) = segment.file_range();
            let file_entry = target == ReleaseTarget::MacosAarch64
                && file_start.checked_add(file_size).is_some_and(|end| {
                    segment.permissions().executable() && (file_start..end).contains(&entry)
                });
            virtual_entry || file_entry
        });
    if !entry_is_executable {
        return Err(ReleaseError::BinaryContract(format!(
            "asset entry point {entry:#x} is not inside an executable segment"
        )));
    }
    Ok(())
}

fn validate_version_output(output: &str) -> Result<(), ReleaseError> {
    let expected = format!("stab {RELEASE_VERSION}\n");
    if output != expected {
        return Err(ReleaseError::BinaryContract(format!(
            "stab --version returned {:?}, expected {expected:?}",
            output
        )));
    }
    Ok(())
}

fn capture_version_from_descriptor(
    root: &Path,
    binary: &File,
    display_path: &Path,
) -> Result<String, ReleaseError> {
    let program = safe_fs::descriptor_program(binary, display_path)?;
    repository::run_capture(
        root,
        &crate::process::Invocation::new(
            crate::process::Program::Retained(&program),
            ["--version"],
            VERSION_TIMEOUT,
            MAX_VERSION_OUTPUT,
        ),
    )
}

#[cfg(test)]
pub(crate) mod tests;
