use crate::error::BenchError;

pub(super) fn require_exact<T: PartialEq + ?Sized>(
    row_id: &str,
    contract: &str,
    actual: &T,
    expected: &T,
) -> Result<(), BenchError> {
    if actual == expected {
        Ok(())
    } else {
        Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!("{contract} semantic preflight produced wrong content"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::require_exact;

    #[test]
    fn exact_preflight_rejects_same_width_wrong_content() {
        let error = require_exact(
            "test-row",
            "canonical output",
            b"abce".as_slice(),
            b"abcd".as_slice(),
        )
        .expect_err("same-width mutation must fail");
        assert!(error.to_string().contains("wrong content"));
    }
}
