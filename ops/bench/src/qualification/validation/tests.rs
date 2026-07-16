use std::path::Path;

use super::*;
use crate::root::RepoRoot;

fn fixture() -> (QualificationSuite, BenchmarkManifest, SourceReferences) {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("resolve repository root");
    let manifest = BenchmarkManifest::read(&root).expect("read benchmark manifest");
    let suite = discovery::generate(&root, &manifest).expect("generate qualification suite");
    let references = discovery::load_source_references(&root).expect("load source references");
    (suite, manifest, references)
}

#[test]
fn validation_rejects_unknown_correctness_fixture_and_measurement_ids() {
    let (mut suite, manifest, references) = fixture();
    let correctness_case = suite
        .qualification_groups
        .iter_mut()
        .find_map(|group| group.correctness_cases.first_mut())
        .expect("qualification correctness case");
    *correctness_case = "CQ-UNKNOWN".to_string();
    suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.row_origin == RowOrigin::Inherited)
        .expect("inherited qualification group")
        .workload_family
        .fixture = FixtureLocator::RepositoryFile {
        path: "oracle/fixtures/inputs/not-owned.data".to_string(),
        sha256: "0".repeat(64),
    };
    let thresholded = suite
        .manifest_rows
        .iter_mut()
        .find(|row| !row.threshold_measurement_pairs.is_empty())
        .expect("thresholded row");
    thresholded
        .threshold_measurement_pairs
        .first_mut()
        .expect("threshold measurement pair")
        .stim_name = "unknown-measurement".to_string();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("unknown references must fail");
    let message = error.to_string();
    assert!(message.contains("unknown correctness case"));
    assert!(message.contains("fixture"));
    assert!(message.contains("measurement pairs disagree"));
}

#[test]
fn validation_rejects_unknown_feature_manifest_threshold_and_waiver_ids() {
    let (mut suite, manifest, mut references) = fixture();
    suite
        .performance_features
        .first_mut()
        .expect("performance feature")
        .id = "PERF-UNKNOWN".to_string();
    suite.manifest_rows.first_mut().expect("manifest row").id = "unknown-manifest-row".to_string();
    references
        .threshold_rows
        .insert("unknown-threshold-row".to_string());
    references
        .beta_waivers
        .insert("unknown-waiver-row".to_string());

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("unknown inventory ids must fail");
    let message = error.to_string();
    assert!(message.contains("unknown performance feature"));
    assert!(message.contains("unknown manifest disposition"));
    assert!(message.contains("threshold references disagree"));
    assert!(message.contains("waiver disposition ids"));
}

#[test]
fn validation_rejects_nonmeasured_parents_and_false_no_comparator_waivers() {
    let (mut suite, manifest, references) = fixture();
    let removed_group = suite
        .qualification_groups
        .iter()
        .find(|group| group.disposition == PerformanceDisposition::NotPerformanceRelevant)
        .expect("removed group")
        .id
        .clone();
    let api = suite
        .public_api_items
        .iter_mut()
        .find(|item| item.disposition == PerformanceDisposition::CoveredByParent)
        .expect("covered API");
    api.parent_group_ids = vec![removed_group];
    suite
        .waiver_rows
        .first_mut()
        .expect("waiver row")
        .qualification_disposition = PerformanceDisposition::NoFaithfulStimComparator;

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("invalid parent and waiver must fail");
    let message = error.to_string();
    assert!(message.contains("absent, cross-domain, or not measured"));
    assert!(message.contains("incorrectly promoted"));
}

#[test]
fn validation_rejects_asymmetric_primary_cli_and_stale_stim_filter() {
    let (mut suite, manifest, references) = fixture();
    let cli_row = suite
        .manifest_rows
        .iter()
        .find(|row| {
            row.classifications
                .contains(&RowClassification::InProcessProcessMismatch)
        })
        .expect("asymmetric CLI row")
        .primary_group_id
        .clone();
    suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == cli_row)
        .expect("CLI group")
        .threshold_policy = ThresholdPolicy::Primary1_25;
    let perf_row = suite
        .manifest_rows
        .iter_mut()
        .find(|row| matches!(row.stim_mapping, StimMapping::StimPerf { .. }))
        .expect("Stim perf row");
    if let StimMapping::StimPerf { filter, .. } = &mut perf_row.stim_mapping {
        *filter = "definitely_missing_symbol".to_string();
    }

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("unfaithful primary mappings must fail");
    let message = error.to_string();
    assert!(message.contains("asymmetric in-process/process primary gate"));
    assert!(message.contains("matches no symbol"));
}

#[test]
fn reworked_heterogeneous_row_can_point_to_an_exact_replacement_group() {
    let (suite, manifest, references) = fixture();
    let row = suite
        .manifest_rows
        .iter()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR legacy row");
    assert_eq!(row.decision, RowDecision::Reworked);
    assert!(
        row.classifications
            .contains(&RowClassification::UnmatchedSubmeasurement)
    );
    let group = suite
        .qualification_groups
        .iter()
        .find(|group| group.id == row.primary_group_id)
        .expect("dense XOR replacement group");
    assert_eq!(group.threshold_policy, ThresholdPolicy::Primary1_25);
    assert_eq!(
        row.replacement_contracts
            .first()
            .expect("one dense XOR replacement")
            .runtime_measurement_id,
        "xor-complete-vector"
    );

    validate(&suite, &manifest, &references, "UNFROZEN")
        .expect("reworked legacy row must not constrain its exact replacement group");
}

#[test]
fn reworked_heterogeneous_primary_threshold_requires_an_exact_replacement_mapping() {
    let (mut suite, manifest, references) = fixture();
    suite
        .manifest_rows
        .iter_mut()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR legacy row")
        .replacement_contracts
        .clear();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("unmapped heterogeneous primary threshold must fail");
    assert!(
        error
            .to_string()
            .contains("without an exact primary replacement mapping")
    );
}

#[test]
fn replacement_mapping_rejects_stale_sources_and_nonpromotable_targets() {
    let (mut suite, manifest, references) = fixture();
    let replacement = suite
        .manifest_rows
        .iter_mut()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR legacy row")
        .replacement_contracts
        .first_mut()
        .expect("dense XOR replacement");
    replacement.legacy_stim_name = "simd_bits_stale".to_string();
    replacement.runtime_group_id = "PERFQ-M5-SPARSE-XOR".to_string();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("stale source and planned target must fail");
    let message = error.to_string();
    assert!(message.contains("is not an exact legacy threshold pair"));
    assert!(message.contains("is not an exact implemented primary contract"));
}

#[test]
fn retained_heterogeneous_row_cannot_claim_a_primary_threshold() {
    let (mut suite, manifest, references) = fixture();
    let row = suite
        .manifest_rows
        .iter_mut()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR legacy row");
    row.decision = RowDecision::Retained;

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("retained unmatched row must not claim a primary threshold");
    assert!(
        error
            .to_string()
            .contains("claims a threshold despite unmatched Stim submeasurements")
    );
}

#[test]
fn comma_filter_resolves_every_exact_and_wildcard_symbol() {
    let symbols = vec![
        "tableau_random_10".to_string(),
        "tableau_random_100".to_string(),
        "tableau_cnot_10Kqubits".to_string(),
    ];
    let filter = "tableau_random*,tableau_cnot_10Kqubits";

    assert!(filter_matches_any(filter, &symbols));
    assert!(
        symbols
            .iter()
            .all(|symbol| filter_selects_symbol(filter, symbol))
    );
}

#[test]
fn validation_rejects_dropped_api_domains_and_wrong_exact_owner() {
    let (mut suite, manifest, references) = fixture();
    let api = suite
        .public_api_items
        .iter_mut()
        .find(|item| !item.supporting_performance_features.is_empty())
        .expect("multi-domain API");
    api.supporting_performance_features.clear();
    api.correctness_case_id = "CQ-UNKNOWN-OWNER".to_string();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("dropped API domains and owner must fail");
    let message = error.to_string();
    assert!(message.contains("differs from its exact CQ0"));
    assert!(message.contains("unknown exact correctness owner"));
}

#[test]
fn validation_rejects_changed_threshold_ratio_and_waiver_reason() {
    let (mut suite, manifest, references) = fixture();
    let thresholded = suite
        .manifest_rows
        .iter_mut()
        .find(|row| !row.threshold_measurement_pairs.is_empty())
        .expect("submeasurement threshold row");
    thresholded
        .threshold_measurement_pairs
        .first_mut()
        .expect("measurement threshold")
        .max_relative_ratio = "1.30".to_string();
    suite
        .waiver_rows
        .first_mut()
        .and_then(|waiver| waiver.reasons.first_mut())
        .expect("waiver reason")
        .reason = "changed waiver reason".to_string();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("changed gate and waiver sources must fail");
    let message = error.to_string();
    assert!(message.contains("expected 1.25"));
    assert!(message.contains("differs from the source waiver ledger"));
}

#[test]
fn stim_mapping_schema_rejects_unknown_fields() {
    let (suite, _, _) = fixture();
    let mut value = serde_json::to_value(suite).expect("serialize suite");
    value
        .pointer_mut("/manifest_rows/0/stim_mapping")
        .and_then(serde_json::Value::as_object_mut)
        .expect("Stim mapping object")
        .insert("unexpected".to_string(), serde_json::Value::Bool(true));

    let error = serde_json::from_value::<QualificationSuite>(value)
        .expect_err("unknown Stim mapping field must fail");

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn validation_rejects_measured_group_without_primary_row_or_correctness_dependency() {
    let (mut suite, manifest, references) = fixture();
    let group = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.correctness_binding == CorrectnessBinding::Unresolved)
        .expect("unresolved measured group");
    group.manifest_row.clear();
    group.planned_correctness_case_id = None;

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("missing primary row and correctness dependency must fail");
    let message = error.to_string();
    assert!(message.contains("qualification primary row"));
    assert!(message.contains("planned correctness dependency"));
}

#[test]
fn validation_rejects_planned_scales_without_generator_or_seed() {
    let (mut suite, manifest, references) = fixture();
    let group = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.row_origin == RowOrigin::Planned)
        .expect("planned qualification group");
    group
        .workload_family
        .scales
        .first_mut()
        .expect("planned scale")
        .parameters = "semantic_items=1".to_string();

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("planned scale without generator and seed must fail");

    assert!(
        error
            .to_string()
            .contains("unregistered generator, mismatched seed, or placeholder value")
    );
}

#[test]
fn validation_rejects_cartesian_checklist_children_and_missing_fixture_digest() {
    let (mut suite, manifest, references) = fixture();
    let checklist = suite
        .checklist_items
        .iter_mut()
        .find(|item| item.raw_status.starts_with("Partial"))
        .expect("partial checklist item");
    checklist
        .selected_child_ownership
        .first_mut()
        .expect("selected child ownership")
        .performance_features
        .push("PERF-RESOURCE-BOUNDARIES".to_string());
    let fixture_group = suite
        .qualification_groups
        .iter_mut()
        .find(|group| {
            matches!(
                group.workload_family.fixture,
                FixtureLocator::RepositoryFile { .. }
            )
        })
        .expect("inherited static fixture group");
    fixture_group.workload_family.fixture = FixtureLocator::Inline {
        id: "wrong-fixture-kind".to_string(),
    };

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("cross-domain child ownership and missing corpus digest must fail");
    let message = error.to_string();
    assert!(message.contains("owns unrelated feature"));
    assert!(message.contains("lacks a typed path, byte length, or corpus digest"));
}

#[test]
fn validation_rejects_unproved_no_comparator_group() {
    let (mut suite, manifest, references) = fixture();
    suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == "PERFQ-RESOURCE-BOUNDARIES")
        .expect("resource-boundary group")
        .disposition = PerformanceDisposition::NoFaithfulStimComparator;

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("unproved no-comparator group must fail");

    assert!(
        error
            .to_string()
            .contains("despite a declared runner or adapter path")
    );
}

#[test]
fn validation_rejects_duplicate_global_child_domain_owner() {
    let (mut suite, manifest, references) = fixture();
    let source_index = suite
        .checklist_items
        .iter()
        .position(|item| !item.selected_child_ownership.is_empty())
        .expect("owned checklist child");
    let ownership = suite
        .checklist_items
        .get(source_index)
        .expect("source checklist item")
        .selected_child_ownership
        .first()
        .expect("child ownership")
        .clone();
    let domain = ownership
        .performance_features
        .first()
        .expect("owned domain")
        .clone();
    let target = suite
        .checklist_items
        .iter_mut()
        .enumerate()
        .find(|(index, item)| {
            *index != source_index
                && item.performance_features.contains(&domain)
                && !item.selected_child_ids.contains(&ownership.child_id)
        })
        .map(|(_, item)| item)
        .expect("second checklist owner in domain");
    target.selected_child_ids.push(ownership.child_id.clone());
    target.selected_child_ownership.push(ownership);

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("duplicate global child/domain owner must fail");

    assert!(
        error
            .to_string()
            .contains("has duplicate primary ownership")
    );
}

#[test]
fn validation_rejects_fake_api_fixture_and_extra_generator_key() {
    let (mut suite, manifest, references) = fixture();
    let group = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id.starts_with("PERFQ-API-"))
        .expect("planned API group");
    let scale = group
        .workload_family
        .scales
        .first_mut()
        .expect("planned API scale");
    scale.parameters = scale
        .parameters
        .split(';')
        .map(str::trim)
        .map(|part| {
            if part.starts_with("fixture_group=") {
                "fixture_group=cq-api-item-fake".to_string()
            } else {
                part.to_string()
            }
        })
        .chain(std::iter::once("mode=anything".to_string()))
        .collect::<Vec<_>>()
        .join("; ");

    let error = validate(&suite, &manifest, &references, "UNFROZEN")
        .expect_err("fake API fixture and extra generator key must fail");
    let message = error.to_string();
    assert!(message.contains("parameter keys"));
    assert!(message.contains("lacks an exact CQ API fixture group"));
}
