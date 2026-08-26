use std::collections::BTreeMap;

use super::{Ledger, ParityError, TestOwner, Tier, invalid};
use crate::process::run_checked;
use crate::{OracleError, RepoRoot};

pub(super) fn check_owner_selectors(root: &RepoRoot, ledger: &Ledger) -> Result<(), OracleError> {
    let tests = collect_owner_tests(ledger, None);
    check_test_listings(root, &tests)?;
    println!(
        "[stab-oracle] parity check resolved {} unique canonical owner tests",
        tests.len()
    );
    Ok(())
}

pub(super) fn run_owner_tests(
    root: &RepoRoot,
    ledger: &Ledger,
    tier: Tier,
) -> Result<(), OracleError> {
    let tests = collect_owner_tests(ledger, Some(tier));
    check_test_listings(root, &tests)?;
    for (display, (test, family_ids)) in &tests {
        println!(
            "[stab-oracle] parity {}: {} [{}]",
            tier.as_str(),
            display,
            family_ids.join(", ")
        );
        run_checked("cargo", test.run_args(), b"", Some(&root.path))?;
    }
    println!(
        "[stab-oracle] parity {} passed {} unique canonical owner tests",
        tier.as_str(),
        tests.len()
    );
    Ok(())
}

pub(super) fn collect_owner_tests(
    ledger: &Ledger,
    tier: Option<Tier>,
) -> BTreeMap<String, (&TestOwner, Vec<&str>)> {
    let mut tests = BTreeMap::<String, (&TestOwner, Vec<&str>)>::new();
    for family in &ledger.families {
        for test in [family.test(), family.stim_reproduction()]
            .into_iter()
            .flatten()
        {
            if tier.is_some_and(|selected| test.minimum_tier > selected) {
                continue;
            }
            let display = test.display();
            let (_, owners) = tests.entry(display).or_insert_with(|| (test, Vec::new()));
            owners.push(&family.id);
        }
    }
    tests
}

fn check_test_listings(
    root: &RepoRoot,
    tests: &BTreeMap<String, (&TestOwner, Vec<&str>)>,
) -> Result<(), OracleError> {
    let mut groups = BTreeMap::<String, Vec<&str>>::new();
    for (display, (test, _)) in tests {
        groups
            .entry(test.listing_group_key())
            .or_default()
            .push(display);
    }
    for displays in groups.values() {
        let first_display = displays
            .first()
            .ok_or_else(|| ParityError::InvalidLedger("empty selector listing group".into()))?;
        let (representative, _) = tests.get(*first_display).ok_or_else(|| {
            ParityError::InvalidLedger(
                format!("selector listing group lost {first_display}").into_boxed_str(),
            )
        })?;
        let listing = run_checked(
            "cargo",
            representative.listing_all_args(),
            b"",
            Some(&root.path),
        )?;
        let listing_stdout = String::from_utf8_lossy(&listing.stdout.bytes);
        for display in displays {
            let (test, family_ids) = tests.get(*display).ok_or_else(|| {
                ParityError::InvalidLedger(
                    format!("selector listing group lost {display}").into_boxed_str(),
                )
            })?;
            require_one_listing_match(display, &test.name, family_ids, &listing_stdout)?;
        }
    }
    Ok(())
}

pub(super) fn require_one_listing_match(
    display: &str,
    test_name: &str,
    family_ids: &[&str],
    stdout: &str,
) -> Result<(), ParityError> {
    let test_line = format!("{test_name}: test");
    let count = stdout.lines().filter(|line| *line == test_line).count();
    if count == 1 {
        Ok(())
    } else {
        Err(invalid(format!(
            "canonical owner selector {display} for [{}] resolved to {count} tests instead of exactly one",
            family_ids.join(", ")
        )))
    }
}
