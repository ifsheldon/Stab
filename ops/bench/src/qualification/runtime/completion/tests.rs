use super::*;
use crate::qualification::runtime::correctness::CorrectnessPreflightStatus;

fn workers() -> WorkerIdentityEvidence {
    WorkerIdentityEvidence {
        stim_source_sha256: "a".repeat(64),
        stim_build_fingerprint: "b".repeat(64),
        stim_binary_sha256: "c".repeat(64),
        stab_source_sha256: "d".repeat(64),
        stab_build_fingerprint: "e".repeat(64),
        stab_binary_sha256: "f".repeat(64),
        contract_preflight_sha256: "1".repeat(64),
    }
}

fn correctness() -> CorrectnessPreflightEvidence {
    CorrectnessPreflightEvidence {
        status: CorrectnessPreflightStatus::Passed,
        case_ids: vec!["case".to_string()],
        reason: "passed".to_string(),
        source_directory: Some("target/qualification/correctness".to_string()),
        qualification_manifest_sha256: Some("2".repeat(64)),
        request_sha256: Some("3".repeat(64)),
        completion_sha256: Some("4".repeat(64)),
        report_sha256: Some("5".repeat(64)),
        preflight_sha256: Some("6".repeat(64)),
    }
}

fn artifact(path: &str) -> CompletionArtifact {
    CompletionArtifact {
        path: path.to_string(),
        report_sha256: "a".repeat(64),
        preflight_sha256: "b".repeat(64),
        markdown_sha256: "c".repeat(64),
    }
}

fn regression(group_id: &str, outcome: SelfRegressionOutcome) -> CompletionRegression {
    CompletionRegression {
        group_id: group_id.to_string(),
        outcome,
        checked_measurements: usize::from(outcome == SelfRegressionOutcome::Passed),
        unseeded_measurements: usize::from(outcome == SelfRegressionOutcome::Unseeded),
    }
}

fn rollup(group_id: &str, tier: QualificationTier) -> CompletionRollup {
    CompletionRollup {
        group_id: group_id.to_string(),
        group_contract_sha256: "d".repeat(64),
        tier,
        workload_id: "dem".to_string(),
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        comparator_sources: vec![("source.cc".to_string(), "e".repeat(64))],
        artifact: artifact(&format!(
            "target/benchmarks/qualification/{group_id}-{}",
            tier_name(tier)
        )),
        source_report_count: 9,
        parity_checked_measurements: 9,
        overall_outcome: GateOutcome::Passed,
    }
}

fn replay_evidence() -> RollupReplayEvidence {
    RollupReplayEvidence {
        output: PathBuf::from("target/benchmarks/qualification/rollup"),
        report_sha256: "a".repeat(64),
        preflight_sha256: "b".repeat(64),
        markdown_sha256: "c".repeat(64),
        group_id: DEM_PARSE_GROUP.to_string(),
        group_contract_sha256: "d".repeat(64),
        tier: QualificationTier::Full,
        performance_inventory_sha256: "e".repeat(64),
        stab_commit: "f".repeat(40),
        stim_commit: STIM_COMMIT.to_string(),
        host_policy_sha256: "1".repeat(64),
        host_profile_id: "controlled-aarch64".to_string(),
        operating_system: "linux".to_string(),
        architecture: "aarch64".to_string(),
        cpu_identity: "cpu".to_string(),
        rust_toolchain: "nightly".to_string(),
        target_triple: "aarch64-unknown-linux-gnu".to_string(),
        toolchain_sha256: "2".repeat(64),
        timing_boundary: TimingBoundary::RawWorkV2,
        workload_id: "dem-parse".to_string(),
        timing_batch_policy: TimingBatchPolicy::CommonIterations,
        comparator_sources: vec![("source.cc".to_string(), "3".repeat(64))],
        workers: workers(),
        correctness_preflight: correctness(),
        correctness_bindings: Vec::new(),
        overall_outcome: GateOutcome::Passed,
        sources: Vec::new(),
        scales: Vec::new(),
    }
}

fn source_reports() -> Vec<CompletionSourceReport> {
    expected_rollup_keys()
        .into_iter()
        .flat_map(|key| {
            let (group_id, tier) = key
                .rsplit_once(':')
                .map(|(group, tier)| {
                    (
                        group,
                        if tier == "full" {
                            QualificationTier::Full
                        } else {
                            QualificationTier::Soak
                        },
                    )
                })
                .expect("rollup key");
            let group_id = group_id.to_string();
            (0..9).map(move |index| CompletionSourceReport {
                group_id: group_id.clone(),
                tier,
                scale_id: format!("scale-{index}"),
                artifact: artifact(&format!(
                    "target/benchmarks/qualification/{group_id}-{}-{index}",
                    tier_name(tier)
                )),
            })
        })
        .collect()
}

fn memory() -> Vec<CompletionMemory> {
    source_reports()
        .into_iter()
        .map(|source| CompletionMemory {
            group_id: source.group_id,
            tier: source.tier,
            scale_id: source.scale_id,
            family_id: "family".to_string(),
            size_class: SizeClass::Small,
            stim_setup_rss_bytes: 1,
            stim_peak_rss_bytes: 2,
            stim_parent_observed_peak_rss_bytes: Some(3),
            stab_setup_rss_bytes: 4,
            stab_peak_rss_bytes: 5,
            stab_parent_observed_peak_rss_bytes: Some(6),
        })
        .collect()
}

fn manifest() -> CompletionManifest {
    CompletionManifest {
        schema_version: COMPLETION_SCHEMA_VERSION,
        output: "target/benchmarks/qualification/completion".to_string(),
        generated_unix_epoch_seconds: 1,
        scope_id: DEM_SCOPE_ID.to_string(),
        performance_inventory_sha256: "1".repeat(64),
        correctness_inventory_sha256: "2".repeat(64),
        parity_policy_sha256: "3".repeat(64),
        regression_policy_sha256: "4".repeat(64),
        regression_baselines_sha256: "5".repeat(64),
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        repository: RepositoryEvidence {
            commit_before: "6".repeat(40),
            commit_after: "6".repeat(40),
            local_modifications_before: false,
            local_modifications_after: false,
        },
        environment: CompletionEnvironment {
            host_policy_sha256: "7".repeat(64),
            host_profile_id: "host".to_string(),
            operating_system: "linux".to_string(),
            architecture: "aarch64".to_string(),
            cpu_identity: "cpu".to_string(),
            rust_toolchain: "nightly".to_string(),
            target_triple: "aarch64-unknown-linux-gnu".to_string(),
            toolchain_sha256: "8".repeat(64),
        },
        workers: workers(),
        timing_boundary: TimingBoundary::RawWorkV2,
        correctness_preflight: correctness(),
        rollups: vec![
            rollup(DEM_PARSE_GROUP, QualificationTier::Full),
            rollup(DEM_PARSE_GROUP, QualificationTier::Soak),
            rollup(DEM_PRINT_GROUP, QualificationTier::Full),
            rollup(DEM_PRINT_GROUP, QualificationTier::Soak),
        ],
        source_reports: source_reports(),
        memory: memory(),
        parity_outcome: GateOutcome::Passed,
        regression_outcomes: vec![
            regression(DEM_PARSE_GROUP, SelfRegressionOutcome::Unseeded),
            regression(DEM_PRINT_GROUP, SelfRegressionOutcome::Unseeded),
        ],
        environment_valid: true,
        memory_scaling_status: MemoryScalingStatus::Recorded,
    }
}

#[test]
fn completion_manifest_rejects_missing_extra_duplicate_and_failed_rollups() {
    let valid = manifest();
    validate_manifest(&valid).expect("valid completion manifest");

    let mut missing = valid.clone();
    missing.rollups.pop();
    assert!(validate_manifest(&missing).is_err());

    let mut extra = valid.clone();
    extra
        .rollups
        .push(rollup(DEM_PARSE_GROUP, QualificationTier::Full));
    assert!(validate_manifest(&extra).is_err());

    let mut duplicate = valid.clone();
    let first_rollup = duplicate.rollups.first().expect("first rollup").clone();
    *duplicate.rollups.get_mut(1).expect("second rollup") = first_rollup;
    assert!(validate_manifest(&duplicate).is_err());

    let mut failed = valid;
    failed
        .rollups
        .first_mut()
        .expect("first rollup")
        .overall_outcome = GateOutcome::Failed;
    assert!(validate_manifest(&failed).is_err());
}

#[test]
fn completion_manifest_distinguishes_unseeded_and_passing_regression() {
    let mut current = manifest();
    validate_manifest(&current).expect("unseeded first-run manifest");
    current.regression_outcomes = vec![
        regression(DEM_PARSE_GROUP, SelfRegressionOutcome::Passed),
        regression(DEM_PRINT_GROUP, SelfRegressionOutcome::Passed),
    ];
    validate_manifest(&current).expect("seeded regression manifest");

    current
        .regression_outcomes
        .first_mut()
        .expect("first regression outcome")
        .unseeded_measurements = 1;
    assert!(validate_manifest(&current).is_err());

    let mut false_unseeded = manifest();
    false_unseeded
        .regression_outcomes
        .first_mut()
        .expect("first regression outcome")
        .unseeded_measurements = 0;
    assert!(validate_manifest(&false_unseeded).is_err());

    let mut wrong_group_order = manifest();
    wrong_group_order.regression_outcomes.swap(0, 1);
    assert!(validate_manifest(&wrong_group_order).is_err());
}

#[test]
fn completion_rejects_mixed_source_host_and_inventory_identities() {
    let first = replay_evidence();
    let second = first.clone();
    shared_identity(&[first.clone(), second]).expect("matching identity");

    let mut mixed_commit = first.clone();
    mixed_commit.stab_commit = "0".repeat(40);
    assert!(matches!(
        shared_identity(&[first.clone(), mixed_commit]),
        Err(CompletionError::MixedIdentity)
    ));

    let mut mixed_host = first.clone();
    mixed_host.cpu_identity = "different-cpu".to_string();
    assert!(matches!(
        shared_identity(&[first.clone(), mixed_host]),
        Err(CompletionError::MixedIdentity)
    ));

    let mut mixed_inventory = first.clone();
    mixed_inventory.performance_inventory_sha256 = "9".repeat(64);
    assert!(matches!(
        shared_identity(&[first, mixed_inventory]),
        Err(CompletionError::MixedIdentity)
    ));
}

#[test]
fn completion_json_and_markdown_replay_are_deterministic() {
    let manifest = manifest();
    let first = canonical_json(&manifest).expect("first manifest");
    let second = canonical_json(&manifest).expect("second manifest");
    assert_eq!(first, second);
    assert_eq!(
        render_markdown(&manifest, &sha256_hex(&first)),
        render_markdown(&manifest, &sha256_hex(&second))
    );
}

#[test]
fn completion_scope_rejects_unknown_missing_and_duplicate_rollups() {
    assert!(matches!(
        require_scope("unknown", EXPECTED_DEM_ROLLUPS),
        Err(CompletionError::UnknownScope(_))
    ));
    assert!(matches!(
        require_scope(DEM_SCOPE_ID, EXPECTED_DEM_ROLLUPS - 1),
        Err(CompletionError::RollupCount(_))
    ));
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/completion",
    ))
    .expect("output");
    let duplicate = PathBuf::from("target/benchmarks/qualification/rollup");
    assert!(matches!(
        admit_paths(&output, &[duplicate.clone(), duplicate]),
        Err(CompletionError::DuplicatePath(_))
    ));
    assert!(matches!(
        admit_paths(&output, &[output.as_path().to_path_buf()]),
        Err(CompletionError::OutputCollision(_))
    ));
}

#[test]
fn legacy_schema_one_receipts_remain_readable_but_not_current() {
    let receipt = serde_json::json!({
        "schema_version": 1,
        "output": "target/benchmarks/qualification/historical",
        "generated_unix_epoch_seconds": 1,
        "group_id": "historical-group",
        "group_contract_sha256": "a".repeat(64),
        "performance_inventory_sha256": "b".repeat(64),
        "correctness_inventory_sha256": "c".repeat(64),
        "stim_tag": STIM_TAG,
        "stim_commit": STIM_COMMIT,
        "repository": {
            "commit_before": "d".repeat(40),
            "commit_after": "d".repeat(40),
            "local_modifications_before": false,
            "local_modifications_after": false
        },
        "environment": {
            "host_policy_sha256": "e".repeat(64),
            "host_profile_id": "host",
            "architecture": "aarch64",
            "cpu_identity": "cpu",
            "target_triple": "aarch64-unknown-linux-gnu",
            "toolchain_sha256": "f".repeat(64)
        },
        "workers": manifest().workers,
        "correctness_preflight": manifest().correctness_preflight,
        "source_reports": [],
        "rollups": [],
        "steps": []
    });
    let bytes = serde_json::to_vec(&receipt).expect("legacy bytes");
    let summary = legacy::parse(&bytes).expect("legacy receipt");
    assert_eq!(summary.group_id, "historical-group");
    assert_eq!(schema_version(&bytes).expect("schema"), 1);
}
