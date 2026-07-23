use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::executable::SealedExecutable;
use super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::protocol::Sha256Digest;
use super::toolchain::ToolchainEvidence;
#[cfg(test)]
use super::worker;
use super::worker::WorkerIdentity;
use crate::root::RepoRoot;

const RECEIPT_SCHEMA_VERSION: u32 = 6;
const BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const BUILD_OUTPUT_LIMIT: usize = 16 << 20;
const MAX_SOURCE_INPUT_BYTES: u64 = 16 << 20;
const RUNTIME_PARENT: &str = "/tmp";
const WORKER_SOURCES: [(&str, &str); 13] = [
    ("worker.rs", "ops/bench/src/qualification/runtime/worker.rs"),
    (
        "worker/bits.rs",
        "ops/bench/src/qualification/runtime/worker/bits.rs",
    ),
    (
        "worker/clifford_string.rs",
        "ops/bench/src/qualification/runtime/worker/clifford_string.rs",
    ),
    (
        "worker/dem_model.rs",
        "ops/bench/src/qualification/runtime/worker/dem_model.rs",
    ),
    (
        "worker/not_zero.rs",
        "ops/bench/src/qualification/runtime/worker/not_zero.rs",
    ),
    (
        "worker/pauli.rs",
        "ops/bench/src/qualification/runtime/worker/pauli.rs",
    ),
    (
        "worker/pauli_iter.rs",
        "ops/bench/src/qualification/runtime/worker/pauli_iter.rs",
    ),
    (
        "worker/prepared.rs",
        "ops/bench/src/qualification/runtime/worker/prepared.rs",
    ),
    (
        "worker/sparse_xor.rs",
        "ops/bench/src/qualification/runtime/worker/sparse_xor.rs",
    ),
    (
        "worker/transpose.rs",
        "ops/bench/src/qualification/runtime/worker/transpose.rs",
    ),
    (
        "worker/workload.rs",
        "ops/bench/src/qualification/runtime/worker/workload.rs",
    ),
    (
        "worker/error.rs",
        "ops/bench/src/qualification/runtime/worker/error.rs",
    ),
    (
        "benchmarks/fixtures/pq2-clifford-string-vectors.json",
        "benchmarks/fixtures/pq2-clifford-string-vectors.json",
    ),
];
const FINGERPRINT_PLACEHOLDER: &str = "$FINGERPRINT";
const RUNTIME_PLACEHOLDER: &str = "$RUNTIME";
const SOURCE_PLACEHOLDER: &str = "$SOURCE";

#[derive(Debug)]
pub(super) struct StabWorkerExecutable {
    _runtime: tempfile::TempDir,
    executable: SealedExecutable,
    identity: WorkerIdentity,
    receipt: StabBuildReceipt,
    source_root: PathBuf,
}

impl StabWorkerExecutable {
    pub(super) fn prepare(
        root: &RepoRoot,
        repository_commit: &str,
        toolchain: &ToolchainEvidence,
    ) -> Result<Self, StabBuildError> {
        let runtime = tempfile::Builder::new()
            .prefix("stab-pq1-build-")
            .tempdir_in(RUNTIME_PARENT)
            .map_err(StabBuildError::Runtime)?;
        let source = runtime.path().join("source");
        let cargo_home = runtime.path().join("cargo-home");
        let cargo_target = runtime.path().join("cargo-target");
        let home = runtime.path().join("home");
        let scratch = runtime.path().join("tmp");
        let xdg = runtime.path().join("xdg");
        for path in [&source, &cargo_home, &cargo_target, &home, &scratch, &xdg] {
            create_private_directory(path)?;
        }
        super::git::materialize_repository_commit(root, repository_commit, &source)?;
        link_cargo_cache(&cargo_home)?;

        let worker_source_sha256 = digest_materialized_worker_source(&source)?;
        let cargo_lock_sha256 = digest_file(&source.join("Cargo.lock"))?;
        let workspace_manifest_sha256 = digest_file(&source.join("Cargo.toml"))?;
        let package_manifest_sha256 = digest_file(&source.join("ops/bench/Cargo.toml"))?;
        let linker = canonical_linker()?;
        let linker_sha256 = super::adapter::sha256_regular_file(&linker, 512 << 20)?;
        let build_arguments = normalized_build_arguments();
        let build_environment = normalized_build_environment(toolchain, &linker, &linker_sha256)?;
        let mut receipt = StabBuildReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            repository_commit: repository_commit.to_string(),
            worker_source_sha256,
            cargo_lock_sha256,
            workspace_manifest_sha256,
            package_manifest_sha256,
            cargo: BuildToolIdentity {
                path: toolchain.cargo_path.clone(),
                sha256: toolchain.cargo_sha256.clone(),
                version: toolchain.cargo_verbose_version.clone(),
            },
            rustc: BuildToolIdentity {
                path: toolchain.rustc_path.clone(),
                sha256: toolchain.rustc_sha256.clone(),
                version: toolchain.rustc_verbose_version.clone(),
            },
            linker_path: linker.to_string_lossy().into_owned(),
            linker_sha256,
            target_triple: toolchain.target_triple.clone(),
            cargo_profile: "release".to_string(),
            build_arguments,
            build_environment,
            build_fingerprint: String::new(),
            binary_sha256: String::new(),
        };
        receipt.build_fingerprint = receipt.recomputed_build_fingerprint()?;
        let arguments = expand_arguments(&receipt.build_arguments, &source)?;
        let environment = expand_environment(
            &receipt.build_environment,
            runtime.path(),
            &source,
            &receipt.build_fingerprint,
        )?;
        let cargo = PathBuf::from(&receipt.cargo.path);
        let output = run_bounded_process(&ProcessRequest {
            program: cargo,
            args: arguments,
            stdin: Vec::new(),
            working_directory: PathBuf::from("/"),
            environment: environment.into(),
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 0,
                stdout: (BUILD_OUTPUT_LIMIT).into(),
                stderr: (BUILD_OUTPUT_LIMIT).into(),
                regular_file_bytes: None,
                timeout: BUILD_TIMEOUT,
            },
        })?;
        if output.status != Some(0) || !output.stderr.is_empty() {
            return Err(StabBuildError::BuildFailed {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let binary = cargo_target
            .join("release")
            .join(format!("stab-bench{}", std::env::consts::EXE_SUFFIX));
        let executable = SealedExecutable::open("stab-worker", &binary)?;
        receipt.binary_sha256 = executable.sha256().to_string();
        let identity = WorkerIdentity {
            source_digest: Sha256Digest::try_new(receipt.worker_source_sha256.clone())?,
            build_fingerprint: Sha256Digest::try_new(receipt.build_fingerprint.clone())?,
        };
        let worker = Self {
            _runtime: runtime,
            executable,
            identity,
            receipt,
            source_root: source,
        };
        worker.verify(toolchain, repository_commit)?;
        Ok(worker)
    }

    pub(super) fn program(&self) -> PathBuf {
        self.executable.program()
    }

    pub(super) fn identity(&self) -> &WorkerIdentity {
        &self.identity
    }

    pub(super) fn binary_sha256(&self) -> &str {
        self.executable.sha256()
    }

    pub(super) fn receipt(&self) -> &StabBuildReceipt {
        &self.receipt
    }

    pub(super) fn verify(
        &self,
        toolchain: &ToolchainEvidence,
        repository_commit: &str,
    ) -> Result<(), StabBuildError> {
        self.executable.verify()?;
        if digest_materialized_worker_source(&self.source_root)?
            != self.receipt.worker_source_sha256
            || self.receipt.binary_sha256 != self.executable.sha256()
            || !self.receipt.validates_report_identity(
                self.identity.source_digest.as_str(),
                self.identity.build_fingerprint.as_str(),
                self.executable.sha256(),
                repository_commit,
                toolchain,
            )
        {
            return Err(StabBuildError::StaleIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StabBuildReceipt {
    schema_version: u32,
    repository_commit: String,
    worker_source_sha256: String,
    cargo_lock_sha256: String,
    workspace_manifest_sha256: String,
    package_manifest_sha256: String,
    cargo: BuildToolIdentity,
    rustc: BuildToolIdentity,
    linker_path: String,
    linker_sha256: String,
    target_triple: String,
    cargo_profile: String,
    build_arguments: Vec<String>,
    build_environment: Vec<BuildEnvironmentEntry>,
    build_fingerprint: String,
    binary_sha256: String,
}

impl StabBuildReceipt {
    pub(super) fn validates_report_identity(
        &self,
        source_sha256: &str,
        build_fingerprint: &str,
        binary_sha256: &str,
        repository_commit: &str,
        toolchain: &ToolchainEvidence,
    ) -> bool {
        self.schema_version == RECEIPT_SCHEMA_VERSION
            && self.repository_commit == repository_commit
            && self.worker_source_sha256 == source_sha256
            && self.build_fingerprint == build_fingerprint
            && self.binary_sha256 == binary_sha256
            && self.cargo_profile == "release"
            && self.target_triple == toolchain.target_triple
            && self.cargo
                == BuildToolIdentity {
                    path: toolchain.cargo_path.clone(),
                    sha256: toolchain.cargo_sha256.clone(),
                    version: toolchain.cargo_verbose_version.clone(),
                }
            && self.rustc
                == BuildToolIdentity {
                    path: toolchain.rustc_path.clone(),
                    sha256: toolchain.rustc_sha256.clone(),
                    version: toolchain.rustc_verbose_version.clone(),
                }
            && canonical_linker().is_ok_and(|linker| linker == Path::new(&self.linker_path))
            && super::adapter::sha256_regular_file(Path::new(&self.linker_path), 512 << 20)
                .is_ok_and(|digest| digest == self.linker_sha256)
            && self.build_arguments == normalized_build_arguments()
            && valid_receipt_digest(&self.cargo_lock_sha256)
            && valid_receipt_digest(&self.workspace_manifest_sha256)
            && valid_receipt_digest(&self.package_manifest_sha256)
            && valid_receipt_digest(&self.linker_sha256)
            && valid_receipt_digest(&self.binary_sha256)
            && normalized_build_environment(
                toolchain,
                Path::new(&self.linker_path),
                &self.linker_sha256,
            )
            .is_ok_and(|expected| expected == self.build_environment)
            && self
                .recomputed_build_fingerprint()
                .is_ok_and(|actual| actual == self.build_fingerprint)
    }

    fn recomputed_build_fingerprint(&self) -> Result<String, StabBuildError> {
        let material = serde_json::to_vec(&serde_json::json!({
            "schema_version": self.schema_version,
            "repository_commit": self.repository_commit,
            "worker_source_sha256": self.worker_source_sha256,
            "cargo_lock_sha256": self.cargo_lock_sha256,
            "workspace_manifest_sha256": self.workspace_manifest_sha256,
            "package_manifest_sha256": self.package_manifest_sha256,
            "cargo": self.cargo,
            "rustc": self.rustc,
            "linker_path": self.linker_path,
            "linker_sha256": self.linker_sha256,
            "target_triple": self.target_triple,
            "cargo_profile": self.cargo_profile,
            "build_arguments": self.build_arguments,
            "build_environment": self.build_environment,
        }))?;
        Ok(hex_digest(&Sha256::digest(material)))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildToolIdentity {
    path: String,
    sha256: String,
    version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildEnvironmentEntry {
    name: String,
    value: String,
}

fn normalized_build_arguments() -> Vec<String> {
    [
        "build",
        "--offline",
        "--locked",
        "--release",
        "--quiet",
        "-p",
        "stab-bench",
        "--bin",
        "stab-bench",
        "--manifest-path",
        "$SOURCE/Cargo.toml",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn normalized_build_environment(
    toolchain: &ToolchainEvidence,
    linker: &Path,
    linker_sha256: &str,
) -> Result<Vec<BuildEnvironmentEntry>, StabBuildError> {
    if !valid_receipt_digest(linker_sha256) {
        return Err(StabBuildError::InvalidDigest("linker_sha256"));
    }
    let rustc_library = rustc_library_path(Path::new(&toolchain.rustc_path))?;
    let rustflags = format!(
        "-Clinker={} -Cstrip=symbols -Clink-arg=-Wl,--build-id=none --remap-path-prefix=$SOURCE=/stab/source --remap-path-prefix=$RUNTIME=/stab/build",
        linker.display()
    );
    let mut entries = vec![
        entry("CARGO_BUILD_JOBS", "1"),
        entry("CARGO_HOME", "$RUNTIME/cargo-home"),
        entry("CARGO_INCREMENTAL", "0"),
        entry("CARGO_NET_OFFLINE", "true"),
        entry("CARGO_TARGET_DIR", "$RUNTIME/cargo-target"),
        entry("CC", &linker.to_string_lossy()),
        entry("HOME", "$RUNTIME/home"),
        entry("LANG", "C"),
        entry("LC_ALL", "C"),
        entry("LD_LIBRARY_PATH", &rustc_library.to_string_lossy()),
        entry("PATH", "/usr/bin:/bin"),
        entry("RUST_BACKTRACE", "0"),
        entry("RUSTC", &toolchain.rustc_path),
        entry("RUSTFLAGS", &rustflags),
        entry("RUSTUP_TOOLCHAIN", &toolchain.rust_toolchain),
        entry("SOURCE_DATE_EPOCH", "0"),
        entry("STAB_PQ1_BUILD_FINGERPRINT", FINGERPRINT_PLACEHOLDER),
        entry("TMPDIR", "$RUNTIME/tmp"),
        entry("TZ", "UTC"),
        entry("XDG_CONFIG_HOME", "$RUNTIME/xdg"),
    ];
    entries.sort();
    if entries
        .windows(2)
        .any(|pair| matches!(pair, [left, right] if left.name == right.name))
    {
        return Err(StabBuildError::DuplicateEnvironment);
    }
    Ok(entries)
}

fn entry(name: &str, value: &str) -> BuildEnvironmentEntry {
    BuildEnvironmentEntry {
        name: name.to_string(),
        value: value.to_string(),
    }
}

fn expand_arguments(arguments: &[String], source: &Path) -> Result<Vec<OsString>, StabBuildError> {
    arguments
        .iter()
        .map(|argument| expand_value(argument, Path::new("/"), source, ""))
        .collect()
}

fn expand_environment(
    entries: &[BuildEnvironmentEntry],
    runtime: &Path,
    source: &Path,
    fingerprint: &str,
) -> Result<Vec<(OsString, OsString)>, StabBuildError> {
    entries
        .iter()
        .map(|entry| {
            Ok((
                OsString::from(&entry.name),
                expand_value(&entry.value, runtime, source, fingerprint)?,
            ))
        })
        .collect()
}

fn expand_value(
    value: &str,
    runtime: &Path,
    source: &Path,
    fingerprint: &str,
) -> Result<OsString, StabBuildError> {
    let runtime = runtime
        .to_str()
        .ok_or_else(|| StabBuildError::NonUtf8Path(runtime.to_path_buf()))?;
    let source = source
        .to_str()
        .ok_or_else(|| StabBuildError::NonUtf8Path(source.to_path_buf()))?;
    Ok(OsString::from(
        value
            .replace(RUNTIME_PLACEHOLDER, runtime)
            .replace(SOURCE_PLACEHOLDER, source)
            .replace(FINGERPRINT_PLACEHOLDER, fingerprint),
    ))
}

fn rustc_library_path(rustc: &Path) -> Result<PathBuf, StabBuildError> {
    let path = rustc
        .parent()
        .and_then(Path::parent)
        .map(|toolchain| toolchain.join("lib"))
        .ok_or_else(|| StabBuildError::MissingRustcLibrary(rustc.to_path_buf()))?;
    if !path.is_dir() {
        return Err(StabBuildError::MissingRustcLibrary(path));
    }
    Ok(path)
}

fn canonical_linker() -> Result<PathBuf, StabBuildError> {
    let requested = PathBuf::from("/usr/bin/cc");
    let path = std::fs::canonicalize(&requested).map_err(|source| StabBuildError::Io {
        path: requested,
        source,
    })?;
    if !path.is_file() {
        return Err(StabBuildError::MissingLinker(path));
    }
    Ok(path)
}

fn create_private_directory(path: &Path) -> Result<(), StabBuildError> {
    std::fs::create_dir(path).map_err(|source| StabBuildError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn link_cargo_cache(private_home: &Path) -> Result<(), StabBuildError> {
    let source_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or(StabBuildError::MissingCargoHome)?;
    if !source_home.is_absolute() {
        return Err(StabBuildError::InvalidCargoHome(source_home));
    }
    for name in ["registry", "git"] {
        let source = source_home.join(name);
        if !source.exists() {
            continue;
        }
        let source = std::fs::canonicalize(&source).map_err(|error| StabBuildError::Io {
            path: source.clone(),
            source: error,
        })?;
        if !source.is_dir() {
            return Err(StabBuildError::InvalidCargoHome(source));
        }
        std::os::unix::fs::symlink(&source, private_home.join(name)).map_err(|source| {
            StabBuildError::Io {
                path: private_home.join(name),
                source,
            }
        })?;
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<String, StabBuildError> {
    super::adapter::sha256_regular_file(path, MAX_SOURCE_INPUT_BYTES).map_err(StabBuildError::from)
}

fn digest_materialized_worker_source(source: &Path) -> Result<String, StabBuildError> {
    let mut digest = Sha256::new();
    for (logical_path, repository_path) in WORKER_SOURCES {
        let bytes = crate::source_file::read_regular_file_bounded(
            &source.join(repository_path),
            usize::try_from(MAX_SOURCE_INPUT_BYTES)
                .map_err(|_| StabBuildError::SourceInput("source byte limit".to_string()))?,
        )
        .map_err(|error| StabBuildError::SourceInput(error.to_string()))?;
        digest.update(
            u64::try_from(logical_path.len())
                .map_err(|_| StabBuildError::SourceInput("logical path length".to_string()))?
                .to_le_bytes(),
        );
        digest.update(logical_path.as_bytes());
        digest.update(
            u64::try_from(bytes.len())
                .map_err(|_| StabBuildError::SourceInput("source length".to_string()))?
                .to_le_bytes(),
        );
        digest.update(bytes);
    }
    Ok(hex_digest(&digest.finalize()))
}

fn valid_receipt_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
pub(super) enum StabBuildError {
    #[error("failed to create a private Stab build runtime: {0}")]
    Runtime(std::io::Error),
    #[error("private Stab build path {path} failed: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "private Stab build failed with status {status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    BuildFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("private Stab build receipt has invalid {0}")]
    InvalidDigest(&'static str),
    #[error("private Stab worker source collection is invalid: {0}")]
    SourceInput(String),
    #[error("private Stab build environment contains duplicate names")]
    DuplicateEnvironment,
    #[error("private Stab build path is not UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("resolved rustc library directory is missing: {0}")]
    MissingRustcLibrary(PathBuf),
    #[error("resolved system linker is missing: {0}")]
    MissingLinker(PathBuf),
    #[error("HOME or CARGO_HOME is required for offline Cargo cache access")]
    MissingCargoHome,
    #[error("Cargo cache home is not an absolute directory: {0}")]
    InvalidCargoHome(PathBuf),
    #[error("sealed Stab worker or build receipt changed identity")]
    StaleIdentity,
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Adapter(#[from] super::adapter::AdapterError),
    #[error(transparent)]
    Executable(#[from] super::executable::ExecutableError),
    #[error(transparent)]
    Worker(#[from] super::worker::WorkerError),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_receipt_hashes_the_materialized_source() {
        let runtime = tempfile::tempdir().expect("temporary source tree");
        for (_, path) in WORKER_SOURCES {
            let source_path = runtime.path().join(path);
            std::fs::create_dir_all(source_path.parent().expect("worker parent"))
                .expect("materialized source directories");
            std::fs::write(&source_path, format!("materialized {path}\n"))
                .expect("materialized worker source");
        }

        let materialized =
            digest_materialized_worker_source(runtime.path()).expect("materialized digest");
        let controller = worker::source_digest().expect("controller digest");
        assert_ne!(materialized, controller.as_str());

        let bits_path = runtime
            .path()
            .join(WORKER_SOURCES.get(1).expect("bits source contract").1);
        std::fs::write(bits_path, b"changed bits source\n").expect("change bits source");
        let changed = digest_materialized_worker_source(runtime.path()).expect("changed digest");
        assert_ne!(materialized, changed);

        let not_zero_path = runtime
            .path()
            .join(WORKER_SOURCES.get(2).expect("not-zero source contract").1);
        std::fs::write(not_zero_path, b"changed not-zero source\n")
            .expect("change not-zero source");
        let changed_again =
            digest_materialized_worker_source(runtime.path()).expect("changed digest");
        assert_ne!(changed, changed_again);
    }

    #[test]
    fn controller_and_materialized_worker_source_collections_match() {
        let root =
            RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .expect("repository root");
        let materialized =
            digest_materialized_worker_source(&root.path).expect("materialized digest");
        let controller = worker::source_digest().expect("controller digest");
        assert_eq!(materialized, controller.as_str());
    }

    #[test]
    fn build_fingerprint_changes_with_commit_tools_and_flags() {
        let toolchain = ToolchainEvidence {
            rust_toolchain: "nightly-2026-06-20".to_string(),
            cargo_profile: "release".to_string(),
            rustup_path: "/usr/bin/rustup".to_string(),
            rustup_sha256: "1".repeat(64),
            cargo_path: "/toolchain/cargo".to_string(),
            cargo_sha256: "2".repeat(64),
            cargo_verbose_version: "cargo 1".to_string(),
            rustc_path: "/toolchain/bin/rustc".to_string(),
            rustc_sha256: "3".repeat(64),
            rustc_verbose_version: "rustc 1\nhost: x86_64-unknown-linux-gnu".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
        };
        let environment = vec![entry("A", "B")];
        let base = StabBuildReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            repository_commit: "a".repeat(40),
            worker_source_sha256: "b".repeat(64),
            cargo_lock_sha256: "c".repeat(64),
            workspace_manifest_sha256: "d".repeat(64),
            package_manifest_sha256: "e".repeat(64),
            cargo: BuildToolIdentity {
                path: toolchain.cargo_path.clone(),
                sha256: toolchain.cargo_sha256.clone(),
                version: toolchain.cargo_verbose_version.clone(),
            },
            rustc: BuildToolIdentity {
                path: toolchain.rustc_path.clone(),
                sha256: toolchain.rustc_sha256.clone(),
                version: toolchain.rustc_verbose_version.clone(),
            },
            linker_path: "/usr/bin/cc".to_string(),
            linker_sha256: "f".repeat(64),
            target_triple: toolchain.target_triple.clone(),
            cargo_profile: "release".to_string(),
            build_arguments: normalized_build_arguments(),
            build_environment: environment,
            build_fingerprint: String::new(),
            binary_sha256: String::new(),
        };
        let fingerprint = base.recomputed_build_fingerprint().expect("fingerprint");
        let mut changed = base.clone();
        changed.repository_commit = "9".repeat(40);
        assert_ne!(
            fingerprint,
            changed
                .recomputed_build_fingerprint()
                .expect("changed commit")
        );
        changed = base.clone();
        changed.cargo.sha256 = "8".repeat(64);
        assert_ne!(
            fingerprint,
            changed
                .recomputed_build_fingerprint()
                .expect("changed tool")
        );
        changed = base;
        changed.build_arguments.push("--features=x".to_string());
        assert_ne!(
            fingerprint,
            changed
                .recomputed_build_fingerprint()
                .expect("changed flags")
        );
    }
}
