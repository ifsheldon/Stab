//! The `e2e-run` controller: selection, pinned binaries, host controls, and bundle publication.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;
use stab_process::Cancellation;
use tempfile::TempDir;

use super::bundle::{file_sha256, publish_bundle};
use super::commands::command_output;
use super::host::{CompetitorScan, HostProfile, validate_host_after};
use super::model::{Case, CaseId, Family, Suite, Tier};
use super::report::{
    CORRECTNESS_SCHEMA_VERSION, CorrectnessReport, DerivedReport, RUN_SCHEMA_VERSION, RawSamples,
    RunMetadata, SAMPLES_SCHEMA_VERSION,
};
use super::rust_pipeline::{execute as execute_rust_pipeline, write_receipt};
use super::{LoadedSuite, load_suite, validate_prerequisites};
use crate::error::BenchError;
use crate::root::RepoRoot;
use crate::stim::{ensure_stim_cli, validate_stim_source};

mod case;

use case::PreparedCase;

/// Bound on Cargo's JSON message stream for the release Stab build.
const CARGO_MESSAGE_LIMIT_BYTES: usize = 64 << 20;
/// Wall-clock bound on the release Stab build.
const CARGO_BUILD_TIMEOUT_SECONDS: u64 = 2 * 60 * 60;

#[derive(Clone, Debug, Args)]
pub(crate) struct RunArgs {
    /// Evidence tier controlling warmups, sample count, and host requirements.
    #[arg(long, value_enum, default_value = "smoke")]
    tier: Tier,

    /// New repository-relative bundle directory under target/benchmarks.
    #[arg(long)]
    out: PathBuf,

    /// Run an exact family or family.case id. Repeat to select several.
    #[arg(long = "only")]
    only: Vec<String>,

    /// CPU used for every measured child process.
    #[arg(long)]
    affinity_cpu: Option<usize>,

    /// Permit a dirty source tree for smoke diagnostics only.
    #[arg(long)]
    allow_dirty: bool,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct WorkerArgs {
    #[arg(long)]
    shots: u64,
    #[arg(long)]
    minimum_logical_failures: u64,
    #[arg(long)]
    maximum_logical_failures: u64,
    #[arg(long)]
    seed: u64,
}

pub(crate) fn run_worker(args: WorkerArgs) -> Result<(), BenchError> {
    let receipt = execute_rust_pipeline(
        args.shots,
        args.minimum_logical_failures,
        args.maximum_logical_failures,
        args.seed,
    )
    .map_err(BenchError::E2e)?;
    write_receipt(&receipt).map_err(BenchError::E2e)
}

/// Fails once SIGINT or SIGTERM has been received, so an interrupted run stops at the next phase
/// boundary and never publishes a bundle.
fn check_cancelled(cancellation: &Cancellation) -> Result<(), BenchError> {
    if cancellation.is_cancelled() {
        return Err(BenchError::E2e(
            "interrupted by SIGINT or SIGTERM; no bundle was published".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn run(root: &RepoRoot, args: RunArgs) -> Result<(), BenchError> {
    let cancellation = Cancellation::for_signals()
        .map_err(|source| BenchError::E2e(format!("cannot install signal handlers: {source}")))?;
    if args.tier != Tier::Smoke && args.allow_dirty {
        return Err(BenchError::E2e(
            "--allow-dirty is available only for smoke diagnostics".to_string(),
        ));
    }
    let loaded = load_suite(root)?;
    validate_prerequisites(root, &loaded.suite)?;
    let selected = select_cases(&loaded.suite, &args.only)?;
    let formal = args.tier != Tier::Smoke;
    let source = SourceIdentity::capture(root)?;
    if !source.clean && !args.allow_dirty {
        return Err(BenchError::E2e(
            "source tree is dirty; commit it or use --allow-dirty for smoke".to_string(),
        ));
    }
    let tier_policy = loaded
        .suite
        .tiers
        .get(&args.tier)
        .ok_or_else(|| BenchError::E2e(format!("suite omits tier {}", args.tier)))?;
    let binaries = Binaries::build(root, &loaded)?;
    check_cancelled(&cancellation)?;
    // The host profile and competitor scan start after the builds, so neither sees the build.
    let competitors = CompetitorScan::new(binaries.measured_paths());
    let host_before = HostProfile::capture(args.affinity_cpu).map_err(BenchError::E2e)?;
    if formal {
        host_before
            .validate_formal(
                loaded.suite.policy.maximum_temperature_millidegrees,
                &competitors,
            )
            .map_err(BenchError::E2e)?;
    }
    let output = root.create_new_benchmark_output_dir(&args.out)?;

    let mut correctness = Vec::new();
    let mut raw_cases = Vec::new();
    for selected_case in selected {
        check_cancelled(&cancellation)?;
        if formal {
            competitors.check().map_err(BenchError::E2e)?;
        }
        println!("[stab-bench] E2E preflight {}", selected_case.id);
        let prepared = PreparedCase::new(
            root,
            selected_case.id,
            selected_case.family,
            selected_case.case,
            &binaries,
            &loaded.suite.policy,
            args.affinity_cpu,
        )?;
        let (case_correctness, expected) = prepared.preflight(&binaries)?;
        correctness.push(case_correctness);
        prepared.warm_up(&binaries, tier_policy.warmups, &cancellation)?;
        raw_cases.push(prepared.measure(
            &binaries,
            tier_policy.samples,
            &expected,
            &cancellation,
        )?);
    }

    check_cancelled(&cancellation)?;
    let host_after = HostProfile::capture(args.affinity_cpu).map_err(BenchError::E2e)?;
    validate_host_after(
        &host_before,
        &host_after,
        loaded.suite.policy.maximum_temperature_millidegrees,
        formal,
        &competitors,
    )
    .map_err(BenchError::E2e)?;
    binaries.verify_unchanged()?;
    if SourceIdentity::capture(root)? != source {
        return Err(BenchError::E2e(
            "source commit or cleanliness changed during the run".to_string(),
        ));
    }
    let metadata = RunMetadata {
        schema_version: RUN_SCHEMA_VERSION,
        suite_sha256: loaded.digest.clone(),
        source_commit: source.commit,
        source_clean: source.clean,
        stim_commit: loaded.suite.stim.commit.clone(),
        stim_binary_sha256: binaries.stim_sha256.clone(),
        stab_binary_sha256: binaries.stab_sha256.clone(),
        bench_binary_sha256: binaries.bench_sha256.clone(),
        rustc: binaries.rustc.clone(),
        target: binaries.target.clone(),
        tier: args.tier,
        formal,
        host_before,
        host_after,
        selected_cases: raw_cases.iter().map(|case| case.case_id.clone()).collect(),
        warmups: Some(tier_policy.warmups),
    };
    let correctness = CorrectnessReport {
        schema_version: CORRECTNESS_SCHEMA_VERSION,
        cases: correctness,
    };
    let samples = RawSamples {
        schema_version: SAMPLES_SCHEMA_VERSION,
        cases: raw_cases,
    };
    let report = DerivedReport::derive(&loaded.suite, &metadata, &correctness, &samples)
        .map_err(BenchError::E2e)?;
    check_cancelled(&cancellation)?;
    publish_bundle(
        &output,
        &loaded.bytes,
        &metadata,
        &correctness,
        &samples,
        &report,
    )?;
    println!(
        "[stab-bench] E2E {} cases={} parity={:?} self={:?} memory={:?} bundle={}",
        args.tier,
        report.cases.len(),
        report.parity,
        report.self_regression,
        report.memory,
        output.display()
    );
    if report.passed {
        Ok(())
    } else {
        Err(BenchError::E2e(format!(
            "E2E {} report failed; retained bundle {}",
            args.tier,
            output.display()
        )))
    }
}

struct SelectedCase<'a> {
    id: CaseId,
    family: &'a Family,
    case: &'a Case,
}

fn select_cases<'a>(
    suite: &'a Suite,
    filters: &[String],
) -> Result<Vec<SelectedCase<'a>>, BenchError> {
    let mut selected = Vec::new();
    let mut matched = BTreeSet::new();
    for (family, case, id) in suite.cases() {
        let include = filters.is_empty()
            || filters.iter().any(|filter| {
                let matches = filter == &family.id || filter == id.as_str();
                if matches {
                    matched.insert(filter.as_str());
                }
                matches
            });
        if include {
            selected.push(SelectedCase { id, family, case });
        }
    }
    if let Some(unmatched) = filters
        .iter()
        .find(|filter| !matched.contains(filter.as_str()))
    {
        return Err(BenchError::E2e(format!(
            "E2E filter {unmatched} matched no family or case"
        )));
    }
    if selected.is_empty() {
        return Err(BenchError::E2e("E2E selection is empty".to_string()));
    }
    Ok(selected)
}

/// The measured source commit and whether the worktree is clean.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct SourceIdentity {
    pub(super) commit: String,
    pub(super) clean: bool,
}

impl SourceIdentity {
    pub(super) fn capture(root: &RepoRoot) -> Result<Self, BenchError> {
        let commit = command_output(
            Path::new("git"),
            &["rev-parse", "HEAD"],
            b"",
            &root.path,
            1 << 20,
            None,
            60,
        )?;
        let status = command_output(
            Path::new("git"),
            &["status", "--porcelain", "--untracked-files=all"],
            b"",
            &root.path,
            8 << 20,
            None,
            60,
        )?;
        Ok(Self {
            commit: String::from_utf8(commit)
                .map_err(|source| BenchError::E2e(source.to_string()))?
                .trim()
                .to_string(),
            clean: status.is_empty(),
        })
    }
}

/// The measured programs. Stim and Stab run from private copies made right after their builds,
/// so a rebuild or edit of the build outputs cannot change what is measured; the benchmark binary
/// is the running launcher and worker. All three are hashed before preflight and again after
/// the last sample.
pub(super) struct Binaries {
    stim: PathBuf,
    stab: PathBuf,
    bench: PathBuf,
    built_stim: PathBuf,
    built_stab: PathBuf,
    stim_sha256: String,
    stab_sha256: String,
    bench_sha256: String,
    rustc: String,
    target: String,
    _directory: TempDir,
}

impl Binaries {
    fn build(root: &RepoRoot, loaded: &LoadedSuite) -> Result<Self, BenchError> {
        let stim_source = root.default_stim_source();
        validate_stim_source(
            &stim_source,
            &loaded.suite.stim.version,
            &loaded.suite.stim.commit,
        )?;
        ensure_stim_cli(root, &stim_source)?;
        let built_stim = root.stim_binary();
        let built_stab = build_release_stab(root)?;
        let bench = std::env::current_exe().map_err(|source| {
            BenchError::E2e(format!("cannot resolve benchmark binary: {source}"))
        })?;
        if !bench.is_file() {
            return Err(BenchError::E2e(format!(
                "benchmark binary is missing at {}",
                bench.display()
            )));
        }
        let (rustc, target) = rustc_identity(root)?;
        let scratch = root.benchmark_root().join("e2e-scratch");
        fs::create_dir_all(&scratch).map_err(|source| {
            BenchError::E2e(format!("failed to create {}: {source}", scratch.display()))
        })?;
        let directory = tempfile::Builder::new()
            .prefix("binaries-")
            .tempdir_in(&scratch)
            .map_err(|source| {
                BenchError::E2e(format!("cannot create measured binary directory: {source}"))
            })?;
        let stim = private_copy(&built_stim, &directory.path().join("stim"))?;
        let stab = private_copy(&built_stab, &directory.path().join("stab"))?;
        Ok(Self {
            stim_sha256: file_sha256(&stim)?,
            stab_sha256: file_sha256(&stab)?,
            bench_sha256: file_sha256(&bench)?,
            stim,
            stab,
            bench,
            built_stim,
            built_stab,
            rustc,
            target,
            _directory: directory,
        })
    }

    /// Paths whose running processes would compete with the measurement.
    fn measured_paths(&self) -> Vec<PathBuf> {
        vec![
            self.stim.clone(),
            self.stab.clone(),
            self.built_stim.clone(),
            self.built_stab.clone(),
        ]
    }

    /// Fails if a measured program changed after its preflight hash.
    fn verify_unchanged(&self) -> Result<(), BenchError> {
        for (name, path, expected) in [
            ("Stim", &self.stim, &self.stim_sha256),
            ("Stab", &self.stab, &self.stab_sha256),
            ("benchmark", &self.bench, &self.bench_sha256),
        ] {
            if file_sha256(path)? != *expected {
                return Err(BenchError::E2e(format!(
                    "{name} binary {} changed during the run",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

/// Builds the release Stab CLI with the committed lockfile and returns the executable Cargo
/// reports for it.
fn build_release_stab(root: &RepoRoot) -> Result<PathBuf, BenchError> {
    let messages = command_output(
        Path::new("cargo"),
        &[
            "build",
            "--release",
            "--locked",
            "--message-format=json-render-diagnostics",
            "-p",
            "stab-cli",
            "--bin",
            "stab",
        ],
        b"",
        &root.path,
        CARGO_MESSAGE_LIMIT_BYTES,
        None,
        CARGO_BUILD_TIMEOUT_SECONDS,
    )?;
    stab_executable_from_cargo_messages(&messages)
}

fn stab_executable_from_cargo_messages(messages: &[u8]) -> Result<PathBuf, BenchError> {
    let mut executable = None;
    for message in cargo_metadata::Message::parse_stream(messages) {
        if let cargo_metadata::Message::CompilerArtifact(artifact) = message.map_err(|source| {
            BenchError::E2e(format!("cargo wrote an unreadable message: {source}"))
        })? && artifact.target.name == "stab"
            && artifact.target.is_bin()
            && let Some(path) = artifact.executable
        {
            executable = Some(path.into_std_path_buf());
        }
    }
    executable.ok_or_else(|| {
        BenchError::E2e("cargo build reported no executable for the stab binary".to_string())
    })
}

/// Copies `source` to the new owner-only file `destination` and leaves it read and execute only.
fn private_copy(source: &Path, destination: &Path) -> Result<PathBuf, BenchError> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    let bytes = fs::read(source)
        .map_err(|error| BenchError::E2e(format!("cannot read {}: {error}", source.display())))?;
    let failed = |error: std::io::Error| {
        BenchError::E2e(format!("cannot write {}: {error}", destination.display()))
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(destination)
        .map_err(failed)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(failed)?;
    drop(file);
    fs::set_permissions(destination, fs::Permissions::from_mode(0o500)).map_err(failed)?;
    Ok(destination.to_path_buf())
}

fn rustc_identity(root: &RepoRoot) -> Result<(String, String), BenchError> {
    let verbose = command_output(
        Path::new("rustc"),
        &["-vV"],
        b"",
        &root.path,
        1 << 20,
        None,
        60,
    )?;
    let verbose =
        String::from_utf8(verbose).map_err(|source| BenchError::E2e(source.to_string()))?;
    let target = verbose
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| BenchError::E2e("rustc -vV omitted host target".to_string()))?
        .to_string();
    let rustc = verbose
        .lines()
        .next()
        .ok_or_else(|| BenchError::E2e("rustc -vV is empty".to_string()))?
        .to_string();
    Ok((rustc, target))
}

#[cfg(test)]
mod tests;
