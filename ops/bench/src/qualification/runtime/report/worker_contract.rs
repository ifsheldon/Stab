use super::super::invocation::{WorkerContractPreflightEvidence, WorkerIdentityEvidence};
use super::ReportError;

pub(in crate::qualification::runtime) fn validate(
    evidence: &WorkerContractPreflightEvidence,
    workers: &WorkerIdentityEvidence,
    root: &crate::root::RepoRoot,
    performance_inventory_sha256: &str,
) -> Result<(), ReportError> {
    let contracts = super::super::group::load_groups(root, performance_inventory_sha256)?;
    if !evidence.validates_source_contract(performance_inventory_sha256, &contracts)
        || workers.contract_preflight_sha256 != evidence.sha256()
        || !evidence.validates_worker_identity(workers)
    {
        return Err(ReportError::WorkerReceipt);
    }
    Ok(())
}
