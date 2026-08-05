use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
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

    pub(crate) fn upload_file(&mut self) -> Result<File, ReleaseError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|source| ReleaseError::io(&self.path, source))?;
        self.file
            .try_clone()
            .map_err(|source| ReleaseError::io(&self.path, source))
    }

    fn revalidate(&self) -> Result<(), ReleaseError> {
        safe_fs::require_same_path_identity(&self.path, self.identity)?;
        let reader = self
            .file
            .try_clone()
            .map_err(|source| ReleaseError::io(&self.path, source))?;
        let bytes = safe_fs::read_bounded_file(reader, &self.path, self.read_limit)?;
        safe_fs::require_same_path_identity(&self.path, self.identity)?;
        if u64::try_from(bytes.len()).ok() != Some(self.bytes)
            || archive::sha256_bytes(&bytes) != self.sha256
        {
            return Err(ReleaseError::BinaryContract(format!(
                "reviewed release asset {} changed after validation",
                self.name
            )));
        }
        Ok(())
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
    validate_relative(assets)?;
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
        program.path().as_os_str(),
        [OsStr::new("--version")],
        VERSION_TIMEOUT,
        MAX_VERSION_OUTPUT,
    )
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
    use std::fs;
    use std::io::Write as _;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    use super::*;
    use crate::RELEASE_TAG;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Stab Test")
            .env("GIT_AUTHOR_EMAIL", "stab-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Stab Test")
            .env("GIT_COMMITTER_EMAIL", "stab-test@example.invalid")
            .status()
            .expect("git command");
        assert!(status.success(), "git {args:?}");
    }

    fn tagged_asset_repository() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().expect("root");
        git(root.path(), &["init", "--quiet"]);
        fs::write(root.path().join(".gitignore"), "target/\n").expect("gitignore");
        fs::write(root.path().join("source.txt"), "reviewed source\n").expect("source");
        fs::write(
            root.path().join("rust-toolchain.toml"),
            include_bytes!("../../../rust-toolchain.toml"),
        )
        .expect("toolchain pin");
        git(
            root.path(),
            &["add", ".gitignore", "source.txt", "rust-toolchain.toml"],
        );
        git(root.path(), &["commit", "--quiet", "-m", "fixture"]);
        git(
            root.path(),
            &["tag", "-a", RELEASE_TAG, "-m", "fixture tag"],
        );
        let assets = PathBuf::from("target/releases/assets");
        let directory = root.path().join(&assets);
        fs::create_dir_all(&directory).expect("asset directory");
        let commit = repository::require_clean_tag(root.path(), RELEASE_TAG).expect("tag");
        let toolchain = repository::capture_toolchain(root.path()).expect("fixture toolchain");
        for (label, bytes) in [
            (
                "linux-aarch64",
                elf_aarch64(object::elf::ET_EXEC.0, object::elf::EM_AARCH64.0),
            ),
            ("macos-aarch64", macho_aarch64_executable()),
        ] {
            let name = format!("stab-{label}");
            let digest = archive::sha256_bytes(&bytes);
            fs::write(directory.join(&name), &bytes).expect("binary");
            fs::write(
                directory.join(format!("{name}.sha256")),
                format!("{digest}  {name}\n"),
            )
            .expect("checksum");
            let manifest = AssetManifest {
                schema_version: ASSET_MANIFEST_SCHEMA_VERSION,
                tag: RELEASE_TAG.to_string(),
                commit: commit.clone(),
                version: RELEASE_VERSION.to_string(),
                target: label.to_string(),
                binary: name.clone(),
                bytes: bytes.len() as u64,
                sha256: digest,
                toolchain: fixture_toolchain(&toolchain, label),
            };
            let mut manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest");
            manifest_bytes.push(b'\n');
            fs::write(directory.join(format!("{name}.json")), manifest_bytes).expect("manifest");
        }
        (root, assets)
    }

    fn fixture_toolchain(
        toolchain: &repository::ToolchainIdentity,
        label: &str,
    ) -> repository::ToolchainIdentity {
        let target = ReleaseTarget::parse(label).expect("release target");
        let target_host = target.rust_host();
        let current_host = verbose_field(&toolchain.rustc_version, "host");
        let active_name = toolchain
            .active_toolchain
            .split_once(" (overridden by '")
            .map(|(name, _)| name)
            .expect("active toolchain name");
        let channel = active_name
            .strip_suffix(&format!("-{current_host}"))
            .expect("toolchain channel");
        let target_name = format!("{channel}-{target_host}");
        let toolchain_root = Path::new("/fixture/.rustup/toolchains").join(&target_name);
        let (libcurl, ssl, os) = match target {
            ReleaseTarget::LinuxAarch64 => (
                "8.20.0-DEV (sys:0.4.88+curl-8.20.0 vendored ssl:OpenSSL/3.6.2)",
                "OpenSSL 3.6.2 7 Apr 2026",
                "Ubuntu 24.4.0 (noble) [64-bit]",
            ),
            ReleaseTarget::MacosAarch64 => (
                "8.7.1 (sys:0.4.88+curl-8.20.0 system ssl:(SecureTransport) LibreSSL/3.3.6)",
                "OpenSSL 3.6.2 7 Apr 2026",
                "Mac OS 15.5.0 [64-bit]",
            ),
        };
        let cargo_header = toolchain
            .cargo_version
            .lines()
            .next()
            .expect("Cargo version header");
        let rustc_header = toolchain
            .rustc_version
            .lines()
            .next()
            .expect("rustc version header");
        repository::ToolchainIdentity {
            cargo_program: toolchain_root.join("bin/cargo").display().to_string(),
            cargo_version: format!(
                "{cargo_header}\nrelease: {}\ncommit-hash: {}\ncommit-date: {}\nhost: {target_host}\nlibgit2: {}\nlibcurl: {libcurl}\nssl: {ssl}\nos: {os}\n",
                verbose_field(&toolchain.cargo_version, "release"),
                verbose_field(&toolchain.cargo_version, "commit-hash"),
                verbose_field(&toolchain.cargo_version, "commit-date"),
                verbose_field(&toolchain.cargo_version, "libgit2"),
            ),
            rustc_program: toolchain_root.join("bin/rustc").display().to_string(),
            rustc_version: format!(
                "{rustc_header}\nbinary: {}\ncommit-hash: {}\ncommit-date: {}\nhost: {target_host}\nrelease: {}\nLLVM version: {}\n",
                verbose_field(&toolchain.rustc_version, "binary"),
                verbose_field(&toolchain.rustc_version, "commit-hash"),
                verbose_field(&toolchain.rustc_version, "commit-date"),
                verbose_field(&toolchain.rustc_version, "release"),
                verbose_field(&toolchain.rustc_version, "LLVM version"),
            ),
            active_toolchain: format!(
                "{target_name} (overridden by '/fixture/repository/rust-toolchain.toml')\n"
            ),
        }
    }

    fn verbose_field<'a>(version: &'a str, field: &str) -> &'a str {
        let prefix = format!("{field}: ");
        version
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .expect("verbose version field")
    }

    fn replace_verbose_field(version: &str, field: &str, replacement: &str) -> String {
        let prefix = format!("{field}: ");
        let mut replaced = false;
        let mut result = String::new();
        for line in version.lines() {
            if line.starts_with(&prefix) {
                result.push_str(field);
                result.push_str(": ");
                result.push_str(replacement);
                replaced = true;
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }
        assert!(replaced, "fixture field {field:?} exists");
        result
    }

    fn remove_verbose_field(version: &str, field: &str) -> String {
        let prefix = format!("{field}: ");
        let mut removed = false;
        let mut result = String::new();
        for line in version.lines() {
            if line.starts_with(&prefix) {
                removed = true;
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }
        assert!(removed, "fixture field {field:?} exists");
        result
    }

    fn mutate_manifest_toolchain(
        root: &Path,
        assets: &Path,
        label: &str,
        mutate: impl FnOnce(&mut repository::ToolchainIdentity),
    ) {
        let path = root.join(assets).join(format!("stab-{label}.json"));
        let bytes = fs::read(&path).expect("read manifest");
        let mut manifest: AssetManifest = serde_json::from_slice(&bytes).expect("parse manifest");
        mutate(&mut manifest.toolchain);
        let mut bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
        bytes.push(b'\n');
        fs::write(path, bytes).expect("write mutated manifest");
    }

    fn write_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) {
        let end = offset.checked_add(value.len()).expect("fixture offset");
        bytes
            .get_mut(offset..end)
            .expect("fixture range")
            .copy_from_slice(value);
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        write_bytes(bytes, offset, &value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        write_bytes(bytes, offset, &value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        write_bytes(bytes, offset, &value.to_le_bytes());
    }

    fn elf_aarch64(kind: u16, machine: u16) -> Vec<u8> {
        const HEADER_BYTES: usize = 64;
        const PROGRAM_HEADER_BYTES: usize = 56;
        const CODE: [u8; 12] = [
            0x00, 0x00, 0x80, 0xd2, // mov x0, #0
            0xa8, 0x0b, 0x80, 0xd2, // mov x8, #93
            0x01, 0x00, 0x00, 0xd4, // svc #0
        ];
        let code_offset = HEADER_BYTES + PROGRAM_HEADER_BYTES;
        let mut bytes = vec![0; code_offset + CODE.len()];
        write_bytes(
            &mut bytes,
            0,
            &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        write_u16(&mut bytes, 16, kind);
        write_u16(&mut bytes, 18, machine);
        write_u32(&mut bytes, 20, 1);
        let base = if kind == object::elf::ET_DYN.0 {
            0
        } else {
            0x40_0000
        };
        write_u64(&mut bytes, 24, base + code_offset as u64);
        write_u64(&mut bytes, 32, HEADER_BYTES as u64);
        write_u16(&mut bytes, 52, 64);
        write_u16(&mut bytes, 54, 56);
        write_u16(&mut bytes, 56, 1);

        write_u32(&mut bytes, HEADER_BYTES, object::elf::PT_LOAD.0);
        write_u32(
            &mut bytes,
            HEADER_BYTES + 4,
            (object::elf::PF_R | object::elf::PF_X).0,
        );
        write_u64(&mut bytes, HEADER_BYTES + 16, base);
        write_u64(&mut bytes, HEADER_BYTES + 24, base);
        let file_size = bytes.len() as u64;
        write_u64(&mut bytes, HEADER_BYTES + 32, file_size);
        write_u64(&mut bytes, HEADER_BYTES + 40, file_size);
        write_u64(&mut bytes, HEADER_BYTES + 48, 0x1000);
        write_bytes(&mut bytes, code_offset, &CODE);
        bytes
    }

    fn macho_aarch64_executable() -> Vec<u8> {
        const HEADER_BYTES: usize = 32;
        const SEGMENT_COMMAND_BYTES: usize = 72;
        const ENTRY_COMMAND_BYTES: usize = 24;
        let code_offset = HEADER_BYTES + SEGMENT_COMMAND_BYTES + ENTRY_COMMAND_BYTES;
        let mut bytes = vec![0; code_offset + 4];
        write_u32(&mut bytes, 0, object::macho::MH_MAGIC_64);
        write_u32(&mut bytes, 4, object::macho::CPU_TYPE_ARM64.0);
        write_u32(&mut bytes, 8, object::macho::CPU_SUBTYPE_ARM64_ALL.0);
        write_u32(&mut bytes, 12, object::macho::MH_EXECUTE.0);
        write_u32(&mut bytes, 16, 2);
        write_u32(&mut bytes, 20, 96);
        write_u32(
            &mut bytes,
            24,
            (object::macho::MH_NOUNDEFS
                | object::macho::MH_DYLDLINK
                | object::macho::MH_TWOLEVEL
                | object::macho::MH_PIE)
                .0,
        );

        let segment = HEADER_BYTES;
        write_u32(&mut bytes, segment, object::macho::LC_SEGMENT_64.0);
        write_u32(&mut bytes, segment + 4, 72);
        write_bytes(&mut bytes, segment + 8, b"__TEXT");
        write_u64(&mut bytes, segment + 24, 0x1_0000_0000);
        write_u64(&mut bytes, segment + 32, 0x1000);
        let file_size = bytes.len() as u64;
        write_u64(&mut bytes, segment + 48, file_size);
        write_u32(&mut bytes, segment + 56, 5);
        write_u32(&mut bytes, segment + 60, 5);

        let entry = segment + SEGMENT_COMMAND_BYTES;
        write_u32(&mut bytes, entry, object::macho::LC_MAIN.0);
        write_u32(&mut bytes, entry + 4, 24);
        write_u64(&mut bytes, entry + 8, code_offset as u64);
        write_bytes(&mut bytes, code_offset, &0xd65f_03c0_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn arbitrary_wrong_version_and_wrong_architecture_binaries_are_rejected() {
        assert!(validate_binary_bytes(b"arbitrary payload", ReleaseTarget::LinuxAarch64).is_err());
        assert!(validate_version_output("stab 9.9.9\n").is_err());
        let wrong_arch = elf_aarch64(object::elf::ET_EXEC.0, object::elf::EM_X86_64.0);
        assert!(validate_binary_bytes(&wrong_arch, ReleaseTarget::LinuxAarch64).is_err());
    }

    #[test]
    fn runnable_target_binaries_are_accepted_and_relocatable_objects_are_rejected() {
        let linux = elf_aarch64(object::elf::ET_EXEC.0, object::elf::EM_AARCH64.0);
        validate_binary_bytes(&linux, ReleaseTarget::LinuxAarch64).expect("Linux AArch64");
        let linux_pie = elf_aarch64(object::elf::ET_DYN.0, object::elf::EM_AARCH64.0);
        validate_binary_bytes(&linux_pie, ReleaseTarget::LinuxAarch64).expect("Linux AArch64 PIE");
        let relocatable = elf_aarch64(object::elf::ET_REL.0, object::elf::EM_AARCH64.0);
        assert!(validate_binary_bytes(&relocatable, ReleaseTarget::LinuxAarch64).is_err());
        let mut missing_entry = linux.clone();
        write_u64(&mut missing_entry, 24, 0);
        assert!(validate_binary_bytes(&missing_entry, ReleaseTarget::LinuxAarch64).is_err());
        let mut non_executable_entry = linux;
        write_u32(&mut non_executable_entry, 68, object::elf::PF_R.0);
        assert!(validate_binary_bytes(&non_executable_entry, ReleaseTarget::LinuxAarch64).is_err());
        let macos = macho_aarch64_executable();
        validate_binary_bytes(&macos, ReleaseTarget::MacosAarch64).expect("macOS AArch64");
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    fn execute_generated_fixture(path: &Path) -> std::io::Result<std::process::ExitStatus> {
        let mut retries = 4_u8;
        loop {
            match Command::new(path).status() {
                Err(error)
                    if error.kind() == std::io::ErrorKind::ExecutableFileBusy && retries > 0 =>
                {
                    retries = retries.saturating_sub(1);
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return result,
            }
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[test]
    fn accepted_linux_fixtures_execute_successfully() {
        for (name, kind) in [
            ("executable", object::elf::ET_EXEC.0),
            ("pie", object::elf::ET_DYN.0),
        ] {
            let root = tempfile::tempdir().expect("root");
            let path = root.path().join(name);
            let mut fixture = fs::File::create(&path).expect("create fixture");
            fixture
                .write_all(&elf_aarch64(kind, object::elf::EM_AARCH64.0))
                .expect("write fixture");
            fixture
                .set_permissions(fs::Permissions::from_mode(0o755))
                .expect("fixture permissions");
            fixture.sync_all().expect("sync fixture");
            drop(fixture);
            assert!(
                execute_generated_fixture(&path)
                    .expect("execute fixture")
                    .success(),
                "{name} fixture did not execute successfully"
            );
        }
    }

    #[test]
    fn version_output_requires_one_exact_lf_terminated_record() {
        validate_version_output("stab 0.2.0\n").expect("exact version");
        for invalid in [
            "stab 0.2.0",
            "stab 0.2.0\r\n",
            "stab 0.2.0\n\n",
            " stab 0.2.0\n",
            "stab 0.2.0 \n",
            "stab 0.2.0\nextra",
        ] {
            assert!(
                validate_version_output(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn version_execution_uses_the_retained_binary_descriptor() {
        let root = tempfile::tempdir().expect("root");
        let directory =
            safe_fs::RetainedDirectory::create_new_under(root.path(), Path::new("bin"), None)
                .expect("directory");
        let original_path = directory.path().join("stab");
        let original = directory
            .write_new(OsStr::new("stab"), b"#!/bin/sh\nprintf 'stab 0.2.0\\n'\n")
            .expect("original");
        original
            .set_permissions(fs::Permissions::from_mode(0o755))
            .expect("permissions");
        drop(original);
        let retained = directory
            .open_regular(OsStr::new("stab"))
            .expect("retained binary");
        fs::rename(&original_path, directory.path().join("displaced")).expect("displace");
        fs::write(&original_path, b"#!/bin/sh\nprintf 'stab 9.9.9\\n'\n").expect("replacement");
        fs::set_permissions(&original_path, fs::Permissions::from_mode(0o755))
            .expect("replacement permissions");

        let version = capture_version_from_descriptor(root.path(), &retained, &original_path)
            .expect("descriptor version");
        assert_eq!(version, "stab 0.2.0\n");
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

    #[test]
    fn reviewed_assets_retain_original_bytes_and_detect_path_replacement() {
        let (root, assets) = tagged_asset_repository();
        let mut reviewed = review_assets(root.path(), &assets, RELEASE_TAG).expect("review assets");
        assert_eq!(reviewed.assets().len(), 6);
        let original = reviewed
            .assets()
            .iter()
            .find(|asset| asset.name() == "stab-linux-aarch64")
            .map(|asset| asset.sha256().to_string())
            .expect("Linux asset");
        let path = root.path().join(&assets).join("stab-linux-aarch64");
        fs::rename(&path, path.with_extension("reviewed")).expect("displace reviewed path");
        fs::write(&path, b"replacement bytes").expect("replacement");

        let retained = reviewed
            .assets_mut()
            .iter_mut()
            .find(|asset| asset.name() == "stab-linux-aarch64")
            .expect("retained Linux asset");
        let retained_bytes = safe_fs::read_bounded_file(
            retained.upload_file().expect("upload descriptor"),
            Path::new("retained Linux asset"),
            MAX_BINARY_BYTES,
        )
        .expect("retained bytes");
        assert_eq!(archive::sha256_bytes(&retained_bytes), original);
        assert!(matches!(
            reviewed.revalidate(),
            Err(ReleaseError::FileIdentityChanged(_))
        ));
    }

    #[test]
    fn reviewed_assets_detect_same_inode_overwrite() {
        let (root, assets) = tagged_asset_repository();
        let reviewed = review_assets(root.path(), &assets, RELEASE_TAG).expect("review assets");
        let path = root.path().join(&assets).join("stab-linux-aarch64");
        let byte_count = fs::metadata(&path).expect("asset metadata").len();
        let byte_count = usize::try_from(byte_count).expect("asset size fits usize");
        fs::write(&path, vec![0x5a; byte_count]).expect("overwrite asset");

        assert!(matches!(
            reviewed.revalidate(),
            Err(ReleaseError::BinaryContract(detail))
                if detail.contains("changed after validation")
        ));
    }

    #[test]
    fn reviewed_assets_detect_same_inode_truncation() {
        let (root, assets) = tagged_asset_repository();
        let reviewed = review_assets(root.path(), &assets, RELEASE_TAG).expect("review assets");
        let path = root.path().join(&assets).join("stab-linux-aarch64");
        let byte_count = fs::metadata(&path).expect("asset metadata").len();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open asset")
            .set_len(byte_count - 1)
            .expect("truncate asset");

        assert!(matches!(
            reviewed.revalidate(),
            Err(ReleaseError::BinaryContract(detail))
                if detail.contains("changed after validation")
        ));
    }

    #[test]
    fn reviewed_assets_detect_same_inode_append() {
        let (root, assets) = tagged_asset_repository();
        let reviewed = review_assets(root.path(), &assets, RELEASE_TAG).expect("review assets");
        let path = root.path().join(&assets).join("stab-linux-aarch64");
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open asset")
            .write_all(b"appended")
            .expect("append asset");

        assert!(reviewed.revalidate().is_err());
    }

    #[test]
    fn reviewed_assets_detect_extra_directory_entry() {
        let (root, assets) = tagged_asset_repository();
        let reviewed = review_assets(root.path(), &assets, RELEASE_TAG).expect("review assets");
        fs::write(root.path().join(&assets).join("unexpected"), b"extra").expect("extra entry");

        assert!(matches!(
            reviewed.revalidate(),
            Err(ReleaseError::BinaryContract(detail))
                if detail.contains("release asset set differs")
        ));
    }

    #[test]
    fn reviewed_assets_reject_mutated_cargo_identity() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "linux-aarch64", |toolchain| {
            toolchain.cargo_version =
                toolchain
                    .cargo_version
                    .replacen("commit-hash: ", "commit-hash: mutated-", 1);
        });

        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_mutated_rustc_identity() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "macos-aarch64", |toolchain| {
            toolchain.rustc_version =
                toolchain
                    .rustc_version
                    .replacen("commit-hash: ", "commit-hash: mutated-", 1);
        });

        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_mutated_active_toolchain() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "linux-aarch64", |toolchain| {
            toolchain.active_toolchain.insert_str(0, "mutated-");
        });

        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_missing_verbose_identity_fields() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "linux-aarch64", |toolchain| {
            toolchain.cargo_version = remove_verbose_field(&toolchain.cargo_version, "ssl");
        });
        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));

        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "macos-aarch64", |toolchain| {
            toolchain.rustc_version =
                remove_verbose_field(&toolchain.rustc_version, "LLVM version");
        });
        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_arbitrary_platform_identity_fields() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "linux-aarch64", |toolchain| {
            toolchain.cargo_version =
                replace_verbose_field(&toolchain.cargo_version, "libcurl", "arbitrary");
        });
        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_linux_metadata_for_macos() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "macos-aarch64", |toolchain| {
            toolchain.cargo_version = replace_verbose_field(
                &toolchain.cargo_version,
                "os",
                "Ubuntu 24.4.0 (noble) [64-bit]",
            );
        });
        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }

    #[test]
    fn reviewed_assets_reject_mismatched_toolchain_roots() {
        let (root, assets) = tagged_asset_repository();
        mutate_manifest_toolchain(root.path(), &assets, "macos-aarch64", |toolchain| {
            let rustc = Path::new(&toolchain.rustc_program)
                .file_name()
                .expect("rustc program name");
            toolchain.rustc_program = Path::new("/different/.rustup/toolchains")
                .join(
                    toolchain
                        .active_toolchain
                        .split_once(" (overridden by '")
                        .map(|(name, _)| name)
                        .expect("active toolchain name"),
                )
                .join("bin")
                .join(rustc)
                .display()
                .to_string();
        });
        assert!(matches!(
            review_assets(root.path(), &assets, RELEASE_TAG),
            Err(ReleaseError::ToolchainIdentity(_))
        ));
    }
}
