use std::sync::OnceLock;

use serde_json::{Value, json};

use super::*;
use crate::qualification::model::{
    ApiPath, CaseId, DeferredProduct, PropertyExecutionMode, SemanticDigest,
};

static REPOSITORY_MANIFEST: OnceLock<QualificationManifest> = OnceLock::new();

#[test]
fn repository_manifest_passes_structural_validation() {
    let manifest = repository_manifest();
    validate(&manifest, super::super::EXPECTED_FROZEN_DIGEST)
        .expect("repository manifest must validate");
}

#[test]
fn validation_requires_exact_deferred_evidence_product_ownership() {
    let mut non_deferred = repository_manifest();
    non_deferred
        .evidence_cases
        .first_mut()
        .expect("first evidence case")
        .deferred_product = Some(DeferredProduct::Diagrams);
    refresh_digest(&mut non_deferred);
    let error = validate(&non_deferred, "UNFROZEN")
        .expect_err("non-deferred evidence cannot name a deferred product");
    assert!(error.to_string().contains("non-deferred evidence case"));

    let mut deferred = repository_manifest();
    let case = deferred
        .evidence_cases
        .first_mut()
        .expect("first evidence case");
    case.status = EvidenceStatus::Deferred;
    case.primary_selector.state = EvidenceState::NotApplicable;
    case.execution = super::super::execution_contract::for_status(EvidenceStatus::Deferred);
    case.deferred_product = None;
    refresh_digest(&mut deferred);
    let error = validate(&deferred, "UNFROZEN")
        .expect_err("deferred evidence must name its deferred product");
    assert!(
        error
            .to_string()
            .contains("does not name its deferred product")
    );
}

#[test]
fn validation_rejects_shared_primary_selectors() {
    let mut manifest = repository_manifest();
    let selector = manifest
        .evidence_cases
        .first()
        .expect("first evidence case")
        .primary_selector
        .clone();
    manifest
        .evidence_cases
        .get_mut(1)
        .expect("second evidence case")
        .primary_selector = selector;
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("shared selector must fail");
    assert!(error.to_string().contains("share primary selector"));
}

#[test]
fn validation_rejects_duplicate_ids() {
    let mut manifest = repository_manifest();
    let id = manifest
        .evidence_cases
        .first()
        .expect("first evidence case")
        .id
        .clone();
    manifest.evidence_cases.get_mut(1).expect("second case").id = id;
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("duplicate id must fail");
    assert!(error.to_string().contains("duplicate evidence case id"));
}

#[test]
fn validation_rejects_duplicate_upstream_anchors() {
    let mut manifest = repository_manifest();
    let first = manifest
        .upstream_cases
        .first()
        .expect("first upstream case")
        .clone();
    let second = manifest
        .upstream_cases
        .get_mut(1)
        .expect("second upstream case");
    second.path = first.path;
    second.symbol = first.symbol;
    second.subcase = first.subcase;
    refresh_digest(&mut manifest);
    let error =
        validate(&manifest, "UNFROZEN").expect_err("duplicate upstream source anchor must fail");
    assert!(error.to_string().contains("duplicate upstream source case"));
}

#[test]
fn validation_rejects_unsafe_upstream_path() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    let path = value
        .get_mut("upstream_cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|case| case.get_mut("path"))
        .expect("upstream path");
    *path = json!("../escape.test.cc");
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("unsafe path must fail during deserialization");
    assert!(error.to_string().contains("source path must be"));
}

#[test]
fn validation_rejects_stale_public_api_owner() {
    let mut manifest = repository_manifest();
    manifest
        .public_api_items
        .first_mut()
        .expect("public API item")
        .owner_case_id = CaseId::try_new("missing-owner".to_string()).expect("valid test id");
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("stale owner must fail");
    assert!(error.to_string().contains("references missing owner"));
}

#[test]
fn public_api_ownership_is_component_delimited() {
    assert!(api_path_is_owned_by(
        "stab_core::Foo",
        "stab_core::Foo::new"
    ));
    assert!(api_path_is_owned_by(
        "stab_core::Foo",
        "stab_core::Foo as Clone for@0123456789ab"
    ));
    assert!(!api_path_is_owned_by("stab_core::Foo", "stab_core::Foobar"));
}

#[test]
fn validation_rejects_evidence_only_public_api_leak() {
    let mut manifest = repository_manifest();
    let item = manifest
        .public_api_items
        .first_mut()
        .expect("public API item");
    item.path =
        ApiPath::try_new(format!("{}::__ops_contract", item.path)).expect("valid test API path");
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("API leak must fail");
    assert!(error.to_string().contains("evidence-only export"));
}

#[test]
fn manifest_schema_denies_unknown_fields() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    value
        .as_object_mut()
        .expect("manifest object")
        .insert("unexpected".to_string(), json!(true));
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("unknown field must fail");
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn manifest_schema_rejects_missing_required_fields() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    value
        .as_object_mut()
        .expect("manifest object")
        .remove("upstream_cases");
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("missing required field must fail");
    assert!(error.to_string().contains("missing field `upstream_cases`"));
}

#[test]
fn manifest_schema_rejects_unknown_upstream_disposition() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    let disposition = value
        .get_mut("upstream_cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|case| case.get_mut("disposition"))
        .expect("upstream disposition");
    *disposition = json!("invented-disposition");
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("unknown disposition must fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn validation_rejects_deferred_case_without_named_product() {
    let mut manifest = repository_manifest();
    let case = manifest
        .upstream_cases
        .iter_mut()
        .find(|case| case.disposition == UpstreamDisposition::DeferredProduct)
        .expect("deferred upstream case");
    case.deferred_product = None;
    refresh_digest(&mut manifest);
    let error =
        validate(&manifest, "UNFROZEN").expect_err("deferred case without product must fail");
    assert!(error.to_string().contains("does not name its product"));
}

#[test]
fn manifest_schema_rejects_invalid_typed_ids_and_api_paths() {
    let mut invalid_id = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *invalid_id
        .get_mut("evidence_cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|case| case.get_mut("id"))
        .expect("evidence id") = json!("Not Valid");
    let error = serde_json::from_value::<QualificationManifest>(invalid_id)
        .expect_err("invalid typed case id must fail");
    assert!(error.to_string().contains("lowercase kebab-case"));

    let mut invalid_path = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *invalid_path
        .get_mut("public_api_items")
        .and_then(Value::as_array_mut)
        .and_then(|items| items.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut("path"))
        .expect("public API path") = json!("");
    let error = serde_json::from_value::<QualificationManifest>(invalid_path)
        .expect_err("invalid typed API path must fail");
    assert!(error.to_string().contains("API path must be nonempty"));
}

#[test]
fn manifest_schema_rejects_oversized_sequences_and_invalid_digest_shape() {
    let mut oversized = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    let features = oversized
        .get_mut("features")
        .and_then(Value::as_array_mut)
        .expect("feature rows");
    let first_feature = features.first().expect("feature row").clone();
    features.push(first_feature);
    let error = serde_json::from_value::<QualificationManifest>(oversized)
        .expect_err("oversized bounded sequence must fail during deserialization");
    assert!(error.to_string().contains("more than 16 entries"));

    let mut bad_digest = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *bad_digest
        .get_mut("semantic_digest")
        .expect("semantic digest") = json!("A".repeat(64));
    let error = serde_json::from_value::<QualificationManifest>(bad_digest)
        .expect_err("uppercase digest must fail during deserialization");
    assert!(error.to_string().contains("lowercase hexadecimal"));
}

#[test]
fn manifest_schema_rejects_oversized_text() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    *value
        .get_mut("upstream_cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|case| case.get_mut("reason"))
        .expect("upstream reason") = json!("x".repeat(2_049));
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("oversized bounded text must fail during deserialization");
    assert!(error.to_string().contains("2048-byte"));
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

#[test]
fn validation_requires_owned_statistical_plans() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .iter_mut()
        .find(|case| case.comparator == Comparator::Statistical)
        .expect("statistical evidence case");
    case.statistical_plan = None;
    refresh_digest(&mut manifest);
    let error =
        validate(&manifest, "UNFROZEN").expect_err("statistical case without plan must fail");
    assert!(error.to_string().contains("has no statistical plan owner"));
}

#[test]
fn validation_requires_typed_property_plans_and_matching_execution_modes() {
    let mut missing = repository_manifest();
    let case = implemented_property_case(&mut missing);
    case.property_plan = None;
    refresh_digest(&mut missing);
    let error = validate(&missing, "UNFROZEN")
        .expect_err("implemented property case without a plan must fail");
    assert!(error.to_string().contains("has no property plan"));

    let mut wrong_mode = repository_manifest();
    let case = implemented_property_case(&mut wrong_mode);
    case.property_plan
        .as_mut()
        .and_then(|reference| reference.plan.as_mut())
        .expect("executable property plan")
        .execution_mode = PropertyExecutionMode::QualificationWorkerSubprocess;
    refresh_digest(&mut wrong_mode);
    let error = validate(&wrong_mode, "UNFROZEN")
        .expect_err("static property corpus assigned to worker mode must fail");
    assert!(
        error
            .to_string()
            .contains("static corpus plan is incomplete")
    );
}

#[test]
fn validation_rejects_stale_execution_tiers_and_limits() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .iter_mut()
        .find(|case| case.status == EvidenceStatus::Implemented)
        .expect("implemented evidence case");
    case.execution
        .tiers
        .push(super::super::model::ExecutionTier::Pr);
    case.execution.timeout_ms = 0;
    case.execution.stdout_limit_bytes = crate::process::OUTPUT_LIMIT_BYTES + 1;
    case.execution.artifact_limit_bytes = 1;
    refresh_digest(&mut manifest);

    let error = validate(&manifest, "UNFROZEN").expect_err("stale execution contract must fail");
    let message = error.to_string();
    assert!(message.contains("repeats an execution tier"));
    assert!(message.contains("timeout is outside"));
    assert!(message.contains("stdout limit"));
    assert!(message.contains("cannot retain bounded stdout and stderr"));
}

#[test]
fn validation_requires_the_complete_resource_case_inventory() {
    let mut manifest = repository_manifest();
    let index = manifest
        .evidence_cases
        .iter()
        .position(|case| case.source_id == "resource-streaming-writer-failure")
        .expect("planned resource case");
    manifest.evidence_cases.remove(index);
    refresh_digest(&mut manifest);
    let error =
        validate(&manifest, "UNFROZEN").expect_err("missing resource boundary family must fail");
    assert!(
        error
            .to_string()
            .contains("CQ-RESOURCE source-owned case inventory")
    );
}

#[test]
fn validation_rejects_behavioral_surface_and_resource_overclaims() {
    let mut manifest = repository_manifest();
    let case = manifest
        .evidence_cases
        .iter_mut()
        .find(|case| case.provenance == EvidenceProvenance::PublicRustApi)
        .expect("public API evidence case");
    case.behavioral_surface = BehavioralSurface::Engine;
    case.negative_axes.push("unowned-overflow".to_string());
    refresh_digest(&mut manifest);
    let error =
        validate(&manifest, "UNFROZEN").expect_err("surface and resource overclaims must fail");
    let message = error.to_string();
    assert!(message.contains("behavioral surface"));
    assert!(message.contains("negative axes without a resource contract"));
}

#[test]
fn manifest_schema_rejects_unknown_feature_id() {
    let mut value = serde_json::to_value(repository_manifest()).expect("serialize manifest");
    let feature_id = value
        .get_mut("features")
        .and_then(Value::as_array_mut)
        .and_then(|features| features.first_mut())
        .and_then(Value::as_object_mut)
        .and_then(|feature| feature.get_mut("id"))
        .expect("feature id");
    *feature_id = json!("CQ-UNKNOWN");
    let error = serde_json::from_value::<QualificationManifest>(value)
        .expect_err("unknown feature must fail");
    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn validation_rejects_duplicate_public_api_paths() {
    let mut manifest = repository_manifest();
    let first = manifest
        .public_api_items
        .first()
        .expect("first public API item")
        .clone();
    let second = manifest
        .public_api_items
        .get_mut(1)
        .expect("second public API item");
    second.crate_name = first.crate_name;
    second.path = first.path;
    second.kind = first.kind;
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("duplicate API path must fail");
    assert!(error.to_string().contains("duplicate public API path"));
}

#[test]
fn validation_rejects_stale_upstream_owner() {
    let mut manifest = repository_manifest();
    let upstream = manifest
        .upstream_cases
        .iter_mut()
        .find(|case| !case.ownerships.is_empty())
        .expect("owned upstream case");
    upstream
        .ownerships
        .first_mut()
        .expect("upstream ownership")
        .owner_case_id = CaseId::try_new("missing-owner".to_string()).expect("valid test id");
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN").expect_err("stale owner must fail");
    assert!(error.to_string().contains("references missing owner"));
}

#[test]
fn validation_rejects_feature_without_closed_primary_case() {
    let mut manifest = repository_manifest();
    for case in &mut manifest.evidence_cases {
        if case.feature_id == FeatureId::StimFormat
            && matches!(
                case.status,
                EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose
            )
        {
            case.status = EvidenceStatus::Planned;
            case.primary_selector.state = EvidenceState::Planned;
        }
    }
    refresh_digest(&mut manifest);
    let error = validate(&manifest, "UNFROZEN")
        .expect_err("feature without closed primary evidence must fail");
    assert!(
        error
            .to_string()
            .contains("CQ-STIM-FORMAT has no implemented or evidence-close primary case")
    );
}

#[test]
fn validation_rejects_semantic_digest_drift() {
    let mut manifest = repository_manifest();
    manifest.semantic_digest = SemanticDigest::ZERO;
    let error = validate(&manifest, "UNFROZEN").expect_err("digest drift must fail");
    assert!(error.to_string().contains("computed"));
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

fn implemented_property_case(manifest: &mut QualificationManifest) -> &mut EvidenceCase {
    manifest
        .evidence_cases
        .iter_mut()
        .find(|case| {
            case.comparator == Comparator::Property && case.status == EvidenceStatus::Implemented
        })
        .expect("implemented property case")
}

fn refresh_digest(manifest: &mut QualificationManifest) {
    manifest.semantic_digest = inventory::semantic_digest(manifest).expect("semantic digest");
}
