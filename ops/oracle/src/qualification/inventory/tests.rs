use std::path::Path;

use serde_json::Value;

use super::{BridgeLedger, InventoryError, build_manifest, parse_bridge, stable_id};
use crate::RepoRoot;
use crate::qualification::model::StableCaseDomain;

const STIM_VERSION: &str = "v1.16.0";
const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";

fn workspace_root() -> RepoRoot {
    RepoRoot {
        path: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf(),
    }
}

fn checked_ledger_value() -> Value {
    let bytes = std::fs::read(workspace_root().qualification_cases()).expect("bridge ledger");
    serde_json::from_slice(&bytes).expect("bridge JSON")
}

fn build_value(
    value: &Value,
) -> Result<super::super::model::QualificationManifest, InventoryError> {
    let bytes = serde_json::to_vec(value).expect("serialize mutated bridge");
    let ledger: BridgeLedger = parse_bridge(Path::new("bridge.json"), &bytes)?;
    build_manifest(ledger, STIM_VERSION, STIM_COMMIT)
}

#[test]
fn checked_bridge_generates_only_finite_runtime_prerequisites() {
    let manifest = super::generate(&workspace_root()).expect("generated qualification bridge");

    assert_eq!(manifest.evidence_cases.len(), 48);
    assert!(manifest.upstream_cases.is_empty());
    assert!(manifest.public_api_items.is_empty());
    assert!(manifest.public_api_aliases.is_empty());
    assert!(manifest.canonical_owner_exceptions.is_empty());
    assert_eq!(
        manifest
            .evidence_cases
            .iter()
            .map(|case| case.source_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        manifest.evidence_cases.len()
    );
}

#[test]
fn bridge_ids_are_stable_for_the_transitional_benchmark_contract() {
    assert_eq!(
        stable_id(
            StableCaseDomain::EvidenceQualification,
            "a2-circuit-model-fingerprint"
        )
        .to_string(),
        "cq-evidence-qualification-e16abe30d8c7992c"
    );
}

#[test]
fn bridge_rejects_duplicate_source_ids() {
    let mut value = checked_ledger_value();
    let cases = value
        .get_mut("cases")
        .and_then(Value::as_array_mut)
        .expect("cases");
    cases.push(cases.first().expect("first case").clone());

    assert!(matches!(
        build_value(&value),
        Err(InventoryError::InvalidBridge(message)) if message.contains("duplicated")
    ));
}

#[test]
fn bridge_rejects_retired_ownership_fields() {
    let mut value = checked_ledger_value();
    value.as_object_mut().expect("bridge object").insert(
        "public_api_aliases".to_string(),
        serde_json::json!([{"obsolete": true}]),
    );

    assert!(matches!(
        build_value(&value),
        Err(InventoryError::InvalidBridge(message)) if message.contains("must remain empty")
    ));
}

#[test]
fn bridge_rejects_nonexact_selectors() {
    let mut value = checked_ledger_value();
    let selector = value
        .get_mut("cases")
        .and_then(Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(|case| case.get_mut("primary_selector"))
        .and_then(|selector| selector.get_mut("value"))
        .and_then(Value::as_array_mut)
        .expect("selector");
    selector.pop();
    selector.pop();

    assert!(matches!(
        build_value(&value),
        Err(InventoryError::InvalidBridge(message)) if message.contains("Cargo selector")
    ));
}

#[test]
fn bridge_rejects_a_different_stim_identity() {
    let mut value = checked_ledger_value();
    value
        .as_object_mut()
        .expect("bridge object")
        .insert("stim_commit".to_string(), Value::String("0".repeat(40)));

    assert!(matches!(
        build_value(&value),
        Err(InventoryError::InvalidBridge(message)) if message.contains("does not match")
    ));
}

#[test]
fn generation_validates_stim_before_reading_the_bridge() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    std::fs::create_dir_all(temporary.path().join("vendor/stim")).expect("fake Stim directory");
    let root = RepoRoot {
        path: temporary.path().to_path_buf(),
    };

    assert!(matches!(
        super::generate(&root),
        Err(InventoryError::StimSource(_))
    ));
}
