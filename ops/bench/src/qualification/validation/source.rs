use std::collections::BTreeSet;

use super::{
    EXPECTED_PERF_SYMBOLS, Issues, PerformanceDisposition, QualificationSuite, RowClassification,
    SourceReferences, StimMapping, WaiverKind, filter_selects_symbol, validate_identifier,
    validate_text,
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
    let mut policy_keys = BTreeSet::new();
    for waiver in &suite.waiver_rows {
        if !ids.insert(waiver.id.as_str()) {
            issues.push(format!("duplicate waiver row {}", waiver.id));
        }
        if !rows.contains(waiver.id.as_str()) {
            issues.push(format!("stale waiver row {}", waiver.id));
        }
        if waiver.policies.is_empty() {
            issues.push(format!("waiver {} names no source policy", waiver.id));
        }
        let mut kinds = BTreeSet::new();
        for policy in &waiver.policies {
            let key = (policy.waiver_file.clone(), waiver.id.clone());
            if !policy_keys.insert(key.clone()) {
                issues.push(format!(
                    "waiver {} repeats source policy {}",
                    waiver.id, policy.waiver_file
                ));
            }
            validate_text("waiver reason", &policy.reason, issues);
            validate_text("waiver follow-up", &policy.follow_up, issues);
            let expected = references.waiver_policies.get(&key);
            if expected != Some(policy) {
                issues.push(format!(
                    "waiver {} policy for {} differs from the source waiver ledger",
                    waiver.id, policy.waiver_file
                ));
            }
            kinds.insert(policy.kind);
            let mut pairs = BTreeSet::new();
            for pair in &policy.measurement_pairs {
                validate_identifier("waiver Stim measurement", &pair.stim_name, issues);
                validate_identifier("waiver Stab measurement", &pair.stab_name, issues);
                if !pairs.insert((pair.stim_name.as_str(), pair.stab_name.as_str())) {
                    issues.push(format!(
                        "waiver {} repeats measurement pair {} -> {}",
                        waiver.id, pair.stim_name, pair.stab_name
                    ));
                }
            }
            match policy.kind {
                WaiverKind::NoComparableBaseline if !policy.measurement_pairs.is_empty() => {
                    issues.push(format!(
                        "no-comparable waiver {} names measurement pairs",
                        waiver.id
                    ));
                }
                WaiverKind::UnstableFaithfulPairs if policy.measurement_pairs.is_empty() => {
                    issues.push(format!(
                        "unstable faithful-pair waiver {} names no measurement pairs",
                        waiver.id
                    ));
                }
                WaiverKind::UnstableFaithfulPairs
                    if policy.waiver_file != "benchmarks/m12-primary-regression-waivers.json" =>
                {
                    issues.push(format!(
                        "unstable faithful-pair waiver {} comes from {}",
                        waiver.id, policy.waiver_file
                    ));
                }
                _ => {}
            }
        }
        if kinds.len() > 1 {
            issues.push(format!("waiver {} mixes policy kinds", waiver.id));
        }
        let adapter_waiver = kinds.contains(&WaiverKind::NoComparableBaseline);
        match (&waiver.retirement_mapping, adapter_waiver) {
            (Some(mapping), true)
                if mapping != "UNMAPPED-WAIVER" && mapping.starts_with("stim_adapter::") => {}
            (None, false) => {}
            _ => issues.push(format!(
                "waiver {} has a retirement mapping inconsistent with its policy kind",
                waiver.id
            )),
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
    let expected_policy_keys = references
        .waiver_policies
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if policy_keys != expected_policy_keys {
        issues.push("waiver source policies do not exactly match source waiver ledgers");
    }
}

pub(super) fn validate_waiver_classifications(
    suite: &QualificationSuite,
    references: &SourceReferences,
    issues: &mut Issues,
) {
    for row in &suite.manifest_rows {
        let adapter_waived = references.beta_waivers.contains(&row.id)
            || references.adapter_regression_waivers.contains(&row.id);
        let adapter_candidate = row
            .classifications
            .contains(&RowClassification::AdapterCandidate);
        if adapter_waived && !adapter_candidate {
            issues.push(format!(
                "waived row {} does not name an adapter retirement path",
                row.id
            ));
        }
        if references.unstable_pair_waivers.contains(&row.id) && adapter_candidate {
            issues.push(format!(
                "unstable faithful-pair waiver {} is incorrectly classified as an adapter candidate",
                row.id
            ));
        }
    }
}
