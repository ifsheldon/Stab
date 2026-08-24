use std::collections::{BTreeMap, BTreeSet};

use super::discovery::{self, PERFORMANCE_FEATURE_IDS, SourceReferences};
use super::model::{
    PerformanceDisposition, QualificationSuite, RowClassification, SCHEMA_VERSION, StimMapping,
    WaiverKind,
};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::BenchmarkManifest;

pub(super) fn validate(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
    references: &SourceReferences,
    expected_digest: &str,
) -> Result<(), BenchError> {
    validate_header(suite, references)?;
    validate_features(suite, references)?;
    validate_rows(suite, manifest, references)?;
    validate_upstream_sources(suite, manifest)?;
    validate_waivers(suite, references)?;

    let computed = discovery::semantic_digest(suite)?;
    if suite.semantic_digest != computed {
        return fail(format!(
            "semantic digest is {}, computed {computed}",
            suite.semantic_digest
        ));
    }
    if expected_digest != "UNFROZEN" && suite.semantic_digest != expected_digest {
        return fail(format!(
            "semantic digest is {}, expected frozen {expected_digest}",
            suite.semantic_digest
        ));
    }
    Ok(())
}

fn validate_header(
    suite: &QualificationSuite,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    if suite.schema_version != SCHEMA_VERSION {
        return fail(format!(
            "schema version is {}, expected {SCHEMA_VERSION}",
            suite.schema_version
        ));
    }
    if suite.stim_version != STIM_TAG || suite.stim_commit != STIM_COMMIT {
        return fail("Stim version or commit differs from the frozen compatibility target");
    }
    if suite.correctness_digest != references.correctness_digest {
        return fail("performance inventory is bound to a stale correctness inventory");
    }
    Ok(())
}

fn validate_features(
    suite: &QualificationSuite,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    let actual = suite
        .performance_features
        .iter()
        .map(|feature| feature.id.as_str())
        .collect::<Vec<_>>();
    if actual != PERFORMANCE_FEATURE_IDS {
        return fail("performance feature ids are incomplete, duplicated, or out of order");
    }
    for feature in &suite.performance_features {
        let expected = references
            .correctness_features
            .iter()
            .filter(|(_, groups)| groups.contains(&feature.id))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if feature.correctness_features != expected {
            return fail(format!(
                "performance feature {} has stale correctness-feature ownership",
                feature.id
            ));
        }
    }
    Ok(())
}

fn validate_rows(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    if suite.manifest_rows.len() != manifest.rows.len() {
        return fail(format!(
            "performance inventory has {} manifest rows, expected {}",
            suite.manifest_rows.len(),
            manifest.rows.len()
        ));
    }
    let source_rows = manifest
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut runtime_links = BTreeSet::new();
    for row in &suite.manifest_rows {
        if !ids.insert(row.id.as_str()) {
            return fail(format!("duplicate manifest disposition {}", row.id));
        }
        let source = source_rows.get(row.id.as_str()).ok_or_else(|| {
            BenchError::Qualification(format!(
                "performance inventory references unknown manifest row {}",
                row.id
            ))
        })?;
        let expected_feature = discovery::manifest_feature(source)?;
        if row.performance_feature != expected_feature
            || row.runtime_group_id != discovery::inherited_runtime_group_id(&row.id)
            || row.disposition != discovery::manifest_disposition(source)
        {
            return fail(format!(
                "manifest row {} has stale feature, runtime parent, or disposition",
                row.id
            ));
        }
        if row.runtime_group_id.as_ref().is_some_and(|group| {
            !runtime_links.insert(group.as_str())
                || row.disposition != PerformanceDisposition::CoveredByParent
        }) {
            return fail(format!(
                "manifest row {} has a duplicate or invalid runtime parent",
                row.id
            ));
        }
        validate_feature_list(
            &row.id,
            expected_feature,
            &row.supporting_performance_features,
        )?;
        validate_thresholds(row, references)?;
        validate_waiver_refs(row, references)?;
        validate_mapping(row, suite)?;
        let classification_count = row.classifications.iter().collect::<BTreeSet<_>>().len();
        if classification_count != row.classifications.len() {
            return fail(format!("manifest row {} repeats a classification", row.id));
        }
        for replacement in &row.replacement_contracts {
            if replacement.legacy_stim_name.is_empty()
                || replacement.legacy_stab_name.is_empty()
                || replacement.runtime_group_id.is_empty()
                || replacement.runtime_measurement_id.is_empty()
            {
                return fail(format!(
                    "manifest row {} has an incomplete replacement contract",
                    row.id
                ));
            }
        }
    }
    if !suite
        .manifest_rows
        .windows(2)
        .all(|pair| matches!(pair, [left, right] if left.id < right.id))
    {
        return fail("manifest dispositions are not in canonical id order");
    }
    Ok(())
}

fn validate_feature_list(
    row_id: &str,
    primary: &str,
    supporting: &[String],
) -> Result<(), BenchError> {
    let feature_ids = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::from([primary]);
    for feature in supporting {
        if !feature_ids.contains(feature.as_str()) || !seen.insert(feature) {
            return fail(format!(
                "manifest row {row_id} has an unknown or duplicate supporting feature {feature}"
            ));
        }
    }
    Ok(())
}

fn validate_thresholds(
    row: &super::model::ManifestRowDisposition,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    let has_threshold = references.threshold_rows.contains(&row.id);
    let expected_refs = has_threshold
        .then(|| "benchmarks/m12-primary-thresholds.json".to_string())
        .into_iter()
        .collect::<Vec<_>>();
    let expected_ratio = references.threshold_ratios.get(&row.id).cloned().flatten();
    let expected_pairs = references
        .threshold_pairs
        .get(&row.id)
        .cloned()
        .unwrap_or_default();
    let actual_pairs = row
        .threshold_measurement_pairs
        .iter()
        .map(|pair| {
            (
                pair.stim_name.clone(),
                pair.stab_name.clone(),
                pair.max_relative_ratio.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    if row.threshold_refs != expected_refs
        || row.threshold_max_relative_ratio != expected_ratio
        || actual_pairs != expected_pairs
        || actual_pairs.len() != row.threshold_measurement_pairs.len()
    {
        return fail(format!(
            "manifest row {} differs from the source threshold policy",
            row.id
        ));
    }
    Ok(())
}

fn validate_waiver_refs(
    row: &super::model::ManifestRowDisposition,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    let mut expected = Vec::new();
    if references.beta_waivers.contains(&row.id) {
        expected.push("benchmarks/m12-primary-beta-waivers.json".to_string());
    }
    if references.regression_waivers.contains(&row.id) {
        expected.push("benchmarks/m12-primary-regression-waivers.json".to_string());
    }
    if row.waiver_refs != expected {
        return fail(format!(
            "manifest row {} differs from the source waiver policy",
            row.id
        ));
    }
    let adapter_waived = references.beta_waivers.contains(&row.id)
        || references.adapter_regression_waivers.contains(&row.id);
    let adapter_candidate = row
        .classifications
        .contains(&RowClassification::AdapterCandidate);
    if adapter_waived && !adapter_candidate
        || references.unstable_pair_waivers.contains(&row.id) && adapter_candidate
    {
        return fail(format!(
            "manifest row {} has a stale waiver classification",
            row.id
        ));
    }
    Ok(())
}

fn validate_mapping(
    row: &super::model::ManifestRowDisposition,
    suite: &QualificationSuite,
) -> Result<(), BenchError> {
    match &row.stim_mapping {
        StimMapping::StimPerf { source, filter } => {
            let known = suite.upstream_perf_sources.iter().any(|candidate| {
                candidate.path == *source
                    && candidate
                        .symbols
                        .iter()
                        .any(|symbol| filter_selects_symbol(filter, symbol))
            });
            if !known {
                return fail(format!(
                    "manifest row {} selects no pinned Stim benchmark symbol",
                    row.id
                ));
            }
        }
        StimMapping::ProcessCli { argv, stdin_path } => {
            if argv.is_empty() || stdin_path.starts_with('/') {
                return fail(format!(
                    "manifest row {} has an invalid process comparator",
                    row.id
                ));
            }
        }
        StimMapping::PlannedAdapter { symbol, source } => {
            if !symbol.starts_with("stim_adapter::") || source.is_empty() {
                return fail(format!(
                    "manifest row {} has an invalid adapter mapping",
                    row.id
                ));
            }
        }
        StimMapping::None { reason } if reason.is_empty() => {
            return fail(format!(
                "manifest row {} has an empty no-comparator reason",
                row.id
            ));
        }
        StimMapping::None { .. } => {}
    }
    Ok(())
}

fn validate_upstream_sources(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
) -> Result<(), BenchError> {
    let row_ids = manifest
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for source in &suite.upstream_perf_sources {
        if !paths.insert(source.path.as_str())
            || !source.path.starts_with("src/")
            || !source.path.ends_with(".perf.cc")
        {
            return fail(format!("invalid upstream perf source {}", source.path));
        }
        for symbol in &source.symbols {
            if symbol.is_empty()
                || !symbol
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || !symbols.insert((source.path.as_str(), symbol.as_str()))
            {
                return fail(format!(
                    "invalid or duplicate upstream symbol {}::{symbol}",
                    source.path
                ));
            }
        }
        if source
            .manifest_rows
            .iter()
            .any(|row| !row_ids.contains(row.as_str()))
        {
            return fail(format!(
                "upstream source {} references an unknown manifest row",
                source.path
            ));
        }
    }
    Ok(())
}

fn validate_waivers(
    suite: &QualificationSuite,
    references: &SourceReferences,
) -> Result<(), BenchError> {
    let expected_ids = references
        .beta_waivers
        .union(&references.regression_waivers)
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_ids = suite
        .waiver_rows
        .iter()
        .map(|waiver| waiver.id.clone())
        .collect::<BTreeSet<_>>();
    if actual_ids != expected_ids || actual_ids.len() != suite.waiver_rows.len() {
        return fail("waiver dispositions do not exactly cover the source waiver ledgers");
    }
    let mut actual_policies = BTreeMap::new();
    for waiver in &suite.waiver_rows {
        if waiver.qualification_disposition != PerformanceDisposition::Diagnostic {
            return fail(format!(
                "waiver {} is not classified as inherited diagnostic evidence",
                waiver.id
            ));
        }
        let no_comparable = waiver
            .policies
            .iter()
            .all(|policy| policy.kind == WaiverKind::NoComparableBaseline);
        if no_comparable
            != waiver
                .retirement_mapping
                .as_deref()
                .is_some_and(|mapping| mapping.starts_with("stim_adapter::"))
        {
            return fail(format!(
                "waiver {} has an invalid retirement mapping",
                waiver.id
            ));
        }
        for policy in &waiver.policies {
            let key = (policy.waiver_file.clone(), waiver.id.clone());
            if actual_policies.insert(key, policy).is_some() {
                return fail(format!("waiver {} repeats a source policy", waiver.id));
            }
        }
    }
    if actual_policies.len() != references.waiver_policies.len()
        || references
            .waiver_policies
            .iter()
            .any(|(key, expected)| actual_policies.get(key) != Some(&expected))
    {
        return fail("waiver policies differ from their source ledgers");
    }
    Ok(())
}

fn filter_selects_symbol(filter: &str, symbol: &str) -> bool {
    filter.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate
            .strip_suffix('*')
            .map_or_else(|| candidate == symbol, |prefix| symbol.starts_with(prefix))
    })
}

fn fail<T>(message: impl Into<String>) -> Result<T, BenchError> {
    Err(BenchError::Qualification(message.into()))
}
