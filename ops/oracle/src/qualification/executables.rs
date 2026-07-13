use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::os::fd::AsRawFd as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{FileExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::receipt::ExecutableIdentity;
use crate::RepoRoot;

mod cmake_support;
mod compiler_support;
mod support_snapshot;

use cmake_support::CmakeSupport;
use compiler_support::CompilerSupport;

const BUILD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const MAX_EXECUTABLE_BYTES: u64 = 1 << 30;
const QUALIFICATION_RUNTIME_PARENT: &str = "/tmp";
const CARGO_INVOCATION_ROOT: &str = "/";
pub(super) const REQUIRED_EXECUTABLE_ROLES: &[&str] = &[
    "cargo",
    "cc",
    "cmake",
    "compiler-assembler",
    "compiler-cc1",
    "compiler-cc1plus",
    "compiler-collect2",
    "compiler-linker",
    "compiler-lto-wrapper",
    "cxx",
    "git",
    "make",
    "qualification-worker",
    "rustc",
    "rustup",
    "stab",
    "stim",
];

#[derive(Debug, Error)]
pub(crate) enum ExecutableError {
    #[error("qualification executable {role} could not be resolved: {reason}")]
    Resolve {
        role: &'static str,
        reason: Box<str>,
    },

    #[error("qualification executable {role} at {path} is invalid: {reason}")]
    Invalid {
        role: &'static str,
        path: PathBuf,
        reason: Box<str>,
    },

    #[error("qualification {step} failed: {reason}")]
    Build {
        step: &'static str,
        reason: Box<str>,
    },
}

#[derive(Debug)]
struct PinnedExecutable {
    path: PathBuf,
    descriptor: std::fs::File,
    identity: ExecutableIdentity,
}

impl PinnedExecutable {
    fn open(role: &'static str, path: &Path) -> Result<Self, ExecutableError> {
        let path = std::fs::canonicalize(path).map_err(|source| ExecutableError::Invalid {
            role,
            path: path.to_path_buf(),
            reason: source.to_string().into_boxed_str(),
        })?;
        let source_descriptor = crate::safe_file::open_regular_file(&path).map_err(|source| {
            ExecutableError::Invalid {
                role,
                path: path.clone(),
                reason: source.to_string().into_boxed_str(),
            }
        })?;
        let metadata = source_descriptor
            .metadata()
            .map_err(|source| ExecutableError::Invalid {
                role,
                path: path.clone(),
                reason: source.to_string().into_boxed_str(),
            })?;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ExecutableError::Invalid {
                role,
                path,
                reason: "file has no executable permission bit".into(),
            });
        }
        if metadata.len() == 0 || metadata.len() > MAX_EXECUTABLE_BYTES {
            return Err(ExecutableError::Invalid {
                role,
                path,
                reason: format!(
                    "file size {} is outside 1..={MAX_EXECUTABLE_BYTES} bytes",
                    metadata.len()
                )
                .into_boxed_str(),
            });
        }
        let (descriptor, sha256) = sealed_executable_copy(role, &source_descriptor, metadata.len())
            .map_err(|source| ExecutableError::Invalid {
                role,
                path: path.clone(),
                reason: source.to_string().into_boxed_str(),
            })?;
        Ok(Self {
            path,
            descriptor,
            identity: ExecutableIdentity {
                role: role.to_string(),
                bytes: metadata.len(),
                sha256,
            },
        })
    }

    fn program(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.descriptor.as_raw_fd()))
    }

    fn identity(&self) -> ExecutableIdentity {
        self.identity.clone()
    }

    fn parent(&self) -> Option<&Path> {
        self.path.parent()
    }
}

fn sealed_executable_copy(
    role: &'static str,
    source: &std::fs::File,
    bytes: u64,
) -> Result<(std::fs::File, String), std::io::Error> {
    use rustix::fs::{MemfdFlags, SealFlags};

    let descriptor =
        rustix::fs::memfd_create(format!("stab-cq1-{role}"), MemfdFlags::ALLOW_SEALING)
            .map_err(std::io::Error::from)?;
    let mut descriptor = std::fs::File::from(descriptor);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 << 10];
    let mut offset = 0_u64;
    while offset < bytes {
        let remaining = usize::try_from((bytes - offset).min(buffer.len() as u64))
            .map_err(|_| std::io::Error::other("executable copy size exceeds usize"))?;
        let read = source.read_at(
            buffer
                .get_mut(..remaining)
                .ok_or_else(|| std::io::Error::other("invalid executable copy buffer range"))?,
            offset,
        )?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "qualification executable changed while copying",
            ));
        }
        let chunk = buffer
            .get(..read)
            .ok_or_else(|| std::io::Error::other("executable copy read exceeded its buffer"))?;
        descriptor.write_all(chunk)?;
        hasher.update(chunk);
        offset = offset
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| std::io::Error::other("executable copy offset overflowed"))?;
    }
    descriptor.sync_all()?;
    descriptor.set_permissions(std::fs::Permissions::from_mode(0o500))?;
    rustix::fs::fcntl_add_seals(
        &descriptor,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
    )
    .map_err(std::io::Error::from)?;
    rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::empty())
        .map_err(std::io::Error::from)?;
    Ok((descriptor, render_sha256(&hasher.finalize())))
}

#[derive(Debug)]
struct PrivateRuntime {
    path: PathBuf,
    parent: std::fs::File,
    directory: std::fs::File,
    name: OsString,
}

impl PrivateRuntime {
    fn create() -> Result<Self, ExecutableError> {
        let temporary = tempfile::Builder::new()
            .prefix(".stab-cq1-metadata-")
            .tempdir_in(QUALIFICATION_RUNTIME_PARENT)
            .map_err(|source| ExecutableError::Build {
                step: "metadata runtime-directory reservation",
                reason: source.to_string().into_boxed_str(),
            })?;
        let path = temporary.path().to_path_buf();
        let name = path
            .file_name()
            .ok_or_else(|| ExecutableError::Build {
                step: "metadata runtime-directory reservation",
                reason: "private runtime path has no final component".into(),
            })?
            .to_owned();
        let parent = crate::safe_file::open_directory(Path::new(QUALIFICATION_RUNTIME_PARENT))
            .map_err(|source| ExecutableError::Build {
                step: "metadata runtime-directory reservation",
                reason: source.to_string().into_boxed_str(),
            })?;
        let directory = crate::qualification::artifact::open_directory_at(&parent, &name).map_err(
            |source| ExecutableError::Build {
                step: "metadata runtime-directory reservation",
                reason: source.to_string().into_boxed_str(),
            },
        )?;
        drop(temporary.keep());
        let runtime = Self {
            path,
            parent,
            directory,
            name,
        };
        rustix::fs::fsync(&runtime.parent).map_err(|source| ExecutableError::Build {
            step: "metadata runtime-directory reservation",
            reason: source.to_string().into_boxed_str(),
        })?;
        Ok(runtime)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateRuntime {
    fn drop(&mut self) {
        drop(crate::qualification::artifact::cleanup_owned_directory(
            &self.parent,
            &self.name,
            &self.directory,
        ));
    }
}

#[derive(Debug)]
pub(crate) struct QualificationMetadataExecutables {
    runtime: PrivateRuntime,
    cargo: PinnedExecutable,
    git: PinnedExecutable,
    rustc: PinnedExecutable,
    rustup: PinnedExecutable,
    environment: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub(crate) struct QualificationExecutables {
    _cc: PinnedExecutable,
    _cmake: PinnedExecutable,
    _cmake_support: CmakeSupport,
    _compiler_support: CompilerSupport,
    _cxx: PinnedExecutable,
    _make: PinnedExecutable,
    worker: PinnedExecutable,
    stab: PinnedExecutable,
    stim: PinnedExecutable,
    environment: Vec<(OsString, OsString)>,
    environment_sha256: String,
    identities: Vec<ExecutableIdentity>,
    metadata: QualificationMetadataExecutables,
}

impl QualificationMetadataExecutables {
    pub(crate) fn prepare(root: &RepoRoot) -> Result<Self, ExecutableError> {
        let runtime = PrivateRuntime::create()?;
        let home = runtime.path().join("home");
        let scratch = runtime.path().join("tmp");
        let xdg = runtime.path().join("xdg");
        for directory in [&home, &scratch, &xdg] {
            std::fs::create_dir(directory).map_err(|source| ExecutableError::Build {
                step: "metadata runtime-directory preparation",
                reason: source.to_string().into_boxed_str(),
            })?;
        }
        let rustup = PinnedExecutable::open("rustup", &resolve_from_path("rustup")?)?;
        let resolution_environment = rustup_resolution_environment(&home, &scratch, &xdg)?;
        let cargo = PinnedExecutable::open(
            "cargo",
            &resolve_rustup_tool(root, &rustup, "cargo", &resolution_environment)?,
        )?;
        let rustc = PinnedExecutable::open(
            "rustc",
            &resolve_rustup_tool(root, &rustup, "rustc", &resolution_environment)?,
        )?;
        let git = PinnedExecutable::open("git", &resolve_from_path("git")?)?;
        let environment = metadata_environment(
            &home,
            &scratch,
            &xdg,
            [&cargo, &git, &rustc, &rustup],
            &rustc,
        )?;
        Ok(Self {
            runtime,
            cargo,
            git,
            rustc,
            rustup,
            environment,
        })
    }

    pub(crate) fn git(&self) -> PathBuf {
        self.git.program()
    }

    pub(crate) fn rustc(&self) -> PathBuf {
        self.rustc.program()
    }

    pub(crate) fn environment(&self) -> &[(OsString, OsString)] {
        &self.environment
    }

    pub(super) fn identities(&self) -> Vec<ExecutableIdentity> {
        let mut identities = vec![
            self.cargo.identity(),
            self.git.identity(),
            self.rustc.identity(),
            self.rustup.identity(),
        ];
        identities.sort();
        identities
    }

    pub(crate) fn runtime_path(&self) -> &Path {
        self.runtime.path()
    }
}

impl QualificationExecutables {
    #[cfg(test)]
    pub(crate) fn prepare(root: &RepoRoot) -> Result<Self, ExecutableError> {
        let metadata = QualificationMetadataExecutables::prepare(root)?;
        Self::prepare_with_metadata(root, metadata)
    }

    pub(crate) fn prepare_with_metadata(
        root: &RepoRoot,
        metadata: QualificationMetadataExecutables,
    ) -> Result<Self, ExecutableError> {
        let cc = PinnedExecutable::open("cc", &resolve_from_path("cc")?)?;
        let cmake = PinnedExecutable::open("cmake", &resolve_from_path("cmake")?)?;
        let cxx = PinnedExecutable::open("cxx", &resolve_from_path("c++")?)?;
        let make = PinnedExecutable::open("make", &resolve_from_path("make")?)?;
        let worker = PinnedExecutable::open(
            "qualification-worker",
            &std::env::current_exe().map_err(|source| ExecutableError::Resolve {
                role: "qualification-worker",
                reason: source.to_string().into_boxed_str(),
            })?,
        )?;

        let cargo_home = metadata.runtime_path().join("cargo-home");
        let cargo_target = metadata.runtime_path().join("cargo-target");
        let home = metadata.runtime_path().join("qualification-home");
        let scratch = metadata.runtime_path().join("qualification-tmp");
        let stim_build = metadata.runtime_path().join("stim-build");
        let xdg = metadata.runtime_path().join("qualification-xdg");
        for directory in [
            &cargo_home,
            &cargo_target,
            &home,
            &scratch,
            &stim_build,
            &xdg,
        ] {
            std::fs::create_dir(directory).map_err(|source| ExecutableError::Build {
                step: "private build-directory preparation",
                reason: source.to_string().into_boxed_str(),
            })?;
        }
        link_cargo_cache(&cargo_home)?;
        let cmake_support = CmakeSupport::prepare(metadata.runtime_path(), &cmake)?;
        let compiler_support =
            compiler_support::resolve(root, &cc, &cxx, &home, &scratch, metadata.runtime_path())?;
        let environment = qualification_environment(
            QualificationEnvironmentInputs {
                cargo_home: &cargo_home,
                cargo_target: &cargo_target,
                home: &home,
                scratch: &scratch,
                xdg: &xdg,
                rustc: &metadata.rustc,
                cc: &cc,
                cxx: &cxx,
                compiler_support: &compiler_support,
                cmake_support: &cmake_support,
            },
            [
                &metadata.cargo,
                &metadata.rustc,
                &cc,
                &cmake,
                &cxx,
                &metadata.git,
                &make,
            ],
        )?;
        let environment_sha256 = environment_sha256(&environment);

        run_cargo_build(
            "private Stab build",
            &metadata.cargo.program(),
            [
                OsString::from("build"),
                OsString::from("--offline"),
                OsString::from("--release"),
                OsString::from("--quiet"),
                OsString::from("-p"),
                OsString::from("stab-cli"),
                OsString::from("--bin"),
                OsString::from("stab"),
                OsString::from("--manifest-path"),
                root.path.join("Cargo.toml").into_os_string(),
            ],
            &environment,
        )?;
        let stab = PinnedExecutable::open(
            "stab",
            &cargo_target
                .join("release")
                .join(format!("stab{}", std::env::consts::EXE_SUFFIX)),
        )?;

        run_build(
            "private Stim configure",
            cmake_support.program(),
            [
                OsString::from("-S"),
                root.stim_source().into_os_string(),
                OsString::from("-B"),
                stim_build.clone().into_os_string(),
                OsString::from("-G"),
                OsString::from("Unix Makefiles"),
                OsString::from("-DCMAKE_BUILD_TYPE=Release"),
                path_definition("CMAKE_MAKE_PROGRAM", &make.program()),
                path_definition("CMAKE_C_COMPILER", &cc.program()),
                path_definition("CMAKE_CXX_COMPILER", &cxx.program()),
            ],
            &scratch,
            &environment,
        )?;
        run_build(
            "private Stim build",
            cmake_support.program(),
            [
                OsString::from("--build"),
                stim_build.clone().into_os_string(),
                OsString::from("--target"),
                OsString::from("stim"),
                OsString::from("--parallel"),
            ],
            &scratch,
            &environment,
        )?;
        let stim = PinnedExecutable::open(
            "stim",
            &stim_build
                .join("out")
                .join(format!("stim{}", std::env::consts::EXE_SUFFIX)),
        )?;
        cmake_support.verify(&cmake)?;
        compiler_support.verify()?;

        let mut identities = vec![
            metadata.cargo.identity(),
            cc.identity(),
            cmake.identity(),
            cxx.identity(),
            metadata.git.identity(),
            make.identity(),
            worker.identity(),
            metadata.rustc.identity(),
            metadata.rustup.identity(),
            stab.identity(),
            stim.identity(),
        ];
        identities.extend(compiler_support.identities());
        identities.sort();
        validate_identities(&identities)?;
        Ok(Self {
            _cc: cc,
            _cmake: cmake,
            _cmake_support: cmake_support,
            _compiler_support: compiler_support,
            _cxx: cxx,
            _make: make,
            worker,
            stab,
            stim,
            environment,
            environment_sha256,
            identities,
            metadata,
        })
    }

    pub(crate) fn cargo(&self) -> PathBuf {
        self.metadata.cargo.program()
    }

    pub(crate) fn worker(&self) -> PathBuf {
        self.worker.program()
    }

    pub(crate) fn stab(&self) -> PathBuf {
        self.stab.program()
    }

    pub(crate) fn stim(&self) -> PathBuf {
        self.stim.program()
    }

    pub(crate) fn environment(&self) -> &[(OsString, OsString)] {
        &self.environment
    }

    pub(crate) fn environment_sha256(&self) -> &str {
        &self.environment_sha256
    }

    pub(crate) fn cargo_working_dir(&self) -> PathBuf {
        cargo_invocation_root().to_path_buf()
    }

    pub(super) fn identities(&self) -> &[ExecutableIdentity] {
        &self.identities
    }

    pub(crate) fn verify_support(&self) -> Result<(), ExecutableError> {
        self._cmake_support.verify(&self._cmake)?;
        self._compiler_support.verify()
    }
}

pub(super) fn validate_identities(
    identities: &[ExecutableIdentity],
) -> Result<(), ExecutableError> {
    let expected = REQUIRED_EXECUTABLE_ROLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = identities
        .iter()
        .map(|identity| identity.role.as_str())
        .collect::<BTreeSet<_>>();
    if actual != expected
        || identities
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if left >= right))
        || identities.iter().any(|identity| {
            identity.bytes == 0
                || identity.sha256.len() != 64
                || !identity
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err(ExecutableError::Resolve {
            role: "qualification-ledger",
            reason: "executable identities are incomplete, duplicated, unordered, or malformed"
                .into(),
        });
    }
    Ok(())
}

pub(crate) fn resolve_from_path(name: &'static str) -> Result<PathBuf, ExecutableError> {
    let path = std::env::var_os("PATH").ok_or_else(|| ExecutableError::Resolve {
        role: name,
        reason: "PATH is not set".into(),
    })?;
    for directory in std::env::split_paths(&path) {
        if !directory.is_absolute() {
            continue;
        }
        let candidate = directory.join(name);
        if candidate.is_file() {
            return std::fs::canonicalize(&candidate).map_err(|source| ExecutableError::Resolve {
                role: name,
                reason: source.to_string().into_boxed_str(),
            });
        }
    }
    Err(ExecutableError::Resolve {
        role: name,
        reason: "no executable regular file was found on absolute PATH entries".into(),
    })
}

fn resolve_rustup_tool(
    root: &RepoRoot,
    rustup: &PinnedExecutable,
    tool: &'static str,
    environment: &[(OsString, OsString)],
) -> Result<PathBuf, ExecutableError> {
    let output = crate::process::run_qualification_process_with_timeout_and_arg0(
        &rustup.program(),
        OsStr::new("rustup"),
        ["which", tool],
        &[],
        Some(&root.path),
        BUILD_TIMEOUT,
        environment,
    )
    .map_err(|source| ExecutableError::Resolve {
        role: tool,
        reason: source.to_string().into_boxed_str(),
    })?;
    if !output.success() {
        return Err(ExecutableError::Resolve {
            role: tool,
            reason: output.stderr.render_for_diagnostics().into_boxed_str(),
        });
    }
    let text =
        std::str::from_utf8(&output.stdout.bytes).map_err(|source| ExecutableError::Resolve {
            role: tool,
            reason: source.to_string().into_boxed_str(),
        })?;
    let path = PathBuf::from(text.trim());
    if !path.is_absolute() {
        return Err(ExecutableError::Resolve {
            role: tool,
            reason: "rustup returned a non-absolute tool path".into(),
        });
    }
    Ok(path)
}

fn rustup_resolution_environment(
    home: &Path,
    scratch: &Path,
    xdg: &Path,
) -> Result<Vec<(OsString, OsString)>, ExecutableError> {
    let rustup_home = source_home("RUSTUP_HOME", ".rustup")?;
    Ok(vec![
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("RUSTUP_HOME"), rustup_home),
        (OsString::from("TMPDIR"), scratch.as_os_str().to_owned()),
        (OsString::from("TZ"), OsString::from("UTC")),
        (
            OsString::from("XDG_CONFIG_HOME"),
            xdg.as_os_str().to_owned(),
        ),
    ])
}

fn metadata_environment<'a>(
    home: &Path,
    scratch: &Path,
    xdg: &Path,
    path_tools: impl IntoIterator<Item = &'a PinnedExecutable>,
    rustc: &PinnedExecutable,
) -> Result<Vec<(OsString, OsString)>, ExecutableError> {
    Ok(vec![
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (
            OsString::from("LD_LIBRARY_PATH"),
            rustc_library_path(rustc)?,
        ),
        (OsString::from("PATH"), controlled_path(path_tools)?),
        (OsString::from("TMPDIR"), scratch.as_os_str().to_owned()),
        (OsString::from("TZ"), OsString::from("UTC")),
        (
            OsString::from("XDG_CONFIG_HOME"),
            xdg.as_os_str().to_owned(),
        ),
    ])
}

struct QualificationEnvironmentInputs<'a> {
    cargo_home: &'a Path,
    cargo_target: &'a Path,
    home: &'a Path,
    scratch: &'a Path,
    xdg: &'a Path,
    rustc: &'a PinnedExecutable,
    cc: &'a PinnedExecutable,
    cxx: &'a PinnedExecutable,
    compiler_support: &'a CompilerSupport,
    cmake_support: &'a CmakeSupport,
}

fn qualification_environment<'a>(
    inputs: QualificationEnvironmentInputs<'_>,
    path_tools: impl IntoIterator<Item = &'a PinnedExecutable>,
) -> Result<Vec<(OsString, OsString)>, ExecutableError> {
    let QualificationEnvironmentInputs {
        cargo_home,
        cargo_target,
        home,
        scratch,
        xdg,
        rustc,
        cc,
        cxx,
        compiler_support,
        cmake_support,
    } = inputs;
    Ok(vec![
        (
            OsString::from("CARGO_HOME"),
            cargo_home.as_os_str().to_owned(),
        ),
        (OsString::from("CARGO_INCREMENTAL"), OsString::from("0")),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        (
            OsString::from("CARGO_TARGET_DIR"),
            cargo_target.as_os_str().to_owned(),
        ),
        (OsString::from("CC"), cc.program().into_os_string()),
        (
            OsString::from("C_INCLUDE_PATH"),
            compiler_support.c_includes.clone(),
        ),
        (
            OsString::from("CFLAGS"),
            compiler_include_flags(&compiler_support.c_includes, false)?,
        ),
        (
            OsString::from("COMPILER_PATH"),
            compiler_support.programs.clone(),
        ),
        (OsString::from("CXX"), cxx.program().into_os_string()),
        (
            OsString::from("CXXFLAGS"),
            compiler_include_flags(&compiler_support.cxx_includes, true)?,
        ),
        (
            OsString::from("CPLUS_INCLUDE_PATH"),
            compiler_support.cxx_includes.clone(),
        ),
        (
            OsString::from("STAB_CQ1_CMAKE_SUPPORT_SHA256"),
            OsString::from(cmake_support.digest()),
        ),
        (
            OsString::from("STAB_CQ1_COMPILER_SUPPORT_SHA256"),
            OsString::from(compiler_support.digest()),
        ),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (
            OsString::from("LIBRARY_PATH"),
            compiler_support.libraries.clone(),
        ),
        (
            OsString::from("LD_LIBRARY_PATH"),
            rustc_library_path(rustc)?,
        ),
        (OsString::from("PATH"), controlled_path(path_tools)?),
        (OsString::from("RUST_BACKTRACE"), OsString::from("0")),
        (OsString::from("RUSTC"), rustc.program().into_os_string()),
        (
            OsString::from("RUSTFLAGS"),
            OsString::from(format!("-Clinker={}", cc.program().display())),
        ),
        (OsString::from("TMPDIR"), scratch.as_os_str().to_owned()),
        (OsString::from("TZ"), OsString::from("UTC")),
        (
            OsString::from("XDG_CONFIG_HOME"),
            xdg.as_os_str().to_owned(),
        ),
    ])
}

fn compiler_include_flags(paths: &OsStr, cxx: bool) -> Result<OsString, ExecutableError> {
    let mut flags = OsString::from("-nostdinc");
    if cxx {
        flags.push(" -nostdinc++");
    }
    for path in std::env::split_paths(paths) {
        if path
            .as_os_str()
            .as_bytes()
            .iter()
            .any(u8::is_ascii_whitespace)
        {
            return Err(ExecutableError::Build {
                step: "qualification environment",
                reason: format!(
                    "compiler support path {} contains unsupported whitespace",
                    path.display()
                )
                .into_boxed_str(),
            });
        }
        flags.push(" -isystem ");
        flags.push(path);
    }
    Ok(flags)
}

fn rustc_library_path(rustc: &PinnedExecutable) -> Result<OsString, ExecutableError> {
    let library = rustc
        .path
        .parent()
        .and_then(Path::parent)
        .map(|toolchain| toolchain.join("lib"))
        .ok_or_else(|| ExecutableError::Build {
            step: "qualification environment",
            reason: "resolved rustc has no toolchain library directory".into(),
        })?;
    if !library.is_dir() {
        return Err(ExecutableError::Build {
            step: "qualification environment",
            reason: format!(
                "rustc toolchain library directory {} is missing",
                library.display()
            )
            .into_boxed_str(),
        });
    }
    Ok(library.into_os_string())
}

fn controlled_path<'a>(
    path_tools: impl IntoIterator<Item = &'a PinnedExecutable>,
) -> Result<OsString, ExecutableError> {
    let mut path_directories = BTreeSet::new();
    for tool in path_tools {
        if let Some(parent) = tool.parent() {
            path_directories.insert(parent.to_path_buf());
        }
    }
    for system in [PathBuf::from("/usr/bin"), PathBuf::from("/bin")] {
        path_directories.insert(system);
    }
    std::env::join_paths(path_directories).map_err(|source| ExecutableError::Build {
        step: "qualification environment",
        reason: source.to_string().into_boxed_str(),
    })
}

fn source_home(variable: &'static str, fallback: &str) -> Result<OsString, ExecutableError> {
    if let Some(value) = std::env::var_os(variable) {
        return Ok(value);
    }
    let home = std::env::var_os("HOME").ok_or_else(|| ExecutableError::Build {
        step: "qualification environment",
        reason: format!("HOME or {variable} is required to resolve installed Rust tools").into(),
    })?;
    Ok(PathBuf::from(home).join(fallback).into_os_string())
}

fn link_cargo_cache(private_cargo_home: &Path) -> Result<(), ExecutableError> {
    let source_cargo_home = PathBuf::from(source_home("CARGO_HOME", ".cargo")?);
    link_cargo_cache_from(&source_cargo_home, private_cargo_home)
}

fn link_cargo_cache_from(
    source_cargo_home: &Path,
    private_cargo_home: &Path,
) -> Result<(), ExecutableError> {
    for name in ["registry", "git"] {
        let source = source_cargo_home.join(name);
        if !source.is_dir() {
            continue;
        }
        std::os::unix::fs::symlink(&source, private_cargo_home.join(name)).map_err(|error| {
            ExecutableError::Build {
                step: "private Cargo cache preparation",
                reason: error.to_string().into_boxed_str(),
            }
        })?;
    }
    Ok(())
}

fn environment_sha256(environment: &[(OsString, OsString)]) -> String {
    let mut entries = environment.to_vec();
    entries.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"stab-cq1/qualification-environment/v1\0");
    for (key, value) in entries {
        hash_os_string(&mut hasher, &key);
        hash_os_string(&mut hasher, &value);
    }
    render_sha256(&hasher.finalize())
}

fn hash_os_string(hasher: &mut Sha256, value: &OsStr) {
    let bytes = value.as_bytes();
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn run_build<I, S>(
    step: &'static str,
    program: &Path,
    args: I,
    working_dir: &Path,
    environment: &[(OsString, OsString)],
) -> Result<(), ExecutableError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = crate::process::run_qualification_process_with_timeout(
        program,
        args,
        &[],
        Some(working_dir),
        BUILD_TIMEOUT,
        environment,
    )
    .map_err(|source| ExecutableError::Build {
        step,
        reason: source.to_string().into_boxed_str(),
    })?;
    if !output.success() {
        return Err(ExecutableError::Build {
            step,
            reason: format!(
                "status {}\nstdout:\n{}\nstderr:\n{}",
                crate::process::display_status(output.status),
                output.stdout.render_for_diagnostics(),
                output.stderr.render_for_diagnostics()
            )
            .into_boxed_str(),
        });
    }
    Ok(())
}

fn run_cargo_build<I, S>(
    step: &'static str,
    program: &Path,
    args: I,
    environment: &[(OsString, OsString)],
) -> Result<(), ExecutableError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_build(step, program, args, cargo_invocation_root(), environment)
}

fn cargo_invocation_root() -> &'static Path {
    Path::new(CARGO_INVOCATION_ROOT)
}

fn path_definition(name: &str, path: &Path) -> OsString {
    let mut value = OsString::from(format!("-D{name}="));
    value.push(path);
    value
}

fn render_sha256(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "executables/tests.rs"]
mod tests;
