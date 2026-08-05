use super::*;
use crate::qualification::runtime::correctness::{
    CorrectnessError, CorrectnessPreflightEvidence, CorrectnessPreflightStatus,
};

mod memory_identity;

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
        source_directory: Some(format!("target/qualification/{group_id}")),
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

fn replay_evidence_with_binding(
    group_id: &str,
    tier: QualificationTier,
    correctness_preflight: CorrectnessPreflightEvidence,
    binding: std::sync::Arc<crate::qualification::runtime::correctness::CorrectnessArtifactBinding>,
) -> RollupReplayEvidence {
    let mut evidence = replay_evidence();
    evidence.group_id = group_id.to_string();
    evidence.tier = tier;
    evidence.correctness_preflight = correctness_preflight;
    evidence.correctness_bindings = vec![binding];
    evidence
}

fn default_binding()
-> std::sync::Arc<crate::qualification::runtime::correctness::CorrectnessArtifactBinding> {
    std::sync::Arc::new(
        crate::qualification::runtime::correctness::CorrectnessArtifactBinding::default(),
    )
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

fn accepted_maximum_memory_receipts() -> Vec<CompletionAcceptedMaximumMemoryReceipt> {
    ACCEPTED_MAXIMUM_MEMORY_GROUPS
        .into_iter()
        .map(|group_id| CompletionAcceptedMaximumMemoryReceipt {
            group_id: group_id.to_string(),
            path: format!("target/benchmarks/qualification/{group_id}-memory"),
            report_sha256: "9".repeat(64),
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
            soft_nofile_limit: RELEASE_SOFT_NOFILE_LIMIT,
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
        accepted_maximum_memory_receipts: accepted_maximum_memory_receipts(),
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
    let mut correctness_by_cases = BTreeMap::new();

    for group_id in &scope.group_ids {
        let group = groups
            .iter()
            .find(|group| group.id.to_string() == *group_id)
            .expect("release group contract");
        let case_ids = scope
            .correctness_case_ids
            .get(group_id)
            .expect("release correctness cases");
        let evidence = correctness_by_cases
            .entry(case_ids.clone())
            .or_insert_with(|| correctness_for(group_id, case_ids, correctness_inventory_sha256))
            .clone();
        result.correctness_preflights.push(CompletionCorrectness {
            group_id: group_id.clone(),
            evidence,
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
fn completion_manifest_authenticates_memory_receipts_and_descriptor_limit() {
    let scope = dem_scope();
    let valid = manifest();
    validate_manifest(&valid, &scope).expect("valid memory contract");

    let mut missing_receipt = valid.clone();
    missing_receipt.accepted_maximum_memory_receipts.pop();
    assert!(validate_manifest(&missing_receipt, &scope).is_err());

    let mut wrong_group = valid.clone();
    wrong_group
        .accepted_maximum_memory_receipts
        .first_mut()
        .expect("memory receipt")
        .group_id = "wrong-group".to_string();
    assert!(validate_manifest(&wrong_group, &scope).is_err());

    let mut missing_limit = valid;
    missing_limit.environment.soft_nofile_limit = 0;
    assert!(validate_manifest(&missing_limit, &scope).is_err());

    let mut release_scope = scope;
    release_scope.id = RELEASE_SCOPE_ID.to_string();
    assert_eq!(
        validate_completion_nofile_limit(&release_scope, Some(RELEASE_SOFT_NOFILE_LIMIT))
            .expect("release descriptor limit"),
        RELEASE_SOFT_NOFILE_LIMIT
    );
    assert!(matches!(
        validate_completion_nofile_limit(&release_scope, Some(RELEASE_SOFT_NOFILE_LIMIT + 1)),
        Err(CompletionError::DescriptorLimit { .. })
    ));
    assert!(matches!(
        validate_completion_nofile_limit(&release_scope, None),
        Err(CompletionError::DescriptorLimit { .. })
    ));
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
                "shared",
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

    let mut unshared = expected_rollup_keys(&scope)
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
    order_and_validate_scope(&scope, &mut unshared).expect("ordered fixture");
    assert!(matches!(
        completion_correctness(&unshared, &scope, &correctness_inventory_sha256),
        Err(CompletionError::CorrectnessArtifactCount)
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
            evidence.correctness_preflight = complete
                .correctness_preflights
                .iter()
                .find(|correctness| correctness.group_id == group_id)
                .expect("group correctness evidence")
                .evidence
                .clone();
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
fn checked_status_manifest_authenticates_current_release_contracts() {
    let root = RepoRoot::resolve(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let suite = crate::qualification::read(&root).expect("performance inventory");
    let scope =
        scope::load(&root, &suite.semantic_digest, RELEASE_SCOPE_ID).expect("A9 release scope");
    let mut manifest = release_manifest(
        &root,
        &suite.semantic_digest,
        &suite.correctness_digest,
        &scope,
    );
    manifest.parity_policy_sha256 =
        super::super::parity::policy_sha256(&root, &suite.semantic_digest).expect("parity policy");
    let regression =
        super::super::self_regression::source_identities(&root, &suite.semantic_digest)
            .expect("regression identities");
    manifest.regression_policy_sha256 = regression.policy_sha256;
    manifest.regression_baselines_sha256 = regression.baselines_sha256;
    let bytes = canonical_json(&manifest).expect("canonical manifest");

    let inspected = inspect_status_manifest(
        &root,
        &bytes,
        &suite.semantic_digest,
        &suite.correctness_digest,
        &manifest.parity_policy_sha256,
        &manifest.regression_policy_sha256,
        &manifest.regression_baselines_sha256,
    )
    .expect("current status manifest");
    assert!(inspected.matches_current_contract);
    assert_eq!(inspected.summary.scope_id, RELEASE_SCOPE_ID);
    assert_eq!(
        inspected.summary.stab_commit,
        manifest.repository.commit_after
    );
    assert_eq!(
        inspected.summary.regression,
        CompletionStatusRegression::Unseeded
    );

    let historical = inspect_status_manifest(
        &root,
        &bytes,
        &"0".repeat(64),
        &suite.correctness_digest,
        &manifest.parity_policy_sha256,
        &manifest.regression_policy_sha256,
        &manifest.regression_baselines_sha256,
    )
    .expect("well-formed historical manifest");
    assert!(!historical.matches_current_contract);

    let mut malformed = manifest;
    malformed.environment.architecture.clear();
    assert!(
        inspect_status_manifest(
            &root,
            &canonical_json(&malformed).expect("canonical malformed manifest"),
            &suite.semantic_digest,
            &suite.correctness_digest,
            &malformed.parity_policy_sha256,
            &malformed.regression_policy_sha256,
            &malformed.regression_baselines_sha256,
        )
        .is_err()
    );
    let mut noncanonical = bytes;
    noncanonical.extend_from_slice(b" \n");
    assert!(
        inspect_status_manifest(
            &root,
            &noncanonical,
            &suite.semantic_digest,
            &suite.correctness_digest,
            &"0".repeat(64),
            &"0".repeat(64),
            &"0".repeat(64),
        )
        .is_err()
    );
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
    object.remove("accepted_maximum_memory_receipts");
    object
        .get_mut("environment")
        .and_then(serde_json::Value::as_object_mut)
        .expect("completion environment")
        .remove("soft_nofile_limit");
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

#[test]
fn schema_three_completion_manifests_remain_readable_but_not_current() {
    let current = manifest();
    let mut value = serde_json::to_value(&current).expect("completion value");
    let object = value.as_object_mut().expect("completion object");
    object.insert("schema_version".to_string(), serde_json::json!(3));
    object.remove("accepted_maximum_memory_receipts");
    object
        .get_mut("environment")
        .and_then(serde_json::Value::as_object_mut)
        .expect("completion environment")
        .remove("soft_nofile_limit");
    let bytes = serde_json::to_vec(&value).expect("schema three bytes");

    let summary = legacy::parse_v3(&bytes).expect("schema three completion");
    assert_eq!(summary.group_id, DEM_SCOPE_ID);
    assert_eq!(summary.output, current.output);
    assert_eq!(schema_version(&bytes).expect("schema"), 3);
}

#[test]
fn completion_retains_one_binding_per_exact_correctness_artifact() {
    let shared = correctness_for("shared", &["case".to_string()], &"2".repeat(64));
    let distinct = correctness_for("distinct", &["case".to_string()], &"2".repeat(64));
    let mut rollups = vec![
        replay_evidence_with_binding(
            "group-a",
            QualificationTier::Full,
            shared.clone(),
            default_binding(),
        ),
        replay_evidence_with_binding(
            "group-a",
            QualificationTier::Soak,
            shared.clone(),
            default_binding(),
        ),
        replay_evidence_with_binding(
            "group-b",
            QualificationTier::Full,
            shared,
            default_binding(),
        ),
        replay_evidence_with_binding(
            "group-c",
            QualificationTier::Full,
            distinct,
            default_binding(),
        ),
    ];
    let mut retained = bindings::RetainedBindings::default();
    for rollup in &mut rollups {
        retained.admit(rollup).expect("admit binding");
    }

    assert_eq!(retained.len(), 2);
    assert!(
        rollups
            .iter()
            .all(|rollup| rollup.correctness_bindings.is_empty())
    );
    assert_eq!(retained.into_values().len(), 2);
}

#[test]
fn completion_rejects_a_rollup_without_one_correctness_binding() {
    let mut rollup = replay_evidence();
    let mut retained = bindings::RetainedBindings::default();

    assert!(matches!(
        retained.admit(&mut rollup),
        Err(CompletionError::GroupCorrectness(group)) if group == DEM_PARSE_GROUP
    ));
}

#[test]
fn repeated_correctness_prerequisites_do_not_grow_retained_bindings() {
    let shared = correctness_for("shared", &["case".to_string()], &"2".repeat(64));
    let mut retained = bindings::RetainedBindings::default();

    for index in 0..MAX_ROLLUPS {
        let mut rollup = replay_evidence_with_binding(
            &format!("group-{index}"),
            QualificationTier::Full,
            shared.clone(),
            default_binding(),
        );
        retained.admit(&mut rollup).expect("admit binding");
        assert_eq!(retained.len(), 1);
        assert!(rollup.correctness_bindings.is_empty());
    }
}

#[test]
fn duplicate_correctness_binding_is_revalidated_before_release() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve correctness repository");
    let relative = Path::new("correctness-source");
    let output = repository.path().join(relative);
    let case = output.join("cases/case-a");
    std::fs::create_dir_all(&case).expect("create correctness case");
    for name in [
        "completion.json",
        "preflight.json",
        "report.json",
        "report.md",
        "request.json",
    ] {
        std::fs::write(output.join(name), format!("{name}\n")).expect("write correctness artifact");
    }
    std::fs::write(case.join("execution-receipt.json"), b"receipt\n")
        .expect("write correctness receipt");
    let first = crate::qualification::runtime::correctness::bind_test_artifact_tree(
        &root,
        relative,
        &["case-a"],
    )
    .expect("bind first correctness tree");
    let second = crate::qualification::runtime::correctness::bind_test_artifact_tree(
        &root,
        relative,
        &["case-a"],
    )
    .expect("bind second correctness tree");
    let preflight = correctness_for("shared", &["case-a".to_string()], &"2".repeat(64));
    let mut first_rollup = replay_evidence_with_binding(
        "group-a",
        QualificationTier::Full,
        preflight.clone(),
        std::sync::Arc::new(first),
    );
    let mut second_rollup = replay_evidence_with_binding(
        "group-b",
        QualificationTier::Full,
        preflight,
        std::sync::Arc::new(second),
    );
    let mut retained = bindings::RetainedBindings::default();
    retained
        .admit(&mut first_rollup)
        .expect("admit current first binding");

    std::fs::write(output.join("unexpected"), b"replacement\n").expect("mutate correctness tree");
    assert!(matches!(
        retained.admit(&mut second_rollup),
        Err(CompletionError::Correctness(
            CorrectnessError::ArtifactChanged(_)
        ))
    ));
}

fn write_performance_artifact(root: &RepoRoot, name: &str) -> (PathBuf, [String; 3]) {
    let relative = PathBuf::from("target/benchmarks/qualification").join(name);
    let directory = root.path.join(&relative);
    std::fs::create_dir_all(&directory).expect("create performance artifact");
    let bytes = [b"report\n".as_slice(), b"preflight\n", b"markdown\n"];
    for (name, bytes) in ["report.json", "preflight.json", "report.md"]
        .into_iter()
        .zip(bytes)
    {
        std::fs::write(directory.join(name), bytes).expect("write performance artifact");
    }
    (
        relative,
        bytes.map(crate::qualification::runtime::run::sha256_hex),
    )
}

#[test]
fn completion_revalidates_retained_rollup_and_source_artifacts() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let (rollup_path, rollup_digests) = write_performance_artifact(&root, "bound-rollup");
    let (source_path, source_digests) = write_performance_artifact(&root, "bound-source");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context =
        RetainedArtifactContext::open(&root, &live_repository).expect("open artifact context");
    let mut evidence = replay_evidence();
    evidence.output = rollup_path;
    evidence.report_sha256 = rollup_digests[0].clone();
    evidence.preflight_sha256 = rollup_digests[1].clone();
    evidence.markdown_sha256 = rollup_digests[2].clone();
    evidence.sources.push(RollupSourceEvidence {
        scale_id: "small".to_string(),
        path: source_path.clone(),
        report_sha256: source_digests[0].clone(),
        preflight_sha256: source_digests[1].clone(),
        markdown_sha256: source_digests[2].clone(),
    });
    let retained = bindings::RetainedRollupArtifacts::bind(&root, &context, &evidence)
        .expect("retain rollup artifacts");
    retained.require_current(&root).expect("artifacts current");

    std::fs::write(root.path.join(source_path).join("report.md"), b"changed\n")
        .expect("mutate retained source");
    assert!(matches!(
        retained.require_current(&root),
        Err(CompletionError::Artifact(
            super::super::artifact::ArtifactError::ConcurrentReplacement("report.md")
        ))
    ));
}

#[test]
fn completion_revalidates_retained_rollup_identity() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
    let (rollup_path, rollup_digests) = write_performance_artifact(&root, "bound-rollup-id");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let context =
        RetainedArtifactContext::open(&root, &live_repository).expect("open artifact context");
    let mut evidence = replay_evidence();
    evidence.output = rollup_path.clone();
    evidence.report_sha256 = rollup_digests[0].clone();
    evidence.preflight_sha256 = rollup_digests[1].clone();
    evidence.markdown_sha256 = rollup_digests[2].clone();
    let retained = bindings::RetainedRollupArtifacts::bind(&root, &context, &evidence)
        .expect("retain rollup artifacts");

    let report = root.path.join(rollup_path).join("report.json");
    let displaced = root.path.join("displaced-rollup-report.json");
    std::fs::rename(&report, &displaced).expect("displace rollup report");
    std::fs::write(&report, b"report\n").expect("write identical rollup report");
    assert!(matches!(
        retained.require_current(&root),
        Err(CompletionError::Artifact(
            super::super::artifact::ArtifactError::ConcurrentReplacement("report.json")
        ))
    ));
}
