use super::{
    BTreeSet, EvidenceMode, GitCommit, IDENTITY_PROBE_TIMEOUT, Implementation, InputDigest,
    InvocationError, OsString, PROTOCOL_OUTPUT_LIMIT, PreparedWorkers, ProcessLimits,
    ProcessRequest, ProtocolExpectation, ProtocolId, STIM_COMMIT, SemanticDigest,
    WorkerContractProbeEvidence, accepted_probe, checked_process, parse_worker_json_lines,
    run_bounded_process, worker_environment,
};

pub(super) struct BitAcceptanceContract<'a> {
    pub(super) case_id: &'static str,
    pub(super) workload: &'static str,
    pub(super) measurement: &'static str,
    pub(super) iterations: u64,
    pub(super) work_items: u64,
    pub(super) input_bytes: u64,
    pub(super) expected_input_digest: &'a InputDigest,
    pub(super) expected_output_digest: &'a SemanticDigest,
}

impl PreparedWorkers {
    pub(super) fn invoke_bit_acceptance(
        &self,
        implementation: Implementation,
        contract: BitAcceptanceContract<'_>,
    ) -> Result<WorkerContractProbeEvidence, InvocationError> {
        let mut arguments = vec![
            OsString::from("--workload"),
            OsString::from(contract.workload),
            OsString::from("--measurement-id"),
            OsString::from(contract.measurement),
            OsString::from("--iterations"),
            OsString::from(contract.iterations.to_string()),
            OsString::from("--work-items"),
            OsString::from(contract.work_items.to_string()),
            OsString::from("--evidence-mode"),
            OsString::from("contract"),
            OsString::from("--start-barrier"),
            OsString::from("true"),
        ];
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
        ProtocolExpectation {
            implementation,
            evidence_mode: EvidenceMode::Contract,
            workload_id: ProtocolId::try_new(contract.workload)?,
            measurement_ids: BTreeSet::from([ProtocolId::try_new(contract.measurement)?]),
            iteration_count: contract.iterations,
            expected_work_count: contract
                .iterations
                .checked_mul(contract.work_items)
                .ok_or(InvocationError::WorkOverflow)?,
            expected_input_bytes: contract.input_bytes,
            expected_input_digest: contract.expected_input_digest.clone(),
            expected_output_digest: Some(contract.expected_output_digest.clone()),
            affinity_cpu: None,
            stim_commit: GitCommit::try_new(STIM_COMMIT)?,
            source_digest,
            build_fingerprint,
        }
        .validate(&rows)?;
        let row = rows.first().ok_or(InvocationError::MissingMeasurement)?;
        accepted_probe(contract.case_id, row)
    }
}
