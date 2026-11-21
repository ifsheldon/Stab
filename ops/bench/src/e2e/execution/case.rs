//! One prepared case: materialized inputs, preflight correctness, warmups, and measured samples.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

use super::Binaries;
use crate::e2e::commands::{PinnedCommands, checked_success, run_command};
use crate::e2e::data::{OutputKey, OutputWitness, compare_witness_censuses};
use crate::e2e::error::{ByteCountOverflow, CaseError, Implementation};
use crate::e2e::model::{
    Case, CaseId, Comparator, Family, FileRole, OutputContract, PipelineInput, Policy, Workload,
};
use crate::e2e::report::{CaseCorrectness, CaseSamples};
use crate::e2e::rust_pipeline::RustWorkerReceipt;
use crate::e2e::statistics::{PairOrder, PairedTiming, StabTiming};
use crate::error::BenchError;
use crate::root::RepoRoot;

pub(super) struct PreparedFile {
    pub(super) name: String,
    pub(super) role: FileRole,
    pub(super) path: PathBuf,
    pub(super) output: Option<OutputContract>,
}

pub(super) struct PreparedCase<'a> {
    id: CaseId,
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
    pub(super) fn new(
        root: &RepoRoot,
        id: CaseId,
        family: &'a Family,
        case: &'a Case,
        binaries: &Binaries,
        policy: &'a Policy,
        cpu: Option<usize>,
    ) -> Result<Self, BenchError> {
        let scratch = root.benchmark_root().join("e2e-scratch");
        if let Err(source) = fs::create_dir_all(&scratch) {
            return Err(crate::e2e::error::RunError::CreateDirectory {
                path: scratch,
                source,
            }
            .into());
        }
        let directory = tempfile::Builder::new()
            .prefix("case-")
            .tempdir_in(&scratch)
            .map_err(CaseError::Scratch)?;
        let mut pinned =
            PinnedCommands::new(&binaries.stim, &binaries.stab, &root.path, policy, cpu);
        let (stdin_recipe, command_args, file_specs) = match &case.workload {
            Workload::Cli {
                args, stdin, files, ..
            } => (Some(stdin), args.as_slice(), files.as_slice()),
            Workload::CliPipeline { .. } | Workload::RustPipeline { .. } => {
                (None, &[][..], &[][..])
            }
        };
        let stdin = stdin_recipe
            .map(|recipe| recipe.materialize(&mut pinned))
            .transpose()?
            .unwrap_or_default();
        let mut prepared_files = Vec::new();
        for file in file_specs {
            let path = directory.path().join(&file.name);
            if file.role == FileRole::Input {
                let recipe = file
                    .data
                    .as_ref()
                    .ok_or_else(|| CaseError::InputWithoutData {
                        file: file.name.clone(),
                    })?;
                let bytes = recipe.materialize(&mut pinned)?;
                fs::write(&path, bytes).map_err(|source| CaseError::WriteInput {
                    path: path.clone(),
                    source,
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
            id,
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

    pub(super) fn preflight(
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
                    census: None,
                };
                Ok((
                    CaseCorrectness {
                        case_id: self.id.clone(),
                        input_sha256: hex::encode(Sha256::digest([])),
                        stim_outputs: BTreeMap::new(),
                        stab_outputs: BTreeMap::from([(OutputKey::Pipeline, witness)]),
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
                compare_witness_censuses(&stim_outputs, &stab_outputs).map_err(
                    |(key, source)| CaseError::Census {
                        case: self.id.clone(),
                        key,
                        source,
                    },
                )?;
                for (side, outputs) in [
                    (Implementation::Stim, &stim_outputs),
                    (Implementation::Stab, &stab_outputs),
                ] {
                    let stdout = outputs
                        .get(&OutputKey::Stdout)
                        .ok_or(CaseError::MissingStdoutWitness { side })?;
                    self.case.work.check(&self.stdin, stdout)?;
                }
                compare_exact_outputs(stdout, &self.files, &stim_outputs, &stab_outputs)?;
                if stim.intermediates != stab.intermediates {
                    return Err(CaseError::IntermediatesDiffer {
                        case: self.id.clone(),
                    }
                    .into());
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
    ) -> Result<BTreeMap<OutputKey, OutputWitness>, BenchError> {
        let stdout = execution
            .stdout
            .as_ref()
            .ok_or_else(|| CaseError::StdoutNotCaptured {
                case: self.id.clone(),
            })?;
        let mut outputs = BTreeMap::from([(OutputKey::Stdout, stdout_contract.validate(stdout)?)]);
        for file in &self.files {
            if file.role != FileRole::Output {
                continue;
            }
            let bytes =
                execution
                    .files
                    .get(&file.name)
                    .ok_or_else(|| CaseError::OmittedOutputFile {
                        case: self.id.clone(),
                        file: file.name.clone(),
                    })?;
            let contract =
                file.output
                    .as_ref()
                    .ok_or_else(|| CaseError::OutputWithoutContract {
                        case: self.id.clone(),
                        file: file.name.clone(),
                    })?;
            outputs.insert(
                OutputKey::File(file.name.clone()),
                contract.validate(bytes)?,
            );
        }
        for (index, bytes) in execution.intermediates.iter().enumerate() {
            outputs.insert(
                OutputKey::Step(index),
                OutputContract::Exact { minimum_bytes: 1 }.validate(bytes)?,
            );
        }
        Ok(outputs)
    }

    pub(super) fn warm_up(
        &self,
        binaries: &Binaries,
        warmups: usize,
        cancellation: &stab_process::Cancellation,
    ) -> Result<(), BenchError> {
        for index in 0..warmups {
            super::check_cancelled(cancellation)?;
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

    pub(super) fn measure(
        &self,
        binaries: &Binaries,
        samples: usize,
        expected: &ExpectedOutput,
        cancellation: &stab_process::Cancellation,
    ) -> Result<CaseSamples, BenchError> {
        let digest = self
            .case
            .digest(&self.family.id)
            .map_err(|source| CaseError::Digest {
                case: self.id.clone(),
                source,
            })?;
        let mut paired = Vec::new();
        let mut stab_only = Vec::new();
        match self.family.comparator {
            Comparator::StimCli => {
                let expected_stim = self.batch_bytes(expected.stim_bytes.ok_or_else(|| {
                    CaseError::NoStimExpectation {
                        case: self.id.clone(),
                    }
                })?)?;
                let expected_stab = self.batch_bytes(expected.stab_bytes)?;
                for index in 0..samples {
                    super::check_cancelled(cancellation)?;
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
                    if stim.output_bytes != expected_stim || stab.output_bytes != expected_stab {
                        return Err(CaseError::OutputSizeChanged {
                            case: self.id.clone(),
                            index,
                        }
                        .into());
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
                let expected_stab = self.batch_bytes(expected.stab_bytes)?;
                for index in 0..samples {
                    super::check_cancelled(cancellation)?;
                    let sample = self.run_rust_batch(&binaries.bench)?;
                    if sample.output_bytes != expected_stab
                        || expected.stab_sha256.as_deref() != Some(sample.output_sha256.as_str())
                    {
                        return Err(CaseError::SemanticOutputChanged {
                            case: self.id.clone(),
                            index,
                        }
                        .into());
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
                .ok_or(CaseError::ByteCount(ByteCountOverflow::CliBatch))?;
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
                        return Err(CaseError::ClearOutput {
                            path: file.path.clone(),
                            source,
                        }
                        .into());
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
                let mut elapsed = Duration::ZERO;
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
                    elapsed = elapsed.checked_add(result.wall_elapsed).ok_or_else(|| {
                        CaseError::PipelineTime {
                            case: self.id.clone(),
                        }
                    })?;
                    peak = peak.max(result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                        CaseError::PipelineRss {
                            case: self.id.clone(),
                        }
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
                let mut final_result = final_result.ok_or_else(|| CaseError::NoFinalStep {
                    case: self.id.clone(),
                })?;
                final_result.wall_elapsed = elapsed;
                final_result.parent_observed_peak_rss_bytes = Some(peak);
                final_result
            }
            Workload::RustPipeline { .. } => {
                return Err(CaseError::RustOnCliRunner {
                    case: self.id.clone(),
                }
                .into());
            }
        };
        checked_success(binary, &result)?;
        let mut files = BTreeMap::new();
        let mut total_output_bytes = result.stdout_bytes;
        for file in &self.files {
            if file.role != FileRole::Output {
                continue;
            }
            let bytes = read_file(&file.path)?;
            total_output_bytes = total_output_bytes
                .checked_add(
                    u64::try_from(bytes.len())
                        .map_err(|_| CaseError::ByteCount(ByteCountOverflow::SideOutput))?,
                )
                .ok_or(CaseError::ByteCount(ByteCountOverflow::TotalOutput))?;
            files.insert(file.name.clone(), bytes);
        }
        Ok(CliExecution {
            seconds: result.wall_elapsed.as_secs_f64(),
            peak_rss_bytes: result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                CaseError::ProcessRss {
                    case: self.id.clone(),
                }
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
                return Err(CaseError::CliOnRustRunner {
                    case: self.id.clone(),
                }
                .into());
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
                .ok_or(CaseError::ByteCount(ByteCountOverflow::RustBatch))?;
            match &output_sha256 {
                Some(expected) if expected != &execution.output_sha256 => {
                    return Err(CaseError::RustBatchChanged {
                        case: self.id.clone(),
                    }
                    .into());
                }
                None => output_sha256 = Some(execution.output_sha256),
                _ => {}
            }
        }
        Ok(RustMeasurement {
            seconds,
            peak_rss_bytes,
            output_bytes,
            output_sha256: output_sha256.ok_or(CaseError::EmptyRustBatch)?,
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
                CaseError::WorkerReceipt {
                    case: self.id.clone(),
                    source,
                }
            })?;
        if receipt.schema_version != 1
            || receipt.shots != shots
            || receipt.logical_failures < minimum_logical_failures
            || receipt.logical_failures > maximum_logical_failures
            || !receipt.elapsed_seconds.is_finite()
            || receipt.elapsed_seconds <= 0.0
        {
            return Err(CaseError::ReceiptContract {
                case: self.id.clone(),
            }
            .into());
        }
        Ok(RustMeasurement {
            seconds: receipt.elapsed_seconds,
            peak_rss_bytes: result.parent_observed_peak_rss_bytes.ok_or_else(|| {
                CaseError::WorkerRss {
                    case: self.id.clone(),
                }
            })?,
            output_bytes: u64::try_from(format!("{}:{}", shots, receipt.logical_failures).len())
                .map_err(|_| CaseError::ByteCount(ByteCountOverflow::RustOutput))?,
            output_sha256: receipt.output_sha256,
        })
    }

    /// One run's output bytes times the case batch.
    fn batch_bytes(&self, per_run: u64) -> Result<u64, BenchError> {
        per_run
            .checked_mul(u64::from(self.case.batch))
            .ok_or_else(|| {
                CaseError::BatchBytes {
                    case: self.id.clone(),
                }
                .into()
            })
    }

    fn sample_work(&self) -> Result<u64, BenchError> {
        self.case
            .work
            .amount
            .checked_mul(u64::from(self.case.batch))
            .ok_or_else(|| {
                CaseError::WorkOverflow {
                    case: self.id.clone(),
                }
                .into()
            })
    }

    fn input_digest(&self) -> Result<String, BenchError> {
        let mut hash = Sha256::new();
        hash.update((self.stdin.len() as u64).to_le_bytes());
        hash.update(&self.stdin);
        for file in &self.files {
            if file.role != FileRole::Input {
                continue;
            }
            let bytes = read_file(&file.path)?;
            hash.update((file.name.len() as u64).to_le_bytes());
            hash.update(file.name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Ok(hex::encode(hash.finalize()))
    }
}

pub(super) struct ExpectedOutput {
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

fn read_file(path: &Path) -> Result<Vec<u8>, CaseError> {
    fs::read(path).map_err(|source| CaseError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

/// Requires byte-identical witnesses for every exact output: exact stdout, exact side files, and
/// every pipeline intermediate.
pub(super) fn compare_exact_outputs(
    stdout: &OutputContract,
    files: &[PreparedFile],
    stim: &BTreeMap<OutputKey, OutputWitness>,
    stab: &BTreeMap<OutputKey, OutputWitness>,
) -> Result<(), BenchError> {
    let mut exact = Vec::new();
    if matches!(stdout, OutputContract::Exact { .. }) {
        exact.push(OutputKey::Stdout);
    }
    for file in files {
        if matches!(file.output, Some(OutputContract::Exact { .. })) {
            exact.push(OutputKey::File(file.name.clone()));
        }
    }
    exact.extend(
        stim.keys()
            .chain(stab.keys())
            .filter(|key| matches!(key, OutputKey::Step(_)))
            .cloned(),
    );
    for key in exact {
        if stim.get(&key) != stab.get(&key) {
            return Err(CaseError::ExactOutputDiffers { key }.into());
        }
    }
    Ok(())
}

pub(super) fn substitute_args(
    args: &[String],
    files: &[PreparedFile],
) -> Result<Vec<OsString>, BenchError> {
    let paths = files
        .iter()
        .map(|file| (format!("{{file:{}}}", file.name), &file.path))
        .collect::<BTreeMap<_, _>>();
    args.iter()
        .map(|arg| {
            if let Some(path) = paths.get(arg) {
                Ok(path.as_os_str().to_os_string())
            } else if arg.contains("{file:") {
                Err(CaseError::Placeholder {
                    argument: arg.clone(),
                }
                .into())
            } else {
                Ok(OsString::from(arg))
            }
        })
        .collect()
}
