//! Oracle orchestration for the pinned Stim v1.16.0 submodule.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

mod fixtures;
mod matrix;

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use thiserror::Error;
use wait_timeout::ChildExt;

const PREFIX: &str = "stab-oracle";
const STIM_TAG: &str = "v1.16.0";
const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
const VENDOR_STIM_PATH: &str = "vendor/stim";
const BUILD_DIR: &str = "target/oracle/stim-v1.16.0";
const BUILD_STAMP_FILE: &str = "stab-oracle-build-stamp.txt";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
const DIAGNOSTIC_LIMIT_BYTES: usize = 4096;

#[derive(Debug, Parser)]
#[command(
    about = "Runs pinned Stim oracle maintenance and smoke comparisons.",
    long_about = "Validates the vendor/stim submodule, builds the pinned C++ Stim binary, and compares named smoke cases against the local Stab CLI."
)]
struct Cli {
    /// Repository root containing Cargo.toml and vendor/stim.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize, update, and validate the pinned Stim submodule.
    Fetch,

    /// Print and assert the pinned Stim version information.
    Version,

    /// Run a named oracle smoke comparison.
    Run {
        /// Smoke case to execute.
        #[arg(long = "case")]
        case: Option<OracleCase>,

        /// Run all manifest fixtures that are marked implemented.
        #[arg(long)]
        implemented_only: bool,

        /// Run implemented manifest fixtures and report red or ignored cases.
        #[arg(long)]
        all: bool,

        /// Run implemented fixtures and report pending fixtures for a milestone such as M4.
        #[arg(long)]
        milestone: Option<String>,

        /// Reconfigure and rebuild the C++ Stim oracle even if a binary exists.
        #[arg(long)]
        rebuild_stim: bool,
    },

    /// List oracle fixtures grouped by milestone, parity mode, and status.
    List {
        /// Only list fixtures for a milestone such as M4.
        #[arg(long)]
        milestone: Option<String>,
    },

    /// Record or check exact-output fixtures from pinned Stim.
    Record {
        /// Fail if generated exact outputs differ from committed fixtures.
        #[arg(long)]
        check_clean: bool,

        /// Reconfigure and rebuild the C++ Stim oracle even if a binary exists.
        #[arg(long)]
        rebuild_stim: bool,
    },

    /// Inspect or validate the compatibility matrix.
    Matrix {
        /// Validate matrix coverage and acceptance metadata.
        #[arg(long)]
        check: bool,

        /// Print rows owned by a milestone such as M4.
        #[arg(long)]
        milestone: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OracleCase {
    /// Compare help command health.
    #[value(name = "smoke/help")]
    SmokeHelp,

    /// Compare deterministic sampling of a tiny measurement-only circuit.
    #[value(name = "smoke/tiny-circuit")]
    SmokeTinyCircuit,
}

#[derive(Debug, Error)]
enum OracleError {
    #[error(transparent)]
    Fixture(#[from] fixtures::FixtureError),

    #[error(transparent)]
    Matrix(#[from] matrix::MatrixError),

    #[error("{0}")]
    InvalidRunSelection(String),

    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Stim source directory does not exist at {0}")]
    MissingStimSource(PathBuf),

    #[error("Stim submodule is at commit {actual}, expected {expected}")]
    WrongStimCommit { actual: String, expected: String },

    #[error("Stim submodule is at tag {actual}, expected {expected}")]
    WrongStimTag { actual: String, expected: String },

    #[error("Stim submodule has tracked local modifications:\n{status}")]
    DirtyStimSource { status: Box<str> },

    #[error("failed to create build directory {path}: {source}")]
    CreateBuildDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write Stim oracle build stamp {path}: {source}")]
    WriteBuildStamp {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("CMake build finished without producing {0}")]
    MissingStimBinary(PathBuf),

    #[error("failed to start {program}: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to write stdin for {program}: {source}")]
    WriteStdin {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to wait for {program}: {source}")]
    Wait {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to capture output from {program}: {source}")]
    CaptureOutput {
        program: String,
        source: std::io::Error,
    },

    #[error("{program} timed out after {seconds}s\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    TimedOut {
        program: String,
        seconds: u64,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error("{program} failed with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    CommandFailed {
        program: String,
        status: String,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error(
        "{case} failed: {reason}\nStim stdout:\n{stim_stdout}\nStim stderr:\n{stim_stderr}\nStab stdout:\n{stab_stdout}\nStab stderr:\n{stab_stderr}"
    )]
    CaseFailed {
        case: &'static str,
        reason: Box<str>,
        stim_stdout: Box<str>,
        stim_stderr: Box<str>,
        stab_stdout: Box<str>,
        stab_stderr: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RepoRoot {
    path: PathBuf,
}

impl RepoRoot {
    fn resolve(path: &Path) -> Result<Self, OracleError> {
        let path = std::fs::canonicalize(path).map_err(|source| OracleError::ResolveRoot {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self { path })
    }

    fn stim_source(&self) -> PathBuf {
        self.path.join(VENDOR_STIM_PATH)
    }

    fn build_dir(&self) -> PathBuf {
        self.path.join(BUILD_DIR)
    }

    fn stim_binary(&self) -> PathBuf {
        self.build_dir()
            .join("out")
            .join(format!("stim{}", std::env::consts::EXE_SUFFIX))
    }

    fn build_stamp(&self) -> PathBuf {
        self.build_dir().join(BUILD_STAMP_FILE)
    }

    fn compatibility_matrix(&self) -> PathBuf {
        self.path.join("oracle").join("compatibility-matrix.csv")
    }

    fn fixture_manifest(&self) -> PathBuf {
        self.path
            .join("oracle")
            .join("fixtures")
            .join("manifest.csv")
    }

    fn stab_cli_binary(&self) -> PathBuf {
        self.path
            .join("target")
            .join("debug")
            .join(format!("stab-cli{}", std::env::consts::EXE_SUFFIX))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StimSourceVersion {
    commit: String,
    tag: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

impl CapturedOutput {
    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn has_non_whitespace(&self) -> bool {
        self.bytes.iter().any(|byte| !byte.is_ascii_whitespace())
    }

    fn render_for_diagnostics(&self) -> String {
        render_bytes_for_diagnostics(&self.bytes, self.truncated)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProcessOutput {
    status: Option<i32>,
    stdout: CapturedOutput,
    stderr: CapturedOutput,
}

impl ProcessOutput {
    fn success(&self) -> bool {
        self.status == Some(0)
    }

    fn stderr_class(&self) -> StderrClass {
        if self.stderr.is_empty() {
            StderrClass::Empty
        } else {
            StderrClass::NonEmpty
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StderrClass {
    Empty,
    NonEmpty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SmokeCase {
    name: &'static str,
    args: Vec<&'static str>,
    stdin: &'static [u8],
    comparator: Comparator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Comparator {
    HelpHealth,
    Exact,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[{PREFIX}] ERROR: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), OracleError> {
    let root = RepoRoot::resolve(&cli.root)?;
    match cli.command {
        Command::Fetch => {
            fetch_stim(&root)?;
            print_version(&root)?;
        }
        Command::Version => {
            print_version(&root)?;
        }
        Command::Run {
            case,
            implemented_only,
            all,
            milestone,
            rebuild_stim,
        } => {
            run_selected_cases(&root, case, implemented_only, all, milestone, rebuild_stim)?;
        }
        Command::List { milestone } => {
            fixtures::list_fixtures(&root, milestone.as_deref())?;
        }
        Command::Record {
            check_clean,
            rebuild_stim,
        } => {
            fixtures::record_fixtures(&root, check_clean, rebuild_stim)?;
        }
        Command::Matrix { check, milestone } => {
            run_matrix_command(&root, check, milestone.as_deref())?;
        }
    }
    Ok(())
}

fn run_selected_cases(
    root: &RepoRoot,
    case: Option<OracleCase>,
    implemented_only: bool,
    all: bool,
    milestone: Option<String>,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let selected = usize::from(case.is_some())
        + usize::from(implemented_only)
        + usize::from(all)
        + usize::from(milestone.is_some());
    if selected != 1 {
        return Err(OracleError::InvalidRunSelection(
            "choose exactly one of --case, --implemented-only, --all, or --milestone".to_string(),
        ));
    }
    if let Some(case) = case {
        return run_smoke_case(root, case, rebuild_stim);
    }
    if implemented_only {
        return fixtures::run_fixtures(root, fixtures::RunMode::ImplementedOnly, rebuild_stim);
    }
    if let Some(milestone) = milestone {
        return fixtures::run_fixtures(root, fixtures::RunMode::Milestone(milestone), rebuild_stim);
    }
    fixtures::run_fixtures(root, fixtures::RunMode::All, rebuild_stim)
}

fn run_matrix_command(
    root: &RepoRoot,
    check: bool,
    milestone: Option<&str>,
) -> Result<(), OracleError> {
    let matrix = matrix::CompatibilityMatrix::read_from_path(root.compatibility_matrix())?;
    if check {
        let report = matrix.check(&root.path)?;
        report.print();
    }
    if let Some(milestone) = milestone {
        matrix.print_milestone(milestone)?;
    }
    if !check && milestone.is_none() {
        matrix.print_summary();
    }
    Ok(())
}

fn fetch_stim(root: &RepoRoot) -> Result<(), OracleError> {
    run_checked(
        "git",
        ["submodule", "update", "--init", "--", VENDOR_STIM_PATH],
        b"",
        Some(&root.path),
    )?;
    validate_stim_source(root)?;
    Ok(())
}

fn print_version(root: &RepoRoot) -> Result<(), OracleError> {
    let version = validate_stim_source(root)?;
    println!("Stim source: {}", VENDOR_STIM_PATH);
    println!("Expected tag: {STIM_TAG}");
    println!("Expected commit: {STIM_COMMIT}");
    println!("Actual tag: {}", version.tag);
    println!("Actual commit: {}", version.commit);
    println!("Status: OK");
    Ok(())
}

fn validate_stim_source(root: &RepoRoot) -> Result<StimSourceVersion, OracleError> {
    let stim_source = root.stim_source();
    if !stim_source.is_dir() {
        return Err(OracleError::MissingStimSource(stim_source));
    }
    let commit = git_output(&stim_source, ["rev-parse", "HEAD"])?;
    if commit != STIM_COMMIT {
        return Err(OracleError::WrongStimCommit {
            actual: commit,
            expected: STIM_COMMIT.to_string(),
        });
    }
    let tag = exact_tag(root)?;
    if tag != STIM_TAG {
        return Err(OracleError::WrongStimTag {
            actual: tag,
            expected: STIM_TAG.to_string(),
        });
    }
    let status = git_output(
        &stim_source,
        ["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !status.is_empty() {
        return Err(OracleError::DirtyStimSource {
            status: status.into_boxed_str(),
        });
    }
    Ok(StimSourceVersion {
        commit: STIM_COMMIT.to_string(),
        tag: STIM_TAG.to_string(),
    })
}

fn exact_tag(root: &RepoRoot) -> Result<String, OracleError> {
    git_output(&root.stim_source(), ["describe", "--tags", "--exact-match"])
}

fn git_output<I, S>(working_dir: &Path, args: I) -> Result<String, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_checked("git", args, b"", Some(working_dir))?;
    Ok(String::from_utf8_lossy(&output.stdout.bytes)
        .trim()
        .to_string())
}

fn ensure_stim_binary(root: &RepoRoot, rebuild: bool) -> Result<PathBuf, OracleError> {
    let version = validate_stim_source(root)?;
    let binary = root.stim_binary();
    if binary.is_file() && !rebuild && build_stamp_matches(root, &version) {
        return Ok(binary);
    }

    let build_dir = root.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(|source| OracleError::CreateBuildDir {
        path: build_dir.clone(),
        source,
    })?;

    run_checked(
        "cmake",
        [
            OsString::from("-S"),
            root.stim_source().into_os_string(),
            OsString::from("-B"),
            build_dir.clone().into_os_string(),
            OsString::from("-DCMAKE_BUILD_TYPE=Release"),
        ],
        b"",
        Some(&root.path),
    )?;
    run_checked(
        "cmake",
        [
            OsString::from("--build"),
            build_dir.into_os_string(),
            OsString::from("--target"),
            OsString::from("stim"),
            OsString::from("--parallel"),
        ],
        b"",
        Some(&root.path),
    )?;
    if !binary.is_file() {
        return Err(OracleError::MissingStimBinary(binary));
    }
    std::fs::write(root.build_stamp(), build_stamp_content(&version)).map_err(|source| {
        OracleError::WriteBuildStamp {
            path: root.build_stamp(),
            source,
        }
    })?;
    Ok(binary)
}

fn build_stamp_matches(root: &RepoRoot, version: &StimSourceVersion) -> bool {
    std::fs::read_to_string(root.build_stamp())
        .is_ok_and(|actual| actual == build_stamp_content(version))
}

fn build_stamp_content(version: &StimSourceVersion) -> String {
    format!(
        "stim_tag={}\nstim_commit={}\nbuild_type=Release\n",
        version.tag, version.commit
    )
}

fn ensure_stab_cli_binary(root: &RepoRoot) -> Result<PathBuf, OracleError> {
    run_checked(
        "cargo",
        ["build", "-q", "-p", "stab-cli"],
        b"",
        Some(&root.path),
    )?;
    Ok(root.stab_cli_binary())
}

fn run_smoke_case(
    root: &RepoRoot,
    case: OracleCase,
    rebuild_stim: bool,
) -> Result<(), OracleError> {
    let smoke_case = smoke_case(case);
    let stim_binary = ensure_stim_binary(root, rebuild_stim)?;
    let stab_binary = ensure_stab_cli_binary(root)?;

    let stim = run_process(
        &stim_binary,
        &smoke_case.args,
        smoke_case.stdin,
        Some(&root.path),
    )?;
    let stab = run_process(
        &stab_binary,
        &smoke_case.args,
        smoke_case.stdin,
        Some(&root.path),
    )?;
    compare_outputs(&smoke_case, &stim, &stab)?;
    println!(
        "[{PREFIX}] PASS {} status={:?} stderr_class={:?}",
        smoke_case.name,
        stab.status,
        stab.stderr_class()
    );
    Ok(())
}

fn smoke_case(case: OracleCase) -> SmokeCase {
    match case {
        OracleCase::SmokeHelp => SmokeCase {
            name: "smoke/help",
            args: vec!["--help"],
            stdin: b"",
            comparator: Comparator::HelpHealth,
        },
        OracleCase::SmokeTinyCircuit => SmokeCase {
            name: "smoke/tiny-circuit",
            args: vec!["sample", "--shots", "2"],
            stdin: b"M 0\n",
            comparator: Comparator::Exact,
        },
    }
}

fn compare_outputs(
    case: &SmokeCase,
    stim: &ProcessOutput,
    stab: &ProcessOutput,
) -> Result<(), OracleError> {
    let reason = match case.comparator {
        Comparator::HelpHealth => compare_help_health(stim, stab),
        Comparator::Exact => compare_exact(stim, stab),
    };
    if let Some(reason) = reason {
        return Err(OracleError::CaseFailed {
            case: case.name,
            reason: reason.into_boxed_str(),
            stim_stdout: stim.stdout.render_for_diagnostics().into_boxed_str(),
            stim_stderr: stim.stderr.render_for_diagnostics().into_boxed_str(),
            stab_stdout: stab.stdout.render_for_diagnostics().into_boxed_str(),
            stab_stderr: stab.stderr.render_for_diagnostics().into_boxed_str(),
        });
    }
    Ok(())
}

fn compare_help_health(stim: &ProcessOutput, stab: &ProcessOutput) -> Option<String> {
    if !stim.success() {
        return Some(format!("Stim help exited with {:?}", stim.status));
    }
    if !stab.success() {
        return Some(format!("Stab help exited with {:?}", stab.status));
    }
    if stim.stderr_class() != stab.stderr_class() {
        return Some(format!(
            "stderr class mismatch: Stim {:?}, Stab {:?}",
            stim.stderr_class(),
            stab.stderr_class()
        ));
    }
    if !stim.stdout.has_non_whitespace() {
        return Some("Stim help wrote empty stdout".to_string());
    }
    if !stab.stdout.has_non_whitespace() {
        return Some("Stab help wrote empty stdout".to_string());
    }
    None
}

fn compare_exact(stim: &ProcessOutput, stab: &ProcessOutput) -> Option<String> {
    if stim.status != stab.status {
        return Some(format!(
            "exit status mismatch: Stim {:?}, Stab {:?}",
            stim.status, stab.status
        ));
    }
    if stim.stderr_class() != stab.stderr_class() {
        return Some(format!(
            "stderr class mismatch: Stim {:?}, Stab {:?}",
            stim.stderr_class(),
            stab.stderr_class()
        ));
    }
    if stim.stdout.bytes != stab.stdout.bytes {
        return Some("stdout mismatch".to_string());
    }
    None
}

fn run_checked<I, S>(
    program: &str,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_process(Path::new(program), args, stdin, working_dir)?;
    if output.success() {
        return Ok(output);
    }
    Err(OracleError::CommandFailed {
        program: program.to_string(),
        status: display_status(output.status),
        stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
        stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
    })
}

fn run_process<I, S>(
    program: &Path,
    args: I,
    stdin: &[u8],
    working_dir: Option<&Path>,
) -> Result<ProcessOutput, OracleError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let program_name = program.display().to_string();
    let mut child = command.spawn().map_err(|source| OracleError::Spawn {
        program: program_name.clone(),
        source,
    })?;
    let stdout = child
        .stdout
        .take()
        .map(spawn_output_reader)
        .ok_or_else(|| OracleError::CaptureOutput {
            program: program_name.clone(),
            source: std::io::Error::other("child stdout was not piped"),
        })?;
    let stderr = child
        .stderr
        .take()
        .map(spawn_output_reader)
        .ok_or_else(|| OracleError::CaptureOutput {
            program: program_name.clone(),
            source: std::io::Error::other("child stderr was not piped"),
        })?;

    let child_stdin = child.stdin.take();
    if let Some(mut child_stdin) = child_stdin {
        child_stdin
            .write_all(stdin)
            .map_err(|source| OracleError::WriteStdin {
                program: program.display().to_string(),
                source,
            })?;
    }
    let status = match child
        .wait_timeout(COMMAND_TIMEOUT)
        .map_err(|source| OracleError::Wait {
            program: program.display().to_string(),
            source,
        })? {
        Some(status) => status,
        None => {
            let _kill_result = child.kill();
            let _wait_result = child.wait();
            let stdout = join_output_reader(&program_name, stdout)?;
            let stderr = join_output_reader(&program_name, stderr)?;
            return Err(OracleError::TimedOut {
                program: program_name,
                seconds: COMMAND_TIMEOUT.as_secs(),
                stdout: stdout.render_for_diagnostics().into_boxed_str(),
                stderr: stderr.render_for_diagnostics().into_boxed_str(),
            });
        }
    };
    let stdout = join_output_reader(&program_name, stdout)?;
    let stderr = join_output_reader(&program_name, stderr)?;
    Ok(ProcessOutput {
        status: status.code(),
        stdout,
        stderr,
    })
}

fn spawn_output_reader<R>(mut reader: R) -> JoinHandle<Result<CapturedOutput, std::io::Error>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        let mut truncated = false;
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let chunk = buffer.get(..bytes_read).ok_or_else(|| {
                std::io::Error::other("bounded output reader exceeded buffer bounds")
            })?;
            let remaining = OUTPUT_LIMIT_BYTES.saturating_sub(bytes.len());
            if bytes_read <= remaining {
                bytes.extend_from_slice(chunk);
            } else {
                let kept = chunk.get(..remaining).ok_or_else(|| {
                    std::io::Error::other("bounded output reader exceeded keep bounds")
                })?;
                bytes.extend_from_slice(kept);
                truncated = true;
            }
        }
        Ok(CapturedOutput { bytes, truncated })
    })
}

fn join_output_reader(
    program: &str,
    handle: JoinHandle<Result<CapturedOutput, std::io::Error>>,
) -> Result<CapturedOutput, OracleError> {
    match handle.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(source)) => Err(OracleError::CaptureOutput {
            program: program.to_string(),
            source,
        }),
        Err(_panic) => Err(OracleError::CaptureOutput {
            program: program.to_string(),
            source: std::io::Error::other("output reader thread panicked"),
        }),
    }
}

fn render_bytes_for_diagnostics(bytes: &[u8], truncated: bool) -> String {
    let mut rendered = String::new();
    let display_len = bytes.len().min(DIAGNOSTIC_LIMIT_BYTES);
    let display_bytes = bytes.get(..display_len).unwrap_or(bytes);
    for byte in display_bytes {
        for escaped in std::ascii::escape_default(*byte) {
            rendered.push(char::from(escaped));
        }
    }
    if truncated || bytes.len() > display_len {
        rendered.push_str("\n[truncated]");
    }
    rendered
}

fn display_status(status: Option<i32>) -> String {
    match status {
        Some(status) => status.to_string(),
        None => "terminated by signal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Comparator, OracleCase, ProcessOutput, SmokeCase, StderrClass, compare_exact,
        compare_help_health, smoke_case,
    };

    #[test]
    fn smoke_tiny_circuit_uses_exact_sampling_comparator() {
        let case = smoke_case(OracleCase::SmokeTinyCircuit);

        assert_eq!(case.name, "smoke/tiny-circuit");
        assert_eq!(case.args, vec!["sample", "--shots", "2"]);
        assert_eq!(case.stdin, b"M 0\n");
        assert_eq!(case.comparator, Comparator::Exact);
    }

    #[test]
    fn smoke_help_uses_health_comparator() {
        let case = smoke_case(OracleCase::SmokeHelp);

        assert_eq!(case.name, "smoke/help");
        assert_eq!(case.args, vec!["--help"]);
        assert_eq!(case.stdin, b"");
        assert_eq!(case.comparator, Comparator::HelpHealth);
    }

    #[test]
    fn exact_comparator_rejects_stdout_drift() {
        let stim = output(Some(0), b"0\n", b"");
        let stab = output(Some(0), b"1\n", b"");

        assert_eq!(
            compare_exact(&stim, &stab),
            Some("stdout mismatch".to_string())
        );
    }

    #[test]
    fn exact_comparator_rejects_invalid_utf8_byte_drift() {
        let stim = output(Some(0), b"\x80\n", b"");
        let stab = output(Some(0), b"\x81\n", b"");

        assert_eq!(
            compare_exact(&stim, &stab),
            Some("stdout mismatch".to_string())
        );
    }

    #[test]
    fn exact_comparator_accepts_matching_tiny_sample() {
        let stim = output(Some(0), b"0\n0\n", b"");
        let stab = output(Some(0), b"0\n0\n", b"");

        assert_eq!(compare_exact(&stim, &stab), None);
    }

    #[test]
    fn help_comparator_requires_successful_nonempty_stdout() {
        let stim = output(Some(0), b"Available stim commands\n", b"");
        let stab = output(Some(0), b"Usage: stab\n", b"");

        assert_eq!(compare_help_health(&stim, &stab), None);
    }

    #[test]
    fn process_output_classifies_stderr() {
        assert_eq!(output(Some(0), b"", b"").stderr_class(), StderrClass::Empty);
        assert_eq!(
            output(Some(0), b"", b"warning").stderr_class(),
            StderrClass::NonEmpty
        );
    }

    #[test]
    fn smoke_case_names_are_stable() {
        let cases = [
            smoke_case(OracleCase::SmokeHelp),
            smoke_case(OracleCase::SmokeTinyCircuit),
        ];
        let names = cases.iter().map(|case| case.name).collect::<Vec<_>>();

        assert_eq!(names, vec!["smoke/help", "smoke/tiny-circuit"]);
    }

    fn output(status: Option<i32>, stdout: &[u8], stderr: &[u8]) -> ProcessOutput {
        ProcessOutput {
            status,
            stdout: super::CapturedOutput {
                bytes: stdout.to_vec(),
                truncated: false,
            },
            stderr: super::CapturedOutput {
                bytes: stderr.to_vec(),
                truncated: false,
            },
        }
    }

    #[allow(
        dead_code,
        reason = "keeps SmokeCase imported in a visible type assertion"
    )]
    fn _assert_smoke_case_is_debug(case: SmokeCase) -> String {
        format!("{case:?}")
    }
}
