use std::collections::BTreeSet;

use super::{
    EXPECTED_PERF_SYMBOLS, Issues, PerformanceDisposition, QualificationSuite, SourceReferences,
    StimMapping, filter_selects_symbol, validate_identifier, validate_text,
};

pub(super) fn validate_upstream_sources(suite: &QualificationSuite, issues: &mut Issues) {
    let mut paths = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    let row_ids = suite
        .manifest_rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let selected_symbols = suite
        .manifest_rows
        .iter()
        .filter_map(|row| match &row.stim_mapping {
            StimMapping::StimPerf { source, filter } => Some((source, filter)),
            _ => None,
        })
        .flat_map(|(selected_source, filter)| {
            suite
                .upstream_perf_sources
                .iter()
                .filter(move |source| &source.path == selected_source)
                .flat_map(move |source| {
                    source
                        .symbols
                        .iter()
                        .filter(move |symbol| filter_selects_symbol(filter, symbol))
                        .map(move |symbol| (source.path.as_str(), symbol.as_str()))
                })
        })
        .collect::<BTreeSet<_>>();
    for source in &suite.upstream_perf_sources {
        if !paths.insert(source.path.as_str()) {
            issues.push(format!("duplicate upstream perf source {}", source.path));
        }
        if !source.path.starts_with("src/") || !source.path.ends_with(".perf.cc") {
            issues.push(format!("unsafe upstream perf source {}", source.path));
        }
        for symbol in &source.symbols {
            validate_identifier("upstream benchmark symbol", symbol, issues);
            if !symbols.insert((source.path.as_str(), symbol.as_str())) {
                issues.push(format!(
                    "duplicate upstream symbol {}::{symbol}",
                    source.path
                ));
            }
            if !selected_symbols.contains(&(source.path.as_str(), symbol.as_str())) {
                issues.push(format!(
                    "upstream symbol {}::{symbol} has no inherited Stim perf selector",
                    source.path
                ));
            }
        }
        for row in &source.manifest_rows {
            if !row_ids.contains(row.as_str()) {
                issues.push(format!(
                    "upstream source {} references unknown row {row}",
                    source.path
                ));
            }
        }
    }
    if symbols.len() != EXPECTED_PERF_SYMBOLS {
        issues.push(format!(
            "upstream perf inventory has {} symbols, expected {EXPECTED_PERF_SYMBOLS}",
            symbols.len()
        ));
    }
}

pub(super) fn validate_waivers(
    suite: &QualificationSuite,
    references: &SourceReferences,
    issues: &mut Issues,
) {
    let rows = suite
        .manifest_rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for waiver in &suite.waiver_rows {
        if !ids.insert(waiver.id.as_str()) {
            issues.push(format!("duplicate waiver row {}", waiver.id));
        }
        if !rows.contains(waiver.id.as_str()) {
            issues.push(format!("stale waiver row {}", waiver.id));
        }
        if waiver.waiver_files.is_empty() {
            issues.push(format!("waiver {} names no source waiver file", waiver.id));
        }
        let reason_files = waiver
            .reasons
            .iter()
            .map(|reason| reason.waiver_file.as_str())
            .collect::<BTreeSet<_>>();
        let waiver_files = waiver
            .waiver_files
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if reason_files != waiver_files {
            issues.push(format!(
                "waiver {} reason sources disagree with waiver files",
                waiver.id
            ));
        }
        for reason in &waiver.reasons {
            validate_text("waiver reason", &reason.reason, issues);
            let key = (reason.waiver_file.clone(), waiver.id.clone());
            if references.waiver_reasons.get(&key) != Some(&reason.reason) {
                issues.push(format!(
                    "waiver {} reason for {} differs from the source waiver ledger",
                    waiver.id, reason.waiver_file
                ));
            }
        }
        if waiver.retirement_mapping == "UNMAPPED-WAIVER"
            || !waiver.retirement_mapping.starts_with("stim_adapter::")
        {
            issues.push(format!(
                "waiver {} lacks an adapter retirement mapping",
                waiver.id
            ));
        }
        if waiver.follow_up.trim().is_empty() || waiver.follow_up == "SOURCE-WAIVER-MISMATCH" {
            issues.push(format!("waiver {} has an invalid follow-up", waiver.id));
        }
        if waiver.qualification_disposition == PerformanceDisposition::NoFaithfulStimComparator {
            issues.push(format!(
                "waiver {} is incorrectly promoted to no-faithful-comparator despite its adapter mapping",
                waiver.id
            ));
        }
    }
    let expected = references
        .beta_waivers
        .union(&references.regression_waivers)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if ids != expected {
        issues.push("waiver disposition ids do not exactly match source waiver ledgers");
    }
}
