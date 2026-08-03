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
            owner: ProtocolId::try_new("stab-model/circuit-printer").expect("owner"),
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
            owner: ProtocolId::try_new("stab-model/circuit-parser").expect("owner"),
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
    groups.extend([
        product_diagnostic_contract(
            super::super::invocation::A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID,
            "circuit-model-fingerprint",
            "fingerprint",
        ),
        product_diagnostic_contract(
            super::super::invocation::A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
            "sampling-request-fingerprint",
            "fingerprint-inclusive",
        ),
        product_diagnostic_contract(
            super::super::invocation::A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID,
            "sampling-request-estimate",
            "estimate",
        ),
        product_diagnostic_contract(
            super::super::invocation::A2_SAMPLER_COMPILE_GROUP_ID,
            "sampler-compile",
            "compile-and-release",
        ),
        product_diagnostic_contract(
            super::super::invocation::A7_EXACT_ML_COMPILE_GROUP_ID,
            "exact-ml-compile",
            "compile-and-release",
        ),
        product_diagnostic_contract(
            super::super::invocation::A7_EXACT_ML_REUSED_DECODE_GROUP_ID,
            "exact-ml-reused-decode",
            "decode-batch",
        ),
        product_diagnostic_contract(
            super::super::invocation::A7_PIPELINE_GROUP_ID,
            "sample-detect-decode-pipeline",
            "sample-detect-decode",
        ),
        product_diagnostic_contract(
            super::super::invocation::A8_EXTERNAL_NOISE_PASS_GROUP_ID,
            "external-noise-pass",
            "run-and-release",
        ),
    ]);
    let product_diagnostic_policies = groups
        .iter()
        .filter(|group| group.claim_class == ClaimClass::ProductDiagnostic)
        .map(product_diagnostic_policy)
        .collect();
    GroupContractFile {
        schema_version: GROUP_CONTRACT_SCHEMA_VERSION,
        timing_boundary: RAW_WORK_TIMING_BOUNDARY,
        performance_inventory_sha256: "a".repeat(64),
        product_diagnostic_suite_timeout_seconds: NonZeroU64::new(600)
            .expect("positive suite timeout"),
        product_diagnostic_policies,
        groups,
    }
}

fn product_diagnostic_contract(
    group_id: &str,
    workload_id: &str,
    measurement_id: &str,
) -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new(group_id).expect("group id"),
        claim_class: ClaimClass::ProductDiagnostic,
        parity_eligibility: ParityEligibility::ReportOnly,
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new(workload_id).expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new(measurement_id).expect("measurement id")],
        scales: vec![ScaleContract {
            id: ProtocolId::try_new("small").expect("scale id"),
            family_id: ProtocolId::try_new("default").expect("family id"),
            size_class: crate::qualification::model::SizeClass::Small,
            work_items: NonZeroU64::new(64).expect("positive work"),
            input_bytes: 429,
            input_digest: InputDigest::try_new("c".repeat(64)).expect("input digest"),
        }],
        correctness_case_ids: vec!["cq-evidence-agent-diagnostic".to_string()],
        owner: ProtocolId::try_new("stab-core/agent-diagnostic").expect("owner"),
        profiler_note: None,
        comparator_sources: Vec::new(),
    }
}

fn product_diagnostic_policy(group: &GroupContract) -> ProductDiagnosticPolicy {
    ProductDiagnosticPolicy {
        group_id: group.id.clone(),
        scales: group
            .scales
            .iter()
            .map(|scale| ProductDiagnosticScalePolicy {
                scale_id: scale.id.clone(),
                batch_policy: ProductDiagnosticBatchPolicy::CalibratedRepeat,
                witness_case_id: group
                    .correctness_case_ids
                    .first()
                    .expect("correctness witness")
                    .clone(),
                expected_output_digest: SemanticDigest::try_new("d".repeat(64))
                    .expect("output digest"),
                max_worker_peak_rss_bytes: Some(
                    NonZeroU64::new(32 << 20).expect("positive memory cap"),
                ),
            })
            .collect(),
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
fn product_diagnostics_require_exact_owners_without_parity_or_profiler_inputs() {
    let valid = valid_contract_file();
    let diagnostic = valid
        .groups
        .iter()
        .find(|group| group.claim_class == ClaimClass::ProductDiagnostic)
        .expect("product diagnostic");
    assert_eq!(diagnostic.parity_eligibility, ParityEligibility::ReportOnly);
    assert_eq!(diagnostic.correctness_case_ids.len(), 1);
    assert!(diagnostic.comparator_sources.is_empty());
    assert!(diagnostic.profiler_note.is_none());
    validate(&valid, &"a".repeat(64)).expect("valid product diagnostic contract");

    let mut invalid = valid;
    invalid
        .groups
        .iter_mut()
        .find(|group| group.claim_class == ClaimClass::ProductDiagnostic)
        .expect("product diagnostic")
        .parity_eligibility = ParityEligibility::ThresholdEligible;
    assert!(matches!(
        validate(&invalid, &"a".repeat(64)),
        Err(GroupError::InvalidGroup(_))
    ));
}

#[test]
fn a7_decoder_diagnostics_are_executable_stab_only_contracts() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("checked performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");

    for (group_id, workload_id, measurement_id, expected_work) in [
        (
            super::super::invocation::A7_EXACT_ML_COMPILE_GROUP_ID,
            "exact-ml-compile",
            "compile-and-release",
            [1_536, 65_536, 4_194_304],
        ),
        (
            super::super::invocation::A7_EXACT_ML_REUSED_DECODE_GROUP_ID,
            "exact-ml-reused-decode",
            "decode-batch",
            [1_024, 65_536, 262_144],
        ),
        (
            super::super::invocation::A7_PIPELINE_GROUP_ID,
            "sample-detect-decode-pipeline",
            "sample-detect-decode",
            [1_024, 16_384, 262_144],
        ),
    ] {
        let contract = file
            .groups
            .iter()
            .find(|contract| contract.id.to_string() == group_id)
            .expect("A7 decoder diagnostic contract");
        assert_eq!(contract.claim_class, ClaimClass::ProductDiagnostic);
        assert_eq!(contract.parity_eligibility, ParityEligibility::ReportOnly);
        assert_eq!(contract.workload_id.to_string(), workload_id);
        assert_eq!(
            contract
                .measurement_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [measurement_id]
        );
        assert_eq!(
            contract
                .scales
                .iter()
                .map(|scale| scale.work_items.get())
                .collect::<Vec<_>>(),
            expected_work
        );
        assert!(!contract.correctness_case_ids.is_empty());
        assert!(contract.comparator_sources.is_empty());
        assert!(contract.profiler_note.is_none());
        assert!(super::super::invocation::supports_group(contract));
        let policy = file
            .product_diagnostic_policies
            .iter()
            .find(|policy| policy.group_id == contract.id)
            .expect("A7 source-owned diagnostic policy");
        assert_eq!(policy.scales.len(), contract.scales.len());
        let expected_batch_policy = if group_id == super::super::invocation::A7_PIPELINE_GROUP_ID {
            ProductDiagnosticBatchPolicy::SinglePass
        } else {
            ProductDiagnosticBatchPolicy::CalibratedRepeat
        };
        assert!(policy.scales.iter().all(|scale| {
            scale.batch_policy == expected_batch_policy
                && contract
                    .correctness_case_ids
                    .contains(&scale.witness_case_id)
        }));
        assert!(
            policy
                .scales
                .get(..2)
                .expect("small and medium scales")
                .iter()
                .all(|scale| scale.max_worker_peak_rss_bytes.is_none())
        );
        assert!(
            policy
                .scales
                .get(2)
                .expect("large scale")
                .max_worker_peak_rss_bytes
                .is_some()
        );

        let mut wrong_measurement = contract.clone();
        wrong_measurement.measurement_ids =
            vec![ProtocolId::try_new("wrong-measurement").expect("measurement id")];
        assert!(!super::super::invocation::supports_group(
            &wrong_measurement
        ));
    }
}

#[test]
fn a8_external_pass_is_an_exact_stab_only_diagnostic_contract() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("checked performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    let contract = file
        .groups
        .iter()
        .find(|contract| {
            contract.id.to_string() == super::super::invocation::A8_EXTERNAL_NOISE_PASS_GROUP_ID
        })
        .expect("A8 external-pass diagnostic contract");

    assert_eq!(contract.claim_class, ClaimClass::ProductDiagnostic);
    assert_eq!(contract.parity_eligibility, ParityEligibility::ReportOnly);
    assert_eq!(contract.workload_id.to_string(), "external-noise-pass");
    assert_eq!(
        contract
            .measurement_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["run-and-release"]
    );
    assert_eq!(
        contract
            .scales
            .iter()
            .map(|scale| scale.work_items.get())
            .collect::<Vec<_>>(),
        [64, 4_096, 65_536]
    );
    assert_eq!(
        contract.correctness_case_ids,
        [
            "cq-evidence-qualification-0b7d994f36856b37",
            "cq-evidence-qualification-462e13db123d5041",
            "cq-evidence-qualification-63d794137c5e40e8",
            "cq-evidence-qualification-7164c6c57e8187cf",
            "cq-evidence-qualification-7a01f0caf77063a0",
            "cq-evidence-qualification-8034f3ff6932ff5e",
            "cq-evidence-qualification-88972b4df64c8539",
            "cq-evidence-qualification-9271cf708f696d42",
            "cq-evidence-qualification-da7f35283dd1c657",
        ]
    );
    assert!(contract.comparator_sources.is_empty());
    assert!(contract.profiler_note.is_none());
    assert!(super::super::invocation::supports_group(contract));

    let policy = file
        .product_diagnostic_policies
        .iter()
        .find(|policy| policy.group_id == contract.id)
        .expect("A8 source-owned diagnostic policy");
    assert_eq!(policy.scales.len(), 3);
    assert!(policy.scales.iter().all(|scale| {
        scale.batch_policy == ProductDiagnosticBatchPolicy::CalibratedRepeat
            && scale.witness_case_id == "cq-evidence-qualification-7a01f0caf77063a0"
            && scale.max_worker_peak_rss_bytes.is_none()
    }));

    let mut wrong_measurement = contract.clone();
    wrong_measurement.measurement_ids =
        vec![ProtocolId::try_new("wrong-measurement").expect("measurement id")];
    assert!(!super::super::invocation::supports_group(
        &wrong_measurement
    ));
}

#[test]
fn product_diagnostic_suite_timeout_is_bounded_and_matches_inventory() {
    let mut invalid = valid_contract_file();
    invalid.product_diagnostic_suite_timeout_seconds =
        NonZeroU64::new(super::MAX_PRODUCT_DIAGNOSTIC_SUITE_TIMEOUT_SECONDS + 1)
            .expect("positive timeout");
    assert!(matches!(
        validate(&invalid, &"a".repeat(64)),
        Err(GroupError::InvalidProductDiagnosticSuiteTimeout(3_601))
    ));

    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
    let (file, _) = load(&root, &suite.semantic_digest).expect("runtime contract");
    suite
        .qualification_groups
        .iter_mut()
        .find(|group| group.id == super::super::invocation::A2_SAMPLER_COMPILE_GROUP_ID)
        .expect("sampler compile diagnostic")
        .timing_policy
        .timeout_seconds = 599;
    assert!(matches!(
        validate_inventory_contracts(&file, &suite),
        Err(GroupError::InventoryContract(group))
            if group == super::super::invocation::A2_SAMPLER_COMPILE_GROUP_ID
    ));
}

#[test]
fn product_diagnostic_policy_is_exactly_scoped_and_memory_bounded() {
    let mut file = valid_contract_file();
    let group = file
        .groups
        .iter()
        .find(|group| group.claim_class == ClaimClass::ProductDiagnostic)
        .expect("product diagnostic")
        .clone();
    let policy = product_diagnostic_policy(&group);
    let policy_index = file
        .product_diagnostic_policies
        .iter()
        .position(|candidate| candidate.group_id == group.id)
        .expect("product policy");
    *file
        .product_diagnostic_policies
        .get_mut(policy_index)
        .expect("product policy") = policy.clone();
    validate(&file, &"a".repeat(64)).expect("valid product policy");

    let mut duplicate = file.clone();
    duplicate.product_diagnostic_policies.push(policy.clone());
    assert!(matches!(
        validate(&duplicate, &"a".repeat(64)),
        Err(GroupError::InvalidProductDiagnosticPolicy(_))
    ));

    let mut missing_scale = file.clone();
    missing_scale
        .product_diagnostic_policies
        .get_mut(policy_index)
        .expect("product policy")
        .scales
        .clear();
    assert!(matches!(
        validate(&missing_scale, &"a".repeat(64)),
        Err(GroupError::InvalidProductDiagnosticPolicy(_))
    ));

    let mut wrong_class = file;
    let promotable_group_id = wrong_class
        .groups
        .iter()
        .find(|group| group.claim_class == ClaimClass::PromotablePerformance)
        .expect("promotable group")
        .id
        .clone();
    wrong_class
        .product_diagnostic_policies
        .get_mut(policy_index)
        .expect("product policy")
        .group_id = promotable_group_id;
    assert!(matches!(
        validate(&wrong_class, &"a".repeat(64)),
        Err(GroupError::InvalidProductDiagnosticPolicy(_))
    ));

    let mut missing_policy = valid_contract_file();
    missing_policy.product_diagnostic_policies.pop();
    assert!(matches!(
        validate(&missing_policy, &"a".repeat(64)),
        Err(GroupError::ProductDiagnosticPolicyCoverage)
    ));

    let mut stale_witness = valid_contract_file();
    stale_witness
        .product_diagnostic_policies
        .get_mut(policy_index)
        .and_then(|policy| policy.scales.first_mut())
        .expect("product scale policy")
        .witness_case_id = "cq-stale".to_string();
    assert!(matches!(
        validate(&stale_witness, &"a".repeat(64)),
        Err(GroupError::InvalidProductDiagnosticPolicy(_))
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
    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
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

    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
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
    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
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
    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
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
    let mut suite = crate::qualification::read(&root).expect("checked performance inventory");
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
    let suite = crate::qualification::read(&root).expect("checked performance inventory");
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
fn late_not_zero_source_contract_retains_observed_failure_owner() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("checked performance inventory");
    let group = load_group(
        &root,
        &suite.semantic_digest,
        super::super::invocation::SIMD_BITS_NOT_ZERO_LATE_GROUP_ID,
    )
    .expect("late-hit not-zero runtime contract");
    let note = group
        .contract
        .profiler_note
        .as_ref()
        .expect("observed failed-or-noisy outcome has durable failure ownership");

    assert_eq!(
        note.path.as_str(),
        "benchmarks/profiler-notes/qualification/perfq-m5-simd-bits-not-zero-late.md"
    );
}

#[test]
fn runtime_contract_rejects_stale_comparator_source_digest() {
    let root = RepoRoot::resolve(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("checked performance inventory");
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
