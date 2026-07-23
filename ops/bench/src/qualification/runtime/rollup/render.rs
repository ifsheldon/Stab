use serde::Serialize;

use super::{
    ROLLUP_PREFLIGHT_SCHEMA_VERSION, RollupError, RollupPreflight, RollupReport, sha256_hex,
};

pub(super) fn render_json(value: &impl Serialize) -> Result<Vec<u8>, RollupError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn preflight(report: &RollupReport, report_json: &[u8]) -> RollupPreflight {
    RollupPreflight {
        schema_version: ROLLUP_PREFLIGHT_SCHEMA_VERSION,
        report_sha256: sha256_hex(report_json),
        output: report.output.clone(),
        group_id: report.group_id.clone(),
        tier: report.tier,
        performance_inventory_sha256: report.performance_inventory_sha256.clone(),
        correctness_inventory_sha256: report.correctness_inventory_sha256.clone(),
        stab_commit: report.stab_commit.clone(),
        producer_repository: report.producer_repository.clone(),
        architecture: report.architecture.clone(),
        target_triple: report.target_triple.clone(),
        required_scales: report
            .scales
            .iter()
            .map(|scale| scale.scale_id.clone())
            .collect(),
        source_reports: report
            .scales
            .iter()
            .map(|scale| scale.source.clone())
            .collect(),
        overall_outcome: report.overall_outcome,
    }
}

pub(super) fn render_markdown(report: &RollupReport, report_sha256: &str) -> String {
    let profiler_note = report
        .profiler_note
        .as_ref()
        .map_or("none", |note| note.path.as_str());
    let group_id = super::super::markdown::inline_code(&report.group_id);
    let owner = super::super::markdown::inline_code(&report.owner);
    let profiler_note = super::super::markdown::inline_code(profiler_note);
    let stab_commit = super::super::markdown::inline_code(&report.stab_commit);
    let stim_commit = super::super::markdown::inline_code(&report.stim_commit);
    let architecture = super::super::markdown::inline_code(&report.architecture);
    let target_triple = super::super::markdown::inline_code(&report.target_triple);
    let host_profile_id = super::super::markdown::inline_code(&report.host_profile_id);
    let cpu_identity = super::super::markdown::inline_code(&report.cpu_identity);
    let contract_preflight =
        super::super::markdown::inline_code(&report.workers.contract_preflight_sha256);
    let report_sha256 = super::super::markdown::inline_code(report_sha256);
    let mut markdown = format!(
        "# Performance Qualification Scale-Family Rollup\n\n- Group: {}\n- Tier: `{:?}`\n- Owner: {}\n- Profiler note: {}\n- Stab commit: {}\n- Stim commit: {}\n- Worker contract preflight: {}\n- Architecture: {} ({})\n- Host profile: {}\n- CPU: {}\n- Required scales: `{}`\n- Passed measurements: `{}`\n- Failed measurements: `{}`\n- Noisy measurements: `{}`\n- Overall outcome: `{:?}`\n- Rollup report SHA-256: {}\n\n## Timing\n\n| Scale | Work items | Measurement | Pairs | Median Stab/Stim | Upper 95% bound | Ratio rMAD | Outcome |\n| --- | ---: | --- | ---: | ---: | ---: | ---: | --- |\n",
        group_id,
        report.tier,
        owner,
        profiler_note,
        stab_commit,
        stim_commit,
        contract_preflight,
        architecture,
        target_triple,
        host_profile_id,
        cpu_identity,
        report.required_scale_count,
        report.passed_measurements,
        report.failed_measurements,
        report.noisy_measurements,
        report.overall_outcome,
        report_sha256,
    );
    for scale in &report.scales {
        for measurement in &scale.measurements {
            let scale_id = super::super::markdown::inline_code(&scale.scale_id);
            let measurement_id = super::super::markdown::inline_code(&measurement.measurement_id);
            markdown.push_str(&format!(
                "| {} | {} | {} | {} | {:.6} | {:.6} | {:.6} | `{:?}` |\n",
                scale_id,
                scale.work_items,
                measurement_id,
                measurement.pair_count,
                measurement.median_ratio,
                measurement.confidence_interval_upper,
                measurement.ratio_relative_mad,
                measurement.outcome,
            ));
        }
    }
    markdown.push_str("\n## Memory\n\n| Scale | Stim setup RSS | Stim peak RSS | Stab setup RSS | Stab peak RSS | Source report |\n| --- | ---: | ---: | ---: | ---: | --- |\n");
    for scale in &report.scales {
        let scale_id = super::super::markdown::inline_code(&scale.scale_id);
        let source_path = super::super::markdown::inline_code(&scale.source.path);
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            scale_id,
            scale.memory.stim_setup_rss_bytes,
            scale.memory.stim_peak_rss_bytes,
            scale.memory.stab_setup_rss_bytes,
            scale.memory.stab_peak_rss_bytes,
            source_path,
        ));
    }
    markdown
}
