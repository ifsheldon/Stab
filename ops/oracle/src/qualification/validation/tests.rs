use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};

use super::*;
use crate::qualification::model::{DeferredProduct, EvidenceProvenance, SemanticDigest};

static REPOSITORY_MANIFEST: OnceLock<QualificationManifest> = OnceLock::new();

#[test]
fn repository_bridge_passes_structural_validation() {
    let manifest = repository_manifest();
    validate(&manifest, super::super::EXPECTED_FROZEN_DIGEST)
        .expect("repository bridge must validate");
    assert_eq!(manifest.evidence_cases.len(), 48);
    assert!(manifest.upstream_cases.is_empty());
    assert!(manifest.public_api_items.is_empty());
    assert!(manifest.public_api_aliases.is_empty());
    assert!(manifest.canonical_owner_exceptions.is_empty());
}

#[test]
fn bridge_rejects_non_prerequisite_qualification_state() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite");
    case.provenance = EvidenceProvenance::OracleFixture;
    case.supporting_selectors
        .push(case.primary_selector.clone());
    refresh_digest(&mut manifest);

    let error = validate(&manifest, "UNFROZEN").expect_err("semantic ownership must fail");
    assert!(
        error
            .to_string()
            .contains("non-prerequisite qualification state")
    );
}

#[test]
fn bridge_rejects_shared_primary_selectors_and_duplicate_ids() {
    let mut shared = repository_manifest();
    let selector = shared
        .evidence_cases
        .first()
        .expect("first benchmark prerequisite")
        .primary_selector
        .clone();
    shared
        .evidence_cases
        .get_mut(1)
        .expect("second benchmark prerequisite")
        .primary_selector = selector;
    refresh_digest(&mut shared);
    let error = validate(&shared, "UNFROZEN").expect_err("shared selector must fail");
    assert!(error.to_string().contains("share primary selector"));

    let mut duplicate = repository_manifest();
    let duplicate_id = duplicate
        .evidence_cases
        .first()
        .expect("first benchmark prerequisite")
        .id
        .clone();
    duplicate
        .evidence_cases
        .get_mut(1)
        .expect("second benchmark prerequisite")
        .id = duplicate_id;
    refresh_digest(&mut duplicate);
    let error = validate(&duplicate, "UNFROZEN").expect_err("duplicate id must fail");
    assert!(error.to_string().contains("duplicate evidence case id"));
}

#[test]
fn bridge_rejects_deferred_or_named_product_state() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite");
    case.status = EvidenceStatus::Deferred;
    case.primary_selector.state = EvidenceState::NotApplicable;
    case.execution = super::super::execution_contract::for_status(EvidenceStatus::Deferred);
    case.deferred_product = None;
    refresh_digest(&mut manifest);

    let error = validate(&manifest, "UNFROZEN").expect_err("deferral must fail");
    assert!(
        error
            .to_string()
            .contains("non-prerequisite qualification state")
    );

    let mut manifest = repository_manifest();
    manifest
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite")
        .deferred_product = Some(DeferredProduct::Diagrams);
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("spurious deferral must fail");
    assert!(
        error
            .to_string()
            .contains("non-prerequisite qualification state")
    );
}

#[test]
fn bridge_rejects_stale_execution_bounds() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite");
    case.execution
        .tiers
        .push(super::super::model::ExecutionTier::Pr);
    case.execution.timeout_ms = 0;
    case.execution.stdout_limit_bytes = crate::process::OUTPUT_LIMIT_BYTES + 1;
    case.execution.artifact_limit_bytes = 1;
    refresh_digest(&mut manifest);

    let error = validate(&manifest, "UNFROZEN").expect_err("stale execution must fail");
    let message = error.to_string();
    assert!(message.contains("repeats an execution tier"));
    assert!(message.contains("timeout is outside"));
    assert!(message.contains("stdout limit"));
    assert!(message.contains("cannot retain bounded stdout and stderr"));
}

#[test]
fn bridge_rejects_behavioral_and_resource_overclaims() {
    let mut surface = repository_manifest();
    surface
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite")
        .behavioral_surface = BehavioralSurface::Cli;
    refresh_digest(&mut surface);
    let error = validate(&surface, "UNFROZEN").expect_err("wrong surface must fail");
    assert!(error.to_string().contains("behavioral surface"));

    let mut resource = repository_manifest();
    let case = resource
        .evidence_cases
        .iter_mut()
        .find(|case| {
            case.resource_contract.kind == super::super::model::ResourceKind::NotApplicable
        })
        .expect("semantic-only prerequisite");
    case.negative_axes.push("unowned-overflow".to_string());
    refresh_digest(&mut resource);
    let error = validate(&resource, "UNFROZEN").expect_err("resource overclaim must fail");
    assert!(
        error
            .to_string()
            .contains("negative axes without a resource contract")
    );
}

#[test]
fn manifest_schema_denies_unknown_and_missing_fields() {
    let mut unknown = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    unknown
        .as_object_mut()
        .expect("manifest object")
        .insert("unexpected".to_string(), json!(true));
    let error = serde_json::from_value::<QualificationManifest>(unknown)
        .expect_err("unknown field must fail");
    assert!(error.to_string().contains("unknown field"));

    let mut missing = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    missing
        .as_object_mut()
        .expect("manifest object")
        .remove("evidence_cases");
    let error = serde_json::from_value::<QualificationManifest>(missing)
        .expect_err("missing field must fail");
    assert!(error.to_string().contains("missing field `evidence_cases`"));
}

#[test]
fn manifest_schema_rejects_invalid_typed_ids_and_features() {
    let mut invalid_id = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *invalid_id
        .get_mut("evidence_cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|case| case.get_mut("id"))
        .expect("evidence id") = json!("Not Valid");
    let error = serde_json::from_value::<QualificationManifest>(invalid_id)
        .expect_err("invalid case id must fail");
    assert!(error.to_string().contains("lowercase kebab-case"));

    let mut invalid_feature =
        serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *invalid_feature
        .get_mut("features")
        .and_then(Value::as_array_mut)
        .and_then(|features| features.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|feature| feature.get_mut("id"))
        .expect("feature id") = json!("CQ-UNKNOWN");
    let error = serde_json::from_value::<QualificationManifest>(invalid_feature)
        .expect_err("unknown feature must fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn manifest_schema_rejects_oversized_sequences_and_invalid_digest_shape() {
    let mut oversized = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    let features = oversized
        .get_mut("features")
        .and_then(Value::as_array_mut)
        .expect("feature rows");
    features.push(features.first().expect("feature row").clone());
    let error = serde_json::from_value::<QualificationManifest>(oversized)
        .expect_err("oversized sequence must fail");
    assert!(error.to_string().contains("more than 16 entries"));

    let mut bad_digest = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *bad_digest.get_mut("semantic_digest").expect("digest") = json!("A".repeat(64));
    let error = serde_json::from_value::<QualificationManifest>(bad_digest)
        .expect_err("uppercase digest must fail");
    assert!(error.to_string().contains("lowercase hexadecimal"));
}

#[test]
fn bridge_rejects_oversized_text_and_semantic_digest_drift() {
    let mut oversized = repository_manifest();
    oversized
        .evidence_cases
        .first_mut()
        .expect("benchmark prerequisite")
        .source_id = "x".repeat(MAX_TEXT_BYTES + 1);
    refresh_digest(&mut oversized);
    let error = validate(&oversized, "UNFROZEN").expect_err("oversized text must fail");
    assert!(
        error
            .to_string()
            .contains("evidence source id is 2049 bytes")
    );

    let mut stale = repository_manifest();
    stale.semantic_digest = SemanticDigest::ZERO;
    let error = validate(&stale, "UNFROZEN").expect_err("digest drift must fail");
    assert!(error.to_string().contains("computed"));
}

#[test]
fn validation_issue_rendering_is_bounded() {
    let mut issues = ValidationIssues::default();
    for index in 0..300 {
        issues.push(format!("issue {index}"));
    }
    let rendered = issues.render();
    assert_eq!(rendered.lines().count(), MAX_VALIDATION_ISSUES + 1);
    assert!(rendered.ends_with("44 additional validation issues omitted"));
}

fn repository_manifest() -> QualificationManifest {
    REPOSITORY_MANIFEST
        .get_or_init(|| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("oracle")
                .join("qualification-manifest.json");
            let bytes = std::fs::read(path).expect("read repository qualification manifest");
            serde_json::from_slice(&bytes).expect("parse repository qualification manifest")
        })
        .clone()
}

fn refresh_digest(manifest: &mut QualificationManifest) {
    manifest.semantic_digest = inventory::semantic_digest(manifest).expect("semantic digest");
}
