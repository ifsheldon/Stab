use super::{FixtureManifest, MANIFEST_CSV};

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

    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    for id in RESULT_FORMAT_FIXTURES {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == id)
            .expect("result-format fixture");
        assert!(
            row.argv_tokens()
                .windows(2)
                .any(|pair| pair == ["-p", "stab-records"]),
            "{id} must execute the canonical stab-records package"
        );
    }
}
