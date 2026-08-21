use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{ReleaseError, process};

const MAX_COMMAND_OUTPUT: usize = 8 << 20;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const TOOLCHAIN_TIMEOUT: Duration = Duration::from_secs(30);
const CARGO_VERBOSE_FIELDS: [&str; 8] = [
    "release",
    "commit-hash",
    "commit-date",
    "host",
    "libgit2",
    "libcurl",
    "ssl",
    "os",
];
const RUSTC_VERBOSE_FIELDS: [&str; 6] = [
    "binary",
    "commit-hash",
    "commit-date",
    "host",
    "release",
    "LLVM version",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolchainIdentity {
    pub(crate) cargo_program: String,
    pub(crate) cargo_version: String,
    pub(crate) rustc_program: String,
    pub(crate) rustc_version: String,
    pub(crate) active_toolchain: String,
}

pub(crate) fn require_clean(root: &Path) -> Result<String, ReleaseError> {
    let status = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
            OsStr::new("--untracked-files=normal"),
            OsStr::new("--ignore-submodules=none"),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    if !status.trim().is_empty() {
        return Err(ReleaseError::DirtyRepository(status.trim().to_string()));
    }
    let commit = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("HEAD"),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    let commit = commit.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ReleaseError::PackageContract(format!(
            "git returned invalid HEAD identity {commit:?}"
        )));
    }
    Ok(commit)
}

pub(crate) fn require_unchanged(root: &Path, before: &str) -> Result<(), ReleaseError> {
    let after = require_clean(root)?;
    if after != before {
        return Err(ReleaseError::RepositoryChanged {
            before: before.to_string(),
            after,
        });
    }
    Ok(())
}

pub(crate) fn require_clean_tag(root: &Path, tag: &str) -> Result<String, ReleaseError> {
    let head = require_clean(root)?;
    let tag_ref = format!("refs/tags/{tag}");
    let kind = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("cat-file"),
            OsStr::new("-t"),
            OsStr::new(&tag_ref),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    if kind.trim() != "tag" {
        return Err(ReleaseError::TagKind {
            tag: tag.to_string(),
        });
    }
    let peeled = format!("{tag_ref}^{{commit}}");
    let tag_commit = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&peeled),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?
    .trim()
    .to_string();
    if tag_commit != head {
        return Err(ReleaseError::TagCommit {
            tag: tag.to_string(),
            tag_commit,
            head,
        });
    }
    Ok(head)
}

pub(crate) fn capture_toolchain(root: &Path) -> Result<ToolchainIdentity, ReleaseError> {
    let cargo = toolchain_program(root, "cargo")?;
    let rustc = toolchain_program(root, "rustc")?;
    let rustup = rustup_program()?;
    let rustup_environment = rustup_environment()?;
    Ok(ToolchainIdentity {
        cargo_program: cargo.to_string_lossy().into_owned(),
        cargo_version: run_capture(
            root,
            cargo.as_os_str(),
            [OsStr::new("--version"), OsStr::new("--verbose")],
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
        rustc_program: rustc.to_string_lossy().into_owned(),
        rustc_version: run_capture(
            root,
            rustc.as_os_str(),
            [OsStr::new("--version"), OsStr::new("--verbose")],
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
        active_toolchain: run_capture_with_environment(
            root,
            rustup.as_os_str(),
            [OsStr::new("show"), OsStr::new("active-toolchain")],
            &rustup_environment,
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
    })
}

pub(crate) fn require_toolchain(
    root: &Path,
    expected: &ToolchainIdentity,
) -> Result<(), ReleaseError> {
    let actual = capture_toolchain(root)?;
    if actual != *expected {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "expected {expected:?}, found {actual:?}"
        )));
    }
    Ok(())
}

pub(crate) fn require_reviewed_asset_toolchain(
    current: &ToolchainIdentity,
    reviewed: &ToolchainIdentity,
    target_host: &str,
) -> Result<(), ReleaseError> {
    let target_family = ToolchainTargetFamily::from_host(target_host)?;
    let current_cargo = parse_version_record("current Cargo", &current.cargo_version)?;
    let current_rustc = parse_version_record("current rustc", &current.rustc_version)?;
    require_complete_fields("current Cargo", &current_cargo, &CARGO_VERBOSE_FIELDS)?;
    require_complete_fields("current rustc", &current_rustc, &RUSTC_VERBOSE_FIELDS)?;
    let current_host = current_rustc.required("host")?;
    let current_name = parse_active_toolchain("current", &current.active_toolchain)?;
    let channel = current_name
        .strip_suffix(&format!("-{current_host}"))
        .ok_or_else(|| {
            ReleaseError::ToolchainIdentity(format!(
                "current active toolchain {current_name:?} does not end with host {current_host:?}"
            ))
        })?;
    let expected_name = format!("{channel}-{target_host}");

    require_toolchain_programs(
        "current",
        &current.cargo_program,
        &current.rustc_program,
        current_name,
    )?;
    require_toolchain_programs(
        "reviewed",
        &reviewed.cargo_program,
        &reviewed.rustc_program,
        &expected_name,
    )?;
    let reviewed_name = parse_active_toolchain("reviewed", &reviewed.active_toolchain)?;
    if reviewed_name != expected_name {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "reviewed active toolchain is {reviewed_name:?}, expected {expected_name:?}"
        )));
    }

    let reviewed_cargo = parse_version_record("reviewed Cargo", &reviewed.cargo_version)?;
    let reviewed_rustc = parse_version_record("reviewed rustc", &reviewed.rustc_version)?;
    require_complete_fields("reviewed Cargo", &reviewed_cargo, &CARGO_VERBOSE_FIELDS)?;
    require_complete_fields("reviewed rustc", &reviewed_rustc, &RUSTC_VERBOSE_FIELDS)?;
    require_cargo_identity(&current_cargo, &reviewed_cargo, target_host, target_family)?;
    require_rustc_identity(&current_rustc, &reviewed_rustc, target_host)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolchainTargetFamily {
    Linux,
    Macos,
}

impl ToolchainTargetFamily {
    fn from_host(host: &str) -> Result<Self, ReleaseError> {
        match host {
            "aarch64-unknown-linux-gnu" => Ok(Self::Linux),
            "aarch64-apple-darwin" => Ok(Self::Macos),
            _ => Err(ReleaseError::ToolchainIdentity(format!(
                "reviewed toolchain host {host:?} has no release target family"
            ))),
        }
    }

    fn require_platform_metadata(self, cargo: &VersionRecord<'_>) -> Result<(), ReleaseError> {
        let libcurl = cargo.required("libcurl")?;
        let ssl = cargo.required("ssl")?;
        let os = cargo.required("os")?;
        require_libcurl_metadata(libcurl)?;
        require_ssl_metadata(ssl)?;
        if !os.ends_with(" [64-bit]")
            || os.bytes().any(|byte| byte.is_ascii_control())
            || match self {
                Self::Linux => !linux_os_metadata(os),
                Self::Macos => !(os.starts_with("Mac OS ") || os.starts_with("macOS ")),
            }
        {
            return Err(ReleaseError::ToolchainIdentity(format!(
                "reviewed Cargo OS metadata {os:?} does not match {self:?}"
            )));
        }
        Ok(())
    }
}

struct VersionRecord<'a> {
    header: &'a str,
    fields: BTreeMap<&'a str, &'a str>,
}

impl<'a> VersionRecord<'a> {
    fn required(&self, key: &str) -> Result<&'a str, ReleaseError> {
        self.fields.get(key).copied().ok_or_else(|| {
            ReleaseError::ToolchainIdentity(format!("toolchain version output is missing {key:?}"))
        })
    }
}

fn parse_version_record<'a>(label: &str, text: &'a str) -> Result<VersionRecord<'a>, ReleaseError> {
    let body = text.strip_suffix('\n').ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!("{label} version output is not LF terminated"))
    })?;
    if body.contains('\r') || body.is_empty() {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} version output has invalid record framing"
        )));
    }
    let mut lines = body.split('\n');
    let header = lines.next().unwrap_or_default();
    if header.is_empty() || header.contains(':') {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} version output has an invalid header"
        )));
    }
    let mut fields = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once(": ").ok_or_else(|| {
            ReleaseError::ToolchainIdentity(format!("{label} version output has an invalid field"))
        })?;
        if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
            return Err(ReleaseError::ToolchainIdentity(format!(
                "{label} version output has an empty or duplicate field"
            )));
        }
    }
    Ok(VersionRecord { header, fields })
}

fn parse_active_toolchain<'a>(label: &str, text: &'a str) -> Result<&'a str, ReleaseError> {
    let body = text.strip_suffix('\n').ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "{label} active-toolchain output is not LF terminated"
        ))
    })?;
    let (name, reason) = body.split_once(" (overridden by '").ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "{label} active toolchain is not bound to rust-toolchain.toml"
        ))
    })?;
    let override_path = reason.strip_suffix("')").ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "{label} active-toolchain output has invalid framing"
        ))
    })?;
    let override_path = Path::new(override_path);
    if name.is_empty()
        || !override_path.is_absolute()
        || override_path.file_name() != Some(OsStr::new("rust-toolchain.toml"))
    {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} active-toolchain output has an invalid source override"
        )));
    }
    Ok(name)
}

fn require_toolchain_programs(
    label: &str,
    cargo_program: &str,
    rustc_program: &str,
    expected_name: &str,
) -> Result<(), ReleaseError> {
    let cargo_root = toolchain_program_root(label, cargo_program, expected_name, "cargo")?;
    let rustc_root = toolchain_program_root(label, rustc_program, expected_name, "rustc")?;
    if cargo_root != rustc_root {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} Cargo and rustc programs do not share one pinned toolchain root"
        )));
    }
    Ok(())
}

fn toolchain_program_root<'a>(
    label: &str,
    program: &'a str,
    expected_name: &str,
    binary: &str,
) -> Result<&'a Path, ReleaseError> {
    let path = Path::new(program);
    let bin_directory = path.parent();
    let toolchain_root = bin_directory.and_then(Path::parent);
    let toolchains_directory = toolchain_root.and_then(Path::parent);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path.file_name() != Some(OsStr::new(binary))
        || bin_directory.and_then(Path::file_name) != Some(OsStr::new("bin"))
        || toolchain_root.and_then(Path::file_name) != Some(OsStr::new(expected_name))
        || toolchains_directory.and_then(Path::file_name) != Some(OsStr::new("toolchains"))
    {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} {binary} program is not from pinned toolchain {expected_name:?}"
        )));
    }
    toolchain_root.ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "{label} {binary} program has no pinned toolchain root"
        ))
    })
}

fn require_complete_fields(
    label: &str,
    record: &VersionRecord<'_>,
    expected: &[&str],
) -> Result<(), ReleaseError> {
    let complete = record.fields.len() == expected.len()
        && expected
            .iter()
            .all(|field| record.fields.contains_key(field));
    if !complete {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "{label} version record fields are incomplete or unexpected"
        )));
    }
    Ok(())
}

fn require_cargo_identity(
    current: &VersionRecord<'_>,
    reviewed: &VersionRecord<'_>,
    target_host: &str,
    target_family: ToolchainTargetFamily,
) -> Result<(), ReleaseError> {
    if current.header != reviewed.header {
        return Err(ReleaseError::ToolchainIdentity(
            "reviewed Cargo version record differs from the current pinned toolchain".to_owned(),
        ));
    }
    for field in ["release", "commit-hash", "commit-date", "libgit2"] {
        if current.required(field)? != reviewed.required(field)? {
            return Err(ReleaseError::ToolchainIdentity(format!(
                "reviewed Cargo field {field:?} differs from the current pinned identity"
            )));
        }
    }
    if reviewed.required("host")? != target_host {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "reviewed Cargo host differs from {target_host:?}"
        )));
    }
    require_library_metadata("libgit2", reviewed.required("libgit2")?)?;
    target_family.require_platform_metadata(reviewed)
}

fn require_rustc_identity(
    current: &VersionRecord<'_>,
    reviewed: &VersionRecord<'_>,
    target_host: &str,
) -> Result<(), ReleaseError> {
    if current.header != reviewed.header {
        return Err(ReleaseError::ToolchainIdentity(
            "reviewed rustc version record differs from the current pinned toolchain".to_string(),
        ));
    }
    for field in [
        "binary",
        "commit-hash",
        "commit-date",
        "release",
        "LLVM version",
    ] {
        if current.required(field)? != reviewed.required(field)? {
            return Err(ReleaseError::ToolchainIdentity(format!(
                "reviewed rustc field {field:?} differs from the current pinned identity"
            )));
        }
    }
    if reviewed.required("host")? != target_host {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "reviewed rustc host differs from {target_host:?}"
        )));
    }
    Ok(())
}

fn require_library_metadata(label: &str, value: &str) -> Result<(), ReleaseError> {
    let (version, details) = value.split_once(" (sys:").ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "reviewed Cargo {label} metadata is not structured"
        ))
    })?;
    let details = details.strip_suffix(')').ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "reviewed Cargo {label} metadata has invalid framing"
        ))
    })?;
    let (sys_version, linkage) = details.split_once(' ').ok_or_else(|| {
        ReleaseError::ToolchainIdentity(format!(
            "reviewed Cargo {label} metadata is missing linkage"
        ))
    })?;
    if !valid_version_token(version) || !valid_version_token(sys_version) || linkage.is_empty() {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "reviewed Cargo {label} metadata has invalid version or linkage"
        )));
    }
    Ok(())
}

fn require_libcurl_metadata(value: &str) -> Result<(), ReleaseError> {
    require_library_metadata("libcurl", value)?;
    let linkage = value
        .split_once(" (sys:")
        .and_then(|(_, details)| details.strip_suffix(')'))
        .and_then(|details| details.split_once(' '))
        .map(|(_, linkage)| linkage)
        .ok_or_else(|| {
            ReleaseError::ToolchainIdentity(
                "reviewed Cargo libcurl metadata is not structured".to_string(),
            )
        })?;
    let known_linkage = linkage.starts_with("vendored ssl:") || linkage.starts_with("system ssl:");
    let known_tls = linkage.contains("OpenSSL/")
        || linkage.contains("LibreSSL/")
        || linkage.contains("SecureTransport");
    if !known_linkage || !known_tls || linkage.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(ReleaseError::ToolchainIdentity(
            "reviewed Cargo libcurl linkage metadata is invalid".to_string(),
        ));
    }
    Ok(())
}

fn linux_os_metadata(value: &str) -> bool {
    [
        "Alpine Linux ",
        "Amazon Linux ",
        "Arch Linux ",
        "CentOS ",
        "Debian ",
        "Fedora Linux ",
        "Linux ",
        "NixOS ",
        "Red Hat Enterprise Linux ",
        "Rocky Linux ",
        "SUSE Linux ",
        "Ubuntu ",
        "openSUSE ",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
}

fn require_ssl_metadata(value: &str) -> Result<(), ReleaseError> {
    if !(value.starts_with("OpenSSL ") || value.starts_with("LibreSSL "))
        || !value.bytes().any(|byte| byte.is_ascii_digit())
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(ReleaseError::ToolchainIdentity(
            "reviewed Cargo SSL metadata is invalid".to_string(),
        ));
    }
    Ok(())
}

fn valid_version_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'_'))
        && value.bytes().any(|byte| byte.is_ascii_digit())
}

pub(crate) fn toolchain_program(root: &Path, program: &str) -> Result<PathBuf, ReleaseError> {
    let rustup = rustup_program()?;
    let output = run_capture_with_environment(
        root,
        rustup.as_os_str(),
        [OsStr::new("which"), OsStr::new(program)],
        &rustup_environment()?,
        TOOLCHAIN_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    let path_text = output
        .strip_suffix("\r\n")
        .or_else(|| output.strip_suffix('\n'))
        .ok_or_else(|| {
            ReleaseError::ToolchainIdentity(format!(
                "rustup which {program} did not return one terminated path"
            ))
        })?;
    if path_text.contains('\n') || path_text.is_empty() {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "rustup which {program} returned an invalid path"
        )));
    }
    let path = PathBuf::from(path_text);
    if !path.is_absolute() {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "rustup which {program} returned a relative path"
        )));
    }
    drop(crate::safe_fs::open_regular_file(&path)?);
    Ok(path)
}

pub(crate) fn run_capture<I, S>(
    root: &Path,
    program: &OsStr,
    args: I,
    timeout: Duration,
    limit: usize,
) -> Result<String, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_capture_with_environment(root, program, args, &[], timeout, limit)
}

pub(crate) fn run_cargo_capture<I, S>(
    root: &Path,
    args: I,
    timeout: Duration,
    limit: usize,
) -> Result<String, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cargo = toolchain_program(root, "cargo")?;
    let rustc_path = toolchain_program(root, "rustc")?;
    let rustc_file = crate::safe_fs::open_regular_file(&rustc_path)?;
    let rustc = crate::safe_fs::descriptor_program(&rustc_file, &rustc_path)?;
    let environment = [(
        OsString::from("RUSTC"),
        rustc.path().as_os_str().to_os_string(),
    )];
    run_capture_with_environment(root, cargo.as_os_str(), args, &environment, timeout, limit)
}

fn run_capture_with_environment<I, S>(
    root: &Path,
    program: &OsStr,
    args: I,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    limit: usize,
) -> Result<String, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = process::run(root, program, args, environment, timeout, limit)?;
    String::from_utf8(output.stdout).map_err(ReleaseError::from)
}

fn rustup_program() -> Result<PathBuf, ReleaseError> {
    let mut candidates = Vec::new();
    if let Some(cargo_home) = std::env::var_os("CARGO_HOME") {
        candidates.push(PathBuf::from(cargo_home).join("bin/rustup"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".cargo/bin/rustup"));
    }
    candidates.push(PathBuf::from("/usr/bin/rustup"));
    for candidate in candidates {
        if crate::safe_fs::open_regular_file(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err(ReleaseError::ToolchainIdentity(
        "could not resolve rustup from CARGO_HOME, HOME, or /usr/bin".to_string(),
    ))
}

fn rustup_environment() -> Result<Vec<(OsString, OsString)>, ReleaseError> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        ReleaseError::ToolchainIdentity(
            "HOME is required to resolve the pinned Rust toolchain".to_string(),
        )
    })?;
    let mut environment = vec![(OsString::from("HOME"), home)];
    if let Some(rustup_home) = std::env::var_os("RUSTUP_HOME") {
        environment.push((OsString::from("RUSTUP_HOME"), rustup_home));
    }
    Ok(environment)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::RELEASE_TAG;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    }

    fn repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("repository");
        git(root.path(), &["init", "-b", "main"]);
        git(root.path(), &["config", "user.name", "Stab Release Test"]);
        git(
            root.path(),
            &["config", "user.email", "release-test@example.invalid"],
        );
        fs::write(root.path().join("tracked"), b"source").expect("tracked file");
        git(root.path(), &["add", "tracked"]);
        git(root.path(), &["commit", "-m", "source"]);
        root
    }

    #[test]
    fn annotated_release_tag_must_match_clean_head() {
        let root = repository();
        git(root.path(), &["tag", "-a", RELEASE_TAG, "-m", "release"]);
        let head = require_clean(root.path()).expect("clean head");
        assert_eq!(
            require_clean_tag(root.path(), RELEASE_TAG).expect("release tag"),
            head
        );

        fs::write(root.path().join("tracked"), b"changed").expect("dirty file");
        assert!(matches!(
            require_clean_tag(root.path(), RELEASE_TAG),
            Err(ReleaseError::DirtyRepository(_))
        ));
    }

    #[test]
    fn lightweight_tags_are_rejected_and_other_annotated_tags_are_supported() {
        let root = repository();
        git(root.path(), &["tag", RELEASE_TAG]);
        assert!(matches!(
            require_clean_tag(root.path(), RELEASE_TAG),
            Err(ReleaseError::TagKind { .. })
        ));
        git(
            root.path(),
            &["tag", "-a", "rehearsal-tag", "-m", "rehearsal"],
        );
        require_clean_tag(root.path(), "rehearsal-tag").expect("alternate annotated tag");
    }
}
