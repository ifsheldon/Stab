use std::collections::BTreeSet;
use std::ffi::OsString;
use std::time::Duration;

use super::super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, parse_worker_json_lines,
};
use super::preflight::{
    WorkerContractProbeEvidence, accepted_probe, expected_accepted_probe, expected_rejected_probe,
    rejected_probe,
};
use super::{
    InvocationError, PROTOCOL_OUTPUT_LIMIT, PreparedWorkers, checked_process, worker_environment,
};
use crate::config::STIM_COMMIT;

pub(super) const SMALL_ITEMS: u64 = 64;
pub(super) const MAX_ITEMS: u64 = 524_288;
const FIRST_OVER_CAP_ITEMS: u64 = MAX_ITEMS + 1;
const SMALL_INPUT_BYTES: u64 = 1_776;
const MAX_INPUT_BYTES: u64 = 14_548_992;
const SMALL_INPUT_DIGEST: &str = "fe2dab309c0d63109124cbaae8fadfe7b72ec523bd1c2252e1a7fc20f1b0d773";
const MAX_INPUT_DIGEST: &str = "127e88c725aa88acdea3be1aed5369af43166e27365e1dbd11dbe89c8e807789";
const SMALL_OUTPUT_DIGEST: &str =
    "02ad6cd3910a69ae416bdaadeb16126fdf813aba8154bb682bf75a01c609093f";
const MAX_OUTPUT_DIGEST: &str = "5bd41410a3ee8859fa7589abe6a20fa61d4e5c06e08105f60a5f3aa474d478b2";
const ACCEPTED_TIMEOUT: Duration = Duration::from_secs(120);
const REJECTED_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
enum DemContractKind {
    Parse,
    Serialize,
}

impl DemContractKind {
    fn workload(self) -> &'static str {
        match self {
            Self::Parse => "dem-parse",
            Self::Serialize => "dem-canonical-print",
        }
    }

    fn measurement(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Serialize => "serialize",
        }
    }

    fn case_prefix(self) -> &'static str {
        match self {
            Self::Parse => "dem-parse",
            Self::Serialize => "dem-canonical-print",
        }
    }
}

impl PreparedWorkers {
    pub(super) fn invoke_dem_model_contract_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let mut probes = Vec::with_capacity(16);
        for implementation in [Implementation::Stab, Implementation::Stim] {
            for kind in [DemContractKind::Parse, DemContractKind::Serialize] {
                probes.push(self.invoke_dem_accepted(implementation, kind, 1, SMALL_ITEMS)?);
                probes.push(self.invoke_dem_accepted(implementation, kind, 2, SMALL_ITEMS)?);
                probes.push(self.invoke_dem_accepted(implementation, kind, 1, MAX_ITEMS)?);
                probes.push(self.invoke_dem_rejected(implementation, kind)?);
            }
        }
        Ok(probes)
    }

    fn invoke_dem_accepted(
        &self,
        implementation: Implementation,
        kind: DemContractKind,
        iterations: u64,
        items: u64,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = dem_arguments(kind, iterations, items, true);
        let (program, source_digest, build_fingerprint) = match implementation {
            Implementation::Stim => (
                self.adapter.path.clone(),
                self.adapter.source_digest.clone(),
                self.adapter.build_fingerprint.clone(),
            ),
            Implementation::Stab => {
                arguments.insert(0, OsString::from("qualification-worker"));
                (
                    self.worker.program(),
                    self.worker.identity().source_digest.clone(),
                    self.worker.identity().build_fingerprint.clone(),
                )
            }
        };
        let output = run_bounded_process(&ProcessRequest {
            program,
            args: arguments,
            stdin: vec![b'\n'],
            working_directory: self.root.clone(),
            environment: worker_environment(),
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 1,
                stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
                stderr_bytes: 64 << 10,
                regular_file_bytes: None,
                timeout: ACCEPTED_TIMEOUT,
            },
        })?;
        let output = checked_process(output, implementation)?;
        let rows = parse_worker_json_lines(&output.stdout)?;
        let (input_bytes, input_digest, output_digest) = if items == SMALL_ITEMS {
            (SMALL_INPUT_BYTES, SMALL_INPUT_DIGEST, SMALL_OUTPUT_DIGEST)
        } else {
            (MAX_INPUT_BYTES, MAX_INPUT_DIGEST, MAX_OUTPUT_DIGEST)
        };
        let work_count = iterations
            .checked_mul(items)
            .ok_or(InvocationError::WorkOverflow)?;
        ProtocolExpectation {
            implementation,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new(kind.workload())?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new(kind.measurement())?]),
            iteration_count: iterations,
            expected_work_count: work_count,
            expected_input_bytes: input_bytes,
            expected_input_digest: InputDigest::try_new(input_digest)?,
            expected_output_digest: Some(SemanticDigest::try_new(output_digest)?),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        let row = rows.first().ok_or(InvocationError::MissingMeasurement)?;
        accepted_probe(&accepted_case_id(kind, iterations, items), row)
    }

    fn invoke_dem_rejected(
        &self,
        implementation: Implementation,
        kind: DemContractKind,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = dem_arguments(kind, 1, FIRST_OVER_CAP_ITEMS, true);
        let program = match implementation {
            Implementation::Stim => self.adapter.path.clone(),
            Implementation::Stab => {
                arguments.insert(0, OsString::from("qualification-worker"));
                self.worker.program()
            }
        };
        let output = run_bounded_process(&ProcessRequest {
            program,
            args: arguments,
            stdin: Vec::new(),
            working_directory: self.root.clone(),
            environment: worker_environment(),
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 0,
                stdout_bytes: PROTOCOL_OUTPUT_LIMIT,
                stderr_bytes: 64 << 10,
                regular_file_bytes: None,
                timeout: REJECTED_TIMEOUT,
            },
        })?;
        verify_rejection(&output, implementation, kind)?;
        rejected_probe(&rejected_case_id(kind), implementation, &output)
    }
}

pub(super) fn expected_dem_model_probes()
-> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
    let mut probes = Vec::with_capacity(16);
    for implementation in [Implementation::Stab, Implementation::Stim] {
        for kind in [DemContractKind::Parse, DemContractKind::Serialize] {
            probes.push(expected_accepted_probe(
                &accepted_case_id(kind, 1, SMALL_ITEMS),
                implementation,
                1,
                SMALL_ITEMS,
                SMALL_INPUT_BYTES,
                SMALL_INPUT_DIGEST,
                SMALL_OUTPUT_DIGEST,
            )?);
            probes.push(expected_accepted_probe(
                &accepted_case_id(kind, 2, SMALL_ITEMS),
                implementation,
                2,
                SMALL_ITEMS * 2,
                SMALL_INPUT_BYTES,
                SMALL_INPUT_DIGEST,
                SMALL_OUTPUT_DIGEST,
            )?);
            probes.push(expected_accepted_probe(
                &accepted_case_id(kind, 1, MAX_ITEMS),
                implementation,
                1,
                MAX_ITEMS,
                MAX_INPUT_BYTES,
                MAX_INPUT_DIGEST,
                MAX_OUTPUT_DIGEST,
            )?);
            probes.push(expected_rejected_probe(
                &rejected_case_id(kind),
                implementation,
                rejection_status(implementation),
                rejection_stderr(implementation),
            )?);
        }
    }
    Ok(probes)
}

fn dem_arguments(
    kind: DemContractKind,
    iterations: u64,
    items: u64,
    start_barrier: bool,
) -> Vec<OsString> {
    vec![
        OsString::from("--workload"),
        OsString::from(kind.workload()),
        OsString::from("--measurement-id"),
        OsString::from(kind.measurement()),
        OsString::from("--iterations"),
        OsString::from(iterations.to_string()),
        OsString::from("--work-items"),
        OsString::from(items.to_string()),
        OsString::from("--evidence-mode"),
        OsString::from("timing"),
        OsString::from("--start-barrier"),
        OsString::from(start_barrier.to_string()),
    ]
}

fn accepted_case_id(kind: DemContractKind, iterations: u64, items: u64) -> String {
    let suffix = if items == MAX_ITEMS {
        "maximum"
    } else if iterations == 2 {
        "small-even"
    } else {
        "small-odd"
    };
    format!("{}-{suffix}", kind.case_prefix())
}

fn rejected_case_id(kind: DemContractKind) -> String {
    format!("{}-first-over-cap", kind.case_prefix())
}

fn rejection_stderr(implementation: Implementation) -> &'static str {
    match implementation {
        Implementation::Stim => {
            "stim qualification adapter: DEM model work count exceeds the source-owned limit\n"
        }
        Implementation::Stab => {
            "[stab-bench] ERROR: performance qualification validation failed:\nDEM model work count 524289 exceeds maximum 524288\n"
        }
    }
}

const fn rejection_status(implementation: Implementation) -> i32 {
    match implementation {
        Implementation::Stim => 2,
        Implementation::Stab => 1,
    }
}

fn verify_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    kind: DemContractKind,
) -> Result<(), InvocationError> {
    if output.status == Some(rejection_status(implementation))
        && output.stdout.is_empty()
        && output.stderr == rejection_stderr(implementation).as_bytes()
    {
        Ok(())
    } else {
        Err(InvocationError::DemWorkRejection {
            implementation,
            workload: kind.workload(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_matrix_has_exact_order_and_shape() {
        let probes = expected_dem_model_probes().expect("DEM probe contract");
        assert_eq!(probes.len(), 16);
        assert!(matches!(
            probes.first(),
            Some(WorkerContractProbeEvidence::Accepted { case_id, implementation: Implementation::Stab, .. })
                if case_id.to_string() == "dem-parse-small-odd"
        ));
        assert!(matches!(
            probes.last(),
            Some(WorkerContractProbeEvidence::Rejected { case_id, implementation: Implementation::Stim, .. })
                if case_id.to_string() == "dem-canonical-print-first-over-cap"
        ));
    }

    #[test]
    fn first_over_cap_receipt_requires_exact_pre_barrier_process_failure() {
        let output = |status, stderr: &str| ProcessResult {
            status: Some(status),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
            parent_observed_peak_rss_bytes: None,
            wall_elapsed: Duration::from_millis(1),
        };
        for implementation in [Implementation::Stim, Implementation::Stab] {
            verify_rejection(
                &output(
                    rejection_status(implementation),
                    rejection_stderr(implementation),
                ),
                implementation,
                DemContractKind::Parse,
            )
            .expect("exact rejection");
        }
        assert!(matches!(
            verify_rejection(
                &output(
                    2,
                    "stim qualification adapter: start barrier must contain exactly one newline\n"
                ),
                Implementation::Stim,
                DemContractKind::Parse,
            ),
            Err(InvocationError::DemWorkRejection { .. })
        ));
    }
}
