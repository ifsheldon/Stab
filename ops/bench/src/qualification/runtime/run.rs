use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::artifact::QualificationOutput;
use super::calibration::{CalibrationDecision, CalibrationPolicy, CalibrationProbe, calibrate};
use super::correctness::{CorrectnessPreflightEvidence, CorrectnessRequirement};
use super::host::{HostEvidence, HostGuard};
use super::invocation::{InvocationRecord, PreparedWorkers, WorkerIdentityEvidence, protocol_ids};
use super::process::{ProcessLimits, ProcessRequest, run_bounded_process};
use super::protocol::{EvidenceMode, Implementation, SemanticDigest, WorkerMeasurement};
use super::statistics::{PairOrder, PairedSample, StatisticsSummary, pair_measurements, summarize};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

pub(super) const REPORT_SCHEMA_VERSION: u32 = 6;
const DEFAULT_OUTPUT: &str = "target/benchmarks/qualification/latest";
const CALIBRATION_ACCEPTANCE_MINIMUM: Duration = Duration::from_millis(250);
const CALIBRATION_TARGET_MINIMUM: Duration = Duration::from_millis(350);
const CALIBRATION_MAXIMUM: Duration = Duration::from_secs(2);
const INVOCATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_ITERATIONS: u64 = 1_000_000_000;
const WARMUP_BATCHES: usize = 3;
const PRIMARY_THRESHOLD: f64 = 1.25;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QualificationTier {
    Pr,
    Full,
    Soak,
}

impl QualificationTier {
    fn pair_count(self) -> usize {
        match self {
            Self::Pr => 3,
            Self::Full => 9,
            Self::Soak => 15,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RunArgs {
    /// Qualification tier controlling the number of retained pairs.
    #[arg(long, value_enum)]
    tier: QualificationTier,

    /// Atomic report directory below target/benchmarks/qualification.
    #[arg(long, default_value = DEFAULT_OUTPUT)]
    out: PathBuf,

    /// Preserve a diagnostic report when source-owned host limits are not met.
    #[arg(long)]
    allow_unverified_host: bool,

    /// Semantic work items per protocol-smoke iteration.
    #[arg(long, default_value = "4096")]
    work_items: NonZeroU64,

    /// CQ1 report directory used by future promotable product groups.
    #[arg(long)]
    correctness_out: Option<PathBuf>,

    /// Exact CQ1 case required by a future promotable product group.
    #[arg(long, requires = "correctness_out")]
    correctness_case: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ClaimClass {
    DiagnosticInfrastructure,
    PromotablePerformance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationReport {
    pub(super) schema_version: u32,
    pub(super) group_id: String,
    pub(super) claim_class: ClaimClass,
    pub(super) tier: QualificationTier,
    pub(super) command: RunCommandEvidence,
    pub(super) generated_unix_epoch_seconds: u64,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) performance_inventory_sha256: String,
    pub(super) correctness_inventory_sha256: String,
    pub(super) repository: RepositoryEvidence,
    pub(super) host: HostEvidence,
    pub(super) toolchain: super::toolchain::ToolchainEvidence,
    pub(super) workers: WorkerIdentityEvidence,
    pub(super) adapter_receipt: super::adapter::AdapterBuildReceipt,
    pub(super) correctness_preflight: CorrectnessPreflightEvidence,
    pub(super) semantic_preflight: PairExecution,
    pub(super) calibration: CalibrationEvidence,
    pub(super) warmups: Vec<PairExecution>,
    pub(super) samples: Vec<PairExecution>,
    pub(super) paired_samples: Vec<PairedSample>,
    pub(super) statistics: Vec<StatisticsSummary>,
    pub(super) worst_confidence_interval_upper: f64,
    pub(super) memory: MemoryEvidence,
    pub(super) promotable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunCommandEvidence {
    pub(super) output: String,
    pub(super) work_items: u64,
    pub(super) allow_unverified_host: bool,
    pub(super) warmup_batches: usize,
    pub(super) paired_samples: usize,
    pub(super) invocation_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RepositoryEvidence {
    pub(super) commit_before: String,
    pub(super) commit_after: String,
    pub(super) local_modifications_before: bool,
    pub(super) local_modifications_after: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairExecution {
    pub(super) pair_index: usize,
    pub(super) order: PairOrder,
    pub(super) stim: InvocationRecord,
    pub(super) stab: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CalibrationProbeEvidence {
    pub(super) iterations: u64,
    pub(super) invocation: InvocationRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImplementationCalibration {
    pub(super) implementation: Implementation,
    pub(super) selected_iterations: u64,
    pub(super) selected_measured_seconds: f64,
    pub(super) probes: Vec<CalibrationProbeEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CalibrationEvidence {
    pub(super) acceptance_minimum_seconds: f64,
    pub(super) target_minimum_seconds: f64,
    pub(super) maximum_seconds: f64,
    pub(super) stim: ImplementationCalibration,
    pub(super) stab: ImplementationCalibration,
    pub(super) common_iterations: u64,
    pub(super) common_validation: PairExecution,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemoryEvidence {
    pub(super) evidence_mode: EvidenceMode,
    pub(super) iterations: u64,
    pub(super) work_count: u64,
    pub(super) stim_setup_rss_bytes: u64,
    pub(super) stim_peak_rss_bytes: u64,
    pub(super) stim_parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) stab_setup_rss_bytes: u64,
    pub(super) stab_peak_rss_bytes: u64,
    pub(super) stab_parent_observed_peak_rss_bytes: Option<u64>,
    pub(super) execution: PairExecution,
}

pub(super) fn run(
    root: &RepoRoot,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    args: RunArgs,
) -> Result<PathBuf, RunError> {
    let repository_before = git_state(root)?;
    let claim_class = ClaimClass::DiagnosticInfrastructure;
    let correctness_preflight = correctness_preflight(
        root,
        claim_class,
        &args,
        correctness_inventory_sha256,
        &repository_before.commit,
    )?;
    let toolchain = super::toolchain::collect(root)?;
    let mut workers = PreparedWorkers::prepare(root)?;
    let host_guard = HostGuard::prepare(root, args.allow_unverified_host)?;
    workers.pin_to_cpu(host_guard.selected_cpu());

    let policy = calibration_policy()?;
    let (stim_decision, stim_probes) =
        calibrate_worker(&workers, Implementation::Stim, args.work_items, policy)?;
    let (stab_decision, stab_probes) =
        calibrate_worker(&workers, Implementation::Stab, args.work_items, policy)?;
    let common_iterations = stim_decision.iterations.max(stab_decision.iterations);
    let semantic_preflight = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Timing,
        None,
    )?;
    pair_execution(&semantic_preflight)?;
    let expected_output_digest = only_row(&semantic_preflight.stim.rows)?
        .output_digest
        .clone();
    let common_validation = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Timing,
        Some(&expected_output_digest),
    )?;
    pair_execution(&common_validation)?;
    validate_common_calibration(&common_validation)?;
    let calibration = CalibrationEvidence {
        acceptance_minimum_seconds: CALIBRATION_ACCEPTANCE_MINIMUM.as_secs_f64(),
        target_minimum_seconds: CALIBRATION_TARGET_MINIMUM.as_secs_f64(),
        maximum_seconds: CALIBRATION_MAXIMUM.as_secs_f64(),
        stim: calibration_evidence(Implementation::Stim, stim_decision, stim_probes),
        stab: calibration_evidence(Implementation::Stab, stab_decision, stab_probes),
        common_iterations: common_iterations.get(),
        common_validation,
    };

    let mut warmups = Vec::with_capacity(WARMUP_BATCHES);
    for pair_index in 0..WARMUP_BATCHES {
        let execution = execute_pair(
            &workers,
            pair_index,
            common_iterations,
            args.work_items,
            EvidenceMode::Timing,
            Some(&expected_output_digest),
        )?;
        pair_execution(&execution)?;
        warmups.push(execution);
    }

    let mut samples = Vec::with_capacity(args.tier.pair_count());
    let mut paired_samples = Vec::with_capacity(args.tier.pair_count());
    for pair_index in 0..args.tier.pair_count() {
        let execution = execute_pair(
            &workers,
            pair_index,
            common_iterations,
            args.work_items,
            EvidenceMode::Timing,
            Some(&expected_output_digest),
        )?;
        paired_samples.extend(pair_execution(&execution)?);
        samples.push(execution);
    }
    let (_, measurement_id) = protocol_ids()?;
    let statistics = vec![summarize(
        measurement_id,
        &paired_samples,
        PRIMARY_THRESHOLD,
    )?];
    let worst_confidence_interval_upper = statistics
        .iter()
        .map(|summary| summary.confidence_interval_upper)
        .reduce(f64::max)
        .ok_or(RunError::MissingStatistics)?;

    let memory_execution = execute_pair(
        &workers,
        0,
        common_iterations,
        args.work_items,
        EvidenceMode::Memory,
        Some(&expected_output_digest),
    )?;
    let memory = memory_evidence(memory_execution, common_iterations)?;
    workers.verify()?;
    let host = host_guard.finish()?;
    let repository_after = git_state(root)?;
    if repository_before.commit != repository_after.commit {
        return Err(RunError::RepositoryChanged {
            before: repository_before.commit,
            after: repository_after.commit,
        });
    }
    let repository = RepositoryEvidence {
        commit_before: repository_before.commit,
        commit_after: repository_after.commit,
        local_modifications_before: repository_before.local_modifications,
        local_modifications_after: repository_after.local_modifications,
    };
    let report = QualificationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        group_id: "pq1-adapter-protocol-smoke".to_string(),
        claim_class,
        tier: args.tier,
        command: RunCommandEvidence {
            output: args.out.to_string_lossy().into_owned(),
            work_items: args.work_items.get(),
            allow_unverified_host: args.allow_unverified_host,
            warmup_batches: WARMUP_BATCHES,
            paired_samples: args.tier.pair_count(),
            invocation_timeout_seconds: INVOCATION_TIMEOUT.as_secs(),
        },
        generated_unix_epoch_seconds: unix_epoch_seconds()?,
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        performance_inventory_sha256: performance_inventory_sha256.to_string(),
        correctness_inventory_sha256: correctness_inventory_sha256.to_string(),
        repository,
        host,
        toolchain,
        workers: workers.identity_evidence(),
        adapter_receipt: workers.adapter_receipt().clone(),
        correctness_preflight,
        semantic_preflight,
        calibration,
        warmups,
        samples,
        paired_samples,
        statistics,
        worst_confidence_interval_upper,
        memory,
        promotable: false,
    };
    super::report::validate_report(&report)?;
    let report_json = render_json(&report)?;
    let preflight = super::report::preflight_artifact(&report, &report_json)?;
    let preflight_json = render_json(&preflight)?;
    let markdown = super::report::render_markdown(&report, &sha256_hex(&report_json));
    let output = QualificationOutput::begin(root, &args.out)?;
    output.write("report.json", &report_json)?;
    output.write("preflight.json", &preflight_json)?;
    output.write("report.md", markdown.as_bytes())?;
    let relative = output.relative().to_path_buf();
    output.commit()?;
    Ok(relative)
}

fn correctness_preflight(
    root: &RepoRoot,
    claim_class: ClaimClass,
    args: &RunArgs,
    correctness_inventory_sha256: &str,
    stab_commit: &str,
) -> Result<CorrectnessPreflightEvidence, RunError> {
    let requirement = match claim_class {
        ClaimClass::DiagnosticInfrastructure => {
            if args.correctness_out.is_some() || !args.correctness_case.is_empty() {
                return Err(RunError::UnexpectedCorrectnessInput);
            }
            CorrectnessRequirement::NotApplicable {
                reason: "PQ1 protocol-smoke validates benchmark infrastructure and cannot support a product performance claim",
            }
        }
        ClaimClass::PromotablePerformance => CorrectnessRequirement::Required {
            output: args
                .correctness_out
                .as_deref()
                .ok_or(RunError::MissingCorrectnessInput)?,
            case_ids: &args.correctness_case,
            expected_manifest_sha256: correctness_inventory_sha256,
            expected_stab_commit: stab_commit,
        },
    };
    Ok(super::correctness::validate(root, requirement)?)
}

fn calibrate_worker(
    workers: &PreparedWorkers,
    implementation: Implementation,
    work_items: NonZeroU64,
    policy: CalibrationPolicy,
) -> Result<(CalibrationDecision, Vec<CalibrationProbeEvidence>), RunError> {
    let mut evidence = Vec::new();
    let decision = calibrate(policy, |iterations| {
        let invocation = workers
            .invoke(
                implementation,
                EvidenceMode::Timing,
                iterations,
                work_items,
                None,
                INVOCATION_TIMEOUT,
            )
            .map_err(|error| error.to_string())?;
        let measured = invocation
            .measured_duration()
            .map_err(|error| error.to_string())?;
        let wall = invocation
            .wall_duration()
            .map_err(|error| error.to_string())?;
        evidence.push(CalibrationProbeEvidence {
            iterations: iterations.get(),
            invocation,
        });
        Ok(CalibrationProbe { measured, wall })
    })?;
    Ok((decision, evidence))
}

fn calibration_policy() -> Result<CalibrationPolicy, RunError> {
    Ok(CalibrationPolicy {
        minimum: CALIBRATION_TARGET_MINIMUM,
        maximum: CALIBRATION_MAXIMUM,
        timeout: INVOCATION_TIMEOUT,
        maximum_iterations: NonZeroU64::new(MAXIMUM_ITERATIONS)
            .ok_or(RunError::InvalidIterationCap)?,
    })
}

fn calibration_evidence(
    implementation: Implementation,
    decision: CalibrationDecision,
    probes: Vec<CalibrationProbeEvidence>,
) -> ImplementationCalibration {
    ImplementationCalibration {
        implementation,
        selected_iterations: decision.iterations.get(),
        selected_measured_seconds: decision.measured.as_secs_f64(),
        probes,
    }
}

fn execute_pair(
    workers: &PreparedWorkers,
    pair_index: usize,
    iterations: NonZeroU64,
    work_items: NonZeroU64,
    evidence_mode: EvidenceMode,
    expected_output_digest: Option<&SemanticDigest>,
) -> Result<PairExecution, RunError> {
    let order = PairOrder::for_pair(pair_index);
    let invoke = |implementation| {
        workers.invoke(
            implementation,
            evidence_mode,
            iterations,
            work_items,
            expected_output_digest,
            INVOCATION_TIMEOUT,
        )
    };
    let (stim, stab) = match order {
        PairOrder::StimThenStab => {
            let stim = invoke(Implementation::Stim)?;
            let stab = invoke(Implementation::Stab)?;
            (stim, stab)
        }
        PairOrder::StabThenStim => {
            let stab = invoke(Implementation::Stab)?;
            let stim = invoke(Implementation::Stim)?;
            (stim, stab)
        }
    };
    Ok(PairExecution {
        pair_index,
        order,
        stim,
        stab,
    })
}

fn pair_execution(execution: &PairExecution) -> Result<Vec<PairedSample>, RunError> {
    Ok(pair_measurements(
        execution.pair_index,
        execution.order,
        &execution.stim.rows,
        &execution.stab.rows,
    )?)
}

fn validate_common_calibration(execution: &PairExecution) -> Result<(), RunError> {
    for invocation in [&execution.stim, &execution.stab] {
        let measured = invocation.measured_duration()?;
        if !common_calibration_duration_is_accepted(measured) {
            return Err(RunError::CommonCalibrationOutOfBounds {
                implementation: invocation.implementation,
                measured,
            });
        }
    }
    Ok(())
}

fn common_calibration_duration_is_accepted(measured: Duration) -> bool {
    measured >= CALIBRATION_ACCEPTANCE_MINIMUM && measured <= CALIBRATION_MAXIMUM
}

fn memory_evidence(
    execution: PairExecution,
    iterations: NonZeroU64,
) -> Result<MemoryEvidence, RunError> {
    let stim = only_row(&execution.stim.rows)?;
    let stab = only_row(&execution.stab.rows)?;
    if stim.evidence_mode != EvidenceMode::Memory
        || stab.evidence_mode != EvidenceMode::Memory
        || stim.work_count != stab.work_count
        || stim.output_digest != stab.output_digest
        || stim.iteration_count != stab.iteration_count
    {
        return Err(RunError::MemorySemanticMismatch);
    }
    let stim_setup = stim.setup_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stim_peak = stim.peak_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stab_setup = stab.setup_rss_bytes.ok_or(RunError::MissingMemory)?;
    let stab_peak = stab.peak_rss_bytes.ok_or(RunError::MissingMemory)?;
    Ok(MemoryEvidence {
        evidence_mode: EvidenceMode::Memory,
        iterations: iterations.get(),
        work_count: stim.work_count,
        stim_setup_rss_bytes: stim_setup,
        stim_peak_rss_bytes: stim_peak,
        stim_parent_observed_peak_rss_bytes: execution.stim.parent_observed_peak_rss_bytes,
        stab_setup_rss_bytes: stab_setup,
        stab_peak_rss_bytes: stab_peak,
        stab_parent_observed_peak_rss_bytes: execution.stab.parent_observed_peak_rss_bytes,
        execution,
    })
}

fn only_row(rows: &[WorkerMeasurement]) -> Result<&WorkerMeasurement, RunError> {
    let [row] = rows else {
        return Err(RunError::ExpectedOneMeasurement(rows.len()));
    };
    Ok(row)
}

#[derive(Clone, Debug)]
struct GitState {
    commit: String,
    local_modifications: bool,
}

fn git_state(root: &RepoRoot) -> Result<GitState, RunError> {
    let git = PathBuf::from("/usr/bin/git");
    if !git.is_file() {
        return Err(RunError::MissingGit(git));
    }
    let scratch = tempfile::Builder::new()
        .prefix("stab-qualification-git-")
        .tempdir()
        .map_err(RunError::GitScratch)?;
    let private_index = scratch.path().join("index");
    let commit = git_text(
        root,
        &git,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        None,
        scratch.path(),
    )?;
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RunError::InvalidGitCommit(commit));
    }
    git_command(
        root,
        &git,
        &["read-tree", "HEAD"],
        Some(&private_index),
        scratch.path(),
    )?;
    let refresh = git_command_allow_status_one(
        root,
        &git,
        &["update-index", "--really-refresh", "--ignore-submodules"],
        Some(&private_index),
        scratch.path(),
    )?;
    let tracked = git_command(
        root,
        &git,
        &[
            "diff-index",
            "--name-only",
            "-z",
            "--no-renames",
            "--ignore-submodules=none",
            "HEAD",
            "--",
        ],
        Some(&private_index),
        scratch.path(),
    )?;
    let staged = git_command(
        root,
        &git,
        &[
            "diff-index",
            "--cached",
            "--name-only",
            "-z",
            "--no-renames",
            "--ignore-submodules=none",
            "HEAD",
            "--",
        ],
        None,
        scratch.path(),
    )?;
    let untracked = git_command(
        root,
        &git,
        &[
            "ls-files",
            "--others",
            "--exclude-per-directory=.gitignore",
            "-z",
            "--",
        ],
        Some(&private_index),
        scratch.path(),
    )?;
    Ok(GitState {
        commit: commit.to_ascii_lowercase(),
        local_modifications: refresh.status == Some(1)
            || !tracked.stdout.is_empty()
            || !staged.stdout.is_empty()
            || !untracked.stdout.is_empty(),
    })
}

fn git_text(
    root: &RepoRoot,
    git: &Path,
    args: &[&str],
    private_index: Option<&Path>,
    scratch: &Path,
) -> Result<String, RunError> {
    let output = git_command(root, git, args, private_index, scratch)?;
    let text = std::str::from_utf8(&output.stdout).map_err(RunError::GitUtf8)?;
    Ok(text.trim_end_matches(['\r', '\n']).to_string())
}

fn git_command(
    root: &RepoRoot,
    git: &Path,
    args: &[&str],
    private_index: Option<&Path>,
    scratch: &Path,
) -> Result<super::process::ProcessResult, RunError> {
    git_command_with_status(root, git, args, private_index, scratch, false)
}

fn git_command_allow_status_one(
    root: &RepoRoot,
    git: &Path,
    args: &[&str],
    private_index: Option<&Path>,
    scratch: &Path,
) -> Result<super::process::ProcessResult, RunError> {
    git_command_with_status(root, git, args, private_index, scratch, true)
}

fn git_command_with_status(
    root: &RepoRoot,
    git: &Path,
    args: &[&str],
    private_index: Option<&Path>,
    scratch: &Path,
    allow_status_one: bool,
) -> Result<super::process::ProcessResult, RunError> {
    let mut command_arguments = vec![
        OsString::from("--no-optional-locks"),
        OsString::from("-c"),
        OsString::from("core.fsmonitor=false"),
        OsString::from("-c"),
        OsString::from("core.untrackedCache=false"),
        OsString::from("-c"),
        OsString::from("core.excludesFile=/dev/null"),
        OsString::from("-c"),
        OsString::from("diff.external="),
        OsString::from("--work-tree"),
        root.path.as_os_str().to_owned(),
    ];
    command_arguments.extend(args.iter().map(OsString::from));
    let mut environment = vec![
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        (OsString::from("HOME"), scratch.as_os_str().to_owned()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            scratch.as_os_str().to_owned(),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
        (OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0")),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
        (OsString::from("GIT_LITERAL_PATHSPECS"), OsString::from("1")),
    ];
    if let Some(index) = private_index {
        environment.push((
            OsString::from("GIT_INDEX_FILE"),
            index.as_os_str().to_owned(),
        ));
    }
    let output = run_bounded_process(&ProcessRequest {
        program: git.to_path_buf(),
        args: command_arguments,
        stdin: Vec::new(),
        working_directory: root.path.clone(),
        environment,
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 0,
            stdout_bytes: 4 << 20,
            stderr_bytes: 64 << 10,
            regular_file_bytes: None,
            timeout: Duration::from_secs(30),
        },
    })?;
    let accepted_status = output.status == Some(0) || allow_status_one && output.status == Some(1);
    if !accepted_status || output.status == Some(0) && !output.stderr.is_empty() {
        return Err(RunError::GitFailed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output)
}

fn render_json(value: &impl Serialize) -> Result<Vec<u8>, RunError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len().saturating_mul(2));
    for byte in digest {
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

fn unix_epoch_seconds() -> Result<u64, RunError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(RunError::Clock)
}

#[derive(Debug, Error)]
pub(super) enum RunError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),
    #[error(transparent)]
    Host(#[from] super::host::HostError),
    #[error(transparent)]
    Invocation(#[from] super::invocation::InvocationError),
    #[error(transparent)]
    Calibration(#[from] super::calibration::CalibrationError),
    #[error(transparent)]
    Correctness(#[from] super::correctness::CorrectnessError),
    #[error(transparent)]
    Toolchain(#[from] super::toolchain::ToolchainError),
    #[error(transparent)]
    Statistics(#[from] super::statistics::StatisticsError),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
    #[error(transparent)]
    Report(#[from] super::report::ReportError),
    #[error("qualification maximum iteration cap must be positive")]
    InvalidIterationCap,
    #[error("diagnostic PQ1 protocol-smoke does not accept product correctness evidence")]
    UnexpectedCorrectnessInput,
    #[error("promotable performance evidence requires a CQ1 report directory and exact cases")]
    MissingCorrectnessInput,
    #[error(
        "common calibrated batch measured {measured:?} for {implementation}, outside the source-owned bounds"
    )]
    CommonCalibrationOutOfBounds {
        implementation: Implementation,
        measured: Duration,
    },
    #[error("qualification produced no measurement statistics")]
    MissingStatistics,
    #[error("memory evidence differs in mode, semantic work, digest, or iteration count")]
    MemorySemanticMismatch,
    #[error("memory evidence omits setup or peak RSS")]
    MissingMemory,
    #[error("qualification expected one measurement but received {0}")]
    ExpectedOneMeasurement(usize),
    #[error("required Git executable is missing: {0}")]
    MissingGit(PathBuf),
    #[error("Git command failed with status {status:?}: {stderr}")]
    GitFailed { status: Option<i32>, stderr: String },
    #[error("Git output is not UTF-8: {0}")]
    GitUtf8(std::str::Utf8Error),
    #[error("failed to create an isolated Git audit directory: {0}")]
    GitScratch(std::io::Error),
    #[error("Git returned invalid commit {0:?}")]
    InvalidGitCommit(String),
    #[error("repository commit changed during qualification: {before} -> {after}")]
    RepositoryChanged { before: String, after: String },
    #[error("system clock is before the Unix epoch: {0}")]
    Clock(std::time::SystemTimeError),
    #[error("failed to serialize qualification evidence: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_git(repository: &Path, arguments: &[&str]) {
        let output = std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .output()
            .expect("run test Git command");
        assert!(
            output.status.success(),
            "Git {:?} failed: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn tiers_have_source_owned_pair_counts() {
        assert_eq!(QualificationTier::Pr.pair_count(), 3);
        assert_eq!(QualificationTier::Full.pair_count(), 9);
        assert_eq!(QualificationTier::Soak.pair_count(), 15);
    }

    #[test]
    fn diagnostic_claim_cannot_be_promotable() {
        assert_ne!(
            ClaimClass::DiagnosticInfrastructure,
            ClaimClass::PromotablePerformance
        );
    }

    #[test]
    fn calibration_guard_band_preserves_the_acceptance_floor() {
        let policy = calibration_policy().expect("calibration policy");
        assert_eq!(policy.minimum, Duration::from_millis(350));
        assert_eq!(CALIBRATION_ACCEPTANCE_MINIMUM, Duration::from_millis(250));
        assert!(policy.minimum > CALIBRATION_ACCEPTANCE_MINIMUM);
        assert!(!common_calibration_duration_is_accepted(
            Duration::from_micros(249_999)
        ));
        assert!(common_calibration_duration_is_accepted(
            Duration::from_millis(250)
        ));
        assert!(common_calibration_duration_is_accepted(
            Duration::from_secs(2)
        ));
        assert!(!common_calibration_duration_is_accepted(
            Duration::from_micros(2_000_001)
        ));
    }

    #[test]
    fn clean_revision_audit_defeats_index_flags_and_local_excludes() {
        let repository = tempfile::tempdir().expect("temporary repository");
        test_git(repository.path(), &["init", "--quiet"]);
        test_git(repository.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            repository.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        std::fs::write(repository.path().join(".gitignore"), "ignored/\n")
            .expect("write ignore policy");
        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("write tracked file");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "initial"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        assert!(!git_state(&root).expect("clean state").local_modifications);

        std::fs::create_dir(repository.path().join("ignored")).expect("create ignored directory");
        std::fs::write(repository.path().join("ignored/generated"), "generated\n")
            .expect("write ignored output");
        assert!(
            !git_state(&root)
                .expect("ignored output state")
                .local_modifications
        );

        test_git(
            repository.path(),
            &["update-index", "--skip-worktree", "tracked.txt"],
        );
        std::fs::write(repository.path().join("tracked.txt"), "hidden change\n")
            .expect("modify skipped file");
        assert!(
            git_state(&root)
                .expect("skip-worktree state")
                .local_modifications
        );

        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("restore tracked file");
        test_git(
            repository.path(),
            &["update-index", "--no-skip-worktree", "tracked.txt"],
        );
        std::fs::write(repository.path().join("tracked.txt"), "staged change\n")
            .expect("write staged change");
        test_git(repository.path(), &["add", "tracked.txt"]);
        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("restore worktree only");
        assert!(
            git_state(&root)
                .expect("staged-only state")
                .local_modifications
        );

        test_git(
            repository.path(),
            &["reset", "--quiet", "HEAD", "--", "tracked.txt"],
        );
        std::fs::write(repository.path().join(".git/info/exclude"), "hidden.txt\n")
            .expect("write local exclude");
        std::fs::write(repository.path().join("hidden.txt"), "untracked\n")
            .expect("write locally excluded file");
        assert!(
            git_state(&root)
                .expect("locally excluded state")
                .local_modifications
        );
    }
}
