use std::num::NonZeroU64;

use clap::Parser;

use super::super::protocol::{InputDigest, ProtocolId};
use super::*;

#[derive(Debug, Parser)]
struct RollupCli {
    #[command(flatten)]
    args: RollupArgs,
}

fn digest(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn contract() -> GroupContract {
    GroupContract {
        id: ProtocolId::try_new("product-group").expect("group id"),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        timing_batch_policy: crate::qualification::model::TimingBatchPolicy::CommonIterations,
        workload_id: ProtocolId::try_new("workload").expect("workload id"),
        measurement_ids: vec![ProtocolId::try_new("parse").expect("measurement id")],
        scales: [("small", 1), ("medium", 2), ("large", 3)]
            .into_iter()
            .map(|(id, work)| ScaleContract {
                id: ProtocolId::try_new(id).expect("scale id"),
                work_items: NonZeroU64::new(work).expect("positive work"),
                input_bytes: work,
                input_digest: InputDigest::try_new(digest(char::from(
                    b'a' + u8::try_from(work).expect("small test work"),
                )))
                .expect("input digest"),
            })
            .collect(),
        correctness_case_ids: vec!["cq-one".to_string()],
        owner: ProtocolId::try_new("owner").expect("owner"),
        profiler_note: None,
        comparator_sources: Vec::new(),
    }
}

fn shared() -> SharedIdentity {
    SharedIdentity {
        group_id: "product-group".to_string(),
        group_contract_sha256: digest('a'),
        claim_class: ClaimClass::PromotablePerformance,
        baseline_eligibility: BaselineEligibility::ThresholdEligible,
        owner: "owner".to_string(),
        profiler_note: None,
        tier: QualificationTier::Full,
        performance_inventory_sha256: digest('b'),
        correctness_inventory_sha256: digest('c'),
        stim_tag: STIM_TAG.to_string(),
        stim_commit: STIM_COMMIT.to_string(),
        stab_commit: "d".repeat(40),
        local_modifications: false,
        host_verified: true,
        host_policy_sha256: digest('e'),
        host_profile_id: "controlled".to_string(),
        operating_system: "linux".to_string(),
        architecture: "aarch64".to_string(),
        cpu_identity: "cpu".to_string(),
        rust_toolchain: "nightly".to_string(),
        target_triple: "aarch64-unknown-linux-gnu".to_string(),
        toolchain_sha256: digest('f'),
        workers: WorkerIdentityEvidence {
            stim_source_sha256: digest('6'),
            stim_build_fingerprint: digest('7'),
            stim_binary_sha256: digest('8'),
            stab_source_sha256: digest('9'),
            stab_build_fingerprint: digest('a'),
            stab_binary_sha256: digest('b'),
            contract_preflight_sha256: digest('c'),
        },
        correctness_preflight: CorrectnessPreflightEvidence {
            status: CorrectnessPreflightStatus::Passed,
            case_ids: vec!["cq-one".to_string()],
            reason: "passed".to_string(),
            source_directory: Some("target/qualification/cq".to_string()),
            qualification_manifest_sha256: Some(digest('1')),
            request_sha256: Some(digest('2')),
            completion_sha256: Some(digest('3')),
            report_sha256: Some(digest('4')),
            preflight_sha256: Some(digest('5')),
        },
    }
}

fn candidate(scale_id: &str, work_items: u64, outcome: GateOutcome) -> Candidate {
    Candidate {
        shared: shared(),
        source: SourceReportBinding {
            path: format!("target/benchmarks/qualification/{scale_id}"),
            report_sha256: digest('6'),
            preflight_sha256: digest('7'),
        },
        generated_unix_epoch_seconds: work_items,
        scale_id: scale_id.to_string(),
        work_items,
        promotable: true,
        measurements: vec![RollupMeasurement {
            measurement_id: "parse".to_string(),
            pair_count: 9,
            median_ratio: 1.3,
            confidence_interval_lower: 1.2,
            confidence_interval_upper: 1.4,
            ratio_relative_mad: 0.01,
            threshold: 1.25,
            outcome,
        }],
        memory: RollupMemory {
            stim_setup_rss_bytes: 1,
            stim_peak_rss_bytes: 2,
            stim_parent_observed_peak_rss_bytes: Some(3),
            stab_setup_rss_bytes: 1,
            stab_peak_rss_bytes: 2,
            stab_parent_observed_peak_rss_bytes: Some(3),
        },
    }
}

fn assemble_candidates(candidates: Vec<Candidate>) -> Result<RollupReport, RollupError> {
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/rollup",
    ))
    .expect("rollup output");
    let contract = contract();
    assemble(
        AssemblyContext {
            contract: &contract,
            group_contract_sha256: &digest('a'),
            expected_performance_inventory_sha256: &digest('b'),
            expected_correctness_inventory_sha256: &digest('c'),
            tier: QualificationTier::Full,
            output_path: &output,
            producer_repository: RepositoryEvidence {
                commit_before: "d".repeat(40),
                commit_after: "d".repeat(40),
                local_modifications_before: false,
                local_modifications_after: false,
            },
        },
        candidates,
    )
}

#[test]
fn loaded_candidate_retains_correctness_tree_binding() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let output = repository.path().join("correctness-source");
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
    let receipt = case.join("execution-receipt.json");
    std::fs::write(&receipt, b"receipt\n").expect("write correctness receipt");
    let binding = super::super::correctness::bind_test_artifact_tree(&output, &["case-a"])
        .expect("bind correctness tree");
    let loaded = vec![LoadedCandidate {
        path: DirectQualificationArtifactPath::try_new(Path::new(
            "target/benchmarks/qualification/source",
        ))
        .expect("source path"),
        report_sha256: digest('1'),
        preflight_sha256: digest('2'),
        markdown_sha256: digest('3'),
        correctness_binding: Arc::new(binding),
        candidate: candidate("small", 1, GateOutcome::Passed),
    }];

    require_current_correctness(&loaded).expect("current correctness tree");
    std::fs::write(&receipt, b"changed\n").expect("mutate correctness receipt");
    assert!(matches!(
        require_current_correctness(&loaded),
        Err(
            super::super::artifact::ArtifactError::ExternalSourceChanged(
                "correctness qualification evidence"
            )
        )
    ));
}

fn assert_reconstruction_rejects_tampering(
    reference: &RollupReport,
    output: &DirectQualificationArtifactPath,
    mutate: impl FnOnce(&mut RollupReport),
) {
    let mut tampered = reference.clone();
    mutate(&mut tampered);
    let tampered_json = render_json(&tampered).expect("tampered JSON");
    let tampered_preflight =
        render_json(&preflight(&tampered, &tampered_json)).expect("tampered preflight");
    parse_existing_rollup(
        &tampered_json,
        &tampered_preflight,
        output,
        &digest('b'),
        &digest('c'),
    )
    .expect("structurally valid tampering reaches reconstruction");
    assert!(matches!(
        require_reconstruction(&tampered_json, reference),
        Err(RollupError::Reconstruction)
    ));
}

#[test]
fn rollup_requires_one_current_report_per_scale_and_preserves_failures() {
    let report = assemble_candidates(vec![
        candidate("large", 3, GateOutcome::Failed),
        candidate("small", 1, GateOutcome::Passed),
        candidate("medium", 2, GateOutcome::Failed),
    ])
    .expect("complete family");
    assert_eq!(
        report
            .scales
            .iter()
            .map(|scale| scale.scale_id.as_str())
            .collect::<Vec<_>>(),
        ["small", "medium", "large"]
    );
    assert_eq!(report.passed_measurements, 1);
    assert_eq!(report.failed_measurements, 2);
    assert_eq!(report.overall_outcome, GateOutcome::Failed);

    assert!(matches!(
        assemble_candidates(vec![
            candidate("small", 1, GateOutcome::Passed),
            candidate("medium", 2, GateOutcome::Passed),
        ]),
        Err(RollupError::InputCount {
            actual: 2,
            expected: 3
        })
    ));

    let mixed = assemble_candidates(vec![
        candidate("small", 1, GateOutcome::Passed),
        candidate("medium", 2, GateOutcome::Noisy),
        candidate("large", 3, GateOutcome::Failed),
    ])
    .expect("mixed nonpassing family");
    assert_eq!(mixed.failed_measurements, 1);
    assert_eq!(mixed.noisy_measurements, 1);
    assert_eq!(mixed.overall_outcome, GateOutcome::Failed);
}

#[test]
fn rollup_rejects_duplicate_nonpromotable_and_mixed_identity_reports() {
    let duplicate = vec![
        candidate("small", 1, GateOutcome::Passed),
        candidate("small", 1, GateOutcome::Passed),
        candidate("large", 3, GateOutcome::Passed),
    ];
    assert!(matches!(
        assemble_candidates(duplicate),
        Err(RollupError::DuplicateScale)
    ));

    let mut nonpromotable = candidate("medium", 2, GateOutcome::Passed);
    nonpromotable.promotable = false;
    assert!(matches!(
        assemble_candidates(vec![
            candidate("small", 1, GateOutcome::Passed),
            nonpromotable,
            candidate("large", 3, GateOutcome::Passed),
        ]),
        Err(RollupError::NonPromotable(scale)) if scale == "medium"
    ));

    let mut mixed = candidate("medium", 2, GateOutcome::Passed);
    mixed.shared.architecture = "x86_64".to_string();
    assert!(matches!(
        assemble_candidates(vec![
            candidate("small", 1, GateOutcome::Passed),
            mixed,
            candidate("large", 3, GateOutcome::Passed),
        ]),
        Err(RollupError::MixedIdentity(scale)) if scale == "medium"
    ));

    let mut mixed_worker = candidate("medium", 2, GateOutcome::Passed);
    mixed_worker.shared.workers.stab_binary_sha256 = digest('c');
    assert!(matches!(
        assemble_candidates(vec![
            candidate("small", 1, GateOutcome::Passed),
            mixed_worker,
            candidate("large", 3, GateOutcome::Passed),
        ]),
        Err(RollupError::MixedIdentity(scale)) if scale == "medium"
    ));

    let mut stale_preflight = candidate("small", 1, GateOutcome::Passed);
    stale_preflight.shared.workers.contract_preflight_sha256 = digest('d');
    assert!(matches!(
        assemble_candidates(vec![
            stale_preflight,
            candidate("medium", 2, GateOutcome::Passed),
            candidate("large", 3, GateOutcome::Passed),
        ]),
        Err(RollupError::MixedIdentity(scale)) if scale == "medium"
    ));
}

#[test]
fn rollup_rejects_stale_scale_work_and_pr_evidence() {
    assert!(matches!(
        assemble_candidates(vec![
            candidate("small", 1, GateOutcome::Passed),
            candidate("medium", 9, GateOutcome::Passed),
            candidate("large", 3, GateOutcome::Passed),
        ]),
        Err(RollupError::ScaleContract(scale)) if scale == "medium"
    ));
    assert!(matches!(
        require_promotable_tier(QualificationTier::Pr),
        Err(RollupError::NonPromotableTier)
    ));
}

#[test]
fn rollup_producer_must_be_clean_unchanged_and_match_source_commit() {
    let state = |commit: char, dirty| super::super::git::RepositoryState {
        commit: commit.to_string().repeat(40),
        local_modifications: dirty,
    };
    assert!(matches!(
        bind_producer_repository(state('d', true), state('d', false), &"d".repeat(40)),
        Err(RollupError::DirtyProducer)
    ));
    assert!(matches!(
        bind_producer_repository(state('d', false), state('e', false), &"d".repeat(40)),
        Err(RollupError::RepositoryChanged { .. })
    ));
    assert!(matches!(
        bind_producer_repository(state('d', false), state('d', false), &"e".repeat(40)),
        Err(RollupError::ProducerCommit { .. })
    ));
    let evidence = bind_producer_repository(state('d', false), state('d', false), &"d".repeat(40))
        .expect("clean source-matching producer");
    assert_eq!(evidence.commit_before, "d".repeat(40));
    assert!(!evidence.local_modifications_after);
    require_current_producer_state(&state('d', false), &evidence)
        .expect("unchanged checkout at publication");
    assert!(matches!(
        require_current_producer_state(&state('d', true), &evidence),
        Err(RollupError::DirtyProducer)
    ));
    assert!(matches!(
        require_current_producer_state(&state('e', false), &evidence),
        Err(RollupError::RepositoryChanged { .. })
    ));
}

#[test]
fn offline_rollup_replay_rejects_noncanonical_preflight_and_tampered_fields() {
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/rollup",
    ))
    .expect("rollup output");
    let report = assemble_candidates(vec![
        candidate("small", 1, GateOutcome::Passed),
        candidate("medium", 2, GateOutcome::Passed),
        candidate("large", 3, GateOutcome::Failed),
    ])
    .expect("complete report");
    let report_json = render_json(&report).expect("canonical report");
    let preflight_json = render_json(&preflight(&report, &report_json)).expect("preflight");
    parse_existing_rollup(
        &report_json,
        &preflight_json,
        &output,
        &digest('b'),
        &digest('c'),
    )
    .expect("valid existing rollup");

    let mut noncanonical = report_json.clone();
    noncanonical.extend_from_slice(b" \n");
    assert!(matches!(
        parse_existing_rollup(
            &noncanonical,
            &preflight_json,
            &output,
            &digest('b'),
            &digest('c'),
        ),
        Err(RollupError::NonCanonicalReport)
    ));
    let mut stale_preflight = preflight_json.clone();
    *stale_preflight.first_mut().expect("nonempty preflight") = b'[';
    assert!(matches!(
        parse_existing_rollup(
            &report_json,
            &stale_preflight,
            &output,
            &digest('b'),
            &digest('c'),
        ),
        Err(RollupError::PreflightBinding)
    ));

    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        tampered.failed_measurements = 0;
    });
    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        tampered.producer_repository.commit_after = "e".repeat(40);
    });
    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        tampered.scales.first_mut().expect("scale").source.path =
            "target/benchmarks/qualification/alternate-small".to_string();
    });
    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        tampered
            .scales
            .first_mut()
            .expect("scale")
            .source
            .report_sha256 = digest('8');
    });
    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        tampered
            .scales
            .first_mut()
            .expect("scale")
            .measurements
            .first_mut()
            .expect("measurement")
            .outcome = GateOutcome::Failed;
    });
    assert_reconstruction_rejects_tampering(&report, &output, |tampered| {
        let memory = &mut tampered.scales.first_mut().expect("scale").memory;
        memory.stab_peak_rss_bytes = memory.stab_peak_rss_bytes.saturating_add(1);
    });

    let mut wrong_output = report.clone();
    wrong_output.output = "target/benchmarks/qualification/other-rollup".to_string();
    let wrong_output_json = render_json(&wrong_output).expect("wrong-output JSON");
    let wrong_output_preflight =
        render_json(&preflight(&wrong_output, &wrong_output_json)).expect("wrong-output preflight");
    assert!(matches!(
        parse_existing_rollup(
            &wrong_output_json,
            &wrong_output_preflight,
            &output,
            &digest('b'),
            &digest('c'),
        ),
        Err(RollupError::OutputBinding)
    ));
}

#[test]
fn rollup_paths_are_direct_siblings_only() {
    assert_eq!(
        DirectQualificationArtifactPath::try_new(Path::new(
            "target/benchmarks/qualification/full-small"
        ))
        .expect("direct artifact"),
        DirectQualificationArtifactPath::try_new(Path::new(
            "target/benchmarks/qualification/full-small"
        ))
        .expect("same direct artifact")
    );
    for invalid in [
        "target/benchmarks/qualification",
        "target/benchmarks/qualification/nested/full-small",
        "target/benchmarks/qualification/../outside",
        "/tmp/full-small",
    ] {
        assert!(DirectQualificationArtifactPath::try_new(Path::new(invalid)).is_err());
    }
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/rollup",
    ))
    .expect("output");
    for injected in [
        "target/benchmarks/qualification/bad|row",
        "target/benchmarks/qualification/bad`row",
        "target/benchmarks/qualification/bad\nrow",
    ] {
        assert!(collect_input_paths([Path::new(injected)], &output).is_err());
    }
}

#[test]
fn rollup_cli_accepts_only_promotable_tiers() {
    let full = RollupCli::try_parse_from([
        "qualification-rollup",
        "--group",
        "product-group",
        "--tier",
        "full",
        "--input",
        "target/benchmarks/qualification/small",
    ])
    .expect("full rollup arguments");
    assert!(matches!(full.args.tier, RollupTier::Full));
    assert!(
        RollupCli::try_parse_from([
            "qualification-rollup",
            "--group",
            "product-group",
            "--tier",
            "pr",
            "--input",
            "target/benchmarks/qualification/small",
        ])
        .is_err()
    );
}
