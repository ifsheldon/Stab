use std::collections::BTreeSet;

use super::{
    EXPECTED_COMMANDS, EXPECTED_FORMAT_ROUTES, EXPECTED_FORMATS, FormatRoute, FormatRouteRole,
    Ledger, RecordType, is_slug, validate_stim_reference,
};
use crate::RepoRoot;

pub(super) fn validate_format_routes(root: &RepoRoot, ledger: &Ledger, errors: &mut Vec<String>) {
    let expected_ids = EXPECTED_FORMAT_ROUTES.into_iter().collect::<BTreeSet<_>>();
    let expected_formats = EXPECTED_FORMATS.into_iter().collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut previous_id: Option<&str> = None;
    for route in &ledger.format_routes {
        if !is_slug(&route.id, false) {
            errors.push(format!("format route {} has an invalid id", route.id));
        }
        if !ids.insert(route.id.as_str()) {
            errors.push(format!("format route {} is duplicated", route.id));
        }
        if previous_id.is_some_and(|previous| previous >= route.id.as_str()) {
            errors.push(format!(
                "format route {} is out of order; routes must be sorted by id",
                route.id
            ));
        }
        previous_id = Some(&route.id);
        if !expected_ids.contains(route.id.as_str()) {
            errors.push(format!(
                "format route {} is not part of the frozen CLI",
                route.id
            ));
        }
        if !EXPECTED_COMMANDS.contains(&route.command.as_str()) {
            errors.push(format!(
                "format route {} names unknown command {}",
                route.id, route.command
            ));
        }
        if route.record_types.is_empty()
            || route.record_types.iter().collect::<BTreeSet<_>>().len() != route.record_types.len()
        {
            errors.push(format!(
                "format route {} must name one or more unique record types",
                route.id
            ));
        }
        let accepted_formats = route
            .accepted_formats
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let rejected_formats = route
            .rejected_formats
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let bug_divergences = route
            .stim_bug_divergences
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let all_formats = accepted_formats
            .union(&rejected_formats)
            .copied()
            .collect::<BTreeSet<_>>()
            .union(&bug_divergences)
            .copied()
            .collect::<BTreeSet<_>>();
        let listed_count = route.accepted_formats.len()
            + route.rejected_formats.len()
            + route.stim_bug_divergences.len();
        if listed_count != EXPECTED_FORMATS.len() || all_formats != expected_formats {
            errors.push(format!(
                "format route {} must classify each of the six Stim result formats exactly once",
                route.id
            ));
        }
        if route.stim_refs.is_empty() {
            errors.push(format!("format route {} has no Stim references", route.id));
        }
        if route.stim_refs.iter().collect::<BTreeSet<_>>().len() != route.stim_refs.len() {
            errors.push(format!(
                "format route {} repeats a Stim reference",
                route.id
            ));
        }
        for reference in &route.stim_refs {
            validate_stim_reference(
                root,
                &format!("format route {}", route.id),
                reference,
                errors,
            );
        }
        validate_format_route_shape(route, errors);
    }
    for missing in expected_ids.difference(&ids) {
        errors.push(format!("format route {missing} is missing"));
    }
}

fn validate_format_route_shape(route: &FormatRoute, errors: &mut Vec<String>) {
    let Some((command, role, record_types)) = expected_format_route_shape(&route.id) else {
        return;
    };
    if route.command != command
        || route.role != role
        || route.record_types.as_slice() != record_types
    {
        errors.push(format!(
            "format route {} has command, role, or record types inconsistent with pinned Stim",
            route.id
        ));
    }
    let (expected_rejections, expected_divergences) = expected_format_route_exceptions(&route.id);
    if route.rejected_formats.as_slice() != expected_rejections
        || route.stim_bug_divergences.as_slice() != expected_divergences
    {
        errors.push(format!(
            "format route {} has rejection or Stim-bug behavior inconsistent with pinned Stim",
            route.id
        ));
    }
    let expected_order = if route.id == "detect-output" {
        Some(super::DetsObservableOrder::PrependByDefault)
    } else {
        None
    };
    if route.dets_observable_order != expected_order {
        errors.push(format!(
            "format route {} has DETS observable ordering inconsistent with pinned Stim",
            route.id
        ));
    }
}

pub(super) fn expected_format_route_exceptions(
    id: &str,
) -> (&'static [&'static str], &'static [&'static str]) {
    match id {
        "convert-observable-output" | "convert-output" => (&[], &["ptb64"]),
        "m2d-observable-output" | "m2d-output" => (&["ptb64"], &[]),
        _ => (&[], &[]),
    }
}

pub(super) fn expected_format_route_shape(
    id: &str,
) -> Option<(&'static str, FormatRouteRole, &'static [RecordType])> {
    Some(match id {
        "convert-input" => (
            "convert",
            FormatRouteRole::Input,
            &[RecordType::M, RecordType::D, RecordType::L],
        ),
        "convert-observable-output" => ("convert", FormatRouteRole::SideOutput, &[RecordType::L]),
        "convert-output" => (
            "convert",
            FormatRouteRole::Output,
            &[RecordType::M, RecordType::D, RecordType::L],
        ),
        "detect-observable-output" => ("detect", FormatRouteRole::SideOutput, &[RecordType::L]),
        "detect-output" => (
            "detect",
            FormatRouteRole::Output,
            &[RecordType::D, RecordType::L],
        ),
        "m2d-measurement-input" => ("m2d", FormatRouteRole::Input, &[RecordType::M]),
        "m2d-observable-output" => ("m2d", FormatRouteRole::SideOutput, &[RecordType::L]),
        "m2d-output" => (
            "m2d",
            FormatRouteRole::Output,
            &[RecordType::D, RecordType::L],
        ),
        "m2d-sweep-input" => ("m2d", FormatRouteRole::Input, &[RecordType::M]),
        "sample-dem-error-output" => ("sample_dem", FormatRouteRole::SideOutput, &[RecordType::M]),
        "sample-dem-observable-output" => {
            ("sample_dem", FormatRouteRole::SideOutput, &[RecordType::L])
        }
        "sample-dem-output" => ("sample_dem", FormatRouteRole::Output, &[RecordType::D]),
        "sample-dem-replay-input" => ("sample_dem", FormatRouteRole::ReplayInput, &[RecordType::M]),
        "sample-output" => ("sample", FormatRouteRole::Output, &[RecordType::M]),
        _ => return None,
    })
}
