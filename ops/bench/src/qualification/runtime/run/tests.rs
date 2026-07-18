use super::*;
use clap::Parser;

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
