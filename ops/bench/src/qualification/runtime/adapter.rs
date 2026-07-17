use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::executable::SealedExecutable;
use super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::protocol::Sha256Digest;
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

const ADAPTER_SOURCE: &str = "benchmarks/stim_adapter/main.cc";
const SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/simd_word_popcount_contract.h";
const SIMD_BITS_XOR_COMPARATOR_SOURCE: &str = "benchmarks/stim_adapter/simd_bits_xor_contract.h";
const SIMD_BITS_NOT_ZERO_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/simd_bits_not_zero_contract.h";
const SPARSE_XOR_COMPARATOR_SOURCE: &str = "benchmarks/stim_adapter/sparse_xor_contract.h";
const BIT_MATRIX_TRANSPOSE_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/bit_matrix_transpose_contract.h";
const PAULI_STRING_MULTIPLY_COMPARATOR_SOURCE: &str =
    "benchmarks/stim_adapter/pauli_string_multiply_contract.h";
const COMPARATOR_SOURCES: [&str; 6] = [
    SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE,
    SIMD_BITS_XOR_COMPARATOR_SOURCE,
    SIMD_BITS_NOT_ZERO_COMPARATOR_SOURCE,
    SPARSE_XOR_COMPARATOR_SOURCE,
    BIT_MATRIX_TRANSPOSE_COMPARATOR_SOURCE,
    PAULI_STRING_MULTIPLY_COMPARATOR_SOURCE,
];
const RECEIPT_SCHEMA_VERSION: u32 = 9;
const MAX_SOURCE_BYTES: u64 = 1 << 20;
const MAX_FLAGS_FILE_BYTES: u64 = 64 << 10;
const MAX_TOOL_BYTES: u64 = 512 << 20;
const MAX_LIBRARY_BYTES: u64 = 1 << 30;
const BUILD_OUTPUT_BYTES: usize = 16 << 20;
const BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);
const RUNTIME_PARENT: &str = "/tmp";
const BUILD_FINGERPRINT_DEFINE: &str = "-DSTAB_ADAPTER_BUILD_FINGERPRINT=";
const FINGERPRINT_PENDING: &str = "PENDING";

#[derive(Debug)]
pub(crate) struct AdapterExecutable {
    pub(crate) path: PathBuf,
    pub(crate) source_digest: Sha256Digest,
    pub(crate) build_fingerprint: Sha256Digest,
    pub(crate) binary_digest: Sha256Digest,
    pub(crate) receipt: AdapterBuildReceipt,
    _runtime: tempfile::TempDir,
    executable: SealedExecutable,
    source_path: PathBuf,
    comparator_source_paths: Vec<PathBuf>,
    library_path: PathBuf,
}

impl AdapterExecutable {
    pub(crate) fn verify(&self) -> Result<(), AdapterError> {
        self.executable.verify()?;
        let source = sha256_regular_file(&self.source_path, MAX_SOURCE_BYTES)?;
        let comparator_sources_match = self.comparator_source_paths.len()
            == self.receipt.comparator_sources.len()
            && self
                .comparator_source_paths
                .iter()
                .zip(&self.receipt.comparator_sources)
                .all(|(path, expected)| {
                    sha256_regular_file(path, MAX_SOURCE_BYTES)
                        .is_ok_and(|actual| actual == expected.sha256)
                });
        let library = sha256_regular_file(&self.library_path, MAX_LIBRARY_BYTES)?;
        if self.path != self.executable.program()
            || self.executable.sha256() != self.binary_digest.as_str()
            || self.receipt.binary_sha256 != self.binary_digest.as_str()
            || self.receipt.adapter_source_sha256 != self.source_digest.as_str()
            || self.receipt.build_fingerprint != self.build_fingerprint.as_str()
            || source != self.receipt.adapter_source_sha256
            || !comparator_sources_match
            || library != self.receipt.stim_library_sha256
            || !self.receipt.validates_report_identity(
                self.source_digest.as_str(),
                self.build_fingerprint.as_str(),
                self.binary_digest.as_str(),
            )
        {
            return Err(AdapterError::StaleBuild);
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
    comparator_sources: Vec<AdapterComparatorSource>,
    stim_library_sha256: String,
    cmake: ToolIdentity,
    cc: ToolIdentity,
    cxx: ToolIdentity,
    make: ToolIdentity,
    configure_arguments: Vec<String>,
    library_build_arguments: Vec<String>,
    stim_compile_flags: Vec<String>,
    compile_arguments: Vec<String>,
    build_environment: Vec<BuildEnvironmentEntry>,
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
            && valid_comparator_sources(&self.comparator_sources)
            && self.build_fingerprint == build_fingerprint
            && self.binary_sha256 == binary_sha256
            && valid_digest(&self.stim_library_sha256)
            && valid_digest(binary_sha256)
            && tool_matches("cmake", &self.cmake)
            && tool_matches("cc", &self.cc)
            && tool_matches("c++", &self.cxx)
            && tool_matches("make", &self.make)
            && self.configure_arguments
                == normalized_configure_arguments(&self.cc, &self.cxx, &self.make)
            && self.library_build_arguments == normalized_library_build_arguments()
            && valid_stim_compile_flags(&self.stim_compile_flags)
            && self.compile_arguments
                == normalized_compile_arguments(
                    source_sha256,
                    build_fingerprint,
                    &self.stim_compile_flags,
                )
            && self.build_environment == normalized_build_environment(&self.cc, &self.cxx)
            && self
                .recomputed_build_fingerprint()
                .is_ok_and(|actual| actual == build_fingerprint)
    }

    pub(super) fn validates_comparator_sources(
        &self,
        sources: &[super::group::ComparatorSourceContract],
    ) -> bool {
        let Some((adapter, comparators)) = sources.split_first() else {
            return true;
        };
        if adapter.path.as_str() != ADAPTER_SOURCE
            || adapter.sha256.as_str() != self.adapter_source_sha256
        {
            return false;
        }
        let mut previous_index = None;
        for comparator in comparators {
            let Some(index) = self.comparator_sources.iter().position(|candidate| {
                candidate.path == comparator.path.as_str()
                    && candidate.sha256 == comparator.sha256.as_str()
            }) else {
                return false;
            };
            if previous_index.is_some_and(|previous| index <= previous) {
                return false;
            }
            previous_index = Some(index);
        }
        true
    }

    fn recomputed_build_fingerprint(&self) -> Result<String, AdapterError> {
        if self.compile_arguments
            != normalized_compile_arguments(
                &self.adapter_source_sha256,
                &self.build_fingerprint,
                &self.stim_compile_flags,
            )
        {
            return Err(AdapterError::ReceiptShape);
        }
        if !valid_stim_compile_flags(&self.stim_compile_flags) {
            return Err(AdapterError::ReceiptShape);
        }
        let pending_arguments = normalized_compile_arguments(
            &self.adapter_source_sha256,
            FINGERPRINT_PENDING,
            &self.stim_compile_flags,
        );
        let material = serde_json::to_vec(&serde_json::json!({
            "schema_version": self.schema_version,
            "stim_tag": self.stim_tag,
            "stim_commit": self.stim_commit,
            "adapter_source_sha256": self.adapter_source_sha256,
            "comparator_sources": self.comparator_sources,
            "stim_library_sha256": self.stim_library_sha256,
            "cmake": self.cmake,
            "cc": self.cc,
            "cxx": self.cxx,
            "make": self.make,
            "configure_arguments": self.configure_arguments,
            "library_build_arguments": self.library_build_arguments,
            "stim_compile_flags": self.stim_compile_flags,
            "compile_arguments": pending_arguments,
            "build_environment": self.build_environment,
        }))?;
        sha256_bytes(&material)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterComparatorSource {
    path: String,
    sha256: String,
}

fn valid_comparator_sources(sources: &[AdapterComparatorSource]) -> bool {
    sources.len() == COMPARATOR_SOURCES.len()
        && sources
            .iter()
            .zip(COMPARATOR_SOURCES)
            .all(|(source, expected_path)| {
                source.path == expected_path && valid_digest(&source.sha256)
            })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolIdentity {
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

pub(crate) fn prepare_adapter(
    root: &RepoRoot,
    repository_commit: &str,
) -> Result<AdapterExecutable, AdapterError> {
    ensure_linux()?;
    super::git::validate_pinned_stim(root)?;
    let runtime = tempfile::Builder::new()
        .prefix("stab-pq1-stim-build-")
        .tempdir_in(RUNTIME_PARENT)
        .map_err(AdapterError::Runtime)?;
    let stab_source = runtime.path().join("stab-source");
    let stim_source = runtime.path().join("stim-source");
    let stim_build = runtime.path().join("stim-build");
    let home = runtime.path().join("home");
    let scratch = runtime.path().join("tmp");
    let xdg = runtime.path().join("xdg");
    for path in [
        &stab_source,
        &stim_source,
        &stim_build,
        &home,
        &scratch,
        &xdg,
    ] {
        create_private_directory(path)?;
    }
    super::git::materialize_repository_commit(root, repository_commit, &stab_source)?;
    super::git::materialize_worktree_commit(
        &root.default_stim_source(),
        STIM_COMMIT,
        &stim_source,
    )?;

    let environment_paths = RuntimePaths {
        runtime: runtime.path(),
        stab_source: &stab_source,
        stim_source: &stim_source,
        stim_build: &stim_build,
    };
    let cmake = tool_identity("cmake", &scratch, runtime.path())?;
    let cc = tool_identity("cc", &scratch, runtime.path())?;
    let cxx = tool_identity("c++", &scratch, runtime.path())?;
    let make = tool_identity("make", &scratch, runtime.path())?;
    let build_environment = normalized_build_environment(&cc, &cxx);
    let environment = expand_environment(&build_environment, &environment_paths)?;
    let configure_arguments = normalized_configure_arguments(&cc, &cxx, &make);
    run_build_command(
        Path::new(&cmake.path),
        expand_values(&configure_arguments, &environment_paths)?,
        &scratch,
        &environment,
        BUILD_TIMEOUT,
    )?;
    let library_build_arguments = normalized_library_build_arguments();
    run_build_command(
        Path::new(&cmake.path),
        expand_values(&library_build_arguments, &environment_paths)?,
        &scratch,
        &environment,
        BUILD_TIMEOUT,
    )?;
    let library_path = stim_build.join("out/libstim.a");
    let library_digest = sha256_regular_file(&library_path, MAX_LIBRARY_BYTES)?;
    let stim_compile_flags = read_stim_compile_flags(&stim_build)?;
    let source_path = stab_source.join(ADAPTER_SOURCE);
    let source_digest = sha256_regular_file(&source_path, MAX_SOURCE_BYTES)?;
    let comparator_source_paths = COMPARATOR_SOURCES
        .into_iter()
        .map(|path| stab_source.join(path))
        .collect::<Vec<_>>();
    let comparator_sources = COMPARATOR_SOURCES
        .into_iter()
        .zip(&comparator_source_paths)
        .map(|(path, materialized_path)| {
            Ok(AdapterComparatorSource {
                path: path.to_string(),
                sha256: sha256_regular_file(materialized_path, MAX_SOURCE_BYTES)?,
            })
        })
        .collect::<Result<Vec<_>, AdapterError>>()?;
    let mut receipt = AdapterBuildReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        adapter_source_sha256: source_digest.clone(),
        comparator_sources,
        stim_library_sha256: library_digest,
        cmake,
        cc,
        cxx,
        make,
        configure_arguments,
        library_build_arguments,
        compile_arguments: normalized_compile_arguments(
            &source_digest,
            FINGERPRINT_PENDING,
            &stim_compile_flags,
        ),
        stim_compile_flags,
        build_environment,
        build_fingerprint: FINGERPRINT_PENDING.to_string(),
        binary_sha256: String::new(),
    };
    receipt.build_fingerprint = receipt.recomputed_build_fingerprint()?;
    receipt.compile_arguments = normalized_compile_arguments(
        &source_digest,
        &receipt.build_fingerprint,
        &receipt.stim_compile_flags,
    );
    let binary = runtime.path().join("stim-qualification-adapter");
    run_build_command(
        Path::new(&receipt.cxx.path),
        expand_values(&receipt.compile_arguments, &environment_paths)?,
        &scratch,
        &environment,
        BUILD_TIMEOUT,
    )?;
    let executable = SealedExecutable::open("stim-adapter", &binary)?;
    receipt.binary_sha256 = executable.sha256().to_string();
    let adapter = AdapterExecutable {
        path: executable.program(),
        source_digest: Sha256Digest::try_new(source_digest)?,
        build_fingerprint: Sha256Digest::try_new(receipt.build_fingerprint.clone())?,
        binary_digest: Sha256Digest::try_new(receipt.binary_sha256.clone())?,
        receipt,
        _runtime: runtime,
        executable,
        source_path,
        comparator_source_paths,
        library_path,
    };
    adapter.verify()?;
    Ok(adapter)
}

fn normalized_configure_arguments(
    cc: &ToolIdentity,
    cxx: &ToolIdentity,
    make: &ToolIdentity,
) -> Vec<String> {
    vec![
        "-S".to_string(),
        "$STIM_SOURCE".to_string(),
        "-B".to_string(),
        "$STIM_BUILD".to_string(),
        "-G".to_string(),
        "Unix Makefiles".to_string(),
        "-DCMAKE_BUILD_TYPE=Release".to_string(),
        format!("-DCMAKE_MAKE_PROGRAM={}", make.path),
        format!("-DCMAKE_C_COMPILER={}", cc.path),
        format!("-DCMAKE_CXX_COMPILER={}", cxx.path),
    ]
}

fn normalized_library_build_arguments() -> Vec<String> {
    [
        "--build",
        "$STIM_BUILD",
        "--target",
        "libstim",
        "--parallel",
        "1",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn normalized_compile_arguments(
    source_digest: &str,
    fingerprint: &str,
    stim_compile_flags: &[String],
) -> Vec<String> {
    let mut arguments = stim_compile_flags.to_vec();
    arguments.extend([
        "-Wextra".to_string(),
        "-Werror".to_string(),
        "-isystem".to_string(),
        "$STIM_SOURCE/src".to_string(),
        format!("-DSTAB_STIM_COMMIT=\"{STIM_COMMIT}\""),
        format!("-DSTAB_ADAPTER_SOURCE_DIGEST=\"{source_digest}\""),
        format!("{BUILD_FINGERPRINT_DEFINE}\"{fingerprint}\""),
        format!("$STAB_SOURCE/{ADAPTER_SOURCE}"),
        "$STIM_BUILD/out/libstim.a".to_string(),
        "-o".to_string(),
        "$RUNTIME/stim-qualification-adapter".to_string(),
    ]);
    arguments
}

fn read_stim_compile_flags(stim_build: &Path) -> Result<Vec<String>, AdapterError> {
    let path = stim_build.join("CMakeFiles/libstim.dir/flags.make");
    let mut file =
        crate::source_file::open_regular_file_bounded_descriptor(&path, MAX_FLAGS_FILE_BYTES)
            .map_err(|error| AdapterError::SafeFile(error.to_string()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_FLAGS_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| AdapterError::Io {
            path: path.clone(),
            source,
        })?;
    if bytes.len() as u64 > MAX_FLAGS_FILE_BYTES {
        return Err(AdapterError::UnsafeFile {
            path,
            maximum: MAX_FLAGS_FILE_BYTES,
        });
    }
    let contents = std::str::from_utf8(&bytes).map_err(|_| AdapterError::StimFlagsUtf8)?;
    let mut matching_lines = contents
        .lines()
        .filter_map(|line| line.strip_prefix("CXX_FLAGS = "));
    let flags = matching_lines.next().ok_or(AdapterError::StimFlagsShape)?;
    if matching_lines.next().is_some() {
        return Err(AdapterError::StimFlagsShape);
    }
    let flags = flags
        .split_ascii_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !valid_stim_compile_flags(&flags) {
        return Err(AdapterError::StimFlagsShape);
    }
    Ok(flags)
}

fn valid_stim_compile_flags(flags: &[String]) -> bool {
    !flags.is_empty()
        && flags.iter().all(|flag| {
            flag.starts_with('-')
                && flag.len() <= 128
                && flag.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b'-' | b'_' | b'=' | b'+' | b'.' | b'/')
                })
        })
        && flags.iter().any(|flag| flag == "-O3")
        && flags.iter().any(|flag| flag == "-DNDEBUG")
        && flags.iter().any(|flag| flag == "-fno-strict-aliasing")
        && flags
            .iter()
            .any(|flag| matches!(flag.as_str(), "-std=c++20" | "-std=gnu++20"))
}

fn normalized_build_environment(
    cc: &ToolIdentity,
    cxx: &ToolIdentity,
) -> Vec<BuildEnvironmentEntry> {
    let mut entries = vec![
        environment_entry("CC", &cc.path),
        environment_entry("CXX", &cxx.path),
        environment_entry("HOME", "$RUNTIME/home"),
        environment_entry("LANG", "C"),
        environment_entry("LC_ALL", "C"),
        environment_entry("PATH", "/usr/bin:/bin"),
        environment_entry("TMPDIR", "$RUNTIME/tmp"),
        environment_entry("TZ", "UTC"),
        environment_entry("XDG_CONFIG_HOME", "$RUNTIME/xdg"),
    ];
    entries.sort();
    entries
}

fn environment_entry(name: &str, value: &str) -> BuildEnvironmentEntry {
    BuildEnvironmentEntry {
        name: name.to_string(),
        value: value.to_string(),
    }
}

struct RuntimePaths<'a> {
    runtime: &'a Path,
    stab_source: &'a Path,
    stim_source: &'a Path,
    stim_build: &'a Path,
}

fn expand_environment(
    entries: &[BuildEnvironmentEntry],
    paths: &RuntimePaths<'_>,
) -> Result<Vec<(OsString, OsString)>, AdapterError> {
    entries
        .iter()
        .map(|entry| {
            Ok((
                OsString::from(&entry.name),
                OsString::from(expand_value(&entry.value, paths)?),
            ))
        })
        .collect()
}

fn expand_values(
    values: &[String],
    paths: &RuntimePaths<'_>,
) -> Result<Vec<OsString>, AdapterError> {
    values
        .iter()
        .map(|value| expand_value(value, paths).map(OsString::from))
        .collect()
}

fn expand_value(value: &str, paths: &RuntimePaths<'_>) -> Result<String, AdapterError> {
    let runtime = path_text(paths.runtime)?;
    let stab_source = path_text(paths.stab_source)?;
    let stim_source = path_text(paths.stim_source)?;
    let stim_build = path_text(paths.stim_build)?;
    Ok(value
        .replace("$STAB_SOURCE", stab_source)
        .replace("$STIM_SOURCE", stim_source)
        .replace("$STIM_BUILD", stim_build)
        .replace("$RUNTIME", runtime))
}

fn path_text(path: &Path) -> Result<&str, AdapterError> {
    path.to_str()
        .ok_or_else(|| AdapterError::NonUtf8Path(path.to_path_buf()))
}

fn tool_identity(
    name: &'static str,
    working_directory: &Path,
    runtime: &Path,
) -> Result<ToolIdentity, AdapterError> {
    let path = resolve_tool(name)?;
    let sha256 = sha256_regular_file(&path, MAX_TOOL_BYTES)?;
    let environment = expand_environment(
        &[
            environment_entry("HOME", "$RUNTIME/home"),
            environment_entry("LANG", "C"),
            environment_entry("LC_ALL", "C"),
            environment_entry("PATH", "/usr/bin:/bin"),
            environment_entry("TMPDIR", "$RUNTIME/tmp"),
            environment_entry("TZ", "UTC"),
            environment_entry("XDG_CONFIG_HOME", "$RUNTIME/xdg"),
        ],
        &RuntimePaths {
            runtime,
            stab_source: runtime,
            stim_source: runtime,
            stim_build: runtime,
        },
    )?;
    let output = run_bounded_process(&ProcessRequest {
        program: path.clone(),
        args: vec![OsString::from("--version")],
        stdin: Vec::new(),
        working_directory: working_directory.to_path_buf(),
        environment,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout_bytes: 64 << 10,
            stderr_bytes: 64 << 10,
            regular_file_bytes: None,
            timeout: TOOL_TIMEOUT,
        },
    })?;
    if output.status != Some(0) || !output.stderr.is_empty() {
        return Err(AdapterError::ToolVersion {
            name,
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let version = std::str::from_utf8(&output.stdout)
        .map_err(|_| AdapterError::ToolVersionUtf8(name))?
        .trim()
        .to_string();
    if version.is_empty() || version.len() > 64 << 10 {
        return Err(AdapterError::ToolVersionShape(name));
    }
    Ok(ToolIdentity {
        path: path.to_string_lossy().into_owned(),
        sha256,
        version,
    })
}

fn resolve_tool(name: &str) -> Result<PathBuf, AdapterError> {
    for directory in [Path::new("/usr/bin"), Path::new("/bin")] {
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

fn tool_matches(name: &str, identity: &ToolIdentity) -> bool {
    !identity.version.is_empty()
        && valid_digest(&identity.sha256)
        && resolve_tool(name).is_ok_and(|path| path == Path::new(&identity.path))
        && sha256_regular_file(Path::new(&identity.path), MAX_TOOL_BYTES)
            .is_ok_and(|digest| digest == identity.sha256)
}

fn run_build_command(
    program: &Path,
    arguments: Vec<OsString>,
    working_directory: &Path,
    environment: &[(OsString, OsString)],
    timeout: Duration,
) -> Result<(), AdapterError> {
    let output = run_bounded_process(&ProcessRequest {
        program: program.to_path_buf(),
        args: arguments,
        stdin: Vec::new(),
        working_directory: working_directory.to_path_buf(),
        environment: environment.to_vec(),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout_bytes: BUILD_OUTPUT_BYTES,
            stderr_bytes: BUILD_OUTPUT_BYTES,
            regular_file_bytes: None,
            timeout,
        },
    })?;
    if output.status == Some(0) {
        Ok(())
    } else {
        Err(AdapterError::Build {
            program: program.to_path_buf(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn create_private_directory(path: &Path) -> Result<(), AdapterError> {
    std::fs::create_dir(path).map_err(|source| AdapterError::Io {
        path: path.to_path_buf(),
        source,
    })
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

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    #[error("failed to create a private Stim build runtime: {0}")]
    Runtime(std::io::Error),
    #[error("required build tool {0:?} was not found on the controlled system PATH")]
    MissingTool(String),
    #[error("build tool {name} --version failed with status {status:?}: {stderr}")]
    ToolVersion {
        name: &'static str,
        status: Option<i32>,
        stderr: String,
    },
    #[error("build tool {0} --version output is not UTF-8")]
    ToolVersionUtf8(&'static str),
    #[error("build tool {0} --version output is empty or oversized")]
    ToolVersionShape(&'static str),
    #[error(
        "adapter build command {program} failed with status {status:?}; stdout={stdout:?}; stderr={stderr:?}"
    )]
    Build {
        program: PathBuf,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("adapter build receipt has an invalid source-owned shape")]
    ReceiptShape,
    #[error("generated Stim compile flags are not UTF-8")]
    StimFlagsUtf8,
    #[error("generated Stim compile flags have an unexpected shape")]
    StimFlagsShape,
    #[error("adapter identity changed after preparation")]
    StaleBuild,
    #[error("adapter path is not UTF-8: {0}")]
    NonUtf8Path(PathBuf),
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
    #[error(transparent)]
    Git(#[from] super::git::GitError),
    #[error(transparent)]
    Executable(#[from] super::executable::ExecutableError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
}

#[cfg(test)]
#[path = "adapter_comparator_source_tests.rs"]
mod comparator_source_tests;

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn actual_tool(name: &'static str) -> ToolIdentity {
        let path = resolve_tool(name).expect("resolve test tool");
        ToolIdentity {
            sha256: sha256_regular_file(&path, MAX_TOOL_BYTES).expect("tool digest"),
            path: path.to_string_lossy().into_owned(),
            version: format!("{name} test version"),
        }
    }

    pub(super) fn test_receipt(source: &str, library: &str, binary: &str) -> AdapterBuildReceipt {
        let cmake = actual_tool("cmake");
        let cc = actual_tool("cc");
        let cxx = actual_tool("c++");
        let make = actual_tool("make");
        let mut receipt = AdapterBuildReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            stim_tag: STIM_TAG.to_string(),
            stim_commit: STIM_COMMIT.to_string(),
            adapter_source_sha256: source.to_string(),
            comparator_sources: COMPARATOR_SOURCES
                .into_iter()
                .map(|path| AdapterComparatorSource {
                    path: path.to_string(),
                    sha256: source.to_string(),
                })
                .collect(),
            stim_library_sha256: library.to_string(),
            configure_arguments: normalized_configure_arguments(&cc, &cxx, &make),
            library_build_arguments: normalized_library_build_arguments(),
            compile_arguments: normalized_compile_arguments(
                source,
                FINGERPRINT_PENDING,
                &test_stim_compile_flags(),
            ),
            stim_compile_flags: test_stim_compile_flags(),
            build_environment: normalized_build_environment(&cc, &cxx),
            cmake,
            cc,
            cxx,
            make,
            build_fingerprint: FINGERPRINT_PENDING.to_string(),
            binary_sha256: binary.to_string(),
        };
        receipt.build_fingerprint = receipt
            .recomputed_build_fingerprint()
            .expect("build fingerprint");
        receipt.compile_arguments = normalized_compile_arguments(
            source,
            &receipt.build_fingerprint,
            &receipt.stim_compile_flags,
        );
        receipt
    }

    fn test_stim_compile_flags() -> Vec<String> {
        [
            "-O3",
            "-DNDEBUG",
            "-std=gnu++20",
            "-Wall",
            "-Wpedantic",
            "-fPIC",
            "-fno-strict-aliasing",
            "-march=native",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn report_identity_rejects_tampered_tools_arguments_and_fingerprint() {
        let source = "a".repeat(64);
        let library = "b".repeat(64);
        let binary = "c".repeat(64);
        let receipt = test_receipt(&source, &library, &binary);
        assert!(receipt.validates_report_identity(&source, &receipt.build_fingerprint, &binary));

        let mut changed_tool = receipt.clone();
        changed_tool.cxx.sha256 = "d".repeat(64);
        assert!(!changed_tool.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        let mut changed_configure = receipt.clone();
        changed_configure
            .configure_arguments
            .push("-DUNREVIEWED=1".to_string());
        assert!(!changed_configure.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        let mut changed_compile = receipt.clone();
        *changed_compile
            .compile_arguments
            .get_mut(1)
            .expect("compile optimization flag") = "-O2".to_string();
        assert!(!changed_compile.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        let mut duplicate_define = receipt.clone();
        duplicate_define.compile_arguments.push(format!(
            "{BUILD_FINGERPRINT_DEFINE}\"{}\"",
            receipt.build_fingerprint
        ));
        assert!(!duplicate_define.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        let mut changed_stim_flags = receipt.clone();
        changed_stim_flags
            .stim_compile_flags
            .retain(|flag| flag != "-march=native");
        assert!(!changed_stim_flags.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));

        let mut refingerprinted_flags = receipt.clone();
        refingerprinted_flags
            .stim_compile_flags
            .push("-mavx2".to_string());
        refingerprinted_flags.build_fingerprint = FINGERPRINT_PENDING.to_string();
        refingerprinted_flags.compile_arguments = normalized_compile_arguments(
            &source,
            FINGERPRINT_PENDING,
            &refingerprinted_flags.stim_compile_flags,
        );
        assert_ne!(
            refingerprinted_flags
                .recomputed_build_fingerprint()
                .expect("changed flag fingerprint"),
            receipt.build_fingerprint
        );

        let mut reordered_flags = receipt.clone();
        reordered_flags.stim_compile_flags.swap(0, 1);
        assert!(!reordered_flags.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        reordered_flags.build_fingerprint = FINGERPRINT_PENDING.to_string();
        reordered_flags.compile_arguments = normalized_compile_arguments(
            &source,
            FINGERPRINT_PENDING,
            &reordered_flags.stim_compile_flags,
        );
        assert_ne!(
            reordered_flags
                .recomputed_build_fingerprint()
                .expect("reordered flag fingerprint"),
            receipt.build_fingerprint
        );

        let mut changed_comparator = receipt.clone();
        changed_comparator
            .comparator_sources
            .first_mut()
            .expect("popcount comparator")
            .sha256 = "e".repeat(64);
        assert!(!changed_comparator.validates_report_identity(
            &source,
            &receipt.build_fingerprint,
            &binary
        ));
        changed_comparator.build_fingerprint = FINGERPRINT_PENDING.to_string();
        changed_comparator.compile_arguments = normalized_compile_arguments(
            &source,
            FINGERPRINT_PENDING,
            &changed_comparator.stim_compile_flags,
        );
        assert_ne!(
            changed_comparator
                .recomputed_build_fingerprint()
                .expect("changed comparator fingerprint"),
            receipt.build_fingerprint
        );

        let popcount_digest = receipt
            .comparator_sources
            .first()
            .expect("popcount comparator")
            .sha256
            .clone();
        let comparator_sources: Vec<super::super::group::ComparatorSourceContract> =
            serde_json::from_value(serde_json::json!([
                {"path": ADAPTER_SOURCE, "sha256": source},
                {
                    "path": SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE,
                    "sha256": popcount_digest
                }
            ]))
            .expect("comparator source contracts");
        assert!(receipt.validates_comparator_sources(&comparator_sources));
        let stale_sources: Vec<super::super::group::ComparatorSourceContract> =
            serde_json::from_value(serde_json::json!([
                {"path": ADAPTER_SOURCE, "sha256": source},
                {
                    "path": SIMD_WORD_POPCOUNT_COMPARATOR_SOURCE,
                    "sha256": "e".repeat(64)
                }
            ]))
            .expect("stale comparator source contracts");
        assert!(!receipt.validates_comparator_sources(&stale_sources));
    }

    #[test]
    fn reads_and_preserves_generated_stim_machine_flags() {
        let runtime = tempfile::tempdir().expect("adapter runtime");
        let directory = runtime.path().join("CMakeFiles/libstim.dir");
        std::fs::create_dir_all(&directory).expect("create flags directory");
        std::fs::write(
            directory.join("flags.make"),
            "CXX_FLAGS = -O3 -DNDEBUG -std=gnu++20 -fno-strict-aliasing -march=native\n",
        )
        .expect("write flags");

        let flags = read_stim_compile_flags(runtime.path()).expect("read flags");
        assert_eq!(flags.last().map(String::as_str), Some("-march=native"));
        let arguments = normalized_compile_arguments(&"a".repeat(64), "PENDING", &flags);
        assert_eq!(
            arguments
                .iter()
                .filter(|argument| argument.as_str() == "-march=native")
                .count(),
            1
        );
        let include_flag = arguments
            .iter()
            .position(|argument| argument == "-isystem")
            .expect("external Stim include flag");
        assert_eq!(
            arguments.get(include_flag + 1).map(String::as_str),
            Some("$STIM_SOURCE/src")
        );
        assert!(arguments.iter().any(|argument| argument == "-Werror"));
    }

    #[test]
    fn rejects_ambiguous_or_injection_capable_stim_flags() {
        let runtime = tempfile::tempdir().expect("adapter runtime");
        let directory = runtime.path().join("CMakeFiles/libstim.dir");
        std::fs::create_dir_all(&directory).expect("create flags directory");
        let flags_path = directory.join("flags.make");
        let valid = "-O3 -DNDEBUG -std=gnu++20 -fno-strict-aliasing";

        std::fs::write(
            &flags_path,
            format!("CXX_FLAGS = {valid}\nCXX_FLAGS = {valid}\n"),
        )
        .expect("write duplicate flags");
        assert!(matches!(
            read_stim_compile_flags(runtime.path()),
            Err(AdapterError::StimFlagsShape)
        ));

        std::fs::write(
            &flags_path,
            format!("CXX_FLAGS = {valid} '-march=native'\n"),
        )
        .expect("write quoted flags");
        assert!(matches!(
            read_stim_compile_flags(runtime.path()),
            Err(AdapterError::StimFlagsShape)
        ));
    }

    #[test]
    fn executable_verification_rejects_materialized_source_drift() {
        let runtime = tempfile::tempdir().expect("adapter runtime");
        let source = runtime.path().join("main.cc");
        let popcount_source = runtime.path().join("simd_word_popcount_contract.h");
        let xor_source = runtime.path().join("simd_bits_xor_contract.h");
        let not_zero_source = runtime.path().join("simd_bits_not_zero_contract.h");
        let sparse_xor_source = runtime.path().join("sparse_xor_contract.h");
        let transpose_source = runtime.path().join("bit_matrix_transpose_contract.h");
        let pauli_source = runtime.path().join("pauli_string_multiply_contract.h");
        let library = runtime.path().join("libstim.a");
        let binary = runtime.path().join("adapter");
        std::fs::write(&source, b"source").expect("write source");
        std::fs::write(&popcount_source, b"source").expect("write popcount source");
        std::fs::write(&xor_source, b"source").expect("write XOR source");
        std::fs::write(&not_zero_source, b"source").expect("write not-zero source");
        std::fs::write(&sparse_xor_source, b"source").expect("write sparse XOR source");
        std::fs::write(&transpose_source, b"source").expect("write transpose source");
        std::fs::write(&pauli_source, b"source").expect("write Pauli source");
        std::fs::write(&library, b"library").expect("write library");
        std::fs::write(&binary, b"binary").expect("write binary");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("make binary executable");
        let executable = SealedExecutable::open("test-adapter", &binary).expect("seal binary");
        let source_digest = sha256_regular_file(&source, MAX_SOURCE_BYTES).expect("source digest");
        let library_digest =
            sha256_regular_file(&library, MAX_LIBRARY_BYTES).expect("library digest");
        let receipt = test_receipt(&source_digest, &library_digest, executable.sha256());
        let adapter = AdapterExecutable {
            path: executable.program(),
            source_digest: Sha256Digest::try_new(source_digest).expect("source identity"),
            build_fingerprint: Sha256Digest::try_new(receipt.build_fingerprint.clone())
                .expect("build fingerprint"),
            binary_digest: Sha256Digest::try_new(receipt.binary_sha256.clone())
                .expect("binary identity"),
            receipt,
            _runtime: runtime,
            executable,
            source_path: source.clone(),
            comparator_source_paths: vec![
                popcount_source.clone(),
                xor_source.clone(),
                not_zero_source.clone(),
                sparse_xor_source.clone(),
                transpose_source.clone(),
                pauli_source.clone(),
            ],
            library_path: library,
        };
        adapter.verify().expect("unchanged adapter verifies");
        std::fs::write(source, b"drifted source").expect("drift source");
        assert!(matches!(adapter.verify(), Err(AdapterError::StaleBuild)));

        std::fs::write(&adapter.source_path, b"source").expect("restore source");
        adapter.verify().expect("restored adapter verifies");
        std::fs::write(popcount_source, b"drifted comparator source")
            .expect("drift comparator source");
        assert!(matches!(adapter.verify(), Err(AdapterError::StaleBuild)));
    }
}
