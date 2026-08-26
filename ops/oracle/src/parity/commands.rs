use std::collections::BTreeSet;

use super::{EXPECTED_COMMAND_SURFACES, Ledger, Status, is_test_name, validate_stim_reference};
use crate::RepoRoot;

pub(super) fn expected_command_options(command: &str) -> Option<&'static [&'static str]> {
    Some(match command {
        "analyze_errors" => &[
            "--allow_gauge_detectors",
            "--approximate_disjoint_errors",
            "--block_decompose_from_introducing_remnant_edges",
            "--decompose_errors",
            "--fold_loops",
            "--ignore_decomposition_failures",
            "--in",
            "--out",
        ],
        "convert" => &[
            "--bits_per_shot",
            "--circuit",
            "--dem",
            "--in",
            "--in_format",
            "--num_detectors",
            "--num_measurements",
            "--num_observables",
            "--obs_out",
            "--obs_out_format",
            "--out",
            "--out_format",
            "--types",
        ],
        "detect" => &[
            "--append_observables",
            "--in",
            "--obs_out",
            "--obs_out_format",
            "--out",
            "--out_format",
            "--seed",
            "--shots",
        ],
        "gen" => &[
            "--after_clifford_depolarization",
            "--after_reset_flip_probability",
            "--before_measure_flip_probability",
            "--before_round_data_depolarization",
            "--code",
            "--distance",
            "--in",
            "--out",
            "--rounds",
            "--task",
        ],
        "m2d" => &[
            "--append_observables",
            "--circuit",
            "--in",
            "--in_format",
            "--obs_out",
            "--obs_out_format",
            "--out",
            "--out_format",
            "--ran_without_feedback",
            "--skip_reference_sample",
            "--sweep",
            "--sweep_format",
        ],
        "sample" => &[
            "--in",
            "--out",
            "--out_format",
            "--seed",
            "--shots",
            "--skip_loop_folding",
            "--skip_reference_sample",
        ],
        "sample_dem" => &[
            "--err_out",
            "--err_out_format",
            "--in",
            "--obs_out",
            "--obs_out_format",
            "--out",
            "--out_format",
            "--replay_err_in",
            "--replay_err_in_format",
            "--seed",
            "--shots",
        ],
        _ => return None,
    })
}

pub(super) fn is_expected_command_option(command: &str, option: &str) -> bool {
    expected_command_options(command).is_some_and(|options| options.contains(&option))
}

pub(super) fn validate_command_surfaces(
    root: &RepoRoot,
    ledger: &Ledger,
    errors: &mut Vec<String>,
) {
    let stab_command = stab_cli::command_descriptor();
    let expected = EXPECTED_COMMAND_SURFACES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut commands = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for surface in &ledger.command_surfaces {
        if !expected.contains(surface.command.as_str()) {
            errors.push(format!(
                "command surface {} is not an in-scope computational command",
                surface.command
            ));
        }
        if !commands.insert(surface.command.as_str()) {
            errors.push(format!("command surface {} is duplicated", surface.command));
        }
        if previous.is_some_and(|value| value >= surface.command.as_str()) {
            errors.push(format!(
                "command surface {} is out of order",
                surface.command
            ));
        }
        previous = Some(&surface.command);

        if surface.options.is_empty() {
            errors.push(format!(
                "command surface {} has no supported options",
                surface.command
            ));
        }
        let mut options = BTreeSet::new();
        let mut previous_option: Option<&str> = None;
        for option in &surface.options {
            if !option.strip_prefix("--").is_some_and(is_test_name) {
                errors.push(format!(
                    "command surface {} has invalid option {option}",
                    surface.command
                ));
            }
            if !options.insert(option.as_str()) {
                errors.push(format!(
                    "command surface {} repeats option {option}",
                    surface.command
                ));
            }
            if previous_option.is_some_and(|value| value >= option.as_str()) {
                errors.push(format!(
                    "command surface {} options are not sorted at {option}",
                    surface.command
                ));
            }
            previous_option = Some(option);
        }
        if let Some(expected_options) = expected_command_options(&surface.command) {
            let expected_options = expected_options.iter().copied().collect::<BTreeSet<_>>();
            for missing in expected_options.difference(&options) {
                errors.push(format!(
                    "command surface {} omits pinned option {missing}",
                    surface.command
                ));
            }
            for extra in options.difference(&expected_options) {
                errors.push(format!(
                    "command surface {} adds non-pinned option {extra}",
                    surface.command
                ));
            }
        }
        if surface.stim_refs.is_empty() {
            errors.push(format!(
                "command surface {} has no pinned Stim references",
                surface.command
            ));
        }
        for reference in &surface.stim_refs {
            validate_stim_reference(
                root,
                &format!("command surface {}", surface.command),
                reference,
                errors,
            );
        }
    }
    for missing in expected.difference(&commands) {
        errors.push(format!("command surface {missing} is missing"));
    }
    validate_stab_command_schema(ledger, &stab_command, errors);
}

pub(super) fn validate_stab_command_schema(
    ledger: &Ledger,
    command: &clap::Command,
    errors: &mut Vec<String>,
) {
    for surface in &ledger.command_surfaces {
        let command_member = format!("command:{}", surface.command);
        let implemented_options = surface
            .options
            .iter()
            .filter(|option| {
                let member = format!("option:{}/{}", surface.command, option);
                ledger.families.iter().any(|family| {
                    matches!(family.status(), Status::Done | Status::Divergence)
                        && family.coverage.contains(&member)
                })
            })
            .collect::<Vec<_>>();
        let command_is_implemented = ledger.families.iter().any(|family| {
            matches!(family.status(), Status::Done | Status::Divergence)
                && family.coverage.contains(&command_member)
        });
        if !command_is_implemented && implemented_options.is_empty() {
            continue;
        }
        let Some(subcommand) = command
            .get_subcommands()
            .find(|candidate| candidate.get_name() == surface.command)
        else {
            errors.push(format!(
                "Stab Clap schema omits in-scope command {}",
                surface.command
            ));
            continue;
        };
        let current_options = subcommand
            .get_arguments()
            .filter_map(clap::Arg::get_long)
            .map(|long| format!("--{long}"))
            .collect::<BTreeSet<_>>();
        for option in implemented_options {
            if !current_options.contains(option) {
                errors.push(format!(
                    "Stab Clap schema command {} omits pinned option {option}",
                    surface.command
                ));
            }
        }
    }
}
