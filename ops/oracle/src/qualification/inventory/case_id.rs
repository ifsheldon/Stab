use sha2::{Digest, Sha256};

use crate::qualification::model::{CaseId, StableCaseDomain};

pub(in crate::qualification) fn stable_id(domain: StableCaseDomain, key: &str) -> CaseId {
    let digest = Sha256::digest(key.as_bytes());
    let suffix = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    CaseId::from_stable_suffix(domain, &suffix)
}
