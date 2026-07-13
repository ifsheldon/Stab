use std::path::Path;
use std::time::Duration;

use super::{FixtureComparator, FixtureError, FixtureRow, RepoRoot, check_expected_process_shape};

pub(super) fn is_direct_rust_fixture(row: &FixtureRow) -> bool {
    row.argv_tokens()
        .first()
        .is_some_and(|token| token == "cargo-test")
}

pub(super) fn run_direct_rust_fixture(
    root: &RepoRoot,
    row: &FixtureRow,
) -> Result<crate::ProcessOutput, FixtureError> {
    run_direct_rust_fixture_with_timeout(root, row, Duration::from_secs(120))
}

pub(super) fn run_direct_rust_fixture_with_timeout(
    root: &RepoRoot,
    row: &FixtureRow,
    timeout: Duration,
) -> Result<crate::ProcessOutput, FixtureError> {
    let tokens = row.argv_tokens();
    let args = std::iter::once("test").chain(tokens.iter().skip(1).map(String::as_str));
    let output = crate::process::run_process_with_timeout(
        Path::new("cargo"),
        args,
        &[],
        Some(&root.path),
        timeout,
    )
    .map_err(|source| FixtureError::CoreFixtureFailed {
        id: row.id.clone(),
        reason: source.to_string(),
    })?;
    check_expected_process_shape(row, &output)?;
    check_direct_rust_fixture_executed_tests(row, &output)?;
    match row.comparator {
        FixtureComparator::Property
        | FixtureComparator::Statistical
        | FixtureComparator::Structural => Ok(output),
        FixtureComparator::ExactOutput | FixtureComparator::HelpHealth => {
            Err(FixtureError::ComparatorMismatch {
                id: row.id.clone(),
                comparator: row.comparator.as_str(),
                reason: "direct Rust fixtures only support test-like comparators".to_string(),
            })
        }
    }
}

pub(super) fn check_direct_rust_fixture_executed_tests(
    row: &FixtureRow,
    output: &crate::ProcessOutput,
) -> Result<(), FixtureError> {
    if output.status == Some(0) {
        let passed = cargo_test_passed_test_count(output);
        let exact = row.argv_tokens().iter().any(|token| token == "--exact");
        if passed == 0 {
            return Err(FixtureError::CoreFixtureFailed {
                id: row.id.clone(),
                reason: "direct Rust fixture passed zero cargo tests; check for an empty filter or ignored test"
                    .to_string(),
            });
        }
        if exact && passed != 1 {
            return Err(FixtureError::CoreFixtureFailed {
                id: row.id.clone(),
                reason: format!(
                    "exact direct Rust fixture passed {passed} cargo tests instead of exactly one"
                ),
            });
        }
    }
    Ok(())
}

pub(crate) fn cargo_test_passed_test_count(output: &crate::ProcessOutput) -> usize {
    count_cargo_test_passed_lines(&output.stdout.bytes)
        + count_cargo_test_passed_lines(&output.stderr.bytes)
}

fn count_cargo_test_passed_lines(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("test result:")
                .and_then(|summary| summary.split(';').next())
                .and_then(|passed| passed.trim().strip_suffix(" passed"))
                .and_then(|prefix| prefix.split_whitespace().next_back())
                .and_then(|count| count.parse::<usize>().ok())
        })
        .sum()
}
