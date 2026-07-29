use super::{FixtureManifest, MANIFEST_CSV};

fn assert_fixtures_execute_package(ids: &[&str], package: &str) {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    for id in ids {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == *id)
            .expect("component-owned fixture");
        assert!(
            row.argv_tokens()
                .windows(2)
                .any(|pair| pair == ["-p", package]),
            "{id} must execute the canonical {package} package"
        );
    }
}

#[test]
fn result_format_fixtures_execute_the_canonical_records_package() {
    const RESULT_FORMAT_FIXTURES: [&str; 6] = [
        "coverage-io-measure-record",
        "coverage-io-measure-record-batch",
        "coverage-io-measure-record-batch-writer",
        "coverage-io-measure-record-reader",
        "coverage-io-measure-record-writer",
        "coverage-io-sparse-shot",
    ];

    assert_fixtures_execute_package(&RESULT_FORMAT_FIXTURES, "stab-records");
}

#[test]
fn model_fixtures_execute_the_canonical_model_package() {
    const MODEL_FIXTURES: [&str; 2] =
        ["coverage-gates-gates", "coverage-util-bot-probability-util"];

    assert_fixtures_execute_package(&MODEL_FIXTURES, "stab-model");
}

#[test]
fn analysis_fixtures_execute_the_canonical_analysis_package() {
    const ANALYSIS_FIXTURES: [&str; 5] = [
        "coverage-simulators-error-analyzer",
        "coverage-simulators-generated-qec-dem",
        "coverage-util-bot-error-decomp",
        "coverage-util-top-circuit-to-dem",
        "pf6-analyzer-generated-qec-rust",
    ];

    assert_fixtures_execute_package(&ANALYSIS_FIXTURES, "stab-analysis");
}

#[test]
fn engine_fixtures_execute_the_canonical_engine_package() {
    const ENGINE_FIXTURES: [&str; 9] = [
        "coverage-simulators-frame-simulator",
        "coverage-simulators-frame-simulator-util",
        "coverage-simulators-frame-simulator-pauli-observables",
        "coverage-simulators-measurements-to-detection-events-rust",
        "coverage-simulators-tableau-simulator",
        "coverage-py-frame-simulator",
        "coverage-py-tableau-simulator",
        "coverage-util-top-count-determined-measurements",
        "pf3-detect-default-false-sweep-core",
    ];

    assert_fixtures_execute_package(&ENGINE_FIXTURES, "stab-engine");
}
