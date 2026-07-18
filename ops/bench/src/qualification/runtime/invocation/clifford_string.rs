use std::collections::BTreeSet;
use std::ffi::OsString;

use super::super::clifford_vectors::{
    CliffordRequestResult, CliffordRequestVector, checked_file, request_for_runtime,
};
use super::super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use super::super::protocol::{
    EvidenceMode, GitCommit, Implementation, InputDigest, ProtocolExpectation, ProtocolId,
    SemanticDigest, parse_worker_json_lines,
};
use super::super::worker::clifford_string::{
    CLIFFORD_DESCRIPTOR_BYTES, CLIFFORD_FIXTURE_SCHEMA, CLIFFORD_GATE_COUNT,
    CLIFFORD_NON_IDENTITY_CYCLE, CLIFFORD_PUBLIC_CAP, CliffordDescriptor, CliffordWorkloadKind,
};
use super::preflight::{
    WorkerContractProbeEvidence, accepted_probe, expected_accepted_probe, expected_rejected_probe,
    rejected_probe,
};
use super::{
    CAP_REJECTION_TIMEOUT, IDENTITY_PROBE_TIMEOUT, InvocationError, PROTOCOL_OUTPUT_LIMIT,
    PreparedWorkers, checked_process, worker_environment,
};
use crate::config::STIM_COMMIT;

pub(in crate::qualification::runtime) const CLIFFORD_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING";
pub(in crate::qualification::runtime) const CLIFFORD_NON_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";

pub(super) fn runtime_descriptor(
    group_id: &str,
    workload: &str,
    width: u64,
) -> Result<Option<String>, InvocationError> {
    if group_id != CLIFFORD_IDENTITY_GROUP_ID && group_id != CLIFFORD_NON_IDENTITY_GROUP_ID {
        return Ok(None);
    }
    let file = checked_file().map_err(InvocationError::CliffordVectorContract)?;
    let request = request_for_runtime(file, workload, width)
        .map_err(InvocationError::CliffordVectorContract)?;
    Ok(Some(request.descriptor_hex.clone()))
}

pub(super) fn expected_clifford_probes() -> Result<Vec<WorkerContractProbeEvidence>, InvocationError>
{
    let file = checked_file().map_err(InvocationError::CliffordVectorContract)?;
    let mut probes = Vec::with_capacity(62);
    for implementation in [Implementation::Stab, Implementation::Stim] {
        for request in &file.requests {
            match request.result {
                CliffordRequestResult::Accepted => {
                    let output_digest = request.output_sha256.as_deref().ok_or_else(|| {
                        InvocationError::CliffordVectorContract(format!(
                            "accepted Clifford request {} lacks output_sha256",
                            request.id
                        ))
                    })?;
                    probes.push(expected_accepted_probe(
                        &request.id,
                        implementation,
                        request.iterations,
                        request
                            .iterations
                            .checked_mul(request.work_items)
                            .ok_or(InvocationError::WorkOverflow)?,
                        CLIFFORD_DESCRIPTOR_BYTES,
                        &request.input_sha256,
                        output_digest,
                    )?);
                }
                CliffordRequestResult::Rejected => {
                    let (status, stderr) = clifford_rejection_expectation(implementation, request)?;
                    probes.push(expected_rejected_probe(
                        &request.id,
                        implementation,
                        status,
                        &stderr,
                    )?);
                }
            }
        }
    }
    Ok(probes)
}

impl PreparedWorkers {
    pub(super) fn invoke_clifford_contract_probes(
        &self,
    ) -> Result<Vec<WorkerContractProbeEvidence>, InvocationError> {
        let file = checked_file().map_err(InvocationError::CliffordVectorContract)?;
        let mut probes = Vec::with_capacity(62);
        for implementation in [Implementation::Stab, Implementation::Stim] {
            for request in &file.requests {
                probes.push(match request.result {
                    CliffordRequestResult::Accepted => {
                        self.invoke_clifford_acceptance(implementation, request)?
                    }
                    CliffordRequestResult::Rejected => {
                        self.invoke_clifford_rejection(implementation, request)?
                    }
                });
            }
        }
        if probes.len() != 62 {
            return Err(InvocationError::CliffordVectorContract(format!(
                "Clifford preflight produced {} receipts, expected 62",
                probes.len()
            )));
        }
        Ok(probes)
    }

    fn invoke_clifford_acceptance(
        &self,
        implementation: Implementation,
        request: &CliffordRequestVector,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = clifford_arguments(request);
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
        let process = run_bounded_process(&ProcessRequest {
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
                timeout: IDENTITY_PROBE_TIMEOUT,
            },
        })?;
        let process = checked_process(process, implementation)?;
        let rows = parse_worker_json_lines(&process.stdout)?;
        let output_digest = request.output_sha256.as_deref().ok_or_else(|| {
            InvocationError::CliffordVectorContract(format!(
                "accepted Clifford request {} lacks output_sha256",
                request.id
            ))
        })?;
        ProtocolExpectation {
            implementation,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new(request.workload.clone())?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new(request.measurement_id.clone())?]),
            iteration_count: request.iterations,
            expected_work_count: request
                .iterations
                .checked_mul(request.work_items)
                .ok_or(InvocationError::WorkOverflow)?,
            expected_input_bytes: CLIFFORD_DESCRIPTOR_BYTES,
            expected_input_digest: InputDigest::try_new(request.input_sha256.clone())?,
            expected_output_digest: Some(SemanticDigest::try_new(output_digest.to_string())?),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        let row = rows
            .into_iter()
            .next()
            .ok_or(InvocationError::MissingMeasurement)?;
        accepted_probe(&request.id, &row)
    }

    fn invoke_clifford_rejection(
        &self,
        implementation: Implementation,
        request: &CliffordRequestVector,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = clifford_arguments(request);
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
                timeout: CAP_REJECTION_TIMEOUT,
            },
        })?;
        checked_clifford_rejection(&output, implementation, request)?;
        rejected_probe(&request.id, implementation, &output)
    }
}

pub(in crate::qualification::runtime) fn clifford_arguments(
    request: &CliffordRequestVector,
) -> Vec<OsString> {
    vec![
        OsString::from("--workload"),
        OsString::from(&request.workload),
        OsString::from("--measurement-id"),
        OsString::from(&request.measurement_id),
        OsString::from("--iterations"),
        OsString::from(request.iterations.to_string()),
        OsString::from("--work-items"),
        OsString::from(request.work_items.to_string()),
        OsString::from("--input-descriptor-hex"),
        OsString::from(&request.descriptor_hex),
        OsString::from("--evidence-mode"),
        OsString::from("timing"),
        OsString::from("--start-barrier"),
        OsString::from("true"),
    ]
}

pub(in crate::qualification::runtime) fn checked_clifford_rejection(
    output: &ProcessResult,
    implementation: Implementation,
    request: &CliffordRequestVector,
) -> Result<(), InvocationError> {
    let (expected_status, expected_stderr) =
        clifford_rejection_expectation(implementation, request)?;
    if output.status != Some(expected_status)
        || !output.stdout.is_empty()
        || output.stderr != expected_stderr.as_bytes()
    {
        return Err(InvocationError::CliffordWorkRejection {
            implementation,
            case_id: request.id.clone(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

pub(super) fn clifford_rejection_expectation(
    implementation: Implementation,
    request: &CliffordRequestVector,
) -> Result<(i32, String), InvocationError> {
    let class = request.expected_rejection_class.as_deref().ok_or_else(|| {
        InvocationError::CliffordVectorContract(format!(
            "rejected Clifford request {} lacks a rejection class",
            request.id
        ))
    })?;
    let kind = kind_from_workload(&request.workload)?;
    let descriptor = request
        .descriptor_hex
        .parse::<CliffordDescriptor>()
        .map_err(|error| InvocationError::CliffordVectorContract(error.to_string()))?;
    let [
        width,
        marker,
        schema,
        gate_count,
        cycle_count,
        complete_span,
        public_cap,
        reserved,
    ] = descriptor.fields();
    let message = match class {
        "width-limit" => {
            format!("Clifford-string width {width} exceeds maximum {CLIFFORD_PUBLIC_CAP}")
        }
        "zero-width" => "Clifford-string width must be positive".to_string(),
        "unknown-marker" => {
            format!("Clifford-string descriptor has unknown workload marker {marker}")
        }
        "wrong-measurement" => match implementation {
            Implementation::Stim => {
                "adapter workload and measurement are not a registered pair".to_string()
            }
            Implementation::Stab => format!(
                "qualification workload {} requires measurement {}, got {}",
                kind.workload(),
                kind.measurement(),
                request.measurement_id
            ),
        },
        "fixture-schema" => field_message("fixture schema", schema, CLIFFORD_FIXTURE_SCHEMA),
        "gate-count" => field_message("canonical gate count", gate_count, CLIFFORD_GATE_COUNT),
        "cycle-count" => field_message(
            "right-cycle count",
            cycle_count,
            match kind {
                CliffordWorkloadKind::Identity => 0,
                CliffordWorkloadKind::NonIdentity => CLIFFORD_NON_IDENTITY_CYCLE,
            },
        ),
        "cross-product-span" => field_message(
            "complete cross-product span",
            complete_span,
            match kind {
                CliffordWorkloadKind::Identity => 0,
                CliffordWorkloadKind::NonIdentity => 552,
            },
        ),
        "public-cap" => field_message("public Clifford-qubit cap", public_cap, CLIFFORD_PUBLIC_CAP),
        "reserved" => field_message("reserved field", reserved, 0),
        "work-overflow" => match implementation {
            Implementation::Stim => "adapter semantic work count overflows u64".to_string(),
            Implementation::Stab => {
                "qualification worker semantic work count overflows u64".to_string()
            }
        },
        _ => {
            return Err(InvocationError::CliffordVectorContract(format!(
                "unknown Clifford rejection class {class}"
            )));
        }
    };
    Ok(match implementation {
        Implementation::Stim => (2, format!("stim qualification adapter: {message}\n")),
        Implementation::Stab => (
            1,
            format!(
                "[stab-bench] ERROR: performance qualification validation failed:\n{message}\n"
            ),
        ),
    })
}

fn kind_from_workload(workload: &str) -> Result<CliffordWorkloadKind, InvocationError> {
    if workload == CliffordWorkloadKind::Identity.workload() {
        Ok(CliffordWorkloadKind::Identity)
    } else if workload == CliffordWorkloadKind::NonIdentity.workload() {
        Ok(CliffordWorkloadKind::NonIdentity)
    } else {
        Err(InvocationError::CliffordVectorContract(format!(
            "unknown Clifford workload {workload}"
        )))
    }
}

fn field_message(name: &str, actual: u64, expected: u64) -> String {
    format!("Clifford-string descriptor {name} is {actual}, expected {expected}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_descriptor_is_exactly_the_checked_scale_descriptor() {
        for (group, kind) in [
            (CLIFFORD_IDENTITY_GROUP_ID, CliffordWorkloadKind::Identity),
            (
                CLIFFORD_NON_IDENTITY_GROUP_ID,
                CliffordWorkloadKind::NonIdentity,
            ),
        ] {
            for width in [10_000, 100_000, 1_000_000] {
                assert_eq!(
                    runtime_descriptor(group, kind.workload(), width).expect("descriptor"),
                    Some(CliffordDescriptor::canonical(kind, width).to_string())
                );
            }
        }
    }

    #[test]
    fn rejection_expectations_cover_every_checked_rejection() {
        let file = checked_file().expect("checked vectors");
        for request in file
            .requests
            .iter()
            .filter(|request| request.result == CliffordRequestResult::Rejected)
        {
            for implementation in [Implementation::Stab, Implementation::Stim] {
                let (status, stderr) =
                    clifford_rejection_expectation(implementation, request).expect("expectation");
                assert_eq!(
                    status,
                    if implementation == Implementation::Stab {
                        1
                    } else {
                        2
                    }
                );
                assert!(!stderr.is_empty());
            }
        }
    }
}
