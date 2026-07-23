use std::collections::BTreeSet;
use std::path::Path;

use super::evidence::infer_feature_from_oracle_argv;
use super::{
    InventoryError, OracleEvidenceRow, generate, oracle_feature_override, stable_id,
    validate_relative_source_path,
};
use crate::RepoRoot;
use crate::qualification::model::{
    EvidenceProvenance, EvidenceStatus, FeatureId, SelectorKind, StableCaseDomain,
};

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
fn flow_utility_cargo_fixtures_do_not_fall_back_to_circuit_api() {
    for argv in [
        "cargo-test|-p|stab-core|detecting_regions_target_api",
        "cargo-test|-p|stab-core|--test|circuit_flows|circuit_has_all_flows",
        "cargo-test|-p|stab-core|has_all_flows",
    ] {
        assert_eq!(
            infer_feature_from_oracle_argv(argv),
            Some(FeatureId::FlowUtils)
        );
    }
    assert_eq!(
        infer_feature_from_oracle_argv(
            "cargo-test|-p|stab-core|--test|circuit_inverse_qec|unitary_subset"
        ),
        None
    );
    assert_eq!(
        infer_feature_from_oracle_argv(
            "cargo-test|-p|stab-core|--test|circuit_api|pf1_circuit_reference_determined_"
        ),
        Some(FeatureId::Sampling)
    );
    for argv in [
        "cargo-test|-p|stab-core|--test|circuit_api|pf1_circuit_file_helpers_",
        "cargo-test|-p|stab-core|--test|circuit_api|pf1_circuit_append_text_",
    ] {
        assert_eq!(
            infer_feature_from_oracle_argv(argv),
            Some(FeatureId::StimFormat)
        );
    }
    for argv in [
        "cargo-test|-p|stab-core|not_detecting_regions_target",
        "cargo-test|-p|stab-core|unrelated::circuit_flows_helper",
        "cargo-test|-p|stab-core|pf1_circuit_reference_determinedness",
        "cargo-test|-p|stab-core|pf1_circuit_file_helpersExtraordinary",
        "cargo-test|-p|stab-core|pf1_circuit_append_textual",
    ] {
        assert_eq!(infer_feature_from_oracle_argv(argv), None, "argv={argv}");
    }
}

#[test]
fn exact_circuit_api_fixtures_override_ambiguous_circuit_source_paths() {
    for id in [
        "pf1-circuit-concat",
        "pf1-circuit-detector-coordinates",
        "pf1-circuit-insert-pop",
        "pf1-circuit-iterators",
    ] {
        assert_eq!(oracle_feature_override(id), Some(FeatureId::CircuitApi));
    }
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
    for (source_id, expected_feature) in [
        ("pf1-circuit-concat", FeatureId::CircuitApi),
        ("pf1-circuit-reference-determined", FeatureId::Sampling),
        ("pf5-detecting-regions-targets-rust", FeatureId::FlowUtils),
    ] {
        let case = manifest
            .evidence_cases
            .iter()
            .find(|case| case.source_id == source_id)
            .expect("generated evidence source must exist");
        assert_eq!(case.feature_id, expected_feature, "source_id={source_id}");
    }
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

    let qualification_cases = manifest
        .evidence_cases
        .iter()
        .filter(|case| case.provenance == EvidenceProvenance::QualificationPlan)
        .filter(|case| case.source_id.starts_with("cq2-stim-format-"))
        .collect::<Vec<_>>();
    assert_eq!(qualification_cases.len(), 24);
    assert!(
        qualification_cases
            .iter()
            .all(|case| case.status == EvidenceStatus::Implemented)
    );

    let canonical = qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-stim-format-canonical-round-trip")
        .expect("canonical qualification parent");
    let from_text_parent = qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-stim-format-from-text-contract")
        .expect("from_text qualification parent");
    let from_text = manifest
        .upstream_cases
        .iter()
        .find(|case| {
            case.path.as_path() == Path::new("src/stim/circuit/circuit.test.cc")
                && case.symbol == "circuit.from_text"
        })
        .expect("from_text upstream case");
    assert_eq!(
        from_text
            .ownerships
            .iter()
            .find(|owner| owner.feature_id == FeatureId::StimFormat)
            .expect("format ownership")
            .owner_case_id,
        from_text_parent.id
    );
    let from_stim_str = manifest
        .public_api_items
        .iter()
        .find(|item| item.path.as_str() == "stab_core::Circuit::from_stim_str")
        .expect("from_stim_str API item");
    assert_eq!(from_stim_str.owner_case_id, canonical.id);

    let dem_qualification_cases = manifest
        .evidence_cases
        .iter()
        .filter(|case| case.provenance == EvidenceProvenance::QualificationPlan)
        .filter(|case| case.source_id.starts_with("cq2-dem-"))
        .collect::<Vec<_>>();
    assert_eq!(dem_qualification_cases.len(), 17);
    assert!(
        dem_qualification_cases
            .iter()
            .all(|case| case.status == EvidenceStatus::Implemented)
    );
    assert!(
        manifest
            .evidence_cases
            .iter()
            .filter(|case| case.feature_id == FeatureId::DemFormat)
            .all(|case| case.status == EvidenceStatus::Implemented)
    );

    let target_parent = dem_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-dem-target-value-and-parse-contract")
        .expect("DEM target qualification parent");
    let dem_target = manifest
        .public_api_items
        .iter()
        .find(|item| item.path.as_str() == "stab_core::DemTarget")
        .expect("DemTarget API item");
    assert_eq!(dem_target.owner_case_id, target_parent.id);
    let target_parser = manifest
        .upstream_cases
        .iter()
        .find(|case| {
            case.path.as_path() == Path::new("src/stim/dem/dem_instruction.test.cc")
                && case.symbol == "dem_instruction.from_str"
        })
        .expect("DEM target parser upstream case");
    assert_eq!(
        target_parser
            .ownerships
            .iter()
            .find(|owner| owner.feature_id == FeatureId::DemFormat)
            .expect("DEM ownership")
            .owner_case_id,
        target_parent.id
    );

    let model_fixture_parent = dem_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-dem-imported-model-source-matrix")
        .expect("imported DEM model parent");
    let model_fixture_ids = model_fixture_parent
        .supporting_selectors
        .iter()
        .filter(|selector| selector.kind == SelectorKind::OracleFixture)
        .flat_map(|selector| selector.value.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        model_fixture_ids,
        BTreeSet::from([
            "coverage-py-dem-repeat",
            "pf1-dem-basic-rust-api",
            "pf1-dem-rust-api",
        ])
    );

    let result_qualification_cases = manifest
        .evidence_cases
        .iter()
        .filter(|case| case.provenance == EvidenceProvenance::QualificationPlan)
        .filter(|case| case.source_id.starts_with("cq2-result-"))
        .collect::<Vec<_>>();
    assert_eq!(result_qualification_cases.len(), 40);
    assert!(
        result_qualification_cases
            .iter()
            .all(|case| case.status == EvidenceStatus::Implemented)
    );
    assert!(
        manifest
            .evidence_cases
            .iter()
            .filter(|case| case.feature_id == FeatureId::ResultFormats)
            .all(|case| case.status == EvidenceStatus::Implemented)
    );

    let sample_format_parent = result_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-result-sample-format-value-contract")
        .expect("SampleFormat qualification parent");
    let result_api_items = manifest
        .public_api_items
        .iter()
        .filter(|item| item.feature_id == FeatureId::ResultFormats)
        .collect::<Vec<_>>();
    assert_eq!(result_api_items.len(), 176);
    assert!(result_api_items.iter().all(|item| {
        manifest
            .evidence_cases
            .iter()
            .any(|case| case.id == item.owner_case_id && case.status == EvidenceStatus::Implemented)
    }));
    let sample_format = result_api_items
        .iter()
        .find(|item| item.path.as_str() == "stab_core::SampleFormat")
        .expect("SampleFormat API item");
    assert_eq!(sample_format.owner_case_id, sample_format_parent.id);
    assert!(result_api_items.iter().any(|item| {
        item.path
            .as_str()
            .starts_with("stab_core::SampleFormat as Clone")
            && item.owner_case_id == sample_format_parent.id
    }));

    let dets_type_parent = result_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-result-dets-public-type-contract")
        .expect("typed DETS qualification parent");
    for path in [
        "stab_core::DetsLayout",
        "stab_core::DetsResultType",
        "stab_core::DetsToken",
        "stab_core::result_formats::DetsLayout",
        "stab_core::result_formats::DetsResultType",
        "stab_core::result_formats::DetsToken",
    ] {
        let item = result_api_items
            .iter()
            .find(|item| item.path.as_str() == path)
            .expect("typed DETS API item");
        assert_eq!(item.owner_case_id, dets_type_parent.id, "path={path}");
    }
    let accepted_corpus_parent = result_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-result-reader-boundary-type-newline-contract")
        .expect("accepted result-format corpus parent");
    for path in [
        "stab_core::result_formats::read_dets_records",
        "stab_core::result_streaming::for_each_dets_record",
        "stab_core::result_streaming::for_each_dets_packed_record",
        "stab_core::result_streaming::for_each_dets_token_record",
        "stab_core::result_streaming::for_each_dets_sparse_shot",
    ] {
        let item = result_api_items
            .iter()
            .find(|item| item.path.as_str() == path)
            .expect("typed DETS reader API item");
        assert_eq!(item.owner_case_id, accepted_corpus_parent.id, "path={path}");
    }

    let duplicate_dense_parent = result_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-result-reader-duplicate-dense-contract")
        .expect("duplicate dense qualification parent");
    let duplicate_dense = manifest
        .upstream_cases
        .iter()
        .find(|case| {
            case.path.as_path() == Path::new("src/stim/io/measure_record_reader.test.cc")
                && case.symbol == "MeasureRecordReader.FormatHits_Repeated_Dense_64"
        })
        .expect("duplicate dense upstream case");
    assert_eq!(
        duplicate_dense
            .ownerships
            .iter()
            .find(|owner| owner.feature_id == FeatureId::ResultFormats)
            .expect("result-format ownership")
            .owner_case_id,
        duplicate_dense_parent.id
    );

    let imported_reader_parent = result_qualification_cases
        .iter()
        .find(|case| case.source_id == "cq2-result-imported-reader-source-matrix")
        .expect("imported reader qualification parent");
    assert!(
        imported_reader_parent
            .supporting_selectors
            .iter()
            .any(|selector| {
                selector.kind == SelectorKind::OracleFixture
                    && selector.value.as_slice() == ["coverage-io-measure-record-reader"]
            })
    );
    let direct_convert_fixture = manifest
        .evidence_cases
        .iter()
        .find(|case| case.source_id == "m7-convert-01-to-dets")
        .expect("direct convert fixture");
    assert_eq!(direct_convert_fixture.status, EvidenceStatus::Implemented);
    assert_eq!(
        direct_convert_fixture.primary_selector.kind,
        SelectorKind::OracleFixture
    );

    let traversal_owner = manifest
        .evidence_cases
        .iter()
        .find(|case| case.source_id == "pfm4-traversal-counts")
        .expect("folded traversal source owner");
    assert!(traversal_owner.supporting_selectors.iter().any(|selector| {
        selector.kind == SelectorKind::OracleFixture
            && selector.value.as_slice() == ["pf4-dem-folded-traversal"]
    }));
}
