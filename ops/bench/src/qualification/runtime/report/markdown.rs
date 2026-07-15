use super::super::run::QualificationReport;

pub(super) fn render(
    report: &QualificationReport,
    report_sha256: &str,
) -> Result<String, super::ReportError> {
    let authoritative = super::authoritative_timing_attempt(report)?;
    let summary = authoritative
        .statistics
        .first()
        .ok_or(super::ReportError::StatisticsSet)?;
    let median = format!("{:.6}", summary.median_ratio);
    let upper = format!("{:.6}", summary.confidence_interval_upper);
    let outcome = format!("{:?}", summary.outcome).to_ascii_lowercase();
    let maximum_temperature = |readings: &[super::super::host::ThermalReading]| {
        readings
            .iter()
            .map(|reading| reading.millidegrees_celsius)
            .max()
            .map_or("unavailable".to_string(), |value| value.to_string())
    };
    let profiler_note = report.profiler_note.as_ref().map_or_else(
        || "not-applicable".to_string(),
        |note| note.path.as_str().to_string(),
    );
    let input = report
        .semantic_preflight
        .stim
        .rows
        .first()
        .map(|row| (row.input_bytes, row.input_digest.as_str()))
        .ok_or(super::ReportError::PairShape)?;
    let code = super::super::markdown::inline_code;
    let group_id = code(&report.group_id);
    let scale_id = code(&report.scale_id);
    let group_contract_sha256 = code(&report.group_contract_sha256);
    let owner = code(&report.owner);
    let profiler_note = code(&profiler_note);
    let input_digest = code(input.1);
    let stim_tag = code(&report.stim_tag);
    let stim_commit = code(&report.stim_commit);
    let stab_commit = code(&report.repository.commit_after);
    let host_profile = code(&report.host.profile_id);
    let cpu_identity = code(&report.host.cpu_identity);
    let frequency_governor = code(&format!("{:?}", report.host.frequency_governor_before));
    let rust_toolchain = code(&report.toolchain.rust_toolchain);
    let target_triple = code(&report.toolchain.target_triple);
    let report_sha256 = code(report_sha256);
    Ok(format!(
        "# Performance Qualification Report\n\n- Group: {}\n- Scale: {} (`{}` work items per iteration)\n- Group contract SHA-256: {}\n- Claim class: `{:?}`\n- Baseline eligibility: `{:?}`\n- Owner: {}\n- Profiler note: {}\n- Input bytes: `{}`\n- Input digest: {}\n- Tier: `{:?}`\n- Stim: {} ({})\n- Stab commit: {}\n- Local modifications: `{}`\n- Host profile: {}\n- Host verified: `{}`\n- CPU: `{}` on {}\n- Frequency governor: {}\n- Maximum thermal reading before: `{}` millidegrees Celsius\n- Maximum thermal reading after: `{}` millidegrees Celsius\n- Rust toolchain: {}\n- Target: {}\n- Calibration target: `{:.3}` seconds\n- Calibration acceptance floor: `{:.3}` seconds\n- Timing attempts retained: `{}`\n- Authoritative timing attempt: `{}`\n- Warmups in authoritative attempt: `{}`\n- Paired samples in authoritative attempt: `{}`\n- Median Stab/Stim ratio: `{}`\n- Upper bootstrap bound: `{}`\n- 1.25 outcome: `{}`\n- Stim setup RSS: `{}` bytes\n- Stim peak RSS: `{}` bytes\n- Stim measured RSS delta: `{}` bytes\n- Stim parent-observed peak RSS: `{}`\n- Stab setup RSS: `{}` bytes\n- Stab peak RSS: `{}` bytes\n- Stab measured RSS delta: `{}` bytes\n- Stab parent-observed peak RSS: `{}`\n- Promotable product claim: `{}`\n- Report SHA-256: {}\n",
        group_id,
        scale_id,
        report.command.work_items,
        group_contract_sha256,
        report.claim_class,
        report.baseline_eligibility,
        owner,
        profiler_note,
        input.0,
        input_digest,
        report.tier,
        stim_tag,
        stim_commit,
        stab_commit,
        report.repository.local_modifications_before || report.repository.local_modifications_after,
        host_profile,
        report.host.verified,
        report.host.selected_cpu,
        cpu_identity,
        frequency_governor,
        maximum_temperature(&report.host.thermal_readings_before),
        maximum_temperature(&report.host.thermal_readings_after),
        rust_toolchain,
        target_triple,
        report.calibration.target_minimum_seconds,
        report.calibration.acceptance_minimum_seconds,
        report.timing_attempts.len(),
        authoritative.attempt_index,
        authoritative.warmups.len(),
        authoritative.samples.len(),
        median,
        upper,
        outcome,
        report.memory.stim_setup_rss_bytes,
        report.memory.stim_peak_rss_bytes,
        report
            .memory
            .stim_peak_rss_bytes
            .saturating_sub(report.memory.stim_setup_rss_bytes),
        display_optional_bytes(report.memory.stim_parent_observed_peak_rss_bytes),
        report.memory.stab_setup_rss_bytes,
        report.memory.stab_peak_rss_bytes,
        report
            .memory
            .stab_peak_rss_bytes
            .saturating_sub(report.memory.stab_setup_rss_bytes),
        display_optional_bytes(report.memory.stab_parent_observed_peak_rss_bytes),
        report.promotable,
        report_sha256,
    ))
}

fn display_optional_bytes(value: Option<u64>) -> String {
    value.map_or_else(
        || "unobserved".to_string(),
        |value| format!("{value} bytes"),
    )
}
