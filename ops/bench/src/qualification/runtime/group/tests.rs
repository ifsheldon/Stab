use super::*;

fn valid_contract_file() -> GroupContractFile {
    let mut groups = vec![
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::PQ1_GROUP_ID).expect("group id"),
            claim_class: ClaimClass::DiagnosticInfrastructure,
            parity_eligibility: ParityEligibility::ReportOnly,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("protocol-smoke").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("main").expect("measurement id")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("default").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(4096).expect("positive work"),
                input_bytes: 0,
                input_digest: InputDigest::try_new(
                    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
                )
                .expect("empty input digest"),
            }],
            correctness_case_ids: Vec::new(),
            owner: ProtocolId::try_new("ops/bench").expect("owner"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        },
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::CIRCUIT_CANONICAL_PRINT_GROUP_ID)
                .expect("group id"),
            claim_class: ClaimClass::PromotablePerformance,
            parity_eligibility: ParityEligibility::ThresholdEligible,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("circuit-canonical-print").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("serialize").expect("measurement id")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(64).expect("positive work"),
                input_bytes: 64,
                input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
            }],
            correctness_case_ids: vec!["cq-evidence-canonical-print".to_string()],
            owner: ProtocolId::try_new("stab-core/circuit-printer").expect("owner"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        },
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
                .expect("group id"),
            claim_class: ClaimClass::PromotablePerformance,
            parity_eligibility: ParityEligibility::ThresholdEligible,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("circuit-parse").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("parse").expect("measurement id")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(64).expect("positive work"),
                input_bytes: 64,
                input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
            }],
            correctness_case_ids: vec!["cq-evidence-example".to_string()],
            owner: ProtocolId::try_new("stab-core/circuit-parser").expect("owner"),
            profiler_note: Some(ProfilerNoteContract {
                path: ProfilerNotePath::try_new(
                    "benchmarks/profiler-notes/qualification/example.md".to_string(),
                )
                .expect("note path"),
                sha256: Sha256Digest::try_new("d".repeat(64)).expect("note digest"),
            }),
            comparator_sources: Vec::new(),
        },
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::GATE_NAME_HASH_GROUP_ID)
                .expect("group id"),
            claim_class: ClaimClass::PromotablePerformance,
            parity_eligibility: ParityEligibility::ThresholdEligible,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("gate-name-hash").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("hash-all-names").expect("measurement id")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(82).expect("positive work"),
                input_bytes: 0,
                input_digest: InputDigest::try_new(
                    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
                )
                .expect("empty input digest"),
            }],
            correctness_case_ids: vec!["cq-evidence-gate-name-hash".to_string()],
            owner: ProtocolId::try_new("stab-core/gates").expect("owner"),
            profiler_note: None,
            comparator_sources: Vec::new(),
        },
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::SIMD_BITS_XOR_GROUP_ID)
                .expect("group id"),
            claim_class: ClaimClass::PromotablePerformance,
            parity_eligibility: ParityEligibility::ThresholdEligible,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("simd-bits-xor").expect("workload id"),
            measurement_ids: vec![
                ProtocolId::try_new("xor-complete-vector").expect("measurement id"),
            ],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(4_096).expect("positive work"),
                input_bytes: 1_024,
                input_digest: InputDigest::try_new("d".repeat(64)).expect("input digest"),
            }],
            correctness_case_ids: vec!["cq-evidence-simd-bits-xor".to_string()],
            owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
            profiler_note: None,
            comparator_sources: comparators::SIMD_BITS_XOR
                .iter()
                .map(|path| ComparatorSourceContract {
                    path: ComparatorSourcePath::try_new((*path).to_string())
                        .expect("comparator path"),
                    sha256: Sha256Digest::try_new("e".repeat(64)).expect("comparator digest"),
                })
                .collect(),
        },
        GroupContract {
            id: ProtocolId::try_new(super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID)
                .expect("group id"),
            claim_class: ClaimClass::PromotablePerformance,
            parity_eligibility: ParityEligibility::ThresholdEligible,
            timing_batch_policy: TimingBatchPolicy::CommonIterations,
            workload_id: ProtocolId::try_new("simd-word-popcount").expect("workload id"),
            measurement_ids: vec![ProtocolId::try_new("toggle-popcount").expect("measurement id")],
            scales: vec![ScaleContract {
                id: ProtocolId::try_new("small").expect("scale id"),
                family_id: ProtocolId::try_new("default").expect("family id"),
                size_class: crate::qualification::model::SizeClass::Small,
                work_items: NonZeroU64::new(4_096).expect("positive work"),
                input_bytes: 512,
                input_digest: InputDigest::try_new("e".repeat(64)).expect("input digest"),
            }],
            correctness_case_ids: vec!["cq-evidence-simd-word-popcount".to_string()],
            owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
            profiler_note: None,
            comparator_sources: comparators::SIMD_WORD_POPCOUNT
                .iter()
                .map(|path| ComparatorSourceContract {
                    path: ComparatorSourcePath::try_new((*path).to_string())
                        .expect("comparator path"),
                    sha256: Sha256Digest::try_new("f".repeat(64)).expect("comparator digest"),
                })
                .collect(),
        },
    ];
    groups.extend([
        not_zero_contract(
            super::super::invocation::SIMD_BITS_NOT_ZERO_EARLY_GROUP_ID,
            "simd-bits-not-zero-early",
        ),
        not_zero_contract(
            super::super::invocation::SIMD_BITS_NOT_ZERO_ALL_ZERO_GROUP_ID,
            "simd-bits-not-zero-zero",
        ),
        not_zero_contract(
            super::super::invocation::SIMD_BITS_NOT_ZERO_LATE_GROUP_ID,
            "simd-bits-not-zero-late",
        ),
        sparse_xor_contract(
            super::super::invocation::SPARSE_XOR_ROW_GROUP_ID,
            "sparse-xor-row",
            "row-xor",
            1_997,
        ),
        sparse_xor_contract(
            super::super::invocation::SPARSE_XOR_ITEM_GROUP_ID,
            "sparse-xor-item",
            "xor-item",
            7,
        ),
        transpose_contract(
            super::super::invocation::BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID,
            "bit-matrix-transpose-in-place",
            "in-place-transpose",
        ),
        transpose_contract(
            super::super::invocation::BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID,
            "bit-matrix-transpose-allocating",
            "allocating-transpose",
        ),
        test_contracts::pauli_contract(),
        test_contracts::pauli_iter_contract(
            super::super::invocation::PAULI_STRING_ITER_RANGE_GROUP_ID,
            "pauli-string-iter-range",
            232,
        ),
        test_contracts::pauli_iter_contract(
            super::super::invocation::PAULI_STRING_ITER_SINGLETON_GROUP_ID,
            "pauli-string-iter-singleton",
            3_000,
        ),
        test_contracts::clifford_contract(
            super::super::invocation::CLIFFORD_IDENTITY_GROUP_ID,
            "clifford-string-right-multiply-identity",
            "right-multiply-identity",
        ),
        test_contracts::clifford_contract(
            super::super::invocation::CLIFFORD_NON_IDENTITY_GROUP_ID,
            "clifford-string-right-multiply-non-identity",
            "right-multiply-non-identity",
        ),
    ]);
    groups.extend(test_contracts::dem_contracts());
    GroupContractFile {
        schema_version: GROUP_CONTRACT_SCHEMA_VERSION,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        performance_inventory_sha256: "a".repeat(64),
        groups,
    }
}

fn not_zero_contract(group_id: &str, workload_id: &str) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        parity_eligibility: ParityEligibility::ThresholdEligible,
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new("not-zero").expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(10_000).expect("positive work"),
            input_bytes: 1_256,
            input_digest: InputDigest::try_new("f".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-evidence-simd-bits-not-zero".to_string()],
        owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::SIMD_BITS_NOT_ZERO
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("a".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

fn sparse_xor_contract(
    group_id: &str,
    workload_id: &str,
    measurement_id: &str,
    work_items: u64,
) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        parity_eligibility: ParityEligibility::ThresholdEligible,
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new(measurement_id).expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(work_items).expect("positive work"),
            input_bytes: 8,
            input_digest: InputDigest::try_new("f".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-evidence-sparse-xor".to_string()],
        owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::SPARSE_XOR
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("b".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

fn transpose_contract(group_id: &str, workload_id: &str, measurement_id: &str) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        parity_eligibility: ParityEligibility::ThresholdEligible,
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new(measurement_id).expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(65_536).expect("positive work"),
            input_bytes: 8_208,
            input_digest: InputDigest::try_new("c".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec![
            "cq-evidence-qualification-4d0291febfd22b68".to_string(),
            "cq-evidence-qualification-66e29faafe5f2856".to_string(),
        ],
        owner: ProtocolId::try_new("stab-core/bits").expect("owner"),
        profiler_note: None,
        comparator_sources: comparators::BIT_MATRIX_TRANSPOSE
            .iter()
            .map(|path| ComparatorSourceContract {
                path: ComparatorSourcePath::try_new((*path).to_string()).expect("comparator path"),
                sha256: Sha256Digest::try_new("c".repeat(64)).expect("comparator digest"),
            })
            .collect(),
    }
}

#[test]
fn diagnostic_groups_are_report_only_and_have_no_correctness_cases() {
    let valid = valid_contract_file();
    validate(&valid, &"a".repeat(64)).expect("valid diagnostic contract");

    let mut thresholded = valid;
    thresholded
        .groups
        .first_mut()
        .expect("one group")
        .parity_eligibility = ParityEligibility::ThresholdEligible;
    assert!(matches!(
        validate(&thresholded, &"a".repeat(64)),
        Err(GroupError::InvalidGroup(_))
    ));
}

#[test]
fn product_contract_allows_profiler_note_to_follow_a_failure() {
    let mut file = valid_contract_file();
    file.groups
        .iter_mut()
        .find(|group| group.claim_class == ClaimClass::PromotablePerformance)
        .expect("product group")
        .profiler_note = None;
    validate(&file, &"a".repeat(64)).expect("product contract without a preemptive note");
}

#[test]
fn source_contract_rejects_unregistered_groups() {
    let mut unsupported = valid_contract_file();
    unsupported.groups.first_mut().expect("diagnostic group").id =
        ProtocolId::try_new("unregistered").expect("group id");
    assert!(matches!(
        validate(&unsupported, &"a".repeat(64)),
        Err(GroupError::UnsupportedRuntimeShape(group)) if group == "unregistered"
    ));
}

#[test]
fn source_contract_rejects_duplicate_and_zero_scales() {
    let mut duplicate = valid_contract_file();
    duplicate
        .groups
        .first_mut()
        .expect("diagnostic group")
        .scales = vec![
        ScaleContract {
            id: ProtocolId::try_new("same").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(1).expect("positive work"),
            input_bytes: 1,
            input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
        },
        ScaleContract {
            id: ProtocolId::try_new("same").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(2).expect("positive work"),
            input_bytes: 2,
            input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
        },
    ];
    assert!(matches!(
        validate(&duplicate, &"a".repeat(64)),
        Err(GroupError::InvalidGroup(_))
    ));

    let zero = serde_json::json!({
        "schema_version": GROUP_CONTRACT_SCHEMA_VERSION,
        "performance_inventory_sha256": "a".repeat(64),
        "groups": [{
            "id": "group",
            "claim_class": "diagnostic-infrastructure",
            "parity_eligibility": "report-only",
            "workload_id": "protocol-smoke",
            "measurement_ids": ["main"],
            "scales": [{
                "id": "zero",
                "work_items": 0,
                "input_bytes": 0,
                "input_digest": "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1"
            }],
            "correctness_case_ids": [],
            "owner": "ops/bench",
            "profiler_note": null
        }]
    });
    assert!(serde_json::from_value::<GroupContractFile>(zero).is_err());

    let mut nonmonotonic = valid_contract_file();
    nonmonotonic
        .groups
        .first_mut()
        .expect("diagnostic group")
        .scales = vec![
        ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(2).expect("positive work"),
            input_bytes: 2,
            input_digest: InputDigest::try_new("a".repeat(64)).expect("input digest"),
        },
        ScaleContract {
            id: ProtocolId::try_new("large").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(1).expect("positive work"),
            input_bytes: 1,
            input_digest: InputDigest::try_new("b".repeat(64)).expect("input digest"),
        },
    ];
    assert!(matches!(
        validate(&nonmonotonic, &"a".repeat(64)),
        Err(GroupError::InvalidGroup(_))
    ));
}

#[test]
fn scale_lookup_is_exact_and_fail_closed() {
    let file = valid_contract_file();
    let group = file.groups.first().expect("diagnostic group");
    assert_eq!(
        group.scale("default").expect("default scale").work_items,
        NonZeroU64::new(4096).expect("positive work")
    );
    assert!(matches!(
        group.scale("Default"),
        Err(GroupError::UnknownScale { group, scale })
            if group == super::super::invocation::PQ1_GROUP_ID && scale == "Default"
    ));
}

#[test]
fn runtime_contract_rejects_inventory_scale_drift() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    validate_inventory_contracts(&file, &suite).expect("matching ledgers");

    let scale = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
        .and_then(|group| group.workload_family.scales.first_mut())
        .expect("circuit parse scale");
    scale.semantic_work = scale.semantic_work.and_then(|work| work.checked_add(1));

    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::InventoryContract(group))
            if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
    ));

    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let scale = suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
        .and_then(|group| group.workload_family.scales.first_mut())
        .expect("circuit parse scale");
    scale.input_digest = Some("e".repeat(64));
    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::InventoryContract(group))
            if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
    ));
}

#[test]
fn runtime_contract_rejects_stale_replacement_measurement() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    suite
        .manifest_rows
        .iter_mut()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR row")
        .replacement_contracts
        .first_mut()
        .expect("dense XOR replacement")
        .runtime_measurement_id = "stale-measurement".to_string();

    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::ReplacementContract { row, group, measurement })
            if row == "m5-simd-bits"
                && group == "PERFQ-M5-SIMD-BITS"
                && measurement == "stale-measurement"
    ));
}

#[test]
fn runtime_contract_rejects_stale_replacement_scale() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    suite
        .manifest_rows
        .iter_mut()
        .find(|row| row.id == "m5-simd-bits")
        .expect("dense XOR row")
        .replacement_contracts
        .first_mut()
        .expect("dense XOR replacement")
        .runtime_scale_id = Some("stale-scale".to_string());

    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::ReplacementContract { row, group, measurement })
            if row == "m5-simd-bits"
                && group == "PERFQ-M5-SIMD-BITS"
                && measurement == "xor-complete-vector"
    ));
}

#[test]
fn runtime_contract_rejects_inventory_groups_without_runtime_owners() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let mut suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    let mut orphan = suite
        .qualification_groups
        .iter()
        .find(|group| group.id == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
        .expect("implemented threshold group")
        .clone();
    orphan.id = "PERFQ-ORPHAN".to_string();
    suite.qualification_groups.push(orphan);

    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::InventoryCoverage {
            runtime_only,
            inventory_only,
        }) if runtime_only.is_empty() && inventory_only == ["PERFQ-ORPHAN"]
    ));

    suite.qualification_groups.retain(|group| {
        group.id != "PERFQ-ORPHAN" && group.id != super::super::invocation::CIRCUIT_PARSE_GROUP_ID
    });
    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::InventoryCoverage {
            runtime_only,
            inventory_only,
        }) if runtime_only == [super::super::invocation::CIRCUIT_PARSE_GROUP_ID]
            && inventory_only.is_empty()
    ));
}

#[test]
fn runtime_contract_rejects_stale_profiler_note_digest() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (mut file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    file.groups
        .iter_mut()
        .find(|group| group.id.to_string() == super::super::invocation::CIRCUIT_PARSE_GROUP_ID)
        .and_then(|group| group.profiler_note.as_mut())
        .expect("profiler note")
        .sha256 = Sha256Digest::try_new("e".repeat(64)).expect("different digest");

    assert!(matches!(
        validate_profiler_notes(&root, &file),
        Err(GroupError::ProfilerNoteDigest(group))
            if group == super::super::invocation::CIRCUIT_PARSE_GROUP_ID
    ));
}

#[test]
fn runtime_contract_rejects_stale_comparator_source_digest() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let manifest = crate::manifest::BenchmarkManifest::read(&root).expect("manifest");
    let suite = super::super::super::discovery::generate(&root, &manifest)
        .expect("generated performance inventory");
    let (mut file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    file.groups
        .iter_mut()
        .find(|group| group.id.to_string() == super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID)
        .and_then(|group| group.comparator_sources.first_mut())
        .expect("comparator source")
        .sha256 = Sha256Digest::try_new("e".repeat(64)).expect("different digest");

    assert!(matches!(
        validate_comparator_sources(&root, &file),
        Err(GroupError::ComparatorSourceDigest(group))
            if group == super::super::invocation::SIMD_WORD_POPCOUNT_GROUP_ID
    ));
}
