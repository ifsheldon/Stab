use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Args;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

use super::bundle::{file_sha256, publish_bundle};
use super::data::{OutputWitness, materialize, validate_output, validate_semantic_work};
use super::host::{HostProfile, validate_host_after};
use super::model::{
    Case, Comparator, Family, FileRole, OutputContract, PipelineInput, Policy, Tier, Workload,
    case_digest,
};
use super::report::{
    CORRECTNESS_SCHEMA_VERSION, CaseCorrectness, CaseSamples, CorrectnessReport,
    RUN_SCHEMA_VERSION, RawSamples, RunMetadata, SAMPLES_SCHEMA_VERSION, derive_report,
};
use super::rust_pipeline::{RustWorkerReceipt, execute as execute_rust_pipeline, write_receipt};
use super::statistics::{PairOrder, PairedTiming, StabTiming};
use super::{LoadedSuite, load_suite, validate_prerequisites};
use crate::error::BenchError;
use crate::process::{
    OutputPolicy, ProcessEnvironment, ProcessLimits, ProcessRequest, ProcessResult,
    run_bounded_process, run_checked_status,
};
use crate::root::RepoRoot;
use crate::stim::{ensure_stim_cli, validate_stim_source};

const STDERR_LIMIT_BYTES: usize = 64 << 10;

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

pub(crate) fn run(root: &RepoRoot, args: RunArgs) -> Result<(), BenchError> {
    if !cfg!(feature = "portable-simd") {
        return Err(BenchError::E2e(
            "e2e-run requires stab-bench's portable-simd feature".to_string(),
        ));
    }
    if args.tier != Tier::Smoke && args.allow_dirty {
        return Err(BenchError::E2e(
            "--allow-dirty is available only for smoke diagnostics".to_string(),
        ));
    }
    let loaded = load_suite(root)?;
    validate_prerequisites(root, &loaded.suite)?;
    let selected = select_cases(&loaded.suite, &args.only)?;
    let formal = args.tier != Tier::Smoke;
    let host_before = HostProfile::capture(args.affinity_cpu).map_err(BenchError::E2e)?;
    if formal {
        host_before
            .validate_formal(loaded.suite.policy.maximum_temperature_millidegrees)
            .map_err(BenchError::E2e)?;
    }
    let source = source_identity(root)?;
    if !source.clean && !args.allow_dirty {
        return Err(BenchError::E2e(
            "source tree is dirty; commit it or use --allow-dirty for smoke".to_string(),
        ));
    }
    let binaries = build_binaries(root, &loaded)?;
    let tier_policy = loaded
        .suite
        .tiers
        .get(&args.tier)
        .ok_or_else(|| BenchError::E2e(format!("suite omits tier {}", args.tier)))?;
    let output = root.create_new_benchmark_output_dir(&args.out)?;

    let mut correctness = Vec::new();
    let mut raw_cases = Vec::new();
    for selected_case in selected {
        println!("[stab-bench] E2E preflight {}", selected_case.id);
        let prepared = PreparedCase::new(
            root,
            selected_case.family,
            selected_case.case,
            &binaries,
            &loaded.suite.policy,
            args.affinity_cpu,
        )?;
        let (case_correctness, expected) = prepared.preflight(&binaries)?;
        correctness.push(case_correctness);
        prepared.warm_up(&binaries, tier_policy.warmups)?;
        let case_samples = prepared.measure(
            &binaries,
            tier_policy.samples,
            &expected,
            &loaded.suite.policy,
        )?;
        raw_cases.push(case_samples);
    }

    let host_after = HostProfile::capture(args.affinity_cpu).map_err(BenchError::E2e)?;
    validate_host_after(
        &host_before,
        &host_after,
        loaded.suite.policy.maximum_temperature_millidegrees,
        formal,
    )
    .map_err(BenchError::E2e)?;
    let metadata = RunMetadata {
        schema_version: RUN_SCHEMA_VERSION,
        suite_sha256: loaded.digest.clone(),
        source_commit: source.commit,
        source_clean: source.clean,
        stim_commit: loaded.suite.stim.commit.clone(),
        stim_binary_sha256: file_sha256(&binaries.stim)?,
        stab_binary_sha256: file_sha256(&binaries.stab)?,
        bench_binary_sha256: file_sha256(&binaries.bench)?,
        rustc: binaries.rustc,
        target: binaries.target,
        tier: args.tier,
        formal,
        host_before,
        host_after,
        selected_cases: raw_cases.iter().map(|case| case.case_id.clone()).collect(),
    };
    let correctness = CorrectnessReport {
        schema_version: CORRECTNESS_SCHEMA_VERSION,
        cases: correctness,
    };
    let samples = RawSamples {
        schema_version: SAMPLES_SCHEMA_VERSION,
        cases: raw_cases,
    };
    let report = derive_report(&loaded.suite, &metadata, &samples).map_err(BenchError::E2e)?;
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
    id: String,
    family: &'a Family,
    case: &'a Case,
}

fn select_cases<'a>(
    suite: &'a super::model::Suite,
    filters: &[String],
) -> Result<Vec<SelectedCase<'a>>, BenchError> {
    let mut selected = Vec::new();
    let mut matched = BTreeSet::new();
    for family in &suite.families {
        for case in &family.cases {
            let id = format!("{}.{}", family.id, case.id);
            let include = filters.is_empty()
                || filters.iter().any(|filter| {
                    let matches = filter == &family.id || filter == &id;
                    if matches {
                        matched.insert(filter.as_str());
                    }
                    matches
                });
            if include {
                selected.push(SelectedCase { id, family, case });
            }
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

pub(super) struct SourceIdentity {
    pub(super) commit: String,
    pub(super) clean: bool,
}

pub(super) fn source_identity(root: &RepoRoot) -> Result<SourceIdentity, BenchError> {
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
    Ok(SourceIdentity {
        commit: String::from_utf8(commit)
            .map_err(|source| BenchError::E2e(source.to_string()))?
            .trim()
            .to_string(),
        clean: status.is_empty(),
    })
}

struct Binaries {
    stim: PathBuf,
    stab: PathBuf,
    bench: PathBuf,
    rustc: String,
    target: String,
}

fn build_binaries(root: &RepoRoot, loaded: &LoadedSuite) -> Result<Binaries, BenchError> {
    let stim_source = root.default_stim_source();
    validate_stim_source(
        &stim_source,
        &loaded.suite.stim.version,
        &loaded.suite.stim.commit,
    )?;
    ensure_stim_cli(root, &stim_source)?;
    run_checked_status(
        "cargo",
        [
            "build",
            "--release",
            "-p",
            "stab-cli",
            "--features",
            "portable-simd",
        ],
        &root.path,
    )?;
    let stab = root
        .path
        .join("target")
        .join("release")
        .join(format!("stab{}", std::env::consts::EXE_SUFFIX));
    if !stab.is_file() {
        return Err(BenchError::E2e(format!(
            "release Stab binary is missing at {}",
            stab.display()
        )));
    }
    let bench = std::env::current_exe()
        .map_err(|source| BenchError::E2e(format!("cannot resolve benchmark binary: {source}")))?;
    if !bench.is_file() {
        return Err(BenchError::E2e(format!(
            "benchmark binary is missing at {}",
            bench.display()
        )));
    }
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
    Ok(Binaries {
        stim: root.stim_binary(),
        stab,
        bench,
        rustc,
        target,
    })
}

struct PreparedFile {
    name: String,
    role: FileRole,
    path: PathBuf,
    output: Option<OutputContract>,
}

struct PreparedCase<'a> {
    id: String,
    family: &'a Family,
    case: &'a Case,
    stdin: Vec<u8>,
    args: Vec<OsString>,
    files: Vec<PreparedFile>,
    _directory: TempDir,
    policy: &'a Policy,
    cpu: Option<usize>,
    working_directory: PathBuf,
}

impl<'a> PreparedCase<'a> {
    fn new(
        root: &RepoRoot,
        family: &'a Family,
        case: &'a Case,
        binaries: &Binaries,
        policy: &'a Policy,
        cpu: Option<usize>,
    ) -> Result<Self, BenchError> {
        let scratch = root.benchmark_root().join("e2e-scratch");
        fs::create_dir_all(&scratch).map_err(|source| {
            BenchError::E2e(format!("failed to create {}: {source}", scratch.display()))
        })?;
        let directory = tempfile::Builder::new()
            .prefix("case-")
            .tempdir_in(&scratch)
            .map_err(|source| BenchError::E2e(format!("cannot create case scratch: {source}")))?;
        let generator =
            |args: &[String]| generated_circuit(binaries, args, &root.path, policy, cpu);
        let (stdin_recipe, command_args, file_specs) = match &case.workload {
            Workload::Cli {
                args, stdin, files, ..
            } => (Some(stdin), args.as_slice(), files.as_slice()),
            Workload::CliPipeline { .. } | Workload::RustPipeline { .. } => {
                (None, &[][..], &[][..])
            }
        };
        let stdin = stdin_recipe
            .map(|recipe| materialize(recipe, generator))
            .transpose()
            .map_err(BenchError::E2e)?
            .unwrap_or_default();
        let mut prepared_files = Vec::new();
        for file in file_specs {
            let path = directory.path().join(&file.name);
            if file.role == FileRole::Input {
                let recipe = file.data.as_ref().ok_or_else(|| {
                    BenchError::E2e(format!("input file {} has no data", file.name))
                })?;
                let bytes = materialize(recipe, |args| {
                    generated_circuit(binaries, args, &root.path, policy, cpu)
                })
                .map_err(BenchError::E2e)?;
                fs::write(&path, bytes).map_err(|source| {
                    BenchError::E2e(format!("cannot write {}: {source}", path.display()))
                })?;
            }
            prepared_files.push(PreparedFile {
                name: file.name.clone(),
                role: file.role,
                path,
                output: file.output.clone(),
            });
        }
        let args = substitute_args(command_args, &prepared_files)?;
        Ok(Self {
            id: format!("{}.{}", family.id, case.id),
            family,
            case,
            stdin,
            args,
            files: prepared_files,
            _directory: directory,
            policy,
            cpu,
            working_directory: root.path.clone(),
        })
    }

    fn preflight(
        &self,
        binaries: &Binaries,
    ) -> Result<(CaseCorrectness, ExpectedOutput), BenchError> {
        match &self.case.workload {
            Workload::RustPipeline {
                shots,
                minimum_logical_failures,
                maximum_logical_failures,
                seed,
            } => {
                let execution = self.run_rust_once(
                    &binaries.bench,
                    *shots,
                    *minimum_logical_failures,
                    *maximum_logical_failures,
                    *seed,
                )?;
                let witness = OutputWitness {
                    bytes: execution.output_bytes,
                    sha256: execution.output_sha256.clone(),
                    one_bits: None,
                };
                Ok((
                    CaseCorrectness {
                        case_id: self.id.clone(),
                        input_sha256: hex::encode(Sha256::digest([])),
                        stim_outputs: BTreeMap::new(),
                        stab_outputs: BTreeMap::from([("pipeline".to_string(), witness.clone())]),
                    },
                    ExpectedOutput {
                        stim_bytes: None,
                        stab_bytes: execution.output_bytes,
                        stab_sha256: Some(execution.output_sha256),
                    },
                ))
            }
            Workload::Cli { stdout, .. } | Workload::CliPipeline { stdout, .. } => {
                let stim = self.run_cli_once(&binaries.stim, true)?;
                let stab = self.run_cli_once(&binaries.stab, true)?;
                let stim_outputs = self.validate_cli_outputs(stdout, &stim)?;
                let stab_outputs = self.validate_cli_outputs(stdout, &stab)?;
                validate_semantic_work(
                    &self.case.work,
                    &self.stdin,
                    stim_outputs.get("stdout").ok_or_else(|| {
                        BenchError::E2e("Stim stdout witness is missing".to_string())
                    })?,
                )
                .map_err(BenchError::E2e)?;
                validate_semantic_work(
                    &self.case.work,
                    &self.stdin,
                    stab_outputs.get("stdout").ok_or_else(|| {
                        BenchError::E2e("Stab stdout witness is missing".to_string())
                    })?,
                )
                .map_err(BenchError::E2e)?;
                compare_exact_outputs(stdout, &self.files, &stim_outputs, &stab_outputs)?;
                if stim.intermediates != stab.intermediates {
                    return Err(BenchError::E2e(format!(
                        "{} pipeline intermediate outputs differ from pinned Stim",
                        self.id
                    )));
                }
                let input_sha256 = self.input_digest()?;
                let expected = ExpectedOutput {
                    stim_bytes: Some(stim.total_output_bytes),
                    stab_bytes: stab.total_output_bytes,
                    stab_sha256: None,
                };
                Ok((
                    CaseCorrectness {
                        case_id: self.id.clone(),
                        input_sha256,
                        stim_outputs,
                        stab_outputs,
                    },
                    expected,
                ))
            }
        }
    }

    fn validate_cli_outputs(
        &self,
        stdout_contract: &OutputContract,
        execution: &CliExecution,
    ) -> Result<BTreeMap<String, OutputWitness>, BenchError> {
        let stdout = execution.stdout.as_ref().ok_or_else(|| {
            BenchError::E2e(format!("{} preflight did not capture stdout", self.id))
        })?;
        let mut outputs = BTreeMap::from([(
            "stdout".to_string(),
            validate_output(stdout_contract, stdout).map_err(BenchError::E2e)?,
        )]);
        for file in &self.files {
            if file.role != FileRole::Output {
                continue;
            }
            let bytes = execution.files.get(&file.name).ok_or_else(|| {
                BenchError::E2e(format!("{} omitted output file {}", self.id, file.name))
            })?;
            let contract = file.output.as_ref().ok_or_else(|| {
                BenchError::E2e(format!(
                    "{} output file {} has no contract",
                    self.id, file.name
                ))
            })?;
            outputs.insert(
                format!("file:{}", file.name),
                validate_output(contract, bytes).map_err(BenchError::E2e)?,
            );
        }
        for (index, bytes) in execution.intermediates.iter().enumerate() {
            outputs.insert(
                format!("step-{index}"),
                validate_output(&OutputContract::Exact { minimum_bytes: 1 }, bytes)
                    .map_err(BenchError::E2e)?,
            );
        }
        Ok(outputs)
    }

    fn warm_up(&self, binaries: &Binaries, warmups: usize) -> Result<(), BenchError> {
        for index in 0..warmups {
            match self.family.comparator {
                Comparator::StimCli => {
                    let (first, second) = if PairOrder::for_index(index) == PairOrder::StimThenStab
                    {
                        (&binaries.stim, &binaries.stab)
                    } else {
                        (&binaries.stab, &binaries.stim)
                    };
                    self.run_cli_batch(first)?;
                    self.run_cli_batch(second)?;
                }
                Comparator::SelfOnly => {
                    self.run_rust_batch(&binaries.bench)?;
                }
            }
        }
        Ok(())
    }

    fn measure(
        &self,
        binaries: &Binaries,
        samples: usize,
        expected: &ExpectedOutput,
        _policy: &Policy,
    ) -> Result<CaseSamples, BenchError> {
        let digest = case_digest(&self.family.id, self.case).map_err(BenchError::E2e)?;
        let mut paired = Vec::new();
        let mut stab_only = Vec::new();
        match self.family.comparator {
            Comparator::StimCli => {
                for index in 0..samples {
                    let order = PairOrder::for_index(index);
                    let (stim, stab) = match order {
                        PairOrder::StimThenStab => {
                            let stim = self.run_cli_batch(&binaries.stim)?;
                            let stab = self.run_cli_batch(&binaries.stab)?;
                            (stim, stab)
                        }
                        PairOrder::StabThenStim => {
                            let stab = self.run_cli_batch(&binaries.stab)?;
                            let stim = self.run_cli_batch(&binaries.stim)?;
                            (stim, stab)
                        }
                    };
                    let expected_stim = expected.stim_bytes.ok_or_else(|| {
                        BenchError::E2e(format!("{} has no Stim output expectation", self.id))
                    })?;
                    if stim.output_bytes != expected_stim * u64::from(self.case.batch)
                        || stab.output_bytes != expected.stab_bytes * u64::from(self.case.batch)
                    {
                        return Err(BenchError::E2e(format!(
                            "{} output size changed during paired sample {index}",
                            self.id
                        )));
                    }
                    paired.push(PairedTiming {
                        index,
                        order,
                        stim_seconds: stim.seconds,
                        stab_seconds: stab.seconds,
                        stim_work: self.sample_work()?,
                        stab_work: self.sample_work()?,
                        stim_peak_rss_bytes: stim.peak_rss_bytes,
                        stab_peak_rss_bytes: stab.peak_rss_bytes,
                        stim_output_bytes: stim.output_bytes,
                        stab_output_bytes: stab.output_bytes,
                    });
                }
            }
            Comparator::SelfOnly => {
                for index in 0..samples {
                    let sample = self.run_rust_batch(&binaries.bench)?;
                    if sample.output_bytes != expected.stab_bytes * u64::from(self.case.batch)
                        || expected.stab_sha256.as_deref() != Some(sample.output_sha256.as_str())
                    {
                        return Err(BenchError::E2e(format!(
                            "{} semantic output changed during Stab sample {index}",
                            self.id
                        )));
                    }
                    stab_only.push(StabTiming {
                        index,
                        seconds: sample.seconds,
                        work: self.sample_work()?,
                        peak_rss_bytes: sample.peak_rss_bytes,
                        output_bytes: sample.output_bytes,
                    });
                }
            }
        }
        Ok(CaseSamples {
            case_id: self.id.clone(),
            case_digest: digest,
            paired,
            stab_only,
        })
    }

    fn run_cli_batch(&self, binary: &Path) -> Result<BatchMeasurement, BenchError> {
        let mut seconds = 0.0;
        let mut peak_rss_bytes = 0;
        let mut output_bytes = 0_u64;
        for _ in 0..self.case.batch {
            let execution = self.run_cli_once(binary, false)?;
            seconds += execution.seconds;
            peak_rss_bytes = peak_rss_bytes.max(execution.peak_rss_bytes);
            output_bytes = output_bytes
                .checked_add(execution.total_output_bytes)
                .ok_or_else(|| BenchError::E2e("CLI output byte count overflow".to_string()))?;
        }
        Ok(BatchMeasurement {
            seconds,
            peak_rss_bytes,
            output_bytes,
        })
    }

    fn run_cli_once(&self, binary: &Path, capture: bool) -> Result<CliExecution, BenchError> {
        for file in &self.files {
            if file.role == FileRole::Output {
                match fs::remove_file(&file.path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(BenchError::E2e(format!(
                            "cannot clear {}: {source}",
                            file.path.display()
                        )));
                    }
                }
            }
        }
        let mut intermediates = Vec::new();
        let result = match &self.case.workload {
            Workload::Cli { .. } => run_command(
                binary,
                &self.args,
                &self.stdin,
                &self.working_directory,
                capture,
                self.cpu,
                self.policy,
            )?,
            Workload::CliPipeline { steps, .. } => {
                let started = Instant::now();
                let mut previous = Vec::new();
                let mut peak = 0_u64;
                let mut final_result = None;
                for (index, step) in steps.iter().enumerate() {
                    let stdin = match step.stdin {
                        PipelineInput::Empty => &[][..],
                        PipelineInput::Previous => previous.as_slice(),
                    };
                    let last = index + 1 == steps.len();
                    let args = step.args.iter().map(OsString::from).collect::<Vec<_>>();
                    let result = run_command(
                        binary,
                        &args,
                        stdin,
                        &self.working_directory,
                        !last || capture,
                        self.cpu,
                        self.policy,
                    )?;
                    peak = peak.max(result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                        BenchError::E2e(format!("{} pipeline RSS was not observed", self.id))
                    })?);
                    if last {
                        final_result = Some(result);
                    } else {
                        previous = result.stdout;
                        if capture {
                            intermediates.push(previous.clone());
                        }
                    }
                }
                let mut final_result = final_result.ok_or_else(|| {
                    BenchError::E2e(format!("{} pipeline has no final step", self.id))
                })?;
                final_result.wall_elapsed = started.elapsed();
                final_result.parent_observed_peak_rss_bytes = Some(peak);
                final_result
            }
            Workload::RustPipeline { .. } => {
                return Err(BenchError::E2e(format!(
                    "{} Rust workload reached the CLI runner",
                    self.id
                )));
            }
        };
        checked_success(binary, &result)?;
        let mut files = BTreeMap::new();
        let mut total_output_bytes = result.stdout_bytes;
        for file in &self.files {
            if file.role != FileRole::Output {
                continue;
            }
            let bytes = fs::read(&file.path).map_err(|source| {
                BenchError::E2e(format!("cannot read {}: {source}", file.path.display()))
            })?;
            total_output_bytes = total_output_bytes
                .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                    BenchError::E2e("side-output byte count does not fit in u64".to_string())
                })?)
                .ok_or_else(|| BenchError::E2e("total output byte count overflow".to_string()))?;
            files.insert(file.name.clone(), bytes);
        }
        Ok(CliExecution {
            seconds: result.wall_elapsed.as_secs_f64(),
            peak_rss_bytes: result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                BenchError::E2e(format!("{} process RSS was not observed", self.id))
            })?,
            total_output_bytes,
            stdout: capture.then_some(result.stdout),
            files,
            intermediates,
        })
    }

    fn run_rust_batch(&self, bench: &Path) -> Result<RustMeasurement, BenchError> {
        let mut seconds = 0.0;
        let mut peak_rss_bytes = 0;
        let mut output_bytes = 0_u64;
        let mut output_sha256 = None;
        for _ in 0..self.case.batch {
            let Workload::RustPipeline {
                shots,
                minimum_logical_failures,
                maximum_logical_failures,
                seed,
            } = self.case.workload
            else {
                return Err(BenchError::E2e(format!(
                    "{} non-Rust workload reached Rust runner",
                    self.id
                )));
            };
            let execution = self.run_rust_once(
                bench,
                shots,
                minimum_logical_failures,
                maximum_logical_failures,
                seed,
            )?;
            seconds += execution.seconds;
            peak_rss_bytes = peak_rss_bytes.max(execution.peak_rss_bytes);
            output_bytes = output_bytes
                .checked_add(execution.output_bytes)
                .ok_or_else(|| BenchError::E2e("Rust output byte count overflow".to_string()))?;
            match &output_sha256 {
                Some(expected) if expected != &execution.output_sha256 => {
                    return Err(BenchError::E2e(format!(
                        "{} Rust batch output changed",
                        self.id
                    )));
                }
                None => output_sha256 = Some(execution.output_sha256),
                _ => {}
            }
        }
        Ok(RustMeasurement {
            seconds,
            peak_rss_bytes,
            output_bytes,
            output_sha256: output_sha256
                .ok_or_else(|| BenchError::E2e("Rust batch is empty".to_string()))?,
        })
    }

    fn run_rust_once(
        &self,
        bench: &Path,
        shots: u64,
        minimum_logical_failures: u64,
        maximum_logical_failures: u64,
        seed: u64,
    ) -> Result<RustMeasurement, BenchError> {
        let args = [
            OsString::from("e2e-worker"),
            OsString::from("--shots"),
            OsString::from(shots.to_string()),
            OsString::from("--minimum-logical-failures"),
            OsString::from(minimum_logical_failures.to_string()),
            OsString::from("--maximum-logical-failures"),
            OsString::from(maximum_logical_failures.to_string()),
            OsString::from("--seed"),
            OsString::from(seed.to_string()),
        ];
        let result = run_command(
            bench,
            &args,
            b"\n",
            &self.working_directory,
            true,
            self.cpu,
            self.policy,
        )?;
        checked_success(bench, &result)?;
        let receipt =
            serde_json::from_slice::<RustWorkerReceipt>(&result.stdout).map_err(|source| {
                BenchError::E2e(format!(
                    "{} Rust worker receipt is invalid: {source}",
                    self.id
                ))
            })?;
        if receipt.schema_version != 1
            || receipt.shots != shots
            || receipt.logical_failures < minimum_logical_failures
            || receipt.logical_failures > maximum_logical_failures
            || !receipt.elapsed_seconds.is_finite()
            || receipt.elapsed_seconds <= 0.0
        {
            return Err(BenchError::E2e(format!(
                "{} Rust worker receipt violates its semantic contract",
                self.id
            )));
        }
        Ok(RustMeasurement {
            seconds: receipt.elapsed_seconds,
            peak_rss_bytes: result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                BenchError::E2e(format!("{} Rust worker RSS was not observed", self.id))
            })?,
            output_bytes: u64::try_from(format!("{}:{}", shots, receipt.logical_failures).len())
                .map_err(|_| BenchError::E2e("Rust output size does not fit in u64".to_string()))?,
            output_sha256: receipt.output_sha256,
        })
    }

    fn sample_work(&self) -> Result<u64, BenchError> {
        self.case
            .work
            .amount
            .checked_mul(u64::from(self.case.batch))
            .ok_or_else(|| BenchError::E2e(format!("{} semantic work overflow", self.id)))
    }

    fn input_digest(&self) -> Result<String, BenchError> {
        let mut hash = Sha256::new();
        hash.update((self.stdin.len() as u64).to_le_bytes());
        hash.update(&self.stdin);
        for file in &self.files {
            if file.role != FileRole::Input {
                continue;
            }
            let bytes = fs::read(&file.path).map_err(|source| {
                BenchError::E2e(format!("cannot read {}: {source}", file.path.display()))
            })?;
            hash.update((file.name.len() as u64).to_le_bytes());
            hash.update(file.name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Ok(hex::encode(hash.finalize()))
    }
}

struct ExpectedOutput {
    stim_bytes: Option<u64>,
    stab_bytes: u64,
    stab_sha256: Option<String>,
}

struct CliExecution {
    seconds: f64,
    peak_rss_bytes: u64,
    total_output_bytes: u64,
    stdout: Option<Vec<u8>>,
    files: BTreeMap<String, Vec<u8>>,
    intermediates: Vec<Vec<u8>>,
}

struct BatchMeasurement {
    seconds: f64,
    peak_rss_bytes: u64,
    output_bytes: u64,
}

struct RustMeasurement {
    seconds: f64,
    peak_rss_bytes: u64,
    output_bytes: u64,
    output_sha256: String,
}

fn generated_circuit(
    binaries: &Binaries,
    args: &[String],
    working_directory: &Path,
    policy: &Policy,
    cpu: Option<usize>,
) -> Result<Vec<u8>, String> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    let stim = run_command(
        &binaries.stim,
        &args,
        b"",
        working_directory,
        true,
        cpu,
        policy,
    )
    .map_err(|source| source.to_string())?;
    let stab = run_command(
        &binaries.stab,
        &args,
        b"",
        working_directory,
        true,
        cpu,
        policy,
    )
    .map_err(|source| source.to_string())?;
    checked_success(&binaries.stim, &stim).map_err(|source| source.to_string())?;
    checked_success(&binaries.stab, &stab).map_err(|source| source.to_string())?;
    if stim.stdout != stab.stdout {
        return Err("generated circuit differs from pinned Stim".to_string());
    }
    Ok(stim.stdout)
}

fn compare_exact_outputs(
    stdout: &OutputContract,
    files: &[PreparedFile],
    stim: &BTreeMap<String, OutputWitness>,
    stab: &BTreeMap<String, OutputWitness>,
) -> Result<(), BenchError> {
    let mut exact = Vec::new();
    if matches!(stdout, OutputContract::Exact { .. }) {
        exact.push("stdout".to_string());
    }
    for file in files {
        if matches!(file.output, Some(OutputContract::Exact { .. })) {
            exact.push(format!("file:{}", file.name));
        }
    }
    exact.extend(stim.keys().filter(|key| key.starts_with("step-")).cloned());
    for key in exact {
        if stim.get(&key) != stab.get(&key) {
            return Err(BenchError::E2e(format!(
                "exact output {key} differs from pinned Stim"
            )));
        }
    }
    Ok(())
}

fn substitute_args(args: &[String], files: &[PreparedFile]) -> Result<Vec<OsString>, BenchError> {
    let paths = files
        .iter()
        .map(|file| (format!("{{file:{}}}", file.name), &file.path))
        .collect::<BTreeMap<_, _>>();
    args.iter()
        .map(|arg| {
            if let Some(path) = paths.get(arg) {
                Ok(path.as_os_str().to_os_string())
            } else if arg.contains("{file:") {
                Err(BenchError::E2e(format!(
                    "unknown or partial file placeholder {arg}"
                )))
            } else {
                Ok(OsString::from(arg))
            }
        })
        .collect()
}

fn run_command(
    binary: &Path,
    args: &[OsString],
    stdin: &[u8],
    working_directory: &Path,
    capture: bool,
    cpu: Option<usize>,
    policy: &Policy,
) -> Result<ProcessResult, BenchError> {
    let output_policy = if capture {
        OutputPolicy::Capture {
            maximum_bytes: policy.maximum_output_bytes,
        }
    } else {
        OutputPolicy::Discard
    };
    run_bounded_process(&ProcessRequest {
        program: binary.to_path_buf(),
        args: args.to_vec(),
        stdin: stdin.to_vec(),
        working_directory: working_directory.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: cpu,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: output_policy,
            stderr: OutputPolicy::Capture {
                maximum_bytes: STDERR_LIMIT_BYTES,
            },
            regular_file_bytes: Some(policy.maximum_output_bytes as u64),
            timeout: Duration::from_secs(policy.command_timeout_seconds),
        },
    })
    .map_err(BenchError::from)
}

fn checked_success(binary: &Path, result: &ProcessResult) -> Result<(), BenchError> {
    if result.status == Some(0) && result.stderr_bytes == 0 {
        return Ok(());
    }
    Err(BenchError::E2e(format!(
        "{} failed with status {:?} and stderr {}",
        binary.display(),
        result.status,
        String::from_utf8_lossy(&result.stderr)
    )))
}

pub(super) fn command_output(
    binary: &Path,
    args: &[&str],
    stdin: &[u8],
    working_directory: &Path,
    maximum_output: usize,
    cpu: Option<usize>,
    timeout_seconds: u64,
) -> Result<Vec<u8>, BenchError> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    let result = run_bounded_process(&ProcessRequest {
        program: binary.to_path_buf(),
        args,
        stdin: stdin.to_vec(),
        working_directory: working_directory.to_path_buf(),
        environment: ProcessEnvironment::Inherit,
        affinity_cpu: cpu,
        limits: ProcessLimits {
            stdin_bytes: stdin.len(),
            stdout: OutputPolicy::Capture {
                maximum_bytes: maximum_output,
            },
            stderr: OutputPolicy::Capture {
                maximum_bytes: maximum_output,
            },
            regular_file_bytes: None,
            timeout: Duration::from_secs(timeout_seconds),
        },
    })?;
    if result.status != Some(0) {
        return Err(BenchError::E2e(format!(
            "{} failed: {}",
            binary.display(),
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    Ok(result.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_rejects_unmatched_filters_and_keeps_source_order() {
        let root = RepoRoot::resolve(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .expect("root");
        let loaded = load_suite(&root).expect("suite");
        let selected = select_cases(
            &loaded.suite,
            &[
                "convert-dense".to_string(),
                "generate-surface.large".to_string(),
            ],
        )
        .expect("selection");
        assert_eq!(
            selected
                .iter()
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>(),
            [
                "generate-surface.large",
                "convert-dense.01-to-b8",
                "convert-dense.b8-to-01"
            ]
        );
        assert!(select_cases(&loaded.suite, &["missing".to_string()]).is_err());
    }

    #[test]
    fn placeholders_are_exact_and_unknown_names_fail() {
        let files = vec![PreparedFile {
            name: "obs".to_string(),
            role: FileRole::Output,
            path: PathBuf::from("/tmp/obs"),
            output: Some(OutputContract::Exact { minimum_bytes: 0 }),
        }];
        assert_eq!(
            substitute_args(&["--obs_out".to_string(), "{file:obs}".to_string()], &files)
                .expect("substitute"),
            [OsString::from("--obs_out"), OsString::from("/tmp/obs")]
        );
        assert!(substitute_args(&["{file:missing}".to_string()], &files).is_err());
    }
}
