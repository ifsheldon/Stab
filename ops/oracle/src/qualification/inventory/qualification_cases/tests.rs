use super::*;
use crate::qualification::inventory::evidence::{make_planned_evidence_case, planned_api_selector};
use crate::qualification::model::{BehavioralSurface, PublicApiKind};

#[test]
fn case_shape_rejects_non_exact_and_cross_mode_selectors() {
    let mut spec = test_spec();
    spec.primary_selector.value.pop();
    assert!(validate_case_shape(&spec).is_err());

    let mut spec = test_spec();
    spec.primary_selector.kind = SelectorKind::OracleFixture;
    assert!(validate_case_shape(&spec).is_err());

    let mut spec = test_spec();
    spec.comparator = Comparator::Statistical;
    assert!(validate_case_shape(&spec).is_err());
}

#[test]
fn claiming_evidence_allows_exact_comparator_refinement_but_rejects_wrong_feature_and_duplicate_owner()
 {
    let spec = test_spec();
    let id = CaseId::try_new("cq-evidence-upstream-test".to_string()).expect("case id");
    let evidence = vec![EvidenceCase {
        id: id.clone(),
        feature_id: FeatureId::StimFormat,
        behavioral_surface: BehavioralSurface::FileFormat,
        provenance: EvidenceProvenance::UpstreamSemanticCase,
        source_id: "source".to_string(),
        comparator: Comparator::Canonical,
        execution: super::super::super::execution_contract::for_status(EvidenceStatus::Planned),
        statistical_plan: None,
        property_plan: None,
        primary_selector: EvidenceSelector {
            state: EvidenceState::Planned,
            kind: SelectorKind::CargoTest,
            value: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "stab-core".to_string(),
                "planned".to_string(),
                "--quiet".to_string(),
                "--exact".to_string(),
            ],
        },
        supporting_selectors: Vec::new(),
        resource_contract: super::super::evidence::semantic_only_resource_contract(),
        negative_axes: Vec::new(),
        performance_groups: vec!["PERF-CIRCUIT-MODEL".to_string()],
        deferred_product: None,
        status: EvidenceStatus::Planned,
    }];
    let mut claimed = BTreeSet::new();
    claim_planned_evidence(
        &spec.id,
        spec.feature_id,
        &id,
        EvidenceProvenance::UpstreamSemanticCase,
        &evidence,
        &mut claimed,
    )
    .expect("first claim");
    assert!(
        claim_planned_evidence(
            &spec.id,
            spec.feature_id,
            &id,
            EvidenceProvenance::UpstreamSemanticCase,
            &evidence,
            &mut claimed,
        )
        .is_err()
    );

    let mut wrong = test_spec();
    wrong.feature_id = FeatureId::DemFormat;
    assert!(
        claim_planned_evidence(
            &wrong.id,
            wrong.feature_id,
            &id,
            EvidenceProvenance::UpstreamSemanticCase,
            &evidence,
            &mut BTreeSet::new(),
        )
        .is_err()
    );
}

#[test]
fn claiming_exact_oracle_fixture_requires_the_same_primary_selector() {
    let spec = test_spec();
    let id = CaseId::try_new("cq-evidence-oracle-test".to_string()).expect("case id");
    let evidence = EvidenceCase {
        id: id.clone(),
        feature_id: spec.feature_id,
        behavioral_surface: BehavioralSurface::FileFormat,
        provenance: EvidenceProvenance::OracleFixture,
        source_id: "fixture".to_string(),
        comparator: Comparator::Structural,
        execution: super::super::super::execution_contract::for_status(EvidenceStatus::Implemented),
        statistical_plan: None,
        property_plan: None,
        primary_selector: spec.primary_selector.clone(),
        supporting_selectors: Vec::new(),
        resource_contract: super::super::evidence::semantic_only_resource_contract(),
        negative_axes: Vec::new(),
        performance_groups: vec!["PERF-CIRCUIT-MODEL".to_string()],
        deferred_product: None,
        status: EvidenceStatus::Implemented,
    };
    let mut claimed = BTreeSet::new();
    claim_oracle_fixture_evidence(
        &spec.id,
        spec.feature_id,
        &id,
        &spec.primary_selector,
        std::slice::from_ref(&evidence),
        &mut claimed,
    )
    .expect("matching exact fixture claim");
    assert!(
        claim_oracle_fixture_evidence(
            &spec.id,
            spec.feature_id,
            &id,
            &spec.primary_selector,
            std::slice::from_ref(&evidence),
            &mut claimed,
        )
        .is_err()
    );

    let mut mismatched = evidence;
    mismatched.primary_selector.value.insert(
        mismatched.primary_selector.value.len().saturating_sub(2),
        "different-test".to_string(),
    );
    assert!(
        claim_oracle_fixture_evidence(
            &spec.id,
            spec.feature_id,
            &id,
            &spec.primary_selector,
            std::slice::from_ref(&mismatched),
            &mut BTreeSet::new(),
        )
        .is_err()
    );
}

#[test]
fn existing_parent_mapping_shape_requires_owned_supported_parent_kind() {
    let mapping = ExistingParentMappingSpec {
        id: "cq2-existing-parent-map".to_string(),
        feature_id: FeatureId::GateContract,
        parent: ExistingParentSpec {
            provenance: EvidenceProvenance::BlockerLedger,
            source_id: "pfm3-contract-fixed-tableau".to_string(),
        },
        upstream_owners: vec![UpstreamOwnerSpec {
            path: RelativeSourcePath::try_new(
                "src/stim/simulators/tableau_simulator.test.cc".into(),
            )
            .expect("path"),
            symbol: "TableauSimulator.unitary_gates_consistent_with_tableau_data_64".to_string(),
            subcase: None,
        }],
        upstream_word_size_families: Vec::new(),
        public_api_owners: Vec::new(),
        oracle_fixture_owners: Vec::new(),
    };
    validate_existing_parent_mapping_shape(&mapping).expect("valid mapping");

    let mut empty = mapping;
    empty.upstream_owners.clear();
    assert!(validate_existing_parent_mapping_shape(&empty).is_err());
    empty.upstream_owners.push(UpstreamOwnerSpec {
        path: RelativeSourcePath::try_new("src/stim/gates/gates.test.cc".into()).expect("path"),
        symbol: "gate_data.lookup".to_string(),
        subcase: None,
    });
    empty.parent.provenance = EvidenceProvenance::QualificationPlan;
    assert!(validate_existing_parent_mapping_shape(&empty).is_err());
}

#[test]
fn expanded_word_size_families_count_toward_case_owner_limit() {
    let mut spec = test_spec();
    let path = RelativeSourcePath::try_new("src/stim/simulators/frame_simulator.test.cc".into())
        .expect("path");
    spec.upstream_word_size_families = (0..=MAX_OWNERS_PER_CASE / 3)
        .map(|index| UpstreamWordSizeFamilySpec {
            path: path.clone(),
            symbol_base: format!("FrameSimulator.family_{index}"),
            word_sizes: vec![64, 128, 256],
        })
        .collect();
    assert!(matches!(
        validate_case_shape(&spec),
        Err(InventoryError::InvalidQualificationCases(message)) if message.contains("has 2049 owners")
    ));
}

#[test]
fn public_api_alias_preserves_planned_and_implemented_canonical_parents() {
    let alias = test_alias();
    let alias_evidence = planned_public_api_evidence(
        alias.alias_owner_path.as_str(),
        FeatureId::GateContract,
        "alias",
    );
    let canonical_evidence = planned_public_api_evidence(
        alias.canonical_owner_path.as_str(),
        FeatureId::GateContract,
        "canonical",
    );
    let mut items = vec![
        public_api_item(
            alias.alias_owner_path.as_str(),
            FeatureId::GateContract,
            &alias_evidence.id,
        ),
        public_api_item(
            "stab_core::analysis::gate_tableau::support",
            FeatureId::GateContract,
            &alias_evidence.id,
        ),
        public_api_item(
            alias.canonical_owner_path.as_str(),
            FeatureId::GateContract,
            &canonical_evidence.id,
        ),
    ];
    let evidence = vec![alias_evidence.clone(), canonical_evidence.clone()];
    let mut claimed = BTreeSet::new();
    apply_public_api_aliases(
        std::slice::from_ref(&alias),
        &mut items,
        &evidence,
        &[],
        &mut claimed,
    )
    .expect("planned canonical alias");
    assert_eq!(
        items.first().expect("alias item").owner_case_id,
        canonical_evidence.id
    );
    assert_eq!(
        items.get(1).expect("alias support item").owner_case_id,
        canonical_evidence.id
    );
    assert_eq!(
        evidence.get(1).expect("canonical evidence").status,
        EvidenceStatus::Planned
    );
    assert_eq!(claimed, BTreeSet::from([alias_evidence.id.clone()]));

    let implemented_id =
        CaseId::try_new("cq-evidence-qualification-canonical".to_string()).expect("case id");
    let implemented = implemented_parent(implemented_id.clone(), FeatureId::GateContract);
    let mut items = vec![
        public_api_item(
            alias.alias_owner_path.as_str(),
            FeatureId::GateContract,
            &alias_evidence.id,
        ),
        public_api_item(
            alias.canonical_owner_path.as_str(),
            FeatureId::GateContract,
            &implemented_id,
        ),
    ];
    let mut claimed = BTreeSet::new();
    apply_public_api_aliases(
        &[alias],
        &mut items,
        std::slice::from_ref(&alias_evidence),
        std::slice::from_ref(&implemented),
        &mut claimed,
    )
    .expect("implemented canonical alias");
    assert_eq!(
        items.first().expect("alias item").owner_case_id,
        implemented_id
    );
    assert_eq!(implemented.status, EvidenceStatus::Implemented);
}

#[test]
fn public_api_alias_validation_fails_closed() {
    let alias = test_alias();
    let alias_evidence = planned_public_api_evidence(
        alias.alias_owner_path.as_str(),
        FeatureId::GateContract,
        "alias",
    );
    let canonical_evidence = planned_public_api_evidence(
        alias.canonical_owner_path.as_str(),
        FeatureId::GateContract,
        "canonical",
    );
    let items = vec![
        public_api_item(
            alias.alias_owner_path.as_str(),
            FeatureId::GateContract,
            &alias_evidence.id,
        ),
        public_api_item(
            alias.canonical_owner_path.as_str(),
            FeatureId::GateContract,
            &canonical_evidence.id,
        ),
    ];
    let evidence = vec![alias_evidence.clone(), canonical_evidence];

    assert_alias_error(
        vec![test_alias(), test_alias()],
        items.clone(),
        evidence.clone(),
        "duplicated",
    );
    let self_alias = PublicApiAliasSpec {
        crate_name: "stab_core".to_string(),
        alias_owner_path: api_path("stab_core::analysis::gate_tableau"),
        canonical_owner_path: api_path("stab_core::analysis::gate_tableau"),
    };
    assert_alias_error(
        vec![self_alias],
        items.clone(),
        evidence.clone(),
        "self-referential",
    );
    assert_alias_error(
        vec![test_alias()],
        items.iter().skip(1).cloned().collect(),
        evidence.clone(),
        "alias",
    );
    assert_alias_error(
        vec![test_alias()],
        items.iter().take(1).cloned().collect(),
        evidence.clone(),
        "canonical",
    );

    let mut cross_feature_items = items.clone();
    cross_feature_items
        .get_mut(1)
        .expect("canonical item")
        .feature_id = FeatureId::Algebra;
    assert_alias_error(
        vec![test_alias()],
        cross_feature_items,
        evidence.clone(),
        "feature",
    );

    let mut stale_parent_items = items.clone();
    stale_parent_items
        .get_mut(1)
        .expect("canonical item")
        .owner_case_id =
        CaseId::try_new("cq-evidence-api-stale-parent".to_string()).expect("case id");
    assert_alias_error(
        vec![test_alias()],
        stale_parent_items,
        evidence.clone(),
        "resolved 0 parent records",
    );

    let mut implemented_alias_evidence = evidence;
    implemented_alias_evidence
        .first_mut()
        .expect("alias evidence")
        .status = EvidenceStatus::Implemented;
    assert_alias_error(
        vec![test_alias()],
        items,
        implemented_alias_evidence,
        "cannot claim",
    );
}

#[test]
fn source_ledger_separates_owner_functions_from_facade_reexports() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let ledger = load(&RepoRoot { path: root_path }).expect("qualification case ledger");
    let aliases = ledger
        .public_api_aliases
        .iter()
        .map(|alias| {
            (
                alias.alias_owner_path.as_str(),
                alias.canonical_owner_path.as_str(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for owner_function in [
        "stab_core::analysis::circuit_without_tags",
        "stab_core::analysis::detector_error_model_without_tags",
        "stab_core::analysis::flattened_detector_error_model",
        "stab_core::analysis::gate_decomposition_to_circuit",
        "stab_core::analysis::rounded_detector_error_model",
        "stab_core::analysis::flattened_circuit",
        "stab_core::analysis::gate_tableau",
        "stab_core::execution::circuit_reference_sample",
    ] {
        assert!(
            !aliases.contains_key(owner_function),
            "owner function {owner_function} must not alias a removed foreign method"
        );
    }

    for (index, (alias, canonical, feature_id)) in [
        (
            "stab_core::analysis::GateUnitaryMatrix",
            "stab_core::GateUnitaryMatrix",
            FeatureId::GateContract,
        ),
        (
            "stab_core::analysis::GateUnitaryMatrix::dimension",
            "stab_core::GateUnitaryMatrix::dimension",
            FeatureId::GateContract,
        ),
        (
            "stab_core::analysis::GateUnitaryMatrix::entry_count",
            "stab_core::GateUnitaryMatrix::entry_count",
            FeatureId::GateContract,
        ),
        (
            "stab_core::analysis::GateUnitaryMatrix::num_qubits",
            "stab_core::GateUnitaryMatrix::num_qubits",
            FeatureId::GateContract,
        ),
        (
            "stab_core::analysis::GateUnitaryMatrix::to_vecs",
            "stab_core::GateUnitaryMatrix::to_vecs",
            FeatureId::GateContract,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(aliases.get(alias), Some(&canonical), "alias {alias}");

        let alias_spec = ledger
            .public_api_aliases
            .iter()
            .find(|spec| spec.alias_owner_path.as_str() == alias)
            .expect("source-owned alias");
        let alias_evidence =
            planned_public_api_evidence(alias, feature_id, &format!("replacement-{index}"));
        let canonical_id =
            CaseId::try_new(format!("cq-evidence-qualification-replacement-{index}"))
                .expect("case id");
        let canonical_parent = implemented_parent(canonical_id.clone(), feature_id);
        let mut items = vec![
            public_api_item(alias, feature_id, &alias_evidence.id),
            public_api_item(canonical, feature_id, &canonical_id),
        ];
        let mut claimed = BTreeSet::new();
        apply_public_api_aliases(
            std::slice::from_ref(alias_spec),
            &mut items,
            std::slice::from_ref(&alias_evidence),
            std::slice::from_ref(&canonical_parent),
            &mut claimed,
        )
        .expect("semantic replacement alias");
        assert_eq!(
            items.first().expect("alias item").owner_case_id,
            canonical_id,
            "alias {alias}"
        );
        assert_eq!(
            claimed,
            BTreeSet::from([alias_evidence.id]),
            "alias {alias}"
        );
    }
}

fn assert_alias_error(
    aliases: Vec<PublicApiAliasSpec>,
    mut items: Vec<PublicApiItem>,
    evidence: Vec<EvidenceCase>,
    expected: &str,
) {
    let error =
        apply_public_api_aliases(&aliases, &mut items, &evidence, &[], &mut BTreeSet::new())
            .expect_err("invalid alias");
    assert!(error.to_string().contains(expected), "{error}");
}

fn test_alias() -> PublicApiAliasSpec {
    PublicApiAliasSpec {
        crate_name: "stab_core".to_string(),
        alias_owner_path: api_path("stab_core::analysis::GateUnitaryMatrix"),
        canonical_owner_path: api_path("stab_core::GateUnitaryMatrix"),
    }
}

fn api_path(value: &str) -> ApiPath {
    ApiPath::try_new(value.to_string()).expect("API path")
}

fn planned_public_api_evidence(
    source_id: &str,
    feature_id: FeatureId,
    id_suffix: &str,
) -> EvidenceCase {
    let id = CaseId::try_new(format!("cq-evidence-api-{id_suffix}")).expect("case id");
    make_planned_evidence_case(
        id.clone(),
        feature_id,
        EvidenceProvenance::PublicRustApi,
        source_id.to_string(),
        Comparator::SemanticInvariant,
        planned_api_selector("stab_core", &id),
    )
}

fn implemented_parent(id: CaseId, feature_id: FeatureId) -> EvidenceCase {
    EvidenceCase {
        id,
        feature_id,
        behavioral_surface: BehavioralSurface::RustApi,
        provenance: EvidenceProvenance::QualificationPlan,
        source_id: "cq2-canonical-parent".to_string(),
        comparator: Comparator::SemanticInvariant,
        execution: super::super::super::execution_contract::for_status(EvidenceStatus::Implemented),
        statistical_plan: None,
        property_plan: None,
        primary_selector: EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::CargoTest,
            value: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "stab-core".to_string(),
                "canonical_parent".to_string(),
                "--quiet".to_string(),
                "--exact".to_string(),
            ],
        },
        supporting_selectors: Vec::new(),
        resource_contract: super::super::evidence::semantic_only_resource_contract(),
        negative_axes: Vec::new(),
        performance_groups: feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect(),
        deferred_product: None,
        status: EvidenceStatus::Implemented,
    }
}

fn public_api_item(path: &str, feature_id: FeatureId, owner_case_id: &CaseId) -> PublicApiItem {
    PublicApiItem {
        id: CaseId::try_new(format!(
            "cq-api-item-{}",
            path.bytes()
                .fold(0_u64, |state, byte| state.wrapping_mul(31)
                    + u64::from(byte))
        ))
        .expect("item id"),
        feature_id,
        crate_name: "stab_core".to_string(),
        path: api_path(path),
        kind: PublicApiKind::Function,
        source_path: RelativeSourcePath::try_new("crates/stab-core/src/analysis/mod.rs".into())
            .expect("source path"),
        source_line: 1,
        owner_case_id: owner_case_id.clone(),
        performance_groups: feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect(),
    }
}

fn test_spec() -> QualificationCaseSpec {
    QualificationCaseSpec {
        id: "cq2-test-case".to_string(),
        feature_id: FeatureId::StimFormat,
        comparator: Comparator::Canonical,
        primary_selector: EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::CargoTest,
            value: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "stab-core".to_string(),
                "--test".to_string(),
                "stim_format".to_string(),
                "parses_and_prints_basic_m4_fixture".to_string(),
                "--quiet".to_string(),
                "--exact".to_string(),
            ],
        },
        resource_contract: super::super::evidence::semantic_only_resource_contract(),
        negative_axes: Vec::new(),
        upstream_owners: Vec::new(),
        upstream_word_size_families: Vec::new(),
        public_api_owners: Vec::new(),
        oracle_fixture_owners: Vec::new(),
        static_property_plan: None,
        standalone: true,
    }
}
