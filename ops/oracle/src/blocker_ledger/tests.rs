use serde_json::Value;

use super::oracle::oracle_class_matches_runner;
use super::{
    BlockerLedger, BlockerLedgerError, MAX_LEDGER_BYTES, OracleEvidenceClass, OracleRunner,
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
        *value.get_mut("schema_version").expect("schema version") = Value::from(2);
    });

    let error = ledger.check(&repo_root()).expect_err("schema mismatch");
    assert!(validation_text(error).contains("schema_version is 2"));
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
        let case = case_mut(value, "pfm2-qec-transforms", "pfm2-python-inverse-empty");
        let test = case.get_mut("test").expect("test reference");
        *test.get_mut("state").expect("test state") = Value::from("existing");
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
fn oracle_evidence_class_requires_typed_runner() {
    assert!(oracle_class_matches_runner(
        OracleEvidenceClass::Direct,
        OracleRunner::StimCli
    ));
    assert!(oracle_class_matches_runner(
        OracleEvidenceClass::RustTestProxy,
        OracleRunner::CargoTest
    ));
    assert!(!oracle_class_matches_runner(
        OracleEvidenceClass::Direct,
        OracleRunner::CargoTest
    ));
    assert!(!oracle_class_matches_runner(
        OracleEvidenceClass::RustTestProxy,
        OracleRunner::StimCli
    ));
    assert!(OracleRunner::from_argv("--workspace").is_none());
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
            .expect("upstream source");
        *upstream.get_mut("anchors").expect("family anchors") = Value::Array(Vec::new());
    });

    let error = ledger
        .check(&repo_root())
        .expect_err("unanchored test family");
    assert!(validation_text(error).contains("must name 1..=16 exact upstream anchors"));
}

#[test]
fn blocker_ledger_forbids_test_family_aggregation_as_completion_evidence() {
    let ledger = mutated_ledger(|value| {
        let case = case_mut(value, "pfm3-gate-execution", "pfm3-contract-fixed-tableau");
        *case.get_mut("status").expect("case status") = Value::from("implemented");
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
