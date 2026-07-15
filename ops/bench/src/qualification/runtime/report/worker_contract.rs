use super::super::invocation::{WorkerContractPreflightEvidence, WorkerIdentityEvidence};
use super::ReportError;

pub(in crate::qualification::runtime) fn validate(
    evidence: &WorkerContractPreflightEvidence,
    workers: &WorkerIdentityEvidence,
) -> Result<(), ReportError> {
    if !evidence.validates_source_contract()
        || workers.contract_preflight_sha256 != evidence.sha256()
        || !evidence.validates_worker_identity(workers)
    {
        return Err(ReportError::WorkerReceipt);
    }
    Ok(())
}
