//! Ledger validation against the expected coverage and the pinned Stim source.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use super::{
    CargoTestTarget, Disposition, DivergenceKind, Evidence, ExpectedCoverage, Family,
    LEDGER_SCHEMA_VERSION, Ledger, Milestone, PRODUCT_OWNERS, ParityError, RouteOutcome, Status,
    TEST_PACKAGES, TestOwner, invalid, is_expected_command_option, validate_command_surfaces,
    validate_format_routes,
};
use crate::{RepoRoot, STIM_COMMIT, STIM_TAG, safe_file};

pub(super) fn validate(
    root: &RepoRoot,
    ledger: &Ledger,
    expected: &ExpectedCoverage,
) -> Result<(), ParityError> {
    let mut errors = Vec::new();
    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        errors.push(format!(
            "schema_version is {}, expected {LEDGER_SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    if ledger.stim.version != STIM_TAG {
        errors.push(format!(
            "stim.version is {}, expected {STIM_TAG}",
            ledger.stim.version
        ));
    }
    if ledger.stim.commit != STIM_COMMIT {
        errors.push(format!(
            "stim.commit is {}, expected {STIM_COMMIT}",
            ledger.stim.commit
        ));
    }
    if ledger.families.is_empty() {
        errors.push("families must not be empty".to_string());
    }

    validate_format_routes(root, ledger, &mut errors);
    validate_command_surfaces(root, ledger, &mut errors);
    let mut expected_members = expected.members.clone();
    for surface in &ledger.command_surfaces {
        for option in &surface.options {
            expected_members.insert(format!("option:{}/{}", surface.command, option));
        }
    }
    let mut route_outcomes = BTreeMap::new();
    for route in &ledger.format_routes {
        for (formats, outcome) in [
            (&route.accepted_formats, RouteOutcome::Accepted),
            (&route.rejected_formats, RouteOutcome::Rejected),
            (&route.stim_bug_divergences, RouteOutcome::StimBugDivergence),
        ] {
            for format in formats {
                let member = format!("route:{}/{format}", route.id);
                expected_members.insert(member.clone());
                route_outcomes.insert(member, outcome);
            }
        }
    }

    let mut ids = BTreeSet::new();
    let mut coverage = BTreeMap::<String, String>::new();
    let mut previous_id: Option<&str> = None;
    for family in &ledger.families {
        validate_family(root, family, &mut errors);
        if !ids.insert(family.id.as_str()) {
            errors.push(format!("family id {} is duplicated", family.id));
        }
        if previous_id.is_some_and(|previous| previous >= family.id.as_str()) {
            errors.push(format!(
                "family {} is out of order; families must be sorted by id",
                family.id
            ));
        }
        previous_id = Some(&family.id);
        for member in &family.coverage {
            if !expected_members.contains(member) {
                errors.push(format!(
                    "family {} names unknown coverage member {member}",
                    family.id
                ));
                continue;
            }
            if let Some(first) = coverage.insert(member.clone(), family.id.clone()) {
                errors.push(format!(
                    "coverage member {member} is owned by both {first} and {}",
                    family.id
                ));
            }
            if matches!(family.status(), Status::Done | Status::Divergence) {
                validate_current_capability(family, member, expected, &route_outcomes, &mut errors);
            }
        }
    }

    for missing in expected_members
        .iter()
        .filter(|member| !coverage.contains_key(*member))
    {
        errors.push(format!("coverage member {missing} has no family owner"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort();
        errors.dedup();
        Err(invalid(errors.join("\n")))
    }
}

pub(super) fn validate_family(root: &RepoRoot, family: &Family, errors: &mut Vec<String>) {
    if !is_slug(&family.id, true) {
        errors.push(format!(
            "family id {} is not a lowercase dotted slug",
            family.id
        ));
    }
    validate_clean_text(&family.id, "contract", &family.contract, 600, errors);
    if family.stim_refs.is_empty() {
        errors.push(format!(
            "family {} has no pinned Stim references",
            family.id
        ));
    }
    let mut refs = BTreeSet::new();
    for reference in &family.stim_refs {
        if !refs.insert(reference.as_str()) {
            errors.push(format!(
                "family {} repeats Stim reference {reference}",
                family.id
            ));
        }
        validate_stim_reference(root, &family.id, reference, errors);
    }
    if family.coverage.iter().collect::<BTreeSet<_>>().len() != family.coverage.len() {
        errors.push(format!("family {} repeats a coverage member", family.id));
    }

    match &family.disposition {
        Disposition::Done { owner, evidence } => {
            validate_owner(&family.id, owner, errors);
            match evidence {
                Evidence::Verified {
                    test,
                    stim_reproduction,
                } => {
                    validate_canonical_test_owner(&family.id, owner, test, errors);
                    if stim_reproduction.is_some() {
                        errors.push(format!(
                            "done family {} cannot carry a Stim-bug reproduction",
                            family.id
                        ));
                    }
                }
                Evidence::NeedsOwner { milestone } if *milestone != Milestone::P1 => {
                    errors.push(format!(
                        "done family {} must receive its lean canonical owner in P1",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { .. } => {}
            }
        }
        Disposition::Missing { owner, .. } => {
            validate_owner(&family.id, owner, errors);
        }
        Disposition::Deferred { rationale } => {
            validate_clean_text(&family.id, "rationale", rationale, 600, errors);
        }
        Disposition::Divergence {
            owner,
            divergence_kind,
            rationale,
            evidence,
            ..
        } => {
            validate_owner(&family.id, owner, errors);
            validate_clean_text(&family.id, "rationale", rationale, 600, errors);
            match evidence {
                Evidence::Verified {
                    test,
                    stim_reproduction,
                } => {
                    validate_canonical_test_owner(&family.id, owner, test, errors);
                    match (divergence_kind, stim_reproduction) {
                        (DivergenceKind::StimBug, Some(reproduction)) => {
                            validate_test_owner(&family.id, reproduction, errors);
                            if reproduction.package != "stab-oracle" {
                                errors.push(format!(
                                    "Stim-bug family {} reproduction must be owned by stab-oracle",
                                    family.id
                                ));
                            }
                            if test.display() == reproduction.display() {
                                errors.push(format!(
                                    "Stim-bug family {} reproduction must be distinct from its Stab regression",
                                    family.id
                                ));
                            }
                        }
                        (DivergenceKind::StimBug, None) => errors.push(format!(
                            "Stim-bug family {} has no independent pinned reproduction",
                            family.id
                        )),
                        (_, Some(_)) => errors.push(format!(
                            "non-bug divergence family {} cannot carry a Stim-bug reproduction",
                            family.id
                        )),
                        (_, None) => {}
                    }
                }
                Evidence::NeedsOwner { .. } if *divergence_kind == DivergenceKind::StimBug => {
                    errors.push(format!(
                        "Stim-bug divergence family {} must have verified independent evidence",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { milestone } if *milestone != Milestone::P1 => {
                    errors.push(format!(
                        "implemented divergence family {} must receive its lean canonical owner in P1",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { .. } => {}
            }
        }
    }
}

pub(super) fn validate_owner(family_id: &str, owner: &str, errors: &mut Vec<String>) {
    if !PRODUCT_OWNERS.contains(&owner) {
        errors.push(format!(
            "family {family_id} has unknown product owner {owner}"
        ));
    }
}

pub(super) fn validate_test_owner(family_id: &str, test: &TestOwner, errors: &mut Vec<String>) {
    if !TEST_PACKAGES.contains(&test.package.as_str()) {
        errors.push(format!(
            "family {family_id} uses unknown test package {}",
            test.package
        ));
    }
    if !is_test_name(&test.name) {
        errors.push(format!(
            "family {family_id} has invalid exact test name {}",
            test.name
        ));
    }
    if let CargoTestTarget::Bin { target } | CargoTestTarget::Test { target } = &test.target
        && !is_test_name(target)
    {
        errors.push(format!(
            "family {family_id} has invalid integration-test target {target}"
        ));
    }
}

pub(super) fn validate_canonical_test_owner(
    family_id: &str,
    owner: &str,
    test: &TestOwner,
    errors: &mut Vec<String>,
) {
    validate_test_owner(family_id, test, errors);
    if test.package != owner && test.package != "stab-oracle" {
        errors.push(format!(
            "family {family_id} is owned by {owner} but its canonical test runs in {}",
            test.package
        ));
    }
}

pub(super) fn is_test_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b':')
        })
}

pub(super) fn validate_stim_reference(
    root: &RepoRoot,
    family_id: &str,
    reference: &str,
    errors: &mut Vec<String>,
) {
    let path = Path::new(reference);
    if reference.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        errors.push(format!(
            "family {family_id} has unsafe Stim reference {reference}"
        ));
        return;
    }
    let resolved = root.stim_source().join(path);
    if safe_file::open_regular_file(&resolved).is_err() {
        errors.push(format!(
            "family {family_id} names missing or unsafe Stim reference {reference}"
        ));
    }
}

pub(super) fn validate_current_capability(
    family: &Family,
    member: &str,
    expected: &ExpectedCoverage,
    route_outcomes: &BTreeMap<String, RouteOutcome>,
    errors: &mut Vec<String>,
) {
    let Some((dimension, value)) = member.split_once(':') else {
        return;
    };
    match dimension {
        "gate" => {
            let gate = stab_model::Gate::from_name(value);
            if gate.is_err()
                || gate
                    .as_ref()
                    .is_ok_and(|gate| gate.canonical_name() != value)
                || !expected.canonical_gates.contains(value)
            {
                errors.push(format!(
                    "family {} claims unavailable canonical gate {value}",
                    family.id
                ));
            }
        }
        "alias" => {
            if stab_model::Gate::from_name(value).is_err() || !expected.aliases.contains(value) {
                errors.push(format!(
                    "family {} claims unavailable gate alias {value}",
                    family.id
                ));
            }
        }
        "format" => {
            if !stab_records::RecordFormat::all().any(|format| format.as_str() == value) {
                errors.push(format!(
                    "family {} claims unavailable result format {value}",
                    family.id
                ));
            }
        }
        "dialect" => {
            let capability_name = match value {
                "stim" => "stim-circuit",
                "dem" => "detector-error-model",
                _ => value,
            };
            if !stab_model::ModelDialect::all().any(|dialect| dialect.as_str() == capability_name) {
                errors.push(format!(
                    "family {} claims unavailable model dialect {value}",
                    family.id
                ));
            }
        }
        "option" => {
            let valid = value
                .split_once('/')
                .is_some_and(|(command, option)| is_expected_command_option(command, option));
            if !valid {
                errors.push(format!(
                    "family {} claims unknown command option {value}",
                    family.id
                ));
            }
        }
        "command" | "consumer" | "target" | "capability" | "extension" | "obsolete" => {}
        "route" => {
            let outcome = route_outcomes.get(member);
            let is_stim_bug = matches!(
                family.disposition,
                Disposition::Divergence {
                    divergence_kind: DivergenceKind::StimBug,
                    ..
                }
            );
            if outcome == Some(&RouteOutcome::StimBugDivergence) && !is_stim_bug {
                errors.push(format!(
                    "family {} owns Stim-bug route member {member} without a Stim-bug divergence disposition",
                    family.id
                ));
            } else if outcome != Some(&RouteOutcome::StimBugDivergence) && is_stim_bug {
                errors.push(format!(
                    "family {} assigns Stim-bug divergence to non-bug route member {member}",
                    family.id
                ));
            }
        }
        _ => errors.push(format!(
            "family {} uses unsupported coverage dimension {dimension}",
            family.id
        )),
    }
}

pub(super) fn is_slug(value: &str, allow_dot: bool) -> bool {
    !value.is_empty()
        && !value.starts_with(['-', '.'])
        && !value.ends_with(['-', '.'])
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'-'
                || (allow_dot && byte == b'.')
        })
}

pub(super) fn validate_clean_text(
    family_id: &str,
    field: &str,
    value: &str,
    maximum_bytes: usize,
    errors: &mut Vec<String>,
) {
    if value.is_empty()
        || value.trim() != value
        || value.contains(['\r', '\n'])
        || value.len() > maximum_bytes
    {
        errors.push(format!(
            "family {family_id} has invalid {field}; it must be one trimmed line of at most {maximum_bytes} bytes"
        ));
    }
}
