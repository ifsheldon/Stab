use super::*;
use clap::Parser;
use std::path::Path;

#[derive(Debug, Parser)]
struct RunCli {
    #[command(flatten)]
    args: RunArgs,
}

#[test]
fn tiers_have_source_owned_pair_counts() {
    assert_eq!(QualificationTier::Pr.pair_count(), 3);
    assert_eq!(QualificationTier::Full.pair_count(), 9);
    assert_eq!(QualificationTier::Soak.pair_count(), 15);
}

#[test]
fn diagnostic_claim_cannot_be_promotable() {
    assert_ne!(
        ClaimClass::DiagnosticInfrastructure,
        ClaimClass::PromotablePerformance
    );
}

#[test]
fn calibration_guard_band_and_wide_ratio_mode_preserve_source_bounds() {
    let policy = calibration_policy().expect("calibration policy");
    assert_eq!(policy.minimum, Duration::from_millis(350));
    assert_eq!(CALIBRATION_ACCEPTANCE_MINIMUM, Duration::from_millis(250));
    assert_eq!(CALIBRATION_MAXIMUM, Duration::from_secs(2));
    assert_eq!(CALIBRATION_WIDE_RATIO_MAXIMUM, Duration::from_secs(20));
    assert!(policy.minimum > CALIBRATION_ACCEPTANCE_MINIMUM);

    assert_eq!(
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            100,
            Duration::from_millis(250),
            Duration::from_secs(2),
        )
        .expect("standard endpoints are accepted"),
        CommonBatchMode::Standard
    );
    assert_eq!(
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            1_000,
            Duration::from_secs(20),
            Duration::from_millis(350),
        )
        .expect("Stim may exceed the standard cap at Stab's selected batch"),
        CommonBatchMode::WideRatio
    );
    assert_eq!(
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            1_000,
            100,
            Duration::from_millis(350),
            Duration::from_secs(5),
        )
        .expect("Stab may exceed the standard cap at Stim's selected batch"),
        CommonBatchMode::WideRatio
    );
}

#[test]
fn wide_ratio_mode_rejects_floor_cap_and_iteration_owner_violations() {
    for result in [
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            1_000,
            Duration::from_millis(249),
            Duration::from_millis(350),
        ),
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            1_000,
            Duration::from_millis(20_001),
            Duration::from_millis(350),
        ),
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            1_000,
            Duration::from_millis(350),
            Duration::from_millis(2_001),
        ),
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            1_000,
            1_000,
            Duration::from_secs(3),
            Duration::from_millis(350),
        ),
        classify_common_calibration(
            TimingBatchPolicy::CommonIterations,
            100,
            1_000,
            Duration::from_secs(3),
            Duration::from_secs(3),
        ),
    ] {
        assert!(matches!(
            result,
            Err(RunError::CommonCalibrationOutOfBounds { .. })
        ));
    }
}

#[test]
fn independent_throughput_accepts_sub_floor_common_semantic_batch_only() {
    assert_eq!(
        classify_common_calibration(
            TimingBatchPolicy::IndependentThroughput,
            400_000,
            28_000_000,
            Duration::from_millis(350),
            Duration::from_millis(5),
        )
        .expect("independent semantic batch may leave the faster side below the timing floor"),
        CommonBatchMode::IndependentThroughput
    );
    for result in [
        classify_common_calibration(
            TimingBatchPolicy::IndependentThroughput,
            400_000,
            28_000_000,
            Duration::ZERO,
            Duration::from_millis(5),
        ),
        classify_common_calibration(
            TimingBatchPolicy::IndependentThroughput,
            400_000,
            28_000_000,
            Duration::from_millis(350),
            Duration::from_millis(2_001),
        ),
    ] {
        assert!(matches!(
            result,
            Err(RunError::CommonCalibrationOutOfBounds { .. })
        ));
    }
}

#[test]
fn selected_common_owner_must_repeat_the_exact_semantic_digest() {
    let common = SemanticDigest::try_new("a".repeat(64)).expect("common digest");
    let different = SemanticDigest::try_new("b".repeat(64)).expect("different digest");
    for (stim_iterations, stab_iterations) in [
        (400_000, 28_000_000),
        (28_000_000, 400_000),
        (400_000, 400_000),
    ] {
        let common_iterations = stim_iterations.min(stab_iterations);
        assert!(selected_output_matches_common(
            common_iterations,
            stim_iterations,
            &common,
            &common,
        ));
        assert!(selected_output_matches_common(
            common_iterations,
            stab_iterations,
            &common,
            &common,
        ));
        assert_eq!(
            selected_output_matches_common(common_iterations, stim_iterations, &common, &different,),
            stim_iterations != common_iterations,
        );
        assert_eq!(
            selected_output_matches_common(common_iterations, stab_iterations, &common, &different,),
            stab_iterations != common_iterations,
        );
    }
}

#[test]
fn run_cli_selects_source_owned_group_and_scale_without_free_work() {
    let defaults = RunCli::try_parse_from(["qualification-run", "--tier", "pr"])
        .expect("default diagnostic run");
    assert_eq!(defaults.args.group, super::super::invocation::PQ1_GROUP_ID);
    assert_eq!(defaults.args.scale, "default");

    let product = RunCli::try_parse_from([
        "qualification-run",
        "--tier",
        "full",
        "--group",
        super::super::invocation::CIRCUIT_PARSE_GROUP_ID,
        "--scale",
        "large",
    ])
    .expect("product group and scale");
    assert_eq!(product.args.scale, "large");

    assert!(
        RunCli::try_parse_from(["qualification-run", "--tier", "pr", "--work-items", "1",])
            .is_err()
    );
}

#[test]
fn output_admission_rejects_nested_and_injection_names_without_mutation() {
    let repository = tempfile::tempdir().expect("temporary repository");
    let root = RepoRoot::resolve(repository.path()).expect("repository root");
    let existing = repository
        .path()
        .join("target/benchmarks/qualification/existing");
    std::fs::create_dir_all(&existing).expect("create existing evidence");
    std::fs::write(existing.join("report.json"), b"retained\n").expect("write evidence");

    for path in [
        Path::new("target/benchmarks/qualification/existing/nested"),
        Path::new("target/benchmarks/qualification/bad|name"),
        Path::new("target/benchmarks/qualification/.publication.lock"),
    ] {
        let result = DirectQualificationArtifactPath::try_new(path).and_then(|output| {
            let repository = RepositoryBinding::open(&root)?;
            QualificationOutput::require_absent_with_repository(&root, &repository, &output)
        });
        assert!(result.is_err(), "path={path:?}");
    }

    assert_eq!(
        std::fs::read(existing.join("report.json")).expect("read retained evidence"),
        b"retained\n"
    );
    assert_eq!(
        std::fs::read_dir(&existing)
            .expect("read existing evidence")
            .count(),
        1
    );
    assert!(!existing.join(".publication.lock").exists());
}

#[test]
fn retained_repository_binding_rejects_root_swap_after_output_admission() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let repository = parent.path().join("repository");
    std::fs::create_dir(&repository).expect("create repository");
    let root = RepoRoot::resolve(&repository).expect("repository root");
    let live_repository = RepositoryBinding::open(&root).expect("bind repository");
    let output = DirectQualificationArtifactPath::try_new(Path::new(
        "target/benchmarks/qualification/root-swap",
    ))
    .expect("direct output path");
    QualificationOutput::require_absent_with_repository(&root, &live_repository, &output)
        .expect("admit output");

    let detached = parent.path().join("detached-repository");
    std::fs::rename(&repository, &detached).expect("detach repository");
    std::fs::create_dir(&repository).expect("replace repository");

    assert!(matches!(
        bound_repository_state(&root, &live_repository),
        Err(super::super::artifact::ArtifactError::RepositoryIdentity)
    ));
    assert!(matches!(
        QualificationOutput::begin_new_with_repository(&root, &live_repository, &output),
        Err(super::super::artifact::ArtifactError::RepositoryIdentity)
    ));
    assert!(!repository.join(output.as_path()).exists());
}

#[test]
fn publication_repository_binding_rejects_commit_and_dirty_state_drift() {
    let expected = RepositoryEvidence {
        commit_before: "a".repeat(40),
        commit_after: "a".repeat(40),
        local_modifications_before: false,
        local_modifications_after: false,
    };
    let current = super::super::git::RepositoryState {
        commit: "a".repeat(40),
        local_modifications: false,
    };
    require_current_repository_state(&current, &expected).expect("matching repository state");

    let mut changed_commit = current.clone();
    changed_commit.commit = "b".repeat(40);
    assert!(require_current_repository_state(&changed_commit, &expected).is_err());

    let mut dirty = current;
    dirty.local_modifications = true;
    assert!(require_current_repository_state(&dirty, &expected).is_err());
}
