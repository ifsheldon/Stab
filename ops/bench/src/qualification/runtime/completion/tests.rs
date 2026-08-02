use super::*;
use crate::qualification::runtime::correctness::{
    CorrectnessPreflightEvidence, CorrectnessPreflightStatus,
};

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
    correctness_for("shared", &["case".to_string()], &"2".repeat(64))
}

fn correctness_for(
    group_id: &str,
    case_ids: &[String],
    correctness_inventory_sha256: &str,
) -> CorrectnessPreflightEvidence {
    let digest = |label: &str| sha256_hex(format!("{group_id}:{label}").as_bytes());
    CorrectnessPreflightEvidence {
        status: CorrectnessPreflightStatus::Passed,
        case_ids: case_ids.to_vec(),
        reason: "passed".to_string(),
        source_directory: Some(format!("target/qualification/correctness/{group_id}")),
        qualification_manifest_sha256: Some(correctness_inventory_sha256.to_string()),
        request_sha256: Some(digest("request")),
        completion_sha256: Some(digest("completion")),
        report_sha256: Some(digest("report")),
        preflight_sha256: Some(digest("preflight")),
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

fn dem_scope() -> CompletionScope {
    CompletionScope {
        id: DEM_SCOPE_ID.to_string(),
        group_ids: vec![DEM_PARSE_GROUP.to_string(), DEM_PRINT_GROUP.to_string()],
        correctness_case_ids: [
            (DEM_PARSE_GROUP.to_string(), vec!["case".to_string()]),
            (DEM_PRINT_GROUP.to_string(), vec!["case".to_string()]),
        ]
        .into_iter()
        .collect(),
        expected_source_reports: 36,
    }
}

fn source_reports() -> Vec<CompletionSourceReport> {
    expected_rollup_keys(&dem_scope())
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
        correctness_preflights: vec![
            CompletionCorrectness {
                group_id: DEM_PARSE_GROUP.to_string(),
                evidence: correctness(),
            },
            CompletionCorrectness {
                group_id: DEM_PRINT_GROUP.to_string(),
                evidence: correctness(),
            },
        ],
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

fn release_manifest(
    root: &RepoRoot,
    performance_inventory_sha256: &str,
    correctness_inventory_sha256: &str,
    scope: &CompletionScope,
) -> CompletionManifest {
    let groups = super::super::group::load_groups(root, performance_inventory_sha256)
        .expect("checked runtime groups");
    let mut result = manifest();
    result.scope_id = scope.id.clone();
    result.performance_inventory_sha256 = performance_inventory_sha256.to_string();
    result.correctness_inventory_sha256 = correctness_inventory_sha256.to_string();
    result.correctness_preflights.clear();
    result.rollups.clear();
    result.source_reports.clear();
    result.memory.clear();
    result.regression_outcomes.clear();

    for group_id in &scope.group_ids {
        let group = groups
            .iter()
            .find(|group| group.id.to_string() == *group_id)
            .expect("release group contract");
        let case_ids = scope
            .correctness_case_ids
            .get(group_id)
            .expect("release correctness cases");
        result.correctness_preflights.push(CompletionCorrectness {
            group_id: group_id.clone(),
            evidence: correctness_for(group_id, case_ids, correctness_inventory_sha256),
        });
        result
            .regression_outcomes
            .push(regression(group_id, SelfRegressionOutcome::Unseeded));

        for tier in [QualificationTier::Full, QualificationTier::Soak] {
            result.rollups.push(CompletionRollup {
                group_id: group_id.clone(),
                group_contract_sha256: sha256_hex(format!("{group_id}:contract").as_bytes()),
                tier,
                workload_id: group.workload_id.to_string(),
                timing_batch_policy: group.timing_batch_policy,
                comparator_sources: group
                    .comparator_sources
                    .iter()
                    .map(|source| {
                        (
                            source.path.as_str().to_string(),
                            source.sha256.as_str().to_string(),
                        )
                    })
                    .collect(),
                artifact: artifact(&format!(
                    "target/benchmarks/qualification/{group_id}-{}-rollup",
                    tier_name(tier)
                )),
                source_report_count: group.scales.len(),
                parity_checked_measurements: group
                    .scales
                    .len()
                    .checked_mul(group.measurement_ids.len())
                    .expect("parity count"),
                overall_outcome: GateOutcome::Passed,
            });
            for scale in &group.scales {
                let scale_id = scale.id.to_string();
                result.source_reports.push(CompletionSourceReport {
                    group_id: group_id.clone(),
                    tier,
                    scale_id: scale_id.clone(),
                    artifact: artifact(&format!(
                        "target/benchmarks/qualification/{group_id}-{}-{scale_id}",
                        tier_name(tier)
                    )),
                });
                result.memory.push(CompletionMemory {
                    group_id: group_id.clone(),
                    tier,
                    scale_id,
                    family_id: scale.family_id.to_string(),
                    size_class: scale.size_class,
                    stim_setup_rss_bytes: 1,
                    stim_peak_rss_bytes: 2,
                    stim_parent_observed_peak_rss_bytes: Some(3),
                    stab_setup_rss_bytes: 4,
                    stab_peak_rss_bytes: 5,
                    stab_parent_observed_peak_rss_bytes: Some(6),
                });
            }
        }
    }
    result
}

#[test]
fn completion_manifest_rejects_missing_extra_duplicate_and_failed_rollups() {
    let valid = manifest();
    let scope = dem_scope();
    validate_manifest(&valid, &scope).expect("valid completion manifest");

    let mut missing = valid.clone();
    missing.rollups.pop();
    assert!(validate_manifest(&missing, &scope).is_err());

    let mut extra = valid.clone();
    extra
        .rollups
        .push(rollup(DEM_PARSE_GROUP, QualificationTier::Full));
    assert!(validate_manifest(&extra, &scope).is_err());

    let mut duplicate = valid.clone();
    let first_rollup = duplicate.rollups.first().expect("first rollup").clone();
    *duplicate.rollups.get_mut(1).expect("second rollup") = first_rollup;
    assert!(validate_manifest(&duplicate, &scope).is_err());

    let mut failed = valid;
    failed
        .rollups
        .first_mut()
        .expect("first rollup")
        .overall_outcome = GateOutcome::Failed;
    assert!(validate_manifest(&failed, &scope).is_err());
}

#[test]
fn completion_manifest_distinguishes_unseeded_and_passing_regression() {
    let mut current = manifest();
    let scope = dem_scope();
    validate_manifest(&current, &scope).expect("unseeded first-run manifest");
    current.regression_outcomes = vec![
        regression(DEM_PARSE_GROUP, SelfRegressionOutcome::Passed),
        regression(DEM_PRINT_GROUP, SelfRegressionOutcome::Passed),
    ];
    validate_manifest(&current, &scope).expect("seeded regression manifest");

    current
        .regression_outcomes
        .first_mut()
        .expect("first regression outcome")
        .unseeded_measurements = 1;
    assert!(validate_manifest(&current, &scope).is_err());

    let mut false_unseeded = manifest();
    false_unseeded
        .regression_outcomes
        .first_mut()
        .expect("first regression outcome")
        .unseeded_measurements = 0;
    assert!(validate_manifest(&false_unseeded, &scope).is_err());

    let mut wrong_group_order = manifest();
    wrong_group_order.regression_outcomes.swap(0, 1);
    assert!(validate_manifest(&wrong_group_order, &scope).is_err());
}

#[test]
fn completion_rejects_mixed_source_host_and_inventory_identities() {
    let first = replay_evidence();
    let second = first.clone();
    shared_identity(&[first.clone(), second]).expect("matching identity");

    let mut group_specific_correctness = first.clone();
    group_specific_correctness.correctness_preflight = correctness_for(
        "another-group",
        &["another-case".to_string()],
        &"2".repeat(64),
    );
    shared_identity(&[first.clone(), group_specific_correctness])
        .expect("correctness is group-specific, not a shared identity");

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
fn completion_requires_exact_correctness_per_group() {
    let scope = dem_scope();
    let correctness_inventory_sha256 = "2".repeat(64);
    let rollups = expected_rollup_keys(&scope)
        .into_iter()
        .map(|key| {
            let (group_id, tier) = key.rsplit_once(':').expect("rollup key");
            let mut evidence = replay_evidence();
            evidence.group_id = group_id.to_string();
            evidence.tier = if tier == "full" {
                QualificationTier::Full
            } else {
                QualificationTier::Soak
            };
            evidence.correctness_preflight = correctness_for(
                group_id,
                scope
                    .correctness_case_ids
                    .get(group_id)
                    .expect("group correctness cases"),
                &correctness_inventory_sha256,
            );
            evidence
        })
        .collect::<Vec<_>>();

    let grouped = completion_correctness(&rollups, &scope, &correctness_inventory_sha256)
        .expect("group-specific correctness");
    assert_eq!(
        grouped
            .iter()
            .map(|correctness| correctness.group_id.as_str())
            .collect::<Vec<_>>(),
        scope
            .group_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );

    let mut mismatched = rollups.clone();
    mismatched
        .iter_mut()
        .find(|rollup| rollup.group_id == DEM_PARSE_GROUP && rollup.tier == QualificationTier::Soak)
        .expect("parse soak")
        .correctness_preflight
        .report_sha256 = Some("9".repeat(64));
    assert!(matches!(
        completion_correctness(&mismatched, &scope, &correctness_inventory_sha256),
        Err(CompletionError::GroupCorrectness(group)) if group == DEM_PARSE_GROUP
    ));

    let mut wrong_cases = rollups;
    wrong_cases
        .iter_mut()
        .filter(|rollup| rollup.group_id == DEM_PARSE_GROUP)
        .for_each(|rollup| rollup.correctness_preflight.case_ids = vec!["wrong-case".to_string()]);
    assert!(matches!(
        completion_correctness(&wrong_cases, &scope, &correctness_inventory_sha256),
        Err(CompletionError::GroupCorrectness(group)) if group == DEM_PARSE_GROUP
    ));

    let mut missing = manifest();
    missing.correctness_preflights.pop();
    assert!(validate_manifest(&missing, &scope).is_err());
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
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("performance inventory");
    assert!(matches!(
        scope::load(&root, &suite.semantic_digest, "unknown"),
        Err(CompletionError::UnknownScope(_))
    ));
    assert!(matches!(
        require_scope(&dem_scope(), 3),
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
fn completion_scopes_cover_historical_dem_and_complete_release_matrix() {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("performance inventory");

    let dem =
        scope::load(&root, &suite.semantic_digest, DEM_SCOPE_ID).expect("historical DEM scope");
    assert_eq!(dem.group_ids.len(), 2);
    assert_eq!(expected_rollup_keys(&dem).len(), 4);
    assert_eq!(dem.expected_source_reports, 36);

    let release =
        scope::load(&root, &suite.semantic_digest, RELEASE_SCOPE_ID).expect("A9 release scope");
    assert_eq!(release.group_ids.len(), 19);
    assert_eq!(expected_rollup_keys(&release).len(), 38);
    assert_eq!(release.expected_source_reports, 138);
    assert!(release.group_ids.contains(&DEM_PARSE_GROUP.to_string()));
    assert!(release.group_ids.contains(&DEM_PRINT_GROUP.to_string()));

    let complete = release_manifest(
        &root,
        &suite.semantic_digest,
        &suite.correctness_digest,
        &release,
    );
    validate_manifest(&complete, &release).expect("complete A9 release manifest");
    assert_eq!(complete.correctness_preflights.len(), 19);
    assert_eq!(complete.rollups.len(), 38);
    assert_eq!(complete.source_reports.len(), 138);
    assert_eq!(complete.memory.len(), 138);
    assert!(
        complete.correctness_preflights.windows(2).any(|pair| {
            pair.first()
                .zip(pair.last())
                .is_some_and(|(first, last)| first.evidence.case_ids != last.evidence.case_ids)
        }),
        "the checked A9 matrix must exercise group-specific correctness sets"
    );

    let mut release_rollups = expected_rollup_keys(&release)
        .into_iter()
        .rev()
        .map(|key| {
            let (group_id, tier) = key.rsplit_once(':').expect("rollup key");
            let mut evidence = replay_evidence();
            evidence.group_id = group_id.to_string();
            evidence.tier = if tier == "full" {
                QualificationTier::Full
            } else {
                QualificationTier::Soak
            };
            evidence.correctness_preflight = correctness_for(
                group_id,
                release
                    .correctness_case_ids
                    .get(group_id)
                    .expect("group correctness cases"),
                &suite.correctness_digest,
            );
            evidence
        })
        .collect::<Vec<_>>();
    order_and_validate_scope(&release, &mut release_rollups).expect("ordered release rollups");
    shared_identity(&release_rollups).expect("shared non-correctness identities");
    assert_eq!(
        completion_correctness(&release_rollups, &release, &suite.correctness_digest)
            .expect("A9 group correctness"),
        complete.correctness_preflights
    );

    let paths = (0..expected_rollup_keys(&release).len())
        .map(|index| PathBuf::from(format!("target/benchmarks/qualification/rollup-{index}")))
        .collect::<Vec<_>>();
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/completion",
    ))
    .expect("output");
    assert_eq!(
        admit_paths(&output, &paths)
            .expect("release rollup paths")
            .len(),
        38
    );

    assert_eq!(
        release_rollups
            .iter()
            .map(|rollup| rollup_key(&rollup.group_id, rollup.tier))
            .collect::<Vec<_>>(),
        expected_rollup_keys(&release)
    );

    let mut missing = release_rollups.clone();
    missing.pop();
    assert!(matches!(
        order_and_validate_scope(&release, &mut missing),
        Err(CompletionError::MissingRollup(_))
    ));

    let mut diagnostic = release_rollups;
    diagnostic.first_mut().expect("first rollup").group_id =
        "PERFQ-A2-SAMPLING-REQUEST-ESTIMATE".to_string();
    assert!(matches!(
        order_and_validate_scope(&release, &mut diagnostic),
        Err(CompletionError::UnknownRollup(_))
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
        "correctness_preflight": manifest()
            .correctness_preflights
            .first()
            .expect("correctness preflight")
            .evidence
            .clone(),
        "source_reports": [],
        "rollups": [],
        "steps": []
    });
    let bytes = serde_json::to_vec(&receipt).expect("legacy bytes");
    let summary = legacy::parse_v1(&bytes).expect("legacy receipt");
    assert_eq!(summary.group_id, "historical-group");
    assert_eq!(schema_version(&bytes).expect("schema"), 1);
}

#[test]
fn schema_two_completion_manifests_remain_readable_but_not_current() {
    let current = manifest();
    let mut value = serde_json::to_value(&current).expect("completion value");
    let object = value.as_object_mut().expect("completion object");
    object.insert("schema_version".to_string(), serde_json::json!(2));
    let correctness = object
        .remove("correctness_preflights")
        .and_then(|value| value.as_array().cloned())
        .and_then(|entries| entries.into_iter().next())
        .and_then(|entry| entry.get("evidence").cloned())
        .expect("legacy correctness preflight");
    object.insert("correctness_preflight".to_string(), correctness);
    let bytes = serde_json::to_vec(&value).expect("schema two bytes");

    let summary = legacy::parse_v2(&bytes).expect("schema two completion");
    assert_eq!(summary.group_id, DEM_SCOPE_ID);
    assert_eq!(summary.output, current.output);
    assert_eq!(schema_version(&bytes).expect("schema"), 2);
}
