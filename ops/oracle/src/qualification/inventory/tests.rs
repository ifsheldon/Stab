use std::collections::BTreeSet;
use std::path::Path;

use super::{
    InventoryError, OracleEvidenceRow, generate, stable_id, validate_relative_source_path,
};
use crate::RepoRoot;
use crate::qualification::model::{SelectorKind, StableCaseDomain};

#[test]
fn source_paths_reject_absolute_parent_and_windows_spellings() {
    for value in ["/tmp/a.test.cc", "../a.test.cc", "dir\\a.test.cc"] {
        assert!(validate_relative_source_path(value, ".test.cc").is_err());
    }
    let path = validate_relative_source_path("src/stim.test.cc", ".test.cc").expect("valid source");
    assert_eq!(path.as_path(), Path::new("src/stim.test.cc"));
}

#[test]
fn stable_ids_are_deterministic_and_domain_separated() {
    assert_eq!(
        stable_id(StableCaseDomain::ApiItem, "same"),
        stable_id(StableCaseDomain::ApiItem, "same")
    );
    assert_ne!(
        stable_id(StableCaseDomain::ApiItem, "same"),
        stable_id(StableCaseDomain::EvidenceApi, "same")
    );
    assert_ne!(
        stable_id(StableCaseDomain::ApiItem, "same"),
        stable_id(StableCaseDomain::ApiItem, "different")
    );
}

#[test]
fn generation_fails_before_discovery_for_an_unvalidated_stim_checkout() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    std::fs::create_dir_all(temporary.path().join("vendor/stim")).expect("fake Stim directory");
    let root = RepoRoot {
        path: temporary.path().to_path_buf(),
    };

    assert!(matches!(
        generate(&root),
        Err(InventoryError::StimSource(_))
    ));
}

#[test]
fn every_implemented_oracle_fixture_has_primary_or_supporting_ownership() {
    let root = RepoRoot {
        path: Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf(),
    };
    let manifest = generate(&root).expect("generated qualification manifest");
    let bytes = crate::safe_file::read_regular_file_bounded(
        &root.fixture_manifest(),
        super::MAX_ORACLE_MANIFEST_BYTES,
    )
    .expect("fixture manifest");
    let implemented = csv::ReaderBuilder::new()
        .from_reader(bytes.as_slice())
        .deserialize::<OracleEvidenceRow>()
        .map(|row| row.expect("fixture row"))
        .filter(|row| row.status == "implemented")
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let represented = manifest
        .evidence_cases
        .iter()
        .flat_map(|case| {
            std::iter::once(case.source_id.clone()).chain(
                std::iter::once(&case.primary_selector)
                    .chain(&case.supporting_selectors)
                    .filter(|selector| selector.kind == SelectorKind::OracleFixture)
                    .flat_map(|selector| selector.value.iter().cloned()),
            )
        })
        .collect::<BTreeSet<_>>();
    let missing = implemented
        .difference(&represented)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "unowned implemented fixtures: {missing:?}"
    );
}
