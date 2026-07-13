use serde_json::Value;

use super::evidence::read_oracle_manifest;
use super::oracle::{oracle_class_matches_runner, oracle_comparator_matches};
use super::{
    BlockerLedger, BlockerLedgerError, ComparatorKind, FixtureId, MAX_LEDGER_BYTES,
    OracleEvidenceClass, OracleRunner,
};

const LEDGER_JSON: &str = include_str!("../../../../docs/plans/blocker-closure-ledger.json");

fn repo_root() -> crate::RepoRoot {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    crate::RepoRoot::resolve(root).expect("resolve repo root")
}

fn parsed_source_ledger() -> BlockerLedger {
    BlockerLedger::from_json(
        std::path::Path::new("docs/plans/blocker-closure-ledger.json"),
        LEDGER_JSON,
    )
    .expect("parse source ledger")
}

fn mutated_ledger_json(mutator: impl FnOnce(&mut Value)) -> String {
    let mut value = serde_json::from_str(LEDGER_JSON).expect("parse source ledger value");
    mutator(&mut value);
    serde_json::to_string(&value).expect("serialize mutated ledger")
}

fn mutated_ledger(mutator: impl FnOnce(&mut Value)) -> BlockerLedger {
    let content = mutated_ledger_json(mutator);
    BlockerLedger::from_json(std::path::Path::new("mutated-ledger.json"), &content)
        .expect("parse mutated ledger")
}

fn blocker_mut<'a>(value: &'a mut Value, id: &str) -> &'a mut Value {
    value
        .get_mut("blockers")
        .and_then(Value::as_array_mut)
        .expect("blocker array")
        .iter_mut()
        .find(|blocker| blocker.get("id").and_then(Value::as_str) == Some(id))
        .expect("named blocker")
}

fn case_mut<'a>(value: &'a mut Value, blocker_id: &str, case_id: &str) -> &'a mut Value {
    blocker_mut(value, blocker_id)
        .get_mut("cases")
        .and_then(Value::as_array_mut)
        .expect("case array")
        .iter_mut()
        .find(|case| case.get("id").and_then(Value::as_str) == Some(case_id))
        .expect("named case")
}

fn validation_text(error: BlockerLedgerError) -> String {
    match error {
        BlockerLedgerError::Validation(message) => message.into(),
        other => other.to_string(),
    }
}

#[test]
fn repository_blocker_ledger_passes_validation() {
    parsed_source_ledger()
        .check(&repo_root())
        .expect("source blocker ledger validation");
}

#[test]
fn blocker_ledger_read_enforces_actual_byte_limit() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("oversized-ledger.json");
    let length = usize::try_from(MAX_LEDGER_BYTES + 1).expect("ledger limit fits usize");
    std::fs::write(&path, vec![b' '; length]).expect("write oversized ledger");

    let error = BlockerLedger::read_from_path(&path).expect_err("oversized ledger");
    assert!(matches!(error, BlockerLedgerError::LedgerTooLarge { .. }));
}

#[cfg(unix)]
#[test]
fn blocker_ledger_rejects_symlink_input() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory");
    let source = directory.path().join("source.json");
    let link = directory.path().join("ledger.json");
    std::fs::write(&source, LEDGER_JSON).expect("write source ledger");
    symlink(&source, &link).expect("create ledger symlink");

    let error = BlockerLedger::read_from_path(&link).expect_err("symlink ledger");
    assert!(matches!(
        error,
        BlockerLedgerError::EvidenceNotRegular { .. }
    ));
}

#[cfg(unix)]
#[test]
fn blocker_ledger_rejects_fifo_without_blocking() {
    use std::time::{Duration, Instant};

    use rustix::fs::{CWD, Mode};

    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("ledger.json");
    std::fs::write(&path, LEDGER_JSON).expect("write regular ledger");

    let started = Instant::now();
    let error = super::evidence::open_regular_file_with_pre_open_hook(&path, || {
        std::fs::remove_file(&path).expect("remove regular ledger");
        rustix::fs::mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).expect("replace with FIFO");
    })
    .expect_err("raced FIFO ledger");
    assert!(matches!(
        error,
        BlockerLedgerError::EvidenceNotRegular { .. }
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn blocker_ledger_rejects_unknown_schema_version() {
    let ledger = mutated_ledger(|value| {
        *value.get_mut("schema_version").expect("schema version") = Value::from(4);
    });

    let error = ledger.check(&repo_root()).expect_err("schema mismatch");
    assert!(validation_text(error).contains("schema_version is 4"));
}

#[test]
fn blocker_ledger_gate_schema_matches_canonical_core_metadata() {
    let mut violations = Vec::new();
    super::gate_contract::validate_gate_schema(&mut violations);
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn blocker_ledger_requires_all_gate_contract_surfaces() {
    let ledger = mutated_ledger(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("gate_surfaces")
            .and_then(Value::as_array_mut)
            .expect("gate surfaces")
            .retain(|surface| surface.as_str() != Some("detector-frame"));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("missing gate surface");
    let message = validation_text(error);
    assert!(message.contains("must cover all eight gate surfaces"));
    assert!(message.contains("detector-frame"));
}

#[test]
fn blocker_ledger_rejects_duplicate_gate_contract_surfaces() {
    let ledger = mutated_ledger(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("gate_surfaces")
            .and_then(Value::as_array_mut)
            .expect("gate surfaces")
            .push(Value::from("parser"));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("duplicate gate surface");
    assert!(validation_text(error).contains("duplicate gate_surfaces"));
}

#[test]
fn blocker_ledger_requires_all_gate_contract_families() {
    let ledger = mutated_ledger(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-identity-noise")
            .get_mut("gate_families")
            .and_then(Value::as_array_mut)
            .expect("gate families")
            .clear();
    });

    let error = ledger.check(&repo_root()).expect_err("missing gate family");
    let message = validation_text(error);
    assert!(message.contains("all nineteen semantic families"));
    assert!(message.contains("identity-noise"));
}

#[test]
fn blocker_ledger_rejects_duplicate_gate_contract_families() {
    let ledger = mutated_ledger(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("gate_families")
            .and_then(Value::as_array_mut)
            .expect("gate families")
            .push(Value::from("fixed-tableau"));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("duplicate gate family");
    assert!(validation_text(error).contains("duplicate gate_families"));
}

#[test]
fn blocker_ledger_rejects_missing_required_blocker() {
    let ledger = mutated_ledger(|value| {
        value
            .get_mut("blockers")
            .and_then(Value::as_array_mut)
            .expect("blocker array")
            .retain(|blocker| {
                blocker.get("id").and_then(Value::as_str) != Some("pfm4-dem-traversal")
            });
    });

    let error = ledger.check(&repo_root()).expect_err("missing blocker");
    assert!(validation_text(error).contains("missing required blocker \"pfm4-dem-traversal\""));
}

#[test]
fn blocker_ledger_rejects_deleted_owned_cases() {
    let ledger = mutated_ledger(|value| {
        blocker_mut(value, "pfm5-detecting-regions")
            .get_mut("cases")
            .and_then(Value::as_array_mut)
            .expect("case array")
            .pop();
    });

    let error = ledger.check(&repo_root()).expect_err("missing owned case");
    assert!(validation_text(error).contains("has 1 cases, expected at least 2"));
}

#[test]
fn blocker_ledger_rejects_duplicate_case_ids() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-start-anticommutation",
        );
        *case.get_mut("id").expect("case id") = Value::from("pfm5-detecting-regions-simple");
    });

    let error = ledger.check(&repo_root()).expect_err("duplicate case");
    assert!(validation_text(error).contains("duplicate case id"));
}

#[test]
fn blocker_ledger_rejects_owned_case_substitution() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-start-anticommutation",
        );
        *case.get_mut("id").expect("case id") =
            Value::from("pfm5-detecting-regions-invented-replacement");
    });

    let error = ledger.check(&repo_root()).expect_err("changed case set");
    assert!(validation_text(error).contains("blocker ledger semantic SHA-256 digest"));
}

#[test]
fn blocker_ledger_rejects_owned_case_semantic_substitution() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-simple",
        );
        *case.get_mut("surface").expect("case surface") = Value::from("invented surface");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("changed case meaning");
    assert!(validation_text(error).contains("blocker ledger semantic SHA-256 digest"));
}

#[test]
fn blocker_ledger_requires_planned_selectors_for_planned_cases() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau");
        *case.get_mut("status").expect("case status") = Value::from("planned");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("dishonest test state");
    assert!(validation_text(error).contains("requires a Planned test selector"));
}

#[test]
fn blocker_ledger_requires_existing_evidence_for_closed_cases() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-simple",
        );
        let oracle = case.get_mut("oracle").expect("oracle reference");
        *oracle.get_mut("state").expect("oracle state") = Value::from("planned");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("planned closed evidence");
    assert!(validation_text(error).contains("requires oracle state Existing"));
}

#[test]
fn blocker_ledger_rejects_stale_existing_artifact_rows() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-simple",
        );
        let oracle = case.get_mut("oracle").expect("oracle reference");
        *oracle.get_mut("value").expect("oracle value") = Value::from("missing-oracle-row");
    });

    let error = ledger.check(&repo_root()).expect_err("stale oracle row");
    assert!(validation_text(error).contains("references missing oracle row"));
}

#[test]
fn blocker_ledger_rejects_stale_existing_benchmark_rows() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-analyzer-sweep", "pfm3-analyzer-sweep-matrix");
        let benchmark = case.get_mut("benchmark").expect("benchmark reference");
        *benchmark.get_mut("value").expect("benchmark value") =
            Value::from("missing-benchmark-row");
    });

    let error = ledger.check(&repo_root()).expect_err("stale benchmark row");
    assert!(validation_text(error).contains("references missing benchmark row"));
}

#[test]
fn blocker_ledger_rejects_unsafe_upstream_paths() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm2-qec-transforms",
            "pfm2-anticommuting-measure-reset",
        );
        let upstream = case.get_mut("upstream").expect("upstream source");
        *upstream.get_mut("path").expect("upstream path") = Value::from("../outside.test.cc");
    });

    let error = ledger.check(&repo_root()).expect_err("unsafe path");
    assert!(validation_text(error).contains("has unsafe upstream path"));
}

#[test]
fn blocker_ledger_requires_benchmark_classification() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau");
        case.get_mut("benchmark")
            .and_then(Value::as_object_mut)
            .expect("benchmark reference")
            .remove("classification");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("missing benchmark class");
    assert!(validation_text(error).contains("lacks a comparability classification"));
}

#[test]
fn blocker_ledger_requires_comparator_field() {
    let content = mutated_ledger_json(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .as_object_mut()
            .expect("case record")
            .remove("comparator");
    });

    let error = BlockerLedger::from_json(
        std::path::Path::new("missing-comparator-ledger.json"),
        &content,
    )
    .expect_err("missing comparator");
    assert!(error.to_string().contains("missing field `comparator`"));
}

#[test]
fn blocker_ledger_requires_concrete_resource_contract() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau");
        *case
            .get_mut("resource_contract")
            .expect("resource contract") = Value::from("");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("empty resource contract");
    assert!(validation_text(error).contains("resource_contract must describe"));
}

#[test]
fn blocker_ledger_requires_benchmark_disposition() {
    let content = mutated_ledger_json(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("benchmark")
            .and_then(Value::as_object_mut)
            .expect("benchmark reference")
            .remove("state");
    });

    let error = BlockerLedger::from_json(
        std::path::Path::new("missing-benchmark-state-ledger.json"),
        &content,
    )
    .expect_err("missing benchmark disposition");
    assert!(error.to_string().contains("missing field `state`"));
}

#[test]
fn blocker_ledger_requires_executable_selector_for_closed_case() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-simple",
        );
        *case
            .get_mut("test")
            .and_then(|test| test.get_mut("selector"))
            .expect("test selector") = Value::Array(Vec::new());
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("empty executable selector");
    assert!(validation_text(error).contains("must use the allowlisted cargo test selector shape"));
}

#[test]
fn blocker_ledger_rejects_zero_trust_selector_arguments() {
    let ledger = mutated_ledger(|value| {
        let selector = case_mut(
            value,
            "pfm5-detecting-regions",
            "pfm5-detecting-regions-simple",
        )
        .get_mut("test")
        .and_then(|test| test.get_mut("selector"))
        .and_then(Value::as_array_mut)
        .expect("test selector");
        selector.push(Value::from("--no-run"));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("non-allowlisted selector argument");
    assert!(validation_text(error).contains("allowlisted cargo test selector shape"));
}

#[test]
fn blocker_ledger_rejects_option_shaped_selector_filter() {
    let selector = ["cargo", "test", "-p", "stab-core", "--workspace", "--quiet"].map(String::from);

    let error = super::selector::CargoTestSelector::parse(&selector)
        .expect_err("option-shaped filter must be rejected");
    assert_eq!(error, "contains an invalid test target or filter");
}

#[test]
fn blocker_selector_places_filter_after_cargo_separator() {
    let selector = ["cargo", "test", "-p", "stab-core", "flow_filter", "--quiet"].map(String::from);
    let parsed = super::selector::CargoTestSelector::parse(&selector).expect("valid selector");

    assert_eq!(
        parsed.args(),
        [
            "test",
            "-p",
            "stab-core",
            "--quiet",
            "--",
            "flow_filter",
            "--list"
        ]
    );
}

#[test]
fn blocker_selector_supports_exact_single_test_contracts() {
    let selector = [
        "cargo",
        "test",
        "-p",
        "stab-core",
        "--test",
        "pfm_b4_flow_evidence",
        "pfm_b4_flow_various_x",
        "--quiet",
        "--exact",
    ]
    .map(String::from);
    let parsed = super::selector::CargoTestSelector::parse(&selector).expect("exact selector");

    assert!(parsed.is_exact());
    assert_eq!(
        parsed.args(),
        [
            "test",
            "-p",
            "stab-core",
            "--test",
            "pfm_b4_flow_evidence",
            "--quiet",
            "--",
            "pfm_b4_flow_various_x",
            "--exact",
            "--list"
        ]
    );
    assert_eq!(
        super::selector::test_listing_match_count(
            "pfm_b4_flow_various_x: test\npfm_b4_flow_various_xcz_feedback: test\n"
        ),
        2
    );
}

#[test]
fn blocker_selector_normalizes_exact_library_fixture_contracts() {
    let selector = super::selector::CargoTestSelector::normalize_fixture_argv(
        "cargo-test|-p|stab-core|--lib|gate::tests::fixed_tableau|--quiet|--|--exact",
    )
    .expect("normalize exact fixture")
    .expect("exact Cargo fixture");
    let parsed =
        super::selector::CargoTestSelector::parse(&selector).expect("parse exact library selector");

    assert!(parsed.is_exact());
    assert_eq!(
        parsed.run_args(),
        [
            "test",
            "-p",
            "stab-core",
            "--lib",
            "--quiet",
            "--",
            "gate::tests::fixed_tableau",
            "--exact",
        ]
    );
}

#[test]
fn oracle_evidence_class_requires_typed_runner() {
    assert!(oracle_class_matches_runner(
        OracleEvidenceClass::Direct,
        OracleRunner::StimCli
    ));
    assert!(oracle_class_matches_runner(
        OracleEvidenceClass::RustTestProxy,
        OracleRunner::CargoTest
    ));
    assert!(oracle_class_matches_runner(
        OracleEvidenceClass::PinnedGolden,
        OracleRunner::CoreFixture
    ));
    assert!(!oracle_class_matches_runner(
        OracleEvidenceClass::Direct,
        OracleRunner::CargoTest
    ));
    assert!(!oracle_class_matches_runner(
        OracleEvidenceClass::RustTestProxy,
        OracleRunner::StimCli
    ));
    assert_eq!(
        OracleRunner::from_argv("core-time-reverse-flows|case"),
        Some(OracleRunner::CoreFixture)
    );
    assert!(OracleRunner::from_argv("--workspace").is_none());
}

#[test]
fn exact_claim_rejects_structural_pinned_golden_row() {
    let root = repo_root();
    let rows = read_oracle_manifest(&root.fixture_manifest()).expect("oracle manifest");
    let row = rows
        .get(&FixtureId("m4-parser-basic".to_string()))
        .expect("structural core row");

    assert!(!oracle_comparator_matches(ComparatorKind::Exact, row));
}

#[test]
fn blocker_ledger_rejects_tampered_pinned_golden_digest() {
    let ledger = mutated_ledger(|value| {
        *case_mut(
            value,
            "pfm2-qec-transforms",
            "pfm2-flow-reverse-measurement",
        )
        .get_mut("oracle")
        .and_then(|oracle| oracle.get_mut("signature"))
        .and_then(|signature| signature.get_mut("expected_stdout_sha256"))
        .expect("pinned-golden digest") = Value::from("0".repeat(64));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("tampered pinned golden");
    assert!(
        validation_text(error).contains("evidence signature is incompatible"),
        "tampered digest must invalidate the frozen evidence binding"
    );
}

#[test]
fn blocker_ledger_rejects_tampered_direct_digest() {
    let ledger = mutated_ledger(|value| {
        *case_mut(value, "pfm6-analyzer-search", "pfm6-analyzer-nested-loop")
            .get_mut("oracle")
            .and_then(|oracle| oracle.get_mut("signature"))
            .and_then(|signature| signature.get_mut("expected_stdout_sha256"))
            .expect("direct digest") = Value::from("0".repeat(64));
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("tampered direct evidence");
    assert!(
        validation_text(error).contains("evidence signature is incompatible"),
        "tampered direct fixture must invalidate the frozen evidence binding"
    );
}

#[test]
fn blocker_ledger_requires_implemented_oracle_rows() {
    let ledger = mutated_ledger(|value| {
        let oracle = case_mut(
            value,
            "pfm2-qec-transforms",
            "pfm2-flow-reverse-measurement",
        )
        .get_mut("oracle")
        .expect("oracle reference");
        *oracle.get_mut("value").expect("oracle value") = Value::from("m0-help-exact");
        *oracle
            .get_mut("classification")
            .expect("oracle classification") = Value::from("direct");
    });

    let error = ledger.check(&repo_root()).expect_err("red oracle evidence");
    assert!(validation_text(error).contains("oracle row \"m0-help-exact\" is not implemented"));
}

#[test]
fn blocker_ledger_freezes_evidence_close_supporting_oracles() {
    let ledger = mutated_ledger(|value| {
        blocker_mut(value, "pfm5-missing-detectors")
            .get_mut("supporting_oracles")
            .and_then(Value::as_array_mut)
            .expect("supporting oracle array")
            .pop();
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("missing supporting oracle");
    assert!(validation_text(error).contains("supporting oracle set differs"));
}

#[test]
fn blocker_ledger_rejects_primary_oracle_signature_drift() {
    let ledger = mutated_ledger(|value| {
        let signature = case_mut(
            value,
            "pfm6-analyzer-search",
            "pfm6-analyzer-loop-carried-observable",
        )
        .get_mut("oracle")
        .and_then(|oracle| oracle.get_mut("signature"))
        .expect("oracle signature");
        *signature.get_mut("argv").expect("oracle argv") =
            Value::from("analyze_errors|--decompose_errors");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("primary oracle signature drift");
    assert!(validation_text(error).contains("evidence signature is incompatible"));
}

#[test]
fn blocker_ledger_rejects_supporting_oracle_signature_drift() {
    let ledger = mutated_ledger(|value| {
        let signature = blocker_mut(value, "pfm5-detecting-regions")
            .get_mut("supporting_oracles")
            .and_then(Value::as_array_mut)
            .and_then(|references| references.first_mut())
            .and_then(|reference| reference.get_mut("signature"))
            .expect("supporting oracle signature");
        *signature
            .get_mut("upstream_source")
            .expect("upstream source") = Value::from("src/stim/incorrect.test.cc");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("supporting oracle signature drift");
    assert!(validation_text(error).contains("frozen signature is incompatible"));
}

#[test]
fn blocker_ledger_freezes_evidence_close_supporting_benchmarks() {
    let ledger = mutated_ledger(|value| {
        blocker_mut(value, "pfm5-detecting-regions")
            .get_mut("supporting_benchmarks")
            .and_then(Value::as_array_mut)
            .expect("supporting benchmark array")
            .pop();
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("missing supporting benchmark");
    assert!(validation_text(error).contains("supporting benchmark set differs"));
}

#[test]
fn blocker_ledger_rejects_supporting_benchmark_threshold_drift() {
    let root = repo_root();
    let ledger = parsed_source_ledger();
    let blocker = ledger
        .blockers
        .iter()
        .find(|blocker| blocker.id == "pfm5-detecting-regions")
        .expect("detecting-regions blocker");
    let mut rows = super::evidence::read_benchmark_manifest(&root.benchmark_manifest())
        .expect("benchmark manifest");
    rows.get_mut(&super::BenchmarkId(
        "pf5-detecting-regions-repeat".to_string(),
    ))
    .expect("supporting benchmark")
    .threshold_class = super::BenchmarkThresholdClass::ReportOnly;
    let mut violations = Vec::new();

    super::support::validate_supporting_benchmarks(blocker, &rows, &mut violations);

    assert!(violations.iter().any(|violation| {
        violation.contains("expected (ContractOnly/NonPrimaryReportOnly/ReportOnly)")
    }));
}

#[test]
fn blocker_ledger_rejects_benchmark_runner_class_drift() {
    let ledger = mutated_ledger(|value| {
        let benchmark = case_mut(value, "pfm3-analyzer-sweep", "pfm3-analyzer-sweep-matrix")
            .get_mut("benchmark")
            .expect("benchmark reference");
        *benchmark
            .get_mut("classification")
            .expect("benchmark classification") = Value::from("direct-match");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("benchmark classification drift");
    assert!(validation_text(error).contains("is incompatible with row"));
}

#[test]
fn blocker_ledger_rejects_untracked_upstream_sources() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(
            value,
            "pfm2-qec-transforms",
            "pfm2-anticommuting-measure-reset",
        )
        .get_mut("upstream")
        .expect("upstream source");
        *upstream.get_mut("path").expect("upstream path") =
            Value::from("src/stim/untracked-source.test.cc");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("untracked upstream source");
    assert!(validation_text(error).contains("is not tracked by pinned Stim"));
}

#[test]
fn blocker_ledger_requires_explicit_source_symbol_provenance() {
    let content = mutated_ledger_json(|value| {
        case_mut(value, "pfm2-qec-transforms", "pfm2-mpad-flow-matrix")
            .get_mut("upstream")
            .and_then(Value::as_object_mut)
            .expect("upstream source")
            .remove("kind");
    });

    let error = BlockerLedger::from_json(
        std::path::Path::new("missing-provenance-kind-ledger.json"),
        &content,
    )
    .expect_err("implicit source symbol provenance");
    assert!(error.to_string().contains("missing field `kind`"));
}

#[test]
fn blocker_ledger_requires_source_symbol_anchor_to_exist() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm2-qec-transforms", "pfm2-mpad-flow-matrix")
            .get_mut("upstream")
            .expect("upstream source");
        *upstream.get_mut("test").expect("source symbol") =
            Value::from("CircuitFlowReverser::invented_symbol");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("missing source symbol");
    let message = validation_text(error);
    assert!(message.contains("source symbol"));
    assert!(message.contains("is absent"));
}

#[test]
fn blocker_ledger_requires_exact_anchors_for_test_families() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("upstream")
            .and_then(Value::as_object_mut)
            .expect("upstream source");
        *upstream.get_mut("kind").expect("provenance kind") = Value::from("test-family");
        upstream.insert("anchors".to_string(), Value::Array(Vec::new()));
        *case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("status")
            .expect("case status") = Value::from("planned");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("unanchored test family");
    assert!(validation_text(error).contains("must name 1..=16 exact upstream anchors"));
}

#[test]
fn blocker_ledger_forbids_test_family_aggregation_as_completion_evidence() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau")
            .get_mut("upstream")
            .and_then(Value::as_object_mut)
            .expect("upstream source");
        *upstream.get_mut("kind").expect("provenance kind") = Value::from("test-family");
        upstream.insert(
            "anchors".to_string(),
            Value::Array(vec![Value::from(
                "FrameSimulator.bulk_operations_consistent_with_tableau_data",
            )]),
        );
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("implemented test family");
    assert!(validation_text(error).contains("uses a test-family aggregation"));
}

#[test]
fn blocker_ledger_requires_reproducible_statistical_plan() {
    let content = mutated_ledger_json(|value| {
        case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .as_object_mut()
            .expect("case record")
            .remove("statistical_plan");
    });
    let ledger = BlockerLedger::from_json(
        std::path::Path::new("missing-statistical-plan-ledger.json"),
        &content,
    )
    .expect("optional statistical plan parses");

    let error = ledger
        .check(&repo_root())
        .expect_err("missing statistical plan");
    assert!(validation_text(error).contains("lacks a reproducible plan"));
}

#[test]
fn blocker_ledger_rejects_weak_statistical_false_positive_budget() {
    let ledger = mutated_ledger(|value| {
        let plan = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .get_mut("statistical_plan")
            .expect("statistical plan");
        *plan
            .get_mut("familywise_false_positive_budget")
            .expect("false-positive budget") = Value::from(0.1);
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("weak false-positive budget");
    assert!(validation_text(error).contains("false-positive budget"));
}

#[test]
fn blocker_ledger_rejects_statistical_probability_drift_from_core_contract() {
    let ledger = mutated_ledger(|value| {
        let buckets = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-channels")
            .get_mut("statistical_plan")
            .and_then(|plan| plan.get_mut("buckets"))
            .and_then(Value::as_array_mut)
            .expect("statistical buckets");
        *buckets
            .first_mut()
            .and_then(|bucket| bucket.get_mut("expected_probability"))
            .expect("first expected probability") = Value::from(0.39);
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("statistical probability drift");
    assert!(validation_text(error).contains("differs from the canonical core gate contract"));
}

#[test]
fn blocker_ledger_rejects_out_of_range_statistical_probability() {
    let ledger = mutated_ledger(|value| {
        let buckets = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .get_mut("statistical_plan")
            .and_then(|plan| plan.get_mut("buckets"))
            .and_then(Value::as_array_mut)
            .expect("statistical buckets");
        *buckets
            .first_mut()
            .and_then(|bucket| bucket.get_mut("expected_probability"))
            .expect("first expected probability") = Value::from(1.1);
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("out-of-range probability");
    assert!(validation_text(error).contains("probability 1.1 is outside [0, 1]"));
}

#[test]
fn blocker_ledger_checks_exact_binomial_familywise_tail() {
    let ledger = mutated_ledger(|value| {
        let plan = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-channels")
            .get_mut("statistical_plan")
            .expect("statistical plan");
        *plan
            .get_mut("familywise_false_positive_budget")
            .expect("false-positive budget") = Value::from(0.000_000_000_001_f64);
    });
    let case = ledger
        .blockers
        .iter()
        .flat_map(|blocker| &blocker.cases)
        .find(|case| case.id == "pfm3-contract-pauli-channels")
        .expect("statistical case");
    let mut violations = Vec::new();
    super::statistical::validate_statistical_plan(case, true, &mut violations);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("exact binomial familywise rejection probability")),
        "{violations:#?}"
    );
}

#[test]
fn blocker_ledger_skips_exact_tail_work_after_digest_mismatch() {
    let ledger = mutated_ledger(|value| {
        let plan = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-channels")
            .get_mut("statistical_plan")
            .expect("statistical plan");
        *plan
            .get_mut("familywise_false_positive_budget")
            .expect("false-positive budget") = Value::from(0.000_000_000_001_f64);
    });

    let error = ledger.check(&repo_root()).expect_err("digest mismatch");
    let message = validation_text(error);
    assert!(message.contains("semantic SHA-256 digest"));
    assert!(!message.contains("exact binomial familywise rejection probability"));
}

#[test]
fn blocker_ledger_caps_aggregate_statistical_bucket_work() {
    let ledger = mutated_ledger(|value| {
        let cases = value
            .get_mut("blockers")
            .and_then(Value::as_array_mut)
            .expect("blockers")
            .iter_mut()
            .flat_map(|blocker| {
                blocker
                    .get_mut("cases")
                    .and_then(Value::as_array_mut)
                    .expect("cases")
            });
        for (case_index, case) in cases.enumerate() {
            let Some(buckets) = case
                .get_mut("statistical_plan")
                .and_then(|plan| plan.get_mut("buckets"))
                .and_then(Value::as_array_mut)
            else {
                continue;
            };
            while buckets.len() < 32 {
                buckets.push(serde_json::json!({
                    "name": format!("extra-{case_index}-{}", buckets.len()),
                    "expected_probability": 0.5
                }));
            }
        }
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("statistical work cap");
    assert!(validation_text(error).contains("statistical bucket evaluations; limit is 128"));
}

#[test]
fn blocker_ledger_rejects_gate_statistical_case_without_core_owner() {
    let ledger = mutated_ledger(|value| {
        *case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .get_mut("id")
            .expect("case id") = Value::from("pfm3-contract-pauli-noise-unowned");
    });

    let error = ledger.check(&repo_root()).expect_err("unowned plan");
    assert!(validation_text(error).contains("has no canonical core statistical plan"));
}

#[test]
fn blocker_ledger_rejects_gtest_name_prefixes() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(
            value,
            "pfm3-gate-execution",
            "pfm3-contract-correlated-errors",
        )
        .get_mut("upstream")
        .expect("upstream");
        *upstream.get_mut("test").expect("test name") =
            Value::from("FrameSimulator.correlated_erro");
    });

    let error = ledger.check(&repo_root()).expect_err("prefix anchor");
    let message = validation_text(error);
    assert!(message.contains("gtest anchor"));
    assert!(message.contains("is absent"));
}

#[test]
fn blocker_ledger_requires_gate_markers_inside_gtest_anchor() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .get_mut("upstream")
            .expect("upstream");
        *upstream.get_mut("test").expect("test name") = Value::from("TableauSimulator.simulate");
    });

    let error = ledger.check(&repo_root()).expect_err("missing markers");
    let message = validation_text(error);
    assert!(
        message.contains("upstream gate marker X_ERROR"),
        "{message}"
    );
    assert!(message.contains("is absent"), "{message}");
}

#[test]
fn blocker_ledger_gate_markers_are_identifier_exact() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-channels")
            .get_mut("upstream")
            .expect("upstream");
        *upstream.get_mut("test").expect("test name") =
            Value::from("ErrorAnalyzer.heralded_pauli_channel_1");
        *upstream.get_mut("subcase").expect("subcase") =
            Value::from("PAULI_CHANNEL_1 probability tuples");
    });

    let error = ledger.check(&repo_root()).expect_err("substring marker");
    let message = validation_text(error);
    assert!(
        message.contains("upstream gate marker PAULI_CHANNEL_1"),
        "{message}"
    );
}

#[test]
fn blocker_ledger_checks_single_character_gate_markers() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-pauli-noise")
            .get_mut("upstream")
            .expect("upstream");
        *upstream.get_mut("test").expect("test name") = Value::from("TableauSimulator.simulate");
        *upstream.get_mut("subcase").expect("subcase") = Value::from("R gate execution");
        *upstream.get_mut("gate_markers").expect("gate markers") = serde_json::json!(["R"]);
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("single-character marker");
    let message = validation_text(error);
    assert!(message.contains("upstream gate marker R"), "{message}");
}

#[test]
fn blocker_ledger_does_not_count_the_gtest_declaration_as_a_gate_marker() {
    let ledger = mutated_ledger(|value| {
        let upstream = case_mut(value, "pfm3-gate-execution", "pfm3-contract-spp")
            .get_mut("upstream")
            .expect("upstream");
        *upstream.get_mut("test").expect("test name") =
            Value::from("gate_decomposition.decompose_spp_or_spp_dag_operation_simple");
        *upstream.get_mut("subcase").expect("subcase") = Value::from("SPP_DAG execution");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("declaration-only marker");
    let message = validation_text(error);
    assert!(
        message.contains("upstream gate marker SPP_DAG"),
        "{message}"
    );
}

#[test]
fn blocker_ledger_core_statistical_plans_meet_their_exact_tail_budgets() {
    for expected in stab_core::__gate_contract_statistical_plans() {
        let mut familywise_bound = 0.0;
        for bucket in expected.buckets {
            let standard_deviation = (bucket.expected_probability
                * (1.0 - bucket.expected_probability)
                / expected.shots as f64)
                .sqrt();
            let allowed_delta = expected
                .absolute_probability_floor
                .max(expected.sigma_multiplier * standard_deviation);
            familywise_bound += super::statistical::binomial_rejection_probability(
                expected.shots,
                bucket.expected_probability,
                allowed_delta,
            );
        }
        assert!(
            familywise_bound <= expected.familywise_false_positive_budget,
            "{} exact familywise bound {familywise_bound:.6e} exceeds {:.6e}",
            expected.case_id,
            expected.familywise_false_positive_budget
        );
    }
}

#[test]
fn blocker_ledger_rejects_control_characters_in_display_text() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau");
        *case.get_mut("surface").expect("surface") = Value::from("sampler\u{1b}[2J");
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("terminal control sequence");
    assert!(validation_text(error).contains("case surface contains control characters"));
}
