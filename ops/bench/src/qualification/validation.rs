use std::collections::{BTreeMap, BTreeSet};

use super::discovery::{self, PERFORMANCE_FEATURE_IDS, SourceReferences};
use super::model::{
    ChecklistScope, CorrectnessBinding, FixtureLocator, InputByteCount, PerformanceDisposition,
    QualificationStatus, QualificationSuite, RowClassification, RowDecision, RowOrigin,
    SCHEMA_VERSION, StimMapping, ThresholdPolicy,
};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::BenchmarkManifest;

mod counts;
mod planned;
mod source;
mod values;

use counts::{validate_classification_count, validate_decision_count, validate_parent_disposition};
use planned::validate_planned_workload;
use values::{
    filter_matches_any, filter_selects_symbol, is_digest, validate_digest,
    validate_fixture_locator, validate_identifier, validate_relative_path, validate_text,
};

const CORRECTNESS_DIGEST: &str = "5d1fc9d21e511e13bef5ceb476dbcf9dd20ed067339edd2891013992fb06ced5";
const EXPECTED_CHECKLIST_ROWS: usize = 126;
const EXPECTED_PUBLIC_API_ITEMS: usize = 1_922;
const EXPECTED_MANIFEST_ROWS: usize = 161;
const EXPECTED_PERF_SOURCES: usize = 23;
const EXPECTED_PERF_SYMBOLS: usize = 74;
const EXPECTED_WAIVERS: usize = 5;
const MAX_ISSUES: usize = 256;

#[derive(Default)]
struct Issues {
    messages: Vec<String>,
    omitted: usize,
}

impl Issues {
    fn push(&mut self, message: impl Into<String>) {
        if self.messages.len() < MAX_ISSUES {
            self.messages.push(message.into());
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }

    fn finish(mut self) -> Result<(), BenchError> {
        if self.messages.is_empty() {
            return Ok(());
        }
        if self.omitted != 0 {
            self.messages.push(format!(
                "{} additional qualification issues omitted",
                self.omitted
            ));
        }
        Err(BenchError::Qualification(self.messages.join("\n")))
    }
}

pub(super) fn validate(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
    references: &SourceReferences,
    expected_digest: &str,
) -> Result<(), BenchError> {
    let mut issues = Issues::default();
    validate_header(suite, manifest, &mut issues);
    validate_features(suite, &mut issues);
    validate_checklist(suite, &mut issues);
    validate_apis(suite, references, &mut issues);
    validate_groups(suite, manifest, references, &mut issues);
    validate_rows(suite, manifest, references, &mut issues);
    source::validate_upstream_sources(suite, &mut issues);
    source::validate_waivers(suite, references, &mut issues);
    issues.finish()?;

    let computed = discovery::semantic_digest(suite)?;
    if suite.semantic_digest != computed {
        return Err(BenchError::Qualification(format!(
            "semantic digest is {}, computed {computed}",
            suite.semantic_digest
        )));
    }
    if expected_digest != "UNFROZEN" && suite.semantic_digest != expected_digest {
        return Err(BenchError::Qualification(format!(
            "semantic digest is {}, expected frozen {expected_digest}",
            suite.semantic_digest
        )));
    }
    Ok(())
}

fn validate_header(suite: &QualificationSuite, manifest: &BenchmarkManifest, issues: &mut Issues) {
    if suite.schema_version != SCHEMA_VERSION {
        issues.push(format!(
            "schema version is {}, expected {SCHEMA_VERSION}",
            suite.schema_version
        ));
    }
    if suite.stim_version != STIM_TAG || suite.stim_commit != STIM_COMMIT {
        issues.push("Stim version or commit differs from the frozen compatibility target");
    }
    if suite.correctness_digest != CORRECTNESS_DIGEST {
        issues.push(format!(
            "correctness digest is {}, expected {CORRECTNESS_DIGEST}",
            suite.correctness_digest
        ));
    }
    for (label, actual, expected) in [
        ("performance features", suite.performance_features.len(), 16),
        (
            "checklist rows",
            suite.checklist_items.len(),
            EXPECTED_CHECKLIST_ROWS,
        ),
        (
            "public API items",
            suite.public_api_items.len(),
            EXPECTED_PUBLIC_API_ITEMS,
        ),
        (
            "manifest dispositions",
            suite.manifest_rows.len(),
            EXPECTED_MANIFEST_ROWS,
        ),
        (
            "benchmark manifest rows",
            manifest.rows.len(),
            EXPECTED_MANIFEST_ROWS,
        ),
        (
            "upstream perf sources",
            suite.upstream_perf_sources.len(),
            EXPECTED_PERF_SOURCES,
        ),
        ("waiver rows", suite.waiver_rows.len(), EXPECTED_WAIVERS),
    ] {
        if actual != expected {
            issues.push(format!("{label} has {actual} rows, expected {expected}"));
        }
    }
}

fn validate_features(suite: &QualificationSuite, issues: &mut Issues) {
    let expected = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let group_ids = suite
        .qualification_groups
        .iter()
        .map(|group| group.id.as_str())
        .collect::<BTreeSet<_>>();
    for feature in &suite.performance_features {
        validate_identifier("performance feature", &feature.id, issues);
        validate_text("feature reason", &feature.reason, issues);
        if !seen.insert(feature.id.as_str()) {
            issues.push(format!("duplicate performance feature {}", feature.id));
        }
        if !expected.contains(feature.id.as_str()) {
            issues.push(format!("unknown performance feature {}", feature.id));
        }
        let mut local_groups = BTreeSet::new();
        for group in &feature.group_ids {
            if !local_groups.insert(group) {
                issues.push(format!("feature {} repeats group {group}", feature.id));
            }
            if !group_ids.contains(group.as_str()) {
                issues.push(format!(
                    "feature {} references unknown group {group}",
                    feature.id
                ));
            }
        }
        if feature.disposition == PerformanceDisposition::Measured && feature.group_ids.is_empty() {
            issues.push(format!(
                "measured feature {} has no measured groups",
                feature.id
            ));
        }
    }
    for missing in expected.difference(&seen) {
        issues.push(format!("missing performance feature {missing}"));
    }
}

fn validate_checklist(suite: &QualificationSuite, issues: &mut Issues) {
    let mut ids = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    let mut done = 0;
    let mut partial = 0;
    let mut deferred = 0;
    let mut global_child_domains = BTreeSet::new();
    let feature_ids = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let group_ids = suite
        .qualification_groups
        .iter()
        .map(|group| group.id.as_str())
        .collect::<BTreeSet<_>>();
    let groups = suite
        .qualification_groups
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    for item in &suite.checklist_items {
        validate_identifier("checklist item", &item.id, issues);
        validate_digest("checklist anchor", &item.anchor_digest, issues);
        validate_text("checklist section", &item.section, issues);
        validate_text("checklist feature", &item.feature, issues);
        validate_text("checklist reason", &item.reason, issues);
        if item.source_line == 0 {
            issues.push(format!("checklist item {} has source line zero", item.id));
        }
        if !ids.insert(item.id.as_str()) {
            issues.push(format!("duplicate checklist id {}", item.id));
        }
        if !anchors.insert((item.source_line, item.anchor_digest.as_str())) {
            issues.push(format!("duplicate checklist anchor for {}", item.id));
        }
        match item.raw_status.as_str() {
            value if value.starts_with("Done") => {
                done += 1;
                if item.scope != ChecklistScope::Selected
                    || item.selected_child.is_none()
                    || item.deferred_child.is_some()
                    || item.deferred_remainder
                    || item.selected_child_ids.is_empty()
                    || !item.deferred_child_ids.is_empty()
                {
                    issues.push(format!(
                        "done checklist item {} has an invalid split",
                        item.id
                    ));
                }
            }
            value if value.starts_with("Partial") => {
                partial += 1;
                if item.scope != ChecklistScope::Selected
                    || item.selected_child.is_none()
                    || item.deferred_child.is_none()
                    || !item.deferred_remainder
                    || item.selected_child_ids.is_empty()
                    || item.deferred_child_ids.is_empty()
                {
                    issues.push(format!(
                        "partial checklist item {} lacks both children",
                        item.id
                    ));
                }
            }
            value if value.starts_with("Deferred") => {
                deferred += 1;
                if item.scope != ChecklistScope::Deferred
                    || item.selected_child.is_some()
                    || item.deferred_child.is_none()
                    || item.deferred_remainder
                    || !item.selected_child_ids.is_empty()
                    || item.deferred_child_ids.is_empty()
                {
                    issues.push(format!(
                        "deferred checklist item {} has an invalid split",
                        item.id
                    ));
                }
            }
            value => issues.push(format!(
                "checklist item {} has unknown status {value:?}",
                item.id
            )),
        }
        let mut child_ids = BTreeSet::new();
        for child in item
            .selected_child_ids
            .iter()
            .chain(&item.deferred_child_ids)
        {
            validate_identifier("checklist child", child, issues);
            if !child_ids.insert(child.as_str()) {
                issues.push(format!(
                    "checklist item {} repeats child id {child}",
                    item.id
                ));
            }
        }
        let mut owned_children = BTreeSet::new();
        let mut owned_features = BTreeSet::new();
        for ownership in &item.selected_child_ownership {
            validate_identifier("owned checklist child", &ownership.child_id, issues);
            if !owned_children.insert(ownership.child_id.as_str()) {
                issues.push(format!(
                    "checklist item {} repeats child ownership {}",
                    item.id, ownership.child_id
                ));
            }
            let mut child_features = BTreeSet::new();
            for feature in &ownership.performance_features {
                if !feature_ids.contains(feature.as_str())
                    || !item.performance_features.contains(feature)
                {
                    issues.push(format!(
                        "checklist item {} child {} owns unrelated feature {feature}",
                        item.id, ownership.child_id
                    ));
                }
                if !child_features.insert(feature.as_str()) {
                    issues.push(format!(
                        "checklist item {} child {} repeats feature {feature}",
                        item.id, ownership.child_id
                    ));
                }
                if !global_child_domains.insert((ownership.child_id.as_str(), feature.as_str())) {
                    issues.push(format!(
                        "checklist child {} has duplicate primary ownership in {feature}",
                        ownership.child_id
                    ));
                }
                owned_features.insert(feature.as_str());
            }
            if ownership.performance_features.is_empty() {
                issues.push(format!(
                    "checklist item {} child {} owns no performance domain",
                    item.id, ownership.child_id
                ));
            }
        }
        let selected_children = item
            .selected_child_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let item_features = item
            .performance_features
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let ownership_matches = if item_features.is_empty() {
            owned_children.is_empty() && owned_features.is_empty()
        } else {
            owned_children == selected_children && owned_features == item_features
        };
        if !ownership_matches {
            issues.push(format!(
                "checklist item {} child ownership does not exactly cover selected ids and performance domains",
                item.id
            ));
        }
        for feature in &item.performance_features {
            if !feature_ids.contains(feature.as_str()) {
                issues.push(format!(
                    "checklist item {} references unknown {feature}",
                    item.id
                ));
            }
        }
        for group in &item.parent_group_ids {
            if !group_ids.contains(group.as_str()) {
                issues.push(format!(
                    "checklist item {} references unknown group {group}",
                    item.id
                ));
            } else if !groups.get(group.as_str()).is_some_and(|parent| {
                let expected_children = item
                    .selected_child_ownership
                    .iter()
                    .filter(|ownership| {
                        ownership
                            .performance_features
                            .contains(&parent.performance_feature)
                    })
                    .map(|ownership| ownership.child_id.as_str())
                    .collect::<Vec<_>>();
                parent.disposition == PerformanceDisposition::Measured
                    && parent.checklist_anchors == [item.id.as_str()]
                    && parent.checklist_child_ids == expected_children
                    && item
                        .performance_features
                        .contains(&parent.performance_feature)
                    && parent.id.starts_with("PERFQ-CHECKLIST-")
            }) {
                issues.push(format!(
                    "checklist item {} parent {group} is not its exact measured feature parent",
                    item.id
                ));
            }
        }
        validate_parent_disposition(
            "checklist item",
            &item.id,
            item.disposition,
            &item.parent_group_ids,
            issues,
        );
    }
    if (done, partial, deferred) != (73, 7, 46) {
        issues.push(format!(
            "checklist status counts are done={done} partial={partial} deferred={deferred}, expected 73/7/46"
        ));
    }
}

fn validate_apis(suite: &QualificationSuite, references: &SourceReferences, issues: &mut Issues) {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut kinds = BTreeMap::<&str, usize>::new();
    let feature_ids = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let measured_groups = suite
        .qualification_groups
        .iter()
        .filter(|group| group.disposition == PerformanceDisposition::Measured)
        .map(|group| (group.id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    for item in &suite.public_api_items {
        validate_identifier("public API item", &item.id, issues);
        validate_text("public API path", &item.path, issues);
        validate_text("public API reason", &item.reason, issues);
        let declared_performance_groups = std::iter::once(&item.performance_feature)
            .chain(&item.supporting_performance_features)
            .cloned()
            .collect::<Vec<_>>();
        if !references.public_api.get(&item.id).is_some_and(|source| {
            source.path == item.path
                && source.kind == item.kind
                && source.owner_case_id == item.correctness_case_id
                && source.performance_groups == declared_performance_groups
        }) {
            issues.push(format!(
                "public API {} differs from its exact CQ0 path, kind, owner, or performance domains",
                item.id
            ));
        }
        if !references
            .correctness_cases
            .contains(&item.correctness_case_id)
        {
            issues.push(format!(
                "public API {} references unknown exact correctness owner {}",
                item.id, item.correctness_case_id
            ));
        }
        if !ids.insert(item.id.as_str()) {
            issues.push(format!("duplicate public API id {}", item.id));
        }
        if !paths.insert(item.path.as_str()) {
            issues.push(format!("duplicate public API path {}", item.path));
        }
        *kinds.entry(&item.kind).or_default() += 1;
        if !feature_ids.contains(item.performance_feature.as_str()) {
            issues.push(format!(
                "public API {} references unknown feature {}",
                item.id, item.performance_feature
            ));
        }
        let mut api_features = BTreeSet::from([item.performance_feature.as_str()]);
        for feature in &item.supporting_performance_features {
            if !feature_ids.contains(feature.as_str()) {
                issues.push(format!(
                    "public API {} references unknown supporting feature {feature}",
                    item.id
                ));
            }
            if !api_features.insert(feature.as_str()) {
                issues.push(format!(
                    "public API {} repeats performance feature {feature}",
                    item.id
                ));
            }
        }
        match item.disposition {
            PerformanceDisposition::CoveredByParent => {
                let mut parent_features = BTreeSet::new();
                for parent in &item.parent_group_ids {
                    if let Some(group) = measured_groups.get(parent.as_str()).filter(|group| {
                        group.public_api_items.contains(&item.path)
                            && api_features.contains(group.performance_feature.as_str())
                            && group.correctness_binding == CorrectnessBinding::ExactApiOwners
                            && group.correctness_cases.contains(&item.correctness_case_id)
                            && group.id.starts_with("PERFQ-API-")
                    }) {
                        parent_features.insert(group.performance_feature.as_str());
                    } else {
                        issues.push(format!(
                            "public API {} parent {parent} is absent, cross-domain, or not measured",
                            item.id
                        ));
                    }
                }
                if parent_features != api_features {
                    issues.push(format!(
                        "public API {} parent domains do not preserve all CQ0 performance domains",
                        item.id
                    ));
                }
            }
            PerformanceDisposition::NotPerformanceRelevant => {
                if !item.parent_group_ids.is_empty() {
                    issues.push(format!(
                        "non-performance API {} has a parent group",
                        item.id
                    ));
                }
            }
            other => issues.push(format!(
                "PQ0 public API {} has unsupported disposition {other:?}",
                item.id
            )),
        }
    }
    let expected = BTreeMap::from([
        ("constant", 1),
        ("enum", 31),
        ("field", 190),
        ("function", 70),
        ("method", 612),
        ("struct", 83),
        ("trait-impl", 694),
        ("type-alias", 7),
        ("variant", 234),
    ]);
    if kinds != expected {
        issues.push(format!("public API kind counts are stale: {kinds:?}"));
    }
}

fn validate_groups(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
    references: &SourceReferences,
    issues: &mut Issues,
) {
    let manifest_ids = manifest
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let manifest_by_id = manifest
        .rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let correctness_ids = suite
        .qualification_groups
        .iter()
        .flat_map(|group| group.correctness_cases.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let checklist_ids = suite
        .checklist_items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let api_paths = suite
        .public_api_items
        .iter()
        .map(|item| item.path.as_str())
        .collect::<BTreeSet<_>>();
    let feature_ids = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut primary_rows = BTreeSet::new();
    for group in &suite.qualification_groups {
        validate_identifier("qualification group", &group.id, issues);
        if !ids.insert(group.id.as_str()) {
            issues.push(format!("duplicate qualification group {}", group.id));
        }
        validate_identifier("qualification primary row", &group.manifest_row, issues);
        if !primary_rows.insert(group.manifest_row.as_str()) {
            issues.push(format!(
                "duplicate qualification primary row {}",
                group.manifest_row
            ));
        }
        if group.row_origin == RowOrigin::Inherited {
            if !manifest_ids.contains(group.manifest_row.as_str()) {
                issues.push(format!(
                    "group {} references unknown manifest row {}",
                    group.id, group.manifest_row
                ));
            }
            if let Some(row) = manifest_by_id.get(group.manifest_row.as_str()) {
                let inherited_scale = group.workload_family.scales.first();
                if row.stdin_path.is_empty() {
                    if !matches!(
                        &group.workload_family.fixture,
                        FixtureLocator::Inline { id } if id == &row.id
                    ) || group.workload_family.deterministic_seed
                        != format!("source-owned-inline:{}", row.id)
                        || inherited_scale
                            .is_none_or(|scale| scale.input_bytes != InputByteCount::NotApplicable)
                    {
                        issues.push(format!(
                            "inherited inline group {} has an invalid corpus or seed contract",
                            group.id
                        ));
                    }
                } else {
                    let fixture_matches = matches!(
                        &group.workload_family.fixture,
                        FixtureLocator::RepositoryFile { path, sha256 }
                            if path == &row.stdin_path && is_digest(sha256)
                    );
                    if !fixture_matches
                        || group.workload_family.deterministic_seed != "corpus-digest-owned"
                        || inherited_scale.is_none_or(|scale| {
                            !matches!(scale.input_bytes, InputByteCount::Exact { bytes } if bytes > 0)
                        })
                    {
                        issues.push(format!(
                            "inherited fixture group {} lacks a typed path, byte length, or corpus digest",
                            group.id
                        ));
                    }
                }
            }
        } else if manifest_ids.contains(group.manifest_row.as_str()) {
            issues.push(format!(
                "planned group {} reuses inherited manifest row {}",
                group.id, group.manifest_row
            ));
        } else {
            validate_planned_workload(group, references, issues);
        }
        validate_fixture_locator(&group.workload_family.fixture, issues);
        validate_relative_path(
            "qualification workload source",
            &group.workload_family.source,
            issues,
        );
        if !feature_ids.contains(group.performance_feature.as_str()) {
            issues.push(format!(
                "group {} references unknown feature {}",
                group.id, group.performance_feature
            ));
        }
        if group.disposition == PerformanceDisposition::Measured {
            if group.workload_family.scales.is_empty() {
                issues.push(format!("measured group {} lacks scales", group.id));
            }
            if group.work_unit.trim().is_empty() {
                issues.push(format!("measured group {} lacks a work unit", group.id));
            }
        }
        if group.disposition == PerformanceDisposition::NoFaithfulStimComparator {
            issues.push(format!(
                "PQ0 group {} claims no faithful Stim comparator despite a declared runner or adapter path",
                group.id
            ));
        }
        validate_text(
            "qualification workload source",
            &group.workload_family.source,
            issues,
        );
        validate_text(
            "qualification deterministic seed",
            &group.workload_family.deterministic_seed,
            issues,
        );
        validate_text("qualification work unit", &group.work_unit, issues);
        validate_text(
            "qualification output shape",
            &group.output_contract.expected_shape,
            issues,
        );
        validate_text(
            "qualification output sink policy",
            &group.output_contract.sink_policy,
            issues,
        );
        validate_text(
            "qualification gate statistic",
            &group.timing_policy.gate_statistic,
            issues,
        );
        validate_text(
            "qualification memory growth",
            &group.memory_policy.expected_growth,
            issues,
        );
        validate_text("qualification owner", &group.owner, issues);
        match group.correctness_binding {
            CorrectnessBinding::ExactApiOwners
                if group.correctness_cases.is_empty()
                    || group.planned_correctness_case_id.is_some() =>
            {
                issues.push(format!(
                    "API-bound group {} lacks exact CQ owners",
                    group.id
                ));
            }
            CorrectnessBinding::ExactApiOwners if !group.id.starts_with("PERFQ-API-") => {
                issues.push(format!(
                    "non-API group {} claims exact API correctness owners",
                    group.id
                ));
            }
            CorrectnessBinding::Unresolved
                if !group.correctness_cases.is_empty()
                    || group.planned_correctness_case_id.is_none() =>
            {
                issues.push(format!(
                    "unresolved group {} lacks one planned correctness dependency or lists borrowed cases",
                    group.id
                ));
            }
            _ => {}
        }
        if let Some(planned) = &group.planned_correctness_case_id {
            validate_identifier("planned correctness case", planned, issues);
        }
        for case in &group.correctness_cases {
            validate_identifier("correctness case", case, issues);
            if !references.correctness_cases.contains(case) {
                issues.push(format!(
                    "group {} references unknown correctness case {case}",
                    group.id
                ));
            }
        }
        for anchor in &group.checklist_anchors {
            if !checklist_ids.contains(anchor.as_str()) {
                issues.push(format!(
                    "group {} references unknown checklist {anchor}",
                    group.id
                ));
            }
        }
        for path in &group.public_api_items {
            if !api_paths.contains(path.as_str()) {
                issues.push(format!(
                    "group {} references unknown public API {path}",
                    group.id
                ));
            }
        }
        let mut scale_ids = BTreeSet::new();
        for scale in &group.workload_family.scales {
            validate_identifier("scale", &scale.id, issues);
            if !scale_ids.insert(scale.id.as_str()) {
                issues.push(format!("group {} repeats scale {}", group.id, scale.id));
            }
            validate_text("scale parameters", &scale.parameters, issues);
        }
        for scale in &group.memory_policy.scale_ids {
            if !scale_ids.contains(scale.as_str()) {
                issues.push(format!(
                    "group {} memory policy references unknown scale {scale}",
                    group.id
                ));
            }
        }
        if group.status != QualificationStatus::Planned {
            issues.push(format!("PQ0 group {} is not planned", group.id));
        }
        validate_text("qualification group reason", &group.reason, issues);
        if group.id.starts_with("PERFQ-API-")
            && (group.public_api_items.is_empty()
                || !group
                    .output_contract
                    .expected_shape
                    .contains("exact named submeasurement"))
        {
            issues.push(format!(
                "API group {} lacks exact path ownership or submeasurement policy",
                group.id
            ));
        }
        if group.id.starts_with("PERFQ-CHECKLIST-") && group.checklist_anchors.len() != 1 {
            issues.push(format!(
                "checklist group {} owns {} anchors instead of one",
                group.id,
                group.checklist_anchors.len()
            ));
        }
        if group.id.starts_with("PERFQ-CHECKLIST-") && group.checklist_child_ids.is_empty() {
            issues.push(format!(
                "checklist group {} owns no exact selected child ids",
                group.id
            ));
        }
        if !group.id.starts_with("PERFQ-CHECKLIST-")
            && (!group.checklist_anchors.is_empty() || !group.checklist_child_ids.is_empty())
        {
            issues.push(format!(
                "non-checklist group {} claims checklist anchors or children",
                group.id
            ));
        }
    }
    if correctness_ids.is_empty() {
        issues.push("qualification groups reference no correctness cases");
    }
}

fn validate_rows(
    suite: &QualificationSuite,
    manifest: &BenchmarkManifest,
    references: &SourceReferences,
    issues: &mut Issues,
) {
    let expected = manifest
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    for orphan in references
        .threshold_rows
        .iter()
        .chain(references.beta_waivers.iter())
        .chain(references.regression_waivers.iter())
        .filter(|id| !expected.contains(id.as_str()))
    {
        issues.push(format!(
            "threshold or waiver source references unknown manifest row {orphan}"
        ));
    }
    let groups = suite
        .qualification_groups
        .iter()
        .map(|group| (group.id.as_str(), group))
        .collect::<BTreeMap<_, _>>();
    let perf_sources = suite
        .upstream_perf_sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let feature_ids = PERFORMANCE_FEATURE_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for row in &suite.manifest_rows {
        if !seen.insert(row.id.as_str()) {
            issues.push(format!("duplicate manifest disposition {}", row.id));
        }
        if !expected.contains(row.id.as_str()) {
            issues.push(format!("unknown manifest disposition {}", row.id));
        }
        match groups.get(row.primary_group_id.as_str()) {
            Some(group)
                if group.row_origin == RowOrigin::Inherited && group.manifest_row == row.id => {}
            Some(_) => issues.push(format!(
                "manifest row {} primary group does not own the row",
                row.id
            )),
            None => issues.push(format!(
                "manifest row {} references unknown group {}",
                row.id, row.primary_group_id
            )),
        }
        let primary_feature = groups
            .get(row.primary_group_id.as_str())
            .map(|group| group.performance_feature.as_str());
        let mut supporting = BTreeSet::new();
        for feature in &row.supporting_performance_features {
            if !feature_ids.contains(feature.as_str()) {
                issues.push(format!(
                    "manifest row {} references unknown supporting feature {feature}",
                    row.id
                ));
            }
            if !supporting.insert(feature.as_str()) {
                issues.push(format!(
                    "manifest row {} repeats supporting feature {feature}",
                    row.id
                ));
            }
            if primary_feature == Some(feature.as_str()) {
                issues.push(format!(
                    "manifest row {} repeats its primary feature as supporting",
                    row.id
                ));
            }
        }
        if row.classifications.is_empty() {
            issues.push(format!("manifest row {} is unclassified", row.id));
        }
        let mut measurement_pairs = BTreeSet::new();
        for pair in &row.threshold_measurement_pairs {
            validate_text("Stim threshold measurement", &pair.stim_name, issues);
            validate_text("Stab threshold measurement", &pair.stab_name, issues);
            if pair.max_relative_ratio != "1.25" {
                issues.push(format!(
                    "manifest row {} measurement pair has ratio {}, expected 1.25",
                    row.id, pair.max_relative_ratio
                ));
            }
            if !measurement_pairs.insert((
                &pair.stim_name,
                &pair.stab_name,
                &pair.max_relative_ratio,
            )) {
                issues.push(format!(
                    "manifest row {} repeats threshold measurement pair {:?}/{:?}",
                    row.id, pair.stim_name, pair.stab_name
                ));
            }
        }
        if !row.threshold_measurement_pairs.is_empty() && row.threshold_refs.is_empty() {
            issues.push(format!(
                "manifest row {} has threshold pairs without a threshold source",
                row.id
            ));
        }
        let thresholded = references.threshold_rows.contains(&row.id);
        if thresholded != !row.threshold_refs.is_empty() {
            issues.push(format!(
                "manifest row {} threshold references disagree with the source threshold ledger",
                row.id
            ));
        }
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
        if actual_pairs != expected_pairs {
            issues.push(format!(
                "manifest row {} measurement pairs disagree with the source threshold ledger",
                row.id
            ));
        }
        let expected_ratio = references.threshold_ratios.get(&row.id).cloned().flatten();
        if row.threshold_max_relative_ratio != expected_ratio {
            issues.push(format!(
                "manifest row {} row-level ratio disagrees with the source threshold ledger",
                row.id
            ));
        }
        if row
            .threshold_max_relative_ratio
            .as_deref()
            .is_some_and(|ratio| ratio != "1.25")
        {
            issues.push(format!(
                "manifest row {} row-level ratio is not the 1.25 target",
                row.id
            ));
        }
        let expected_waiver_refs = [
            references
                .beta_waivers
                .contains(&row.id)
                .then_some("benchmarks/m12-primary-beta-waivers.json"),
            references
                .regression_waivers
                .contains(&row.id)
                .then_some("benchmarks/m12-primary-regression-waivers.json"),
        ]
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
        let actual_waiver_refs = row
            .waiver_refs
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual_waiver_refs != expected_waiver_refs {
            issues.push(format!(
                "manifest row {} waiver references disagree with source waiver ledgers",
                row.id
            ));
        }
        if row
            .classifications
            .contains(&RowClassification::HeterogeneousMeasurements)
            && !row
                .classifications
                .contains(&RowClassification::UnmatchedSubmeasurement)
            && row.threshold_refs.is_empty()
        {
            issues.push(format!(
                "heterogeneous row {} has no exact threshold pairing or unmatched marker",
                row.id
            ));
        }
        match (&row.stim_mapping, row.decision) {
            (StimMapping::None { .. }, RowDecision::Diagnostic) => {}
            (StimMapping::None { .. }, _) => issues.push(format!(
                "manifest row {} lacks a Stim mapping without a diagnostic decision",
                row.id
            )),
            _ => {}
        }
        match &row.stim_mapping {
            StimMapping::StimPerf { source, filter } => match perf_sources.get(source.as_str()) {
                Some(perf_source) if filter_matches_any(filter, &perf_source.symbols) => {
                    for pair in &row.threshold_measurement_pairs {
                        if !perf_source.symbols.contains(&pair.stim_name)
                            || !filter_selects_symbol(filter, &pair.stim_name)
                        {
                            issues.push(format!(
                                "manifest row {} threshold names unknown Stim measurement {:?}",
                                row.id, pair.stim_name
                            ));
                        }
                    }
                }
                Some(_) => issues.push(format!(
                    "manifest row {} Stim filter {filter:?} matches no symbol in {source}",
                    row.id
                )),
                None => issues.push(format!(
                    "manifest row {} references unknown Stim perf source {source}",
                    row.id
                )),
            },
            StimMapping::ProcessCli { argv, stdin_path } => {
                validate_text("process CLI argv", argv, issues);
                if !stdin_path.is_empty() {
                    validate_relative_path("process CLI stdin", stdin_path, issues);
                }
            }
            StimMapping::PlannedAdapter { symbol, source } => {
                validate_identifier("planned adapter symbol", symbol, issues);
                validate_relative_path("planned adapter source", source, issues);
                if !source.starts_with("src/")
                    && !source.starts_with("doc/")
                    && row.decision != RowDecision::Removed
                {
                    issues.push(format!(
                        "manifest row {} adapter source is not a pinned Stim source",
                        row.id
                    ));
                }
            }
            StimMapping::None { reason } => {
                validate_text("missing comparator reason", reason, issues)
            }
        }
        if row
            .classifications
            .contains(&RowClassification::InProcessProcessMismatch)
            && groups
                .get(row.primary_group_id.as_str())
                .is_some_and(|group| group.threshold_policy == ThresholdPolicy::Primary1_25)
        {
            issues.push(format!(
                "manifest row {} uses an asymmetric in-process/process primary gate",
                row.id
            ));
        }
        if row
            .classifications
            .contains(&RowClassification::UnmatchedSubmeasurement)
            && groups
                .get(row.primary_group_id.as_str())
                .is_some_and(|group| group.threshold_policy != ThresholdPolicy::ReportOnly)
        {
            issues.push(format!(
                "manifest row {} claims a threshold despite unmatched Stim submeasurements",
                row.id
            ));
        }
        if !row.waiver_refs.is_empty()
            && !row
                .classifications
                .contains(&RowClassification::AdapterCandidate)
        {
            issues.push(format!(
                "waived row {} does not name an adapter retirement path",
                row.id
            ));
        }
    }
    for missing in expected.difference(&seen) {
        issues.push(format!("manifest row {missing} has no disposition"));
    }
    let mut primary_owners = BTreeMap::<&str, usize>::new();
    for row in &suite.manifest_rows {
        if let Some(group) = groups.get(row.primary_group_id.as_str()) {
            *primary_owners
                .entry(group.performance_feature.as_str())
                .or_default() += 1;
        }
    }
    let expected_primary_owners = BTreeMap::from([
        ("PERF-CIRCUIT-MODEL", 8),
        ("PERF-DEM-MODEL", 9),
        ("PERF-RESULT-IO", 6),
        ("PERF-GATE-CONTRACT", 3),
        ("PERF-BIT-KERNELS", 5),
        ("PERF-STABILIZER-ALGEBRA", 6),
        ("PERF-GENERATION", 23),
        ("PERF-CONVERT-CLI", 11),
        ("PERF-SAMPLING", 10),
        ("PERF-DETECTION", 15),
        ("PERF-DEM-SAMPLING", 7),
        ("PERF-ERROR-ANALYSIS", 11),
        ("PERF-SEARCH-AND-MATCHING", 21),
        ("PERF-FLOWS-AND-DETECTOR-UTILITIES", 22),
        ("PERF-CLI-STARTUP-AND-ERRORS", 3),
        ("PERF-RESOURCE-BOUNDARIES", 1),
    ]);
    if primary_owners != expected_primary_owners {
        issues.push(format!(
            "manifest primary performance ownership is stale: {primary_owners:?}"
        ));
    }
    validate_decision_count(suite, RowDecision::Retained, 15, issues);
    validate_decision_count(suite, RowDecision::Reworked, 135, issues);
    validate_decision_count(suite, RowDecision::Diagnostic, 4, issues);
    validate_decision_count(suite, RowDecision::Superseded, 5, issues);
    validate_decision_count(suite, RowDecision::Removed, 2, issues);
    validate_classification_count(suite, RowClassification::Faithful, 15, issues);
    validate_classification_count(suite, RowClassification::Diagnostic, 134, issues);
    validate_classification_count(suite, RowClassification::Proxy, 10, issues);
    validate_classification_count(suite, RowClassification::Stale, 2, issues);
    validate_classification_count(suite, RowClassification::Duplicate, 5, issues);
    validate_classification_count(suite, RowClassification::MissingScale, 124, issues);
    validate_classification_count(
        suite,
        RowClassification::MissingCorrectnessPreflight,
        159,
        issues,
    );
    validate_classification_count(suite, RowClassification::MissingOutputDigest, 159, issues);
    validate_classification_count(suite, RowClassification::MissingComparator, 73, issues);
    validate_classification_count(suite, RowClassification::AdapterCandidate, 73, issues);
    validate_classification_count(
        suite,
        RowClassification::InProcessProcessMismatch,
        58,
        issues,
    );
    validate_classification_count(
        suite,
        RowClassification::HeterogeneousMeasurements,
        21,
        issues,
    );
    validate_classification_count(
        suite,
        RowClassification::UnmatchedSubmeasurement,
        15,
        issues,
    );
}

#[cfg(all(test, unix))]
mod tests;
