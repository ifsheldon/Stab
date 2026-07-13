use std::ffi::{OsStr, OsString};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::protocol::Sha256Digest;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

const ADAPTER_SOURCE: &str = "benchmarks/stim_adapter/main.cc";
const ADAPTER_OUTPUT: &str = "target/benchmarks/stim-adapter";
const RECEIPT_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 1 << 20;
const MAX_TOOL_BYTES: u64 = 512 << 20;
const MAX_LIBRARY_BYTES: u64 = 1 << 30;
const MAX_RECEIPT_BYTES: usize = 1 << 20;
const BUILD_OUTPUT_BYTES: usize = 16 << 20;
const BUILD_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Clone, Debug)]
pub(crate) struct AdapterExecutable {
    pub(crate) path: PathBuf,
    pub(crate) source_digest: Sha256Digest,
    pub(crate) build_fingerprint: Sha256Digest,
    pub(crate) binary_digest: Sha256Digest,
    pub(crate) receipt: AdapterBuildReceipt,
    source_path: PathBuf,
    library_path: PathBuf,
}

impl AdapterExecutable {
    pub(crate) fn verify(&self) -> Result<(), AdapterError> {
        let binary = sha256_regular_file(&self.path, MAX_TOOL_BYTES)?;
        let source = sha256_regular_file(&self.source_path, MAX_SOURCE_BYTES as u64)?;
        let library = sha256_regular_file(&self.library_path, MAX_LIBRARY_BYTES)?;
        let cmake = sha256_regular_file(Path::new(&self.receipt.cmake.path), MAX_TOOL_BYTES)?;
        let cxx = sha256_regular_file(Path::new(&self.receipt.cxx.path), MAX_TOOL_BYTES)?;
        if binary != self.binary_digest.as_str()
            || self.receipt.binary_sha256 != self.binary_digest.as_str()
            || self.receipt.adapter_source_sha256 != self.source_digest.as_str()
            || self.receipt.build_fingerprint != self.build_fingerprint.as_str()
            || source != self.receipt.adapter_source_sha256
            || library != self.receipt.stim_library_sha256
            || cmake != self.receipt.cmake.sha256
            || cxx != self.receipt.cxx.sha256
        {
            return Err(AdapterError::StaleBuild(
                "adapter identity changed after preparation".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterBuildReceipt {
    schema_version: u32,
    stim_tag: String,
    stim_commit: String,
    adapter_source_sha256: String,
    stim_library_sha256: String,
    cmake: ToolIdentity,
    cxx: ToolIdentity,
    compile_arguments: Vec<String>,
    build_fingerprint: String,
    binary_sha256: String,
}

impl AdapterBuildReceipt {
    pub(super) fn validates_report_identity(
        &self,
        source_sha256: &str,
        build_fingerprint: &str,
        binary_sha256: &str,
    ) -> bool {
        self.schema_version == RECEIPT_SCHEMA_VERSION
            && self.stim_tag == STIM_TAG
            && self.stim_commit == STIM_COMMIT
            && self.adapter_source_sha256 == source_sha256
            && self.build_fingerprint == build_fingerprint
            && self.binary_sha256 == binary_sha256
            && valid_digest(&self.stim_library_sha256)
            && valid_tool_identity(&self.cmake)
            && valid_tool_identity(&self.cxx)
            && !self.compile_arguments.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolIdentity {
    path: String,
    sha256: String,
    version: String,
}

fn valid_tool_identity(identity: &ToolIdentity) -> bool {
    !identity.path.is_empty() && !identity.version.is_empty() && valid_digest(&identity.sha256)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn prepare_adapter(root: &RepoRoot) -> Result<AdapterExecutable, AdapterError> {
    ensure_linux()?;
    crate::stim::validate_stim_source(&root.default_stim_source())
        .map_err(|error| AdapterError::Stim(error.to_string()))?;
    let output = root
        .create_benchmark_output_dir(Path::new(ADAPTER_OUTPUT))
        .map_err(|error| AdapterError::Output(error.to_string()))?;
    let home = ensure_directory(&output.join("home"))?;
    let cmake = tool_identity(root, "cmake", &home)?;
    let cxx = tool_identity(root, "c++", &home)?;
    configure_and_build_stim_library(root, &cmake, &home)?;
    let library = root.stim_library();
    let library_digest = sha256_regular_file(&library, MAX_LIBRARY_BYTES)?;
    let source_path = root.path.join(ADAPTER_SOURCE);
    let source_bytes =
        crate::source_file::read_repo_regular_file_bounded(root, &source_path, MAX_SOURCE_BYTES)
            .map_err(|error| AdapterError::Source(error.to_string()))?;
    let source_digest = sha256_bytes(&source_bytes)?;
    let binary = output.join(format!(
        "stim-qualification-adapter{}",
        std::env::consts::EXE_SUFFIX
    ));
    let receipt_path = output.join("build-receipt.json");
    let fingerprint_arguments =
        compile_arguments(root, &library, &binary, &source_digest, "PENDING");
    let build_fingerprint = build_fingerprint(
        &source_digest,
        &library_digest,
        &cmake,
        &cxx,
        &fingerprint_arguments,
    )?;
    let compile_arguments =
        compile_arguments(root, &library, &binary, &source_digest, &build_fingerprint);
    let expected = AdapterBuildReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        adapter_source_sha256: source_digest.clone(),
        stim_library_sha256: library_digest,
        cmake: cmake.clone(),
        cxx: cxx.clone(),
        compile_arguments: compile_arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect(),
        build_fingerprint: build_fingerprint.clone(),
        binary_sha256: String::new(),
    };

    if let Some(receipt) = reusable_receipt(root, &receipt_path, &binary, &expected)? {
        return executable(binary, source_path, library, receipt);
    }
    build_adapter(root, &home, &cxx, &source_path, &binary, &compile_arguments)?;
    let binary_digest = sha256_regular_file(&binary, MAX_TOOL_BYTES)?;
    let receipt = AdapterBuildReceipt {
        binary_sha256: binary_digest,
        ..expected
    };
    let mut bytes = serde_json::to_vec_pretty(&receipt).map_err(AdapterError::Json)?;
    bytes.push(b'\n');
    crate::source_file::atomic_write_repo_regular_file(root, &receipt_path, &bytes)
        .map_err(|error| AdapterError::Output(error.to_string()))?;
    executable(binary, source_path, library, receipt)
}

fn executable(
    binary: PathBuf,
    source_path: PathBuf,
    library_path: PathBuf,
    receipt: AdapterBuildReceipt,
) -> Result<AdapterExecutable, AdapterError> {
    Ok(AdapterExecutable {
        path: binary,
        source_digest: Sha256Digest::try_new(receipt.adapter_source_sha256.clone())?,
        build_fingerprint: Sha256Digest::try_new(receipt.build_fingerprint.clone())?,
        binary_digest: Sha256Digest::try_new(receipt.binary_sha256.clone())?,
        receipt,
        source_path,
        library_path,
    })
}

fn reusable_receipt(
    root: &RepoRoot,
    receipt_path: &Path,
    binary: &Path,
    expected: &AdapterBuildReceipt,
) -> Result<Option<AdapterBuildReceipt>, AdapterError> {
    if !receipt_path.exists() && !binary.exists() {
        return Ok(None);
    }
    if !receipt_path.is_file() || !binary.is_file() {
        return Err(AdapterError::StaleBuild(
            "adapter binary and receipt must both be regular files".to_string(),
        ));
    }
    let bytes =
        crate::source_file::read_repo_regular_file_bounded(root, receipt_path, MAX_RECEIPT_BYTES)
            .map_err(|error| AdapterError::StaleBuild(error.to_string()))?;
    let receipt: AdapterBuildReceipt =
        serde_json::from_slice(&bytes).map_err(AdapterError::Json)?;
    let mut comparable = receipt.clone();
    comparable.binary_sha256.clear();
    if &comparable != expected {
        return Ok(None);
    }
    let actual_binary = sha256_regular_file(binary, MAX_TOOL_BYTES)?;
    if actual_binary != receipt.binary_sha256 {
        return Err(AdapterError::StaleBuild(
            "adapter binary digest disagrees with its build receipt".to_string(),
        ));
    }
    Ok(Some(receipt))
}

fn configure_and_build_stim_library(
    root: &RepoRoot,
    cmake: &ToolIdentity,
    home: &Path,
) -> Result<(), AdapterError> {
    std::fs::create_dir_all(root.build_dir()).map_err(|source| AdapterError::CreateDirectory {
        path: root.build_dir(),
        source,
    })?;
    run_build_command(
        root,
        Path::new(&cmake.path),
        vec![
            OsString::from("-S"),
            root.default_stim_source().into_os_string(),
            OsString::from("-B"),
            root.build_dir().into_os_string(),
            OsString::from("-DCMAKE_BUILD_TYPE=Release"),
        ],
        home,
    )?;
    run_build_command(
        root,
        Path::new(&cmake.path),
        vec![
            OsString::from("--build"),
            root.build_dir().into_os_string(),
            OsString::from("--target"),
            OsString::from("libstim"),
            OsString::from("--parallel"),
        ],
        home,
    )?;
    if !root.stim_library().is_file() {
        return Err(AdapterError::MissingLibrary(root.stim_library()));
    }
    Ok(())
}

fn build_adapter(
    root: &RepoRoot,
    home: &Path,
    cxx: &ToolIdentity,
    _source: &Path,
    binary: &Path,
    compile_arguments: &[OsString],
) -> Result<(), AdapterError> {
    let temporary = binary.with_extension(format!("building-{}", std::process::id()));
    if temporary.exists() {
        return Err(AdapterError::StaleBuild(format!(
            "temporary adapter output {} already exists",
            temporary.display()
        )));
    }
    let mut arguments = compile_arguments.to_vec();
    let output_index = arguments
        .iter()
        .position(|argument| argument == OsStr::new("-o"))
        .ok_or_else(|| AdapterError::Build("compile arguments omit -o".to_string()))?;
    let output = arguments
        .get_mut(output_index + 1)
        .ok_or_else(|| AdapterError::Build("compile arguments omit the output path".to_string()))?;
    *output = temporary.as_os_str().to_owned();
    let result = run_build_command(root, Path::new(&cxx.path), arguments, home);
    if result.is_err() {
        drop(std::fs::remove_file(&temporary));
        return result;
    }
    let metadata = std::fs::symlink_metadata(&temporary).map_err(|source| AdapterError::Io {
        path: temporary.clone(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(AdapterError::Build(format!(
            "compiler output {} is not a regular file",
            temporary.display()
        )));
    }
    if binary.exists()
        && !std::fs::symlink_metadata(binary)
            .map_err(|source| AdapterError::Io {
                path: binary.to_path_buf(),
                source,
            })?
            .file_type()
            .is_file()
    {
        return Err(AdapterError::StaleBuild(format!(
            "adapter destination {} is not a regular file",
            binary.display()
        )));
    }
    std::fs::rename(&temporary, binary).map_err(|source| AdapterError::Io {
        path: binary.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn compile_arguments(
    root: &RepoRoot,
    library: &Path,
    binary: &Path,
    source_digest: &str,
    build_fingerprint: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("-std=c++20"),
        OsString::from("-O3"),
        OsString::from("-DNDEBUG"),
        OsString::from("-Wall"),
        OsString::from("-Wextra"),
        OsString::from("-Wpedantic"),
        OsString::from("-Werror"),
        OsString::from("-fno-strict-aliasing"),
        OsString::from("-I"),
        root.default_stim_source().join("src").into_os_string(),
        OsString::from(format!("-DSTAB_STIM_COMMIT=\"{STIM_COMMIT}\"")),
        OsString::from(format!("-DSTAB_ADAPTER_SOURCE_DIGEST=\"{source_digest}\"")),
        OsString::from(format!(
            "-DSTAB_ADAPTER_BUILD_FINGERPRINT=\"{build_fingerprint}\""
        )),
        root.path.join(ADAPTER_SOURCE).into_os_string(),
        library.as_os_str().to_owned(),
        OsString::from("-o"),
        binary.as_os_str().to_owned(),
    ]
}

fn build_fingerprint(
    source_digest: &str,
    library_digest: &str,
    cmake: &ToolIdentity,
    cxx: &ToolIdentity,
    arguments: &[OsString],
) -> Result<String, AdapterError> {
    let material = serde_json::to_vec(&serde_json::json!({
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "stim_commit": STIM_COMMIT,
        "source_digest": source_digest,
        "library_digest": library_digest,
        "cmake": cmake,
        "cxx": cxx,
        "arguments": arguments.iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
    }))
    .map_err(AdapterError::Json)?;
    sha256_bytes(&material)
}

fn tool_identity(root: &RepoRoot, name: &str, home: &Path) -> Result<ToolIdentity, AdapterError> {
    let path = resolve_tool(name)?;
    let sha256 = sha256_regular_file(&path, MAX_TOOL_BYTES)?;
    let output = run_build_process(
        root,
        &path,
        vec![OsString::from("--version")],
        home,
        Duration::from_secs(30),
    )?;
    if output.status != Some(0) {
        return Err(AdapterError::Build(format!(
            "{name} --version failed with status {:?}",
            output.status
        )));
    }
    let version = std::str::from_utf8(&output.stdout)
        .map_err(|_| AdapterError::Build(format!("{name} --version is not UTF-8")))?
        .trim()
        .to_string();
    if version.is_empty() || version.len() > 16 << 10 {
        return Err(AdapterError::Build(format!(
            "{name} --version has an invalid length"
        )));
    }
    Ok(ToolIdentity {
        path: path.to_string_lossy().into_owned(),
        sha256,
        version,
    })
}

fn resolve_tool(name: &str) -> Result<PathBuf, AdapterError> {
    let path =
        std::env::var_os("PATH").ok_or_else(|| AdapterError::MissingTool(name.to_string()))?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(name);
        if !candidate.is_file() {
            continue;
        }
        let canonical = std::fs::canonicalize(&candidate).map_err(|source| AdapterError::Io {
            path: candidate,
            source,
        })?;
        if canonical.is_file() {
            return Ok(canonical);
        }
    }
    Err(AdapterError::MissingTool(name.to_string()))
}

fn run_build_command(
    root: &RepoRoot,
    program: &Path,
    arguments: Vec<OsString>,
    home: &Path,
) -> Result<(), AdapterError> {
    let output = run_build_process(root, program, arguments, home, BUILD_TIMEOUT)?;
    if output.status == Some(0) {
        Ok(())
    } else {
        Err(AdapterError::Build(format!(
            "{} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            program.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn run_build_process(
    root: &RepoRoot,
    program: &Path,
    arguments: Vec<OsString>,
    home: &Path,
    timeout: Duration,
) -> Result<super::process::ProcessResult, AdapterError> {
    run_bounded_process(&ProcessRequest {
        program: program.to_path_buf(),
        args: arguments,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment: controlled_environment(home)?,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout_bytes: BUILD_OUTPUT_BYTES,
            stderr_bytes: BUILD_OUTPUT_BYTES,
            regular_file_bytes: None,
            timeout,
        },
    })
    .map_err(|error| AdapterError::Process(error.to_string()))
}

fn controlled_environment(home: &Path) -> Result<Vec<(OsString, OsString)>, AdapterError> {
    Ok(vec![
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (
            OsString::from("PATH"),
            std::env::join_paths([Path::new("/usr/bin"), Path::new("/bin")])
                .map_err(|error| AdapterError::Build(error.to_string()))?,
        ),
        (OsString::from("TZ"), OsString::from("UTC")),
    ])
}

fn ensure_directory(path: &Path) -> Result<PathBuf, AdapterError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => return Ok(path.to_path_buf()),
        Ok(_) => {
            return Err(AdapterError::CreateDirectory {
                path: path.to_path_buf(),
                source: std::io::Error::other("existing path is not a nonsymlink directory"),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(AdapterError::CreateDirectory {
                path: path.to_path_buf(),
                source,
            });
        }
    }
    std::fs::create_dir(path).map_err(|source| AdapterError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(path.to_path_buf())
}

pub(super) fn sha256_regular_file(path: &Path, maximum: u64) -> Result<String, AdapterError> {
    let mut file = crate::source_file::open_regular_file_bounded_descriptor(path, maximum)
        .map_err(|error| AdapterError::SafeFile(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 << 10];
    let mut total = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|source| AdapterError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).map_err(|_| AdapterError::SizeOverflow)?)
            .ok_or(AdapterError::SizeOverflow)?;
        if total > maximum {
            return Err(AdapterError::UnsafeFile {
                path: path.to_path_buf(),
                maximum,
            });
        }
        let chunk = buffer.get(..count).ok_or(AdapterError::SizeOverflow)?;
        hasher.update(chunk);
    }
    hex_digest(&hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> Result<String, AdapterError> {
    hex_digest(&Sha256::digest(bytes))
}

fn hex_digest(bytes: &[u8]) -> Result<String, AdapterError> {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").map_err(|_| AdapterError::DigestEncoding)?;
    }
    Ok(output)
}

fn ensure_linux() -> Result<(), AdapterError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(AdapterError::UnsupportedHost)
    }
}

#[derive(Debug, Error)]
pub(crate) enum AdapterError {
    #[error("pinned Stim qualification adapters require Linux")]
    UnsupportedHost,
    #[error("pinned Stim source validation failed: {0}")]
    Stim(String),
    #[error("adapter output path validation failed: {0}")]
    Output(String),
    #[error("adapter source validation failed: {0}")]
    Source(String),
    #[error("failed to create adapter directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("required build tool {0:?} was not found on PATH")]
    MissingTool(String),
    #[error("pinned Stim library is missing after build: {0}")]
    MissingLibrary(PathBuf),
    #[error("adapter build failed: {0}")]
    Build(String),
    #[error("adapter process failed: {0}")]
    Process(String),
    #[error("stale adapter build: {0}")]
    StaleBuild(String),
    #[error("failed to access adapter file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("adapter file {path} is not a bounded regular file below {maximum} bytes")]
    UnsafeFile { path: PathBuf, maximum: u64 },
    #[error("adapter descriptor-safe file validation failed: {0}")]
    SafeFile(String),
    #[error("adapter file size accounting overflowed")]
    SizeOverflow,
    #[error("failed to encode adapter SHA-256 digest")]
    DigestEncoding,
    #[error("adapter JSON is invalid: {0}")]
    Json(serde_json::Error),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_fingerprint_changes_with_source_library_and_flags() {
        let tool = ToolIdentity {
            path: "/usr/bin/tool".to_string(),
            sha256: "a".repeat(64),
            version: "tool 1".to_string(),
        };
        let base = build_fingerprint(
            &"b".repeat(64),
            &"c".repeat(64),
            &tool,
            &tool,
            &[OsString::from("-O3")],
        )
        .expect("base fingerprint");
        let changed = build_fingerprint(
            &"d".repeat(64),
            &"c".repeat(64),
            &tool,
            &tool,
            &[OsString::from("-O3")],
        )
        .expect("changed fingerprint");
        assert_ne!(base, changed);
        let changed_library = build_fingerprint(
            &"b".repeat(64),
            &"e".repeat(64),
            &tool,
            &tool,
            &[OsString::from("-O3")],
        )
        .expect("changed library fingerprint");
        let changed_flags = build_fingerprint(
            &"b".repeat(64),
            &"c".repeat(64),
            &tool,
            &tool,
            &[OsString::from("-O2")],
        )
        .expect("changed flags fingerprint");
        assert_ne!(base, changed_library);
        assert_ne!(base, changed_flags);
    }

    #[test]
    fn reusable_receipt_rejects_commit_drift_and_stale_binary_digest() {
        let repository = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        let output = repository.path().join("target/benchmarks/stim-adapter");
        std::fs::create_dir_all(&output).expect("create adapter output");
        let binary = output.join("stim-adapter");
        let receipt_path = output.join("build-receipt.json");
        std::fs::write(&binary, b"adapter").expect("write adapter binary");
        let tool = ToolIdentity {
            path: "/usr/bin/tool".to_string(),
            sha256: "a".repeat(64),
            version: "tool 1".to_string(),
        };
        let expected = AdapterBuildReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            stim_tag: STIM_TAG.to_string(),
            stim_commit: STIM_COMMIT.to_string(),
            adapter_source_sha256: "b".repeat(64),
            stim_library_sha256: "c".repeat(64),
            cmake: tool.clone(),
            cxx: tool,
            compile_arguments: vec!["-O3".to_string()],
            build_fingerprint: "d".repeat(64),
            binary_sha256: String::new(),
        };
        let mut drifted = expected.clone();
        drifted.stim_commit = "f".repeat(40);
        drifted.binary_sha256 = "e".repeat(64);
        std::fs::write(
            &receipt_path,
            serde_json::to_vec(&drifted).expect("serialize drifted receipt"),
        )
        .expect("write drifted receipt");
        assert!(
            reusable_receipt(&root, &receipt_path, &binary, &expected)
                .expect("drift is a rebuild decision")
                .is_none()
        );

        let mut stale = expected.clone();
        stale.binary_sha256 = "e".repeat(64);
        std::fs::write(
            &receipt_path,
            serde_json::to_vec(&stale).expect("serialize stale receipt"),
        )
        .expect("write stale receipt");
        assert!(matches!(
            reusable_receipt(&root, &receipt_path, &binary, &expected),
            Err(AdapterError::StaleBuild(_))
        ));
    }

    #[test]
    fn executable_verification_rejects_source_digest_drift() {
        let directory = tempfile::tempdir().expect("temporary adapter files");
        let source = directory.path().join("main.cc");
        let library = directory.path().join("libstim.a");
        let binary = directory.path().join("adapter");
        let cmake = directory.path().join("cmake");
        let cxx = directory.path().join("cxx");
        for (path, bytes) in [
            (&source, b"source".as_slice()),
            (&library, b"library".as_slice()),
            (&binary, b"binary".as_slice()),
            (&cmake, b"cmake".as_slice()),
            (&cxx, b"cxx".as_slice()),
        ] {
            std::fs::write(path, bytes).expect("write adapter identity file");
        }
        let source_digest =
            sha256_regular_file(&source, MAX_SOURCE_BYTES as u64).expect("source digest");
        let binary_digest = sha256_regular_file(&binary, MAX_TOOL_BYTES).expect("binary digest");
        let receipt = AdapterBuildReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            stim_tag: STIM_TAG.to_string(),
            stim_commit: STIM_COMMIT.to_string(),
            adapter_source_sha256: source_digest.clone(),
            stim_library_sha256: sha256_regular_file(&library, MAX_LIBRARY_BYTES)
                .expect("library digest"),
            cmake: ToolIdentity {
                path: cmake.to_string_lossy().into_owned(),
                sha256: sha256_regular_file(&cmake, MAX_TOOL_BYTES).expect("cmake digest"),
                version: "cmake test".to_string(),
            },
            cxx: ToolIdentity {
                path: cxx.to_string_lossy().into_owned(),
                sha256: sha256_regular_file(&cxx, MAX_TOOL_BYTES).expect("cxx digest"),
                version: "cxx test".to_string(),
            },
            compile_arguments: vec!["-O3".to_string()],
            build_fingerprint: "d".repeat(64),
            binary_sha256: binary_digest.clone(),
        };
        let executable = AdapterExecutable {
            path: binary,
            source_digest: Sha256Digest::try_new(source_digest).expect("source identity"),
            build_fingerprint: Sha256Digest::try_new("d".repeat(64)).expect("build fingerprint"),
            binary_digest: Sha256Digest::try_new(binary_digest).expect("binary identity"),
            receipt,
            source_path: source.clone(),
            library_path: library,
        };
        executable.verify().expect("unchanged identity verifies");
        std::fs::write(source, b"drifted source").expect("drift source");
        assert!(matches!(
            executable.verify(),
            Err(AdapterError::StaleBuild(_))
        ));
    }
}
