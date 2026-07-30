use std::path::Path;

use super::{
    A6_PRIMARY_PROFILER_NOTE_ROOT, A6_PROFILER_NOTE_ROOTS, A6_SELECTED_PAIR_GATES,
    MAX_POLICY_BYTES, focused_error, revision,
};
use crate::compare::{
    HOT_PATH_PROFILER_NOTE_RATIO, rebuild_compare_row_raw_evidence, validate_profiler_note_content,
};
use crate::error::BenchError;
use crate::regression_waivers::{apply_regression_waivers, parse_regression_waivers};
use crate::report::{BETA_GATE_MAX_RELATIVE_RATIO, CompareReport, CompareRowResult};
use crate::root::RepoRoot;
use crate::thresholds::{apply_regression_thresholds, parse_thresholds};

pub(super) fn require_matrix_policies(
    root: &RepoRoot,
    report: &CompareReport,
) -> Result<(), BenchError> {
    let threshold_path = Path::new("benchmarks/m12-primary-thresholds.json");
    let threshold_bytes = revision::read_tracked_source_file(
        root,
        &report.stab.commit,
        threshold_path,
        MAX_POLICY_BYTES,
    )?
    .ok_or_else(|| focused_error("A6 threshold policy is not tracked at the source revision"))?;
    let thresholds = parse_thresholds(threshold_path, &threshold_bytes)?;
    let waiver_path = Path::new("benchmarks/m12-primary-regression-waivers.json");
    let waiver_bytes = revision::read_tracked_source_file(
        root,
        &report.stab.commit,
        waiver_path,
        MAX_POLICY_BYTES,
    )?
    .ok_or_else(|| {
        focused_error("A6 regression-waiver policy is not tracked at the source revision")
    })?;
    let waivers = parse_regression_waivers(waiver_path, &waiver_bytes)?;
    let mut expected = report.rows.clone();
    for (actual, row) in report.rows.iter().zip(&mut expected) {
        rebuild_compare_row_raw_evidence(row);
        require_raw_derived_fields(actual, row)?;
        row.regression_threshold_status = "not-configured".to_string();
        row.regression_threshold_max_ratio = None;
        row.regression_threshold_waiver_reason = None;
        row.regression_threshold_waiver_follow_up = None;
        row.regression_threshold_error = None;
        row.profiler_note_status.clear();
        row.profiler_note_path = None;
        row.profiler_note_error = None;
    }
    require_a6_selected_pair_gates(&expected)?;
    let threshold_findings = apply_regression_thresholds(&mut expected, &thresholds);
    let waiver_findings = apply_regression_waivers(&mut expected, &waivers);
    let profiler_blockers =
        reapply_tracked_profiler_notes(root, &report.stab.commit, &mut expected)?;
    let mut blockers = threshold_findings.blockers;
    blockers.extend(waiver_findings.blockers);
    blockers.extend(profiler_blockers);
    if !blockers.is_empty() {
        return Err(focused_error(format!(
            "matrix source-owned policy replay failed:\n{}",
            blockers.join("\n")
        )));
    }
    for (actual, replayed) in report.rows.iter().zip(&expected) {
        if actual.regression_threshold_status != replayed.regression_threshold_status
            || actual.regression_threshold_max_ratio != replayed.regression_threshold_max_ratio
            || actual.regression_threshold_waiver_reason
                != replayed.regression_threshold_waiver_reason
            || actual.regression_threshold_waiver_follow_up
                != replayed.regression_threshold_waiver_follow_up
            || actual.regression_threshold_error != replayed.regression_threshold_error
            || actual.profiler_note_status != replayed.profiler_note_status
            || actual.profiler_note_path != replayed.profiler_note_path
            || actual.profiler_note_error != replayed.profiler_note_error
        {
            return Err(focused_error(format!(
                "matrix policy result for {} differs from source-owned replay",
                actual.id
            )));
        }
    }
    Ok(())
}

fn reapply_tracked_profiler_notes(
    root: &RepoRoot,
    revision: &str,
    rows: &mut [CompareRowResult],
) -> Result<Vec<String>, BenchError> {
    let mut blockers = Vec::new();
    for row in rows {
        if !row
            .relative_ratio
            .is_some_and(|ratio| ratio > HOT_PATH_PROFILER_NOTE_RATIO)
        {
            row.profiler_note_status = "not-required".to_string();
            continue;
        }
        let file_name = format!("{}.md", row.id);
        let mut matches = Vec::new();
        for root_path in A6_PROFILER_NOTE_ROOTS {
            let relative = Path::new(root_path).join(&file_name);
            if let Some(bytes) =
                revision::read_tracked_source_file(root, revision, &relative, MAX_POLICY_BYTES)?
            {
                matches.push((relative, bytes));
            }
        }
        match matches.as_slice() {
            [] => {
                row.profiler_note_status = "missing".to_string();
                row.profiler_note_path = Some(
                    Path::new(A6_PRIMARY_PROFILER_NOTE_ROOT)
                        .join(&file_name)
                        .display()
                        .to_string(),
                );
                row.profiler_note_error = Some("profiler note is missing".to_string());
                blockers.push(format!("{}: profiler note is missing", row.id));
            }
            [(path, bytes)] => {
                let content = std::str::from_utf8(bytes).map_err(|error| {
                    focused_error(format!(
                        "tracked profiler note {} is not UTF-8: {error}",
                        path.display()
                    ))
                })?;
                if let Err(error) = validate_profiler_note_content(content) {
                    row.profiler_note_status = "invalid".to_string();
                    row.profiler_note_path = Some(path.display().to_string());
                    row.profiler_note_error = Some(error.message().to_string());
                    blockers.push(format!(
                        "{}: tracked profiler note {} is invalid",
                        row.id,
                        path.display()
                    ));
                } else {
                    row.profiler_note_status = "present".to_string();
                    row.profiler_note_path = Some(path.display().to_string());
                }
            }
            _ => {
                let paths = matches
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                row.profiler_note_status = "invalid".to_string();
                row.profiler_note_path =
                    matches.first().map(|(path, _)| path.display().to_string());
                row.profiler_note_error = Some(format!(
                    "profiler note exists in multiple directories: {paths}"
                ));
                blockers.push(format!(
                    "{}: profiler note exists in multiple directories: {paths}",
                    row.id
                ));
            }
        }
    }
    Ok(blockers)
}

pub(super) fn require_raw_derived_fields(
    actual: &CompareRowResult,
    rebuilt: &CompareRowResult,
) -> Result<(), BenchError> {
    if actual.stim_median_seconds == rebuilt.stim_median_seconds
        && actual.stab_median_seconds == rebuilt.stab_median_seconds
        && actual.relative_ratio == rebuilt.relative_ratio
        && actual.measurement_ratios == rebuilt.measurement_ratios
        && actual.stab_allocation_count_max == rebuilt.stab_allocation_count_max
        && actual.stab_allocation_bytes_max == rebuilt.stab_allocation_bytes_max
        && actual.stab_resident_bytes_max == rebuilt.stab_resident_bytes_max
        && actual.stab_resident_delta_bytes_max == rebuilt.stab_resident_delta_bytes_max
        && actual.pass_fail_status == rebuilt.pass_fail_status
    {
        Ok(())
    } else {
        Err(focused_error(format!(
            "matrix raw-derived timing or memory fields for {} do not reconstruct from bound measurements",
            actual.id
        )))
    }
}

pub(super) fn require_a6_selected_pair_gates(rows: &[CompareRowResult]) -> Result<(), BenchError> {
    for (row_id, stim_name, stab_name) in A6_SELECTED_PAIR_GATES {
        let row = rows
            .iter()
            .find(|row| row.id == row_id)
            .ok_or_else(|| focused_error(format!("A6 matrix omits selected pair row {row_id}")))?;
        let stim = row
            .stim_measurements
            .iter()
            .find(|measurement| measurement.name == stim_name)
            .ok_or_else(|| {
                focused_error(format!(
                    "A6 selected pair {row_id} omits Stim measurement {stim_name}"
                ))
            })?;
        let stab = row
            .stab_measurements
            .iter()
            .find(|measurement| measurement.name == stab_name)
            .ok_or_else(|| {
                focused_error(format!(
                    "A6 selected pair {row_id} omits Stab measurement {stab_name}"
                ))
            })?;
        if !stim.seconds.is_finite()
            || stim.seconds <= 0.0
            || !stab.seconds.is_finite()
            || stab.seconds < 0.0
        {
            return Err(focused_error(format!(
                "A6 selected pair {row_id} has invalid raw timing values"
            )));
        }
        let ratio = stab.seconds / stim.seconds;
        if ratio > BETA_GATE_MAX_RELATIVE_RATIO {
            return Err(focused_error(format!(
                "A6 selected pair {row_id} {stab_name}/{stim_name} is {ratio:.3}x, above {BETA_GATE_MAX_RELATIVE_RATIO:.2}x"
            )));
        }
    }
    Ok(())
}
