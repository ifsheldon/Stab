use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::time::Duration;

use sha2::{Digest as _, Sha256};

use super::{
    FixtureComparator, FixtureError, FixtureManifest, FixtureStatus, RepoRoot,
    check_direct_rust_fixture_executed_tests, check_expected_process_shape, compare_fixture,
    is_core_fixture, is_direct_rust_fixture, outputs, run_core_fixture,
};

#[derive(Debug)]
pub(crate) struct QualificationFixtureFailure {
    reason: Box<str>,
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    completed: bool,
    completed_statistical_shots: u64,
    completed_statistical_comparisons: u32,
    completed_statistical_batches: u32,
    status: Option<i32>,
}

pub(crate) struct QualificationFixtureFailureParts {
    pub(crate) reason: Box<str>,
    pub(crate) stdout: Option<Vec<u8>>,
    pub(crate) stderr: Option<Vec<u8>>,
    pub(crate) completed: bool,
    pub(crate) completed_statistical_shots: u64,
    pub(crate) completed_statistical_comparisons: u32,
    pub(crate) completed_statistical_batches: u32,
    pub(crate) status: Option<i32>,
}

impl QualificationFixtureFailure {
    fn message(reason: impl Into<Box<str>>) -> Self {
        Self {
            reason: reason.into(),
            stdout: None,
            stderr: None,
            completed: false,
            completed_statistical_shots: 0,
            completed_statistical_comparisons: 0,
            completed_statistical_batches: 0,
            status: None,
        }
    }

    fn process(reason: impl Into<Box<str>>, output: &crate::ProcessOutput) -> Self {
        Self::process_with_statistical_completion(reason, output, StatisticalCompletion::default())
    }

    fn process_with_statistical_completion(
        reason: impl Into<Box<str>>,
        output: &crate::ProcessOutput,
        completion: StatisticalCompletion,
    ) -> Self {
        Self {
            reason: reason.into(),
            stdout: Some(output.stdout.bytes.clone()),
            stderr: Some(output.stderr.bytes.clone()),
            completed: true,
            completed_statistical_shots: completion.shots,
            completed_statistical_comparisons: completion.comparisons,
            completed_statistical_batches: completion.batches,
            status: output.status,
        }
    }

    fn oracle(source: crate::OracleError) -> Self {
        Self::oracle_with_completion(source, StatisticalCompletion::default())
    }

    fn oracle_with_completion(
        source: crate::OracleError,
        completion: StatisticalCompletion,
    ) -> Self {
        let (stdout, stderr) = source
            .captured_streams()
            .map(|(stdout, stderr)| (Some(stdout.bytes.clone()), Some(stderr.bytes.clone())))
            .unwrap_or((None, None));
        Self {
            reason: source.to_string().into_boxed_str(),
            stdout,
            stderr,
            completed: false,
            completed_statistical_shots: completion.shots,
            completed_statistical_comparisons: completion.comparisons,
            completed_statistical_batches: completion.batches,
            status: None,
        }
    }

    pub(crate) fn into_parts(self) -> QualificationFixtureFailureParts {
        QualificationFixtureFailureParts {
            reason: self.reason,
            stdout: self.stdout,
            stderr: self.stderr,
            completed: self.completed,
            completed_statistical_shots: self.completed_statistical_shots,
            completed_statistical_comparisons: self.completed_statistical_comparisons,
            completed_statistical_batches: self.completed_statistical_batches,
            status: self.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StatisticalCompletion {
    pub(super) shots: u64,
    pub(super) comparisons: u32,
    pub(super) batches: u32,
}

impl fmt::Display for QualificationFixtureFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for QualificationFixtureFailure {}

pub(crate) struct QualificationFixtureRunner<'a> {
    root: &'a RepoRoot,
    manifest: FixtureManifest,
    executables: Option<&'a crate::qualification::executables::QualificationExecutables>,
}

impl<'a> QualificationFixtureRunner<'a> {
    pub(crate) fn new(root: &'a RepoRoot) -> Result<Self, FixtureError> {
        let manifest = FixtureManifest::read(root)?;
        manifest.check(root)?;
        Ok(Self {
            root,
            manifest,
            executables: None,
        })
    }

    pub(crate) fn for_run(
        root: &'a RepoRoot,
        executables: &'a crate::qualification::executables::QualificationExecutables,
    ) -> Result<Self, FixtureError> {
        let mut runner = Self::new(root)?;
        runner.executables = Some(executables);
        Ok(runner)
    }

    pub(crate) fn run(
        &mut self,
        id: &str,
        timeout: Duration,
    ) -> Result<crate::ProcessOutput, QualificationFixtureFailure> {
        self.run_with_seed(id, timeout, None)
    }

    pub(crate) fn run_with_seed(
        &mut self,
        id: &str,
        timeout: Duration,
        seed: Option<u64>,
    ) -> Result<crate::ProcessOutput, QualificationFixtureFailure> {
        let mut row = self
            .manifest
            .rows
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| {
                QualificationFixtureFailure::message(format!(
                    "qualification fixture {id} is absent from the validated manifest"
                ))
            })?
            .clone();
        if row.status != FixtureStatus::Implemented {
            return Err(QualificationFixtureFailure::message(format!(
                "qualification fixture {id} has {}, expected implemented",
                row.status.as_str()
            )));
        }
        if let Some(seed) = seed {
            if row.comparator != FixtureComparator::Statistical {
                return Err(QualificationFixtureFailure::message(format!(
                    "qualification fixture {id} seed override requires a statistical fixture"
                )));
            }
            row.argv = super::statistical::argv_with_seed(&row.argv, seed).ok_or_else(|| {
                QualificationFixtureFailure::message(format!(
                    "qualification fixture {id} has no --seed option to override"
                ))
            })?;
        }
        if is_core_fixture(&row) {
            return run_core_fixture_in_worker(self.root, self.run_executables()?, &row, timeout);
        }
        if is_direct_rust_fixture(&row) {
            return run_direct_rust_qualification(
                self.root,
                self.run_executables()?,
                &row,
                timeout,
            );
        }
        let stdin = row
            .stdin(self.root)
            .map_err(|source| QualificationFixtureFailure::message(source.to_string()))?;
        let executables = self.run_executables()?;
        let stim_command = outputs::prepare_command(self.root, &row, "qualification-stim")
            .map_err(|source| QualificationFixtureFailure::message(source.to_string()))?;
        let stab_command = outputs::prepare_command(self.root, &row, "qualification-stab")
            .map_err(|source| QualificationFixtureFailure::message(source.to_string()))?;
        let stim = run_prepared_fixture_process_with_timeout(
            self.root,
            &executables.stim(),
            &stim_command,
            &stdin,
            timeout,
            executables.environment(),
        );
        let stab = run_prepared_fixture_process_with_timeout(
            self.root,
            &executables.stab(),
            &stab_command,
            &stdin,
            timeout,
            executables.environment(),
        );
        let completion = completed_statistical_work(
            &row,
            stim.as_ref().ok(),
            stab.as_ref().ok(),
            &stim_command.outputs,
            &stab_command.outputs,
        )?;
        let (stim, stab) = match (stim, stab) {
            (Ok(stim), Ok(stab)) => (stim, stab),
            (Err(stim_error), Ok(_)) => {
                return Err(QualificationFixtureFailure::oracle_with_completion(
                    stim_error, completion,
                ));
            }
            (Ok(_), Err(stab_error)) => {
                return Err(QualificationFixtureFailure::oracle_with_completion(
                    stab_error, completion,
                ));
            }
            (Err(stim_error), Err(stab_error)) => {
                return Err(QualificationFixtureFailure::message(format!(
                    "Stim and Stab statistical processes both failed; Stim: {stim_error}; Stab: {stab_error}"
                )));
            }
        };
        compare_fixture(&row, &stim, &stab).map_err(|source| {
            QualificationFixtureFailure::process_with_statistical_completion(
                source.to_string(),
                &stab,
                completion,
            )
        })?;
        outputs::compare_outputs(&row, &stim_command.outputs, &stab_command.outputs).map_err(
            |source| {
                QualificationFixtureFailure::process_with_statistical_completion(
                    source.to_string(),
                    &stab,
                    completion,
                )
            },
        )?;
        Ok(stab)
    }

    pub(crate) fn selector_sha256(&self, id: &str) -> Result<String, QualificationFixtureFailure> {
        let row = self
            .manifest
            .rows
            .iter()
            .find(|row| row.id == id)
            .ok_or_else(|| {
                QualificationFixtureFailure::message(format!(
                    "qualification fixture {id} is absent from the validated manifest"
                ))
            })?;
        let stdin = row
            .stdin(self.root)
            .map_err(|source| QualificationFixtureFailure::message(source.to_string()))?;
        let expected_stdout = if row.expected_stdout_path.is_empty() {
            Vec::new()
        } else {
            super::paths::read_fixture_file(self.root, &row.expected_stdout_path)
                .map_err(|source| QualificationFixtureFailure::message(source.to_string()))?
        };
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, b"stab-cq1-fixture-selector-v1");
        for field in [
            row.id.as_bytes(),
            row.milestone.as_str().as_bytes(),
            row.upstream_source.as_bytes(),
            row.parity_mode.as_str().as_bytes(),
            row.comparator.as_str().as_bytes(),
            row.command_shape.as_bytes(),
            row.argv.as_bytes(),
            row.stdin_path.as_bytes(),
            row.expected_stdout_path.as_bytes(),
            row.expected_stderr_class.as_str().as_bytes(),
            row.status.as_str().as_bytes(),
            row.statistical_plan.as_bytes(),
        ] {
            hash_field(&mut hasher, field);
        }
        hash_field(&mut hasher, &row.expected_status.to_le_bytes());
        hash_field(&mut hasher, &stdin);
        hash_field(&mut hasher, &expected_stdout);
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn run_executables(
        &self,
    ) -> Result<
        &'a crate::qualification::executables::QualificationExecutables,
        QualificationFixtureFailure,
    > {
        self.executables.ok_or_else(|| {
            QualificationFixtureFailure::message(
                "qualification fixture execution has no pinned executable set",
            )
        })
    }
}

pub(super) fn completed_statistical_work(
    row: &super::FixtureRow,
    stim: Option<&crate::ProcessOutput>,
    stab: Option<&crate::ProcessOutput>,
    stim_outputs: &[outputs::FixtureOutput],
    stab_outputs: &[outputs::FixtureOutput],
) -> Result<StatisticalCompletion, QualificationFixtureFailure> {
    if row.comparator != FixtureComparator::Statistical {
        return Ok(StatisticalCompletion::default());
    }
    let Ok(source) = super::statistical::source_for_plan(&row.statistical_plan) else {
        return Ok(StatisticalCompletion::default());
    };
    let completed_side = |process: Option<&crate::ProcessOutput>,
                          output: &[outputs::FixtureOutput]| {
        let process = process?;
        match source {
            super::statistical::StatisticalSource::Stdout => {
                super::statistical::completed_shots(&row.statistical_plan, &process.stdout.bytes)
            }
            super::statistical::StatisticalSource::FixtureOutput => {
                outputs::completed_statistical_shots_for_output(row, output)
            }
        }
    };
    let mut completion = StatisticalCompletion::default();
    for shots in [
        completed_side(stim, stim_outputs),
        completed_side(stab, stab_outputs),
    ]
    .into_iter()
    .flatten()
    {
        completion.shots = completion.shots.checked_add(shots).ok_or_else(|| {
            QualificationFixtureFailure::message("completed statistical shot count overflowed")
        })?;
        completion.comparisons = completion.comparisons.checked_add(1).ok_or_else(|| {
            QualificationFixtureFailure::message(
                "completed statistical comparison count overflowed",
            )
        })?;
        completion.batches = completion.batches.checked_add(1).ok_or_else(|| {
            QualificationFixtureFailure::message("completed statistical batch count overflowed")
        })?;
    }
    Ok(completion)
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(bytes.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(bytes);
}

pub(crate) fn run_qualification_core_worker(
    root: &RepoRoot,
    id: &str,
) -> Result<crate::ProcessOutput, FixtureError> {
    let manifest = FixtureManifest::read(root)?;
    manifest.check(root)?;
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id == id)
        .ok_or_else(|| FixtureError::QualificationCase {
            id: id.to_string(),
            reason: "fixture id is absent from the validated manifest".to_string(),
        })?;
    if row.status != FixtureStatus::Implemented || !is_core_fixture(row) {
        return Err(FixtureError::QualificationCase {
            id: id.to_string(),
            reason: "qualification worker requires an implemented core fixture".to_string(),
        });
    }
    run_core_fixture(root, row)
}

pub(crate) fn qualification_exact_cargo_selectors(
    root: &RepoRoot,
) -> Result<Vec<(String, Vec<String>)>, FixtureError> {
    let manifest = FixtureManifest::read(root)?;
    manifest.check(root)?;
    manifest
        .rows
        .iter()
        .filter(|row| row.status == FixtureStatus::Implemented && is_direct_rust_fixture(row))
        .filter_map(|row| {
            match crate::blocker_ledger::selector::CargoTestSelector::normalize_fixture_argv(
                &row.argv,
            ) {
                Ok(Some(selector)) => Some(Ok((row.id.clone(), selector))),
                Ok(None) => None,
                Err(reason) => Some(Err(reason)),
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|reason| FixtureError::Validation(reason.into()))
}

pub(crate) fn qualification_expected_statuses(
    root: &RepoRoot,
) -> Result<BTreeMap<String, i32>, FixtureError> {
    let manifest = FixtureManifest::read(root)?;
    manifest.check(root)?;
    Ok(manifest
        .rows
        .iter()
        .filter(|row| row.status == FixtureStatus::Implemented)
        .map(|row| (row.id.clone(), row.expected_status))
        .collect())
}

pub(crate) fn qualification_statistical_plan_summaries(
    root: &RepoRoot,
) -> Result<Vec<super::FixtureStatisticalPlanSummary>, FixtureError> {
    let manifest = FixtureManifest::read(root)?;
    manifest
        .rows
        .iter()
        .filter(|row| {
            row.comparator == FixtureComparator::Statistical
                && row.status == FixtureStatus::Implemented
        })
        .map(|row| {
            let argv_tokens = row.argv_tokens();
            super::statistical::qualification_plan_summary(
                &row.id,
                &row.statistical_plan,
                &argv_tokens,
            )
            .map_err(|reason| {
                FixtureError::Validation(
                    format!(
                        "{} invalid qualification statistical plan: {reason}",
                        row.id
                    )
                    .into_boxed_str(),
                )
            })
        })
        .collect()
}

fn run_core_fixture_in_worker(
    root: &RepoRoot,
    executables: &crate::qualification::executables::QualificationExecutables,
    row: &super::FixtureRow,
    timeout: Duration,
) -> Result<crate::ProcessOutput, QualificationFixtureFailure> {
    let args = vec![
        OsString::from("--root"),
        root.path.as_os_str().to_owned(),
        OsString::from("qualification"),
        OsString::from("correctness"),
        OsString::from("worker"),
        OsString::from("fixture"),
        OsString::from("--id"),
        OsString::from(&row.id),
    ];
    let mut output = crate::process::run_qualification_process_with_timeout(
        &executables.worker(),
        &args,
        &[],
        Some(root.path.as_path()),
        timeout,
        executables.environment(),
    )
    .map_err(QualificationFixtureFailure::oracle)?;
    if restore_fixture_contract_status(&mut output, row.expected_status) {
        Ok(output)
    } else {
        Err(QualificationFixtureFailure::process(
            format!(
                "qualification worker failed with status {}: {}",
                crate::process::display_status(output.status),
                output.stderr.render_for_diagnostics()
            ),
            &output,
        ))
    }
}

fn restore_fixture_contract_status(
    worker_output: &mut crate::ProcessOutput,
    expected_status: i32,
) -> bool {
    if !worker_output.success() {
        return false;
    }
    worker_output.status = Some(expected_status);
    true
}

fn run_direct_rust_qualification(
    root: &RepoRoot,
    executables: &crate::qualification::executables::QualificationExecutables,
    row: &super::FixtureRow,
    timeout: Duration,
) -> Result<crate::ProcessOutput, QualificationFixtureFailure> {
    let tokens = row.argv_tokens();
    let args = std::iter::once("test").chain(tokens.iter().skip(1).map(String::as_str));
    let output = crate::process::run_qualification_process_with_timeout(
        &executables.cargo(),
        args,
        &[],
        Some(root.path.as_path()),
        timeout,
        executables.environment(),
    )
    .map_err(QualificationFixtureFailure::oracle)?;
    check_expected_process_shape(row, &output)
        .map_err(|source| QualificationFixtureFailure::process(source.to_string(), &output))?;
    check_direct_rust_fixture_executed_tests(row, &output)
        .map_err(|source| QualificationFixtureFailure::process(source.to_string(), &output))?;
    if matches!(
        row.comparator,
        FixtureComparator::Property
            | FixtureComparator::Statistical
            | FixtureComparator::Structural
    ) {
        Ok(output)
    } else {
        Err(QualificationFixtureFailure::process(
            "direct Rust fixture has a non-test comparator",
            &output,
        ))
    }
}

fn run_prepared_fixture_process_with_timeout(
    root: &RepoRoot,
    program: &std::path::Path,
    command: &outputs::PreparedFixtureCommand,
    stdin: &[u8],
    timeout: Duration,
    environment: &[(OsString, OsString)],
) -> Result<crate::ProcessOutput, crate::OracleError> {
    let monitored_files = command
        .outputs
        .iter()
        .map(outputs::FixtureOutput::monitored_file)
        .collect::<Vec<_>>();
    crate::process::run_qualification_process_with_timeout_and_monitored_files(
        program,
        &command.argv,
        stdin,
        Some(&root.path),
        timeout,
        &monitored_files,
        outputs::AUXILIARY_OUTPUT_LIMIT_BYTES,
        environment,
    )
}

#[cfg(test)]
mod tests {
    use crate::{CapturedOutput, ProcessOutput};

    #[test]
    fn seed_override_replaces_only_the_declared_seed_value() {
        assert_eq!(
            super::super::statistical::argv_with_seed(
                "sample|--shots|10|--seed|5|--out_format|01",
                17
            ),
            Some("sample|--shots|10|--seed|17|--out_format|01".to_string())
        );
        assert_eq!(
            super::super::statistical::argv_with_seed(
                "sample|--shots|10|--seed=5|--out_format|01",
                17
            ),
            Some("sample|--shots|10|--seed=17|--out_format|01".to_string())
        );
        assert_eq!(
            super::super::statistical::argv_with_seed("sample|--shots|10", 17),
            None
        );
    }

    #[test]
    fn successful_worker_transport_restores_the_fixture_process_status() {
        let mut output = ProcessOutput {
            status: Some(0),
            stdout: CapturedOutput {
                bytes: b"contract output".to_vec(),
                truncated: false,
            },
            stderr: CapturedOutput {
                bytes: b"contract error".to_vec(),
                truncated: false,
            },
        };

        assert!(super::restore_fixture_contract_status(&mut output, 1));
        assert_eq!(output.status, Some(1));
        assert_eq!(output.stdout.bytes, b"contract output");
        assert_eq!(output.stderr.bytes, b"contract error");
    }
}
