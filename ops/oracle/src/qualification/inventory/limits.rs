use super::{InventoryError, MAX_CASES};

pub(super) fn ensure_limit(kind: &'static str, actual: usize) -> Result<(), InventoryError> {
    if actual > MAX_CASES {
        Err(InventoryError::TooManyRecords {
            kind,
            actual,
            limit: MAX_CASES,
        })
    } else {
        Ok(())
    }
}
