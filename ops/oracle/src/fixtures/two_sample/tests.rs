use super::{compare_two_sample_plan, validate_two_sample_plan};
use crate::fixtures::{FixtureError, FixtureManifest, RunFilter, compare_fixture};

/// The shot count and per-row false-positive budget used by the surface-code manifest rows.
const SHOTS: usize = 20_000;
const PLAN: &str = "sample_count=20000; fixed_seed=5; columns=4; two-sample columns within 6 sigma; joint parity=0,1; false_positive_rate<=0.000001";

/// Deterministic xorshift64* stream so the "same distribution" sample is reproducible.
struct Xorshift(u64);

impl Xorshift {
    fn next_unit(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Draws `SHOTS` records whose columns are 1 independently with the given probabilities.
fn independent_records(seed: u64, probabilities: &[f64]) -> Vec<u8> {
    let mut rng = Xorshift(seed);
    let mut bytes = Vec::with_capacity(SHOTS * (probabilities.len() + 1));
    for _ in 0..SHOTS {
        for &probability in probabilities {
            bytes.push(if rng.next_unit() < probability {
                b'1'
            } else {
                b'0'
            });
        }
        bytes.push(b'\n');
    }
    bytes
}

/// Builds records where column `column` is 1 on exactly the first `hits` shots and every other
/// column follows `base`, so shifts are exact rather than sampled.
fn records_with_column_hits(base: &[u8], column: usize, hits: usize) -> Vec<u8> {
    let mut bytes = base.to_vec();
    for (shot, record) in bytes.chunks_mut(5).enumerate() {
        if let Some(bit) = record.get_mut(column) {
            *bit = if shot < hits { b'1' } else { b'0' };
        }
    }
    bytes
}

#[test]
fn accepts_independent_draws_from_one_distribution() {
    // Interior, rare, and both zero-variance edges (constant 0 and constant 1) in one plan.
    let probabilities = [0.5, 0.1, 0.0, 1.0];
    let stim = independent_records(0x5eed_0001, &probabilities);
    let stab = independent_records(0x5eed_0002, &probabilities);

    assert_eq!(compare_two_sample_plan(PLAN, &stim, &stab), None);
}

#[test]
fn rejects_ten_percent_relative_shift_in_one_column_at_fixture_shot_count() {
    let base = independent_records(0x5eed_0003, &[0.5, 0.1, 0.0, 1.0]);
    // 0.50 versus 0.55 at 20000 shots per side is z ~= 10; a 5% shift (z ~= 5) would not be
    // claimed at 6 sigma, which is the documented power floor for p = 0.5 columns.
    let stim = records_with_column_hits(&base, 0, 10_000);
    let stab = records_with_column_hits(&base, 0, 11_000);

    let reason = compare_two_sample_plan(PLAN, &stim, &stab).expect("shift must be rejected");

    assert!(reason.contains("column 0: Stim 10000/20000"), "{reason}");
    assert!(!reason.contains("column 1:"), "{reason}");
}

#[test]
fn joint_parity_rejects_correlation_change_with_identical_marginals() {
    // Stim draws columns 0 and 1 as one fair coin; Stab draws them independently. Every marginal
    // is 1/2 on both sides, so only the declared parity statistic can see the difference.
    let mut rng = Xorshift(0x5eed_0004);
    let mut stim = Vec::new();
    for _ in 0..SHOTS {
        let bit = if rng.next_unit() < 0.5 { b'1' } else { b'0' };
        stim.extend_from_slice(&[bit, bit, b'0', b'1', b'\n']);
    }
    let stab = independent_records(0x5eed_0005, &[0.5, 0.5, 0.0, 1.0]);

    let reason = compare_two_sample_plan(PLAN, &stim, &stab).expect("correlation change");

    assert!(reason.contains("joint parity=0,1"), "{reason}");
    assert!(!reason.contains("column "), "{reason}");
}

#[test]
fn rejects_deterministic_column_disagreement() {
    let probabilities = [0.5, 0.1, 0.0, 1.0];
    let stim = independent_records(0x5eed_0006, &probabilities);
    let stab = records_with_column_hits(&stim, 2, SHOTS);

    let reason = compare_two_sample_plan(PLAN, &stim, &stab).expect("deterministic mismatch");

    assert!(
        reason.contains("column 2: Stim 0/20000 (0.00000) vs Stab 20000/20000 (1.00000), deterministic disagreement"),
        "{reason}"
    );
}

#[test]
fn rejects_record_width_and_shot_count_mismatches() {
    let stim = independent_records(0x5eed_0007, &[0.5, 0.1, 0.0, 1.0]);
    let wide = independent_records(0x5eed_0008, &[0.5, 0.1, 0.0, 1.0, 0.5]);
    let mut short = stim.clone();
    short.truncate(short.len() - 5);

    let width = compare_two_sample_plan(PLAN, &stim, &wide).expect("width mismatch");
    let count = compare_two_sample_plan(PLAN, &short, &stim).expect("shot count mismatch");
    let mut unterminated = stim.clone();
    unterminated.pop();
    let unterminated =
        compare_two_sample_plan(PLAN, &stim, &unterminated).expect("missing newline");

    assert!(
        width.contains("Stab shot 0 has 5 column(s), plan declares columns=4"),
        "{width}"
    );
    assert!(
        count.contains("Stim stdout has 19999 shot(s), plan declares sample_count=20000"),
        "{count}"
    );
    assert!(
        unterminated.contains("does not end with a newline"),
        "{unterminated}"
    );
}

#[test]
fn validation_rejects_budget_overclaims_and_bad_joint_sets() {
    let argv = argv(&[
        "sample",
        "--shots",
        "20000",
        "--seed",
        "5",
        "--out_format",
        "01",
    ]);
    // 1000 columns at 5 sigma estimates ~1.2e-3 false positives, far above 1e-6.
    let overclaim = "sample_count=20000; fixed_seed=5; columns=1000; two-sample columns within 5 sigma; false_positive_rate<=0.000001";
    // Each joint statistic is one more Bonferroni term: 400 columns fit 1e-6 at 6 sigma
    // (~8.1e-7), but 400 columns plus 200 joint statistics do not (~1.2e-6).
    let joints = (0..200)
        .map(|column| format!("joint any={column}"))
        .collect::<Vec<_>>()
        .join("; ");
    let joint_overclaim = format!(
        "sample_count=20000; fixed_seed=5; columns=400; two-sample columns within 6 sigma; {joints}; false_positive_rate<=0.000001"
    );
    let out_of_range = PLAN.replace("joint parity=0,1", "joint any=0..4");

    assert!(
        validate_two_sample_plan(overclaim, &argv)
            .expect("overclaim")
            .contains("over 1000 test(s)")
    );
    let without_joints = joint_overclaim.replace(&format!("{joints}; "), "");
    assert_eq!(validate_two_sample_plan(&without_joints, &argv), None);
    assert!(
        validate_two_sample_plan(&joint_overclaim, &argv)
            .expect("joint overclaim")
            .contains("over 600 test(s)")
    );
    assert!(
        validate_two_sample_plan(&out_of_range, &argv)
            .expect("joint range")
            .contains("outside the declared 4 column(s)")
    );
    assert_eq!(validate_two_sample_plan(PLAN, &argv), None);
}

#[test]
fn validation_rejects_argv_that_disagrees_with_plan() {
    let cases = [
        (
            argv(&["sample", "--shots", "1000", "--seed", "5"]),
            "sample_count=20000 does not match --shots 1000",
        ),
        (
            argv(&["sample", "--shots", "20000", "--seed", "6"]),
            "fixed_seed=5 does not match --seed 6",
        ),
        (
            argv(&[
                "sample",
                "--shots",
                "20000",
                "--seed",
                "5",
                "--out_format",
                "b8",
            ]),
            "argv requests --out_format b8",
        ),
    ];
    for (argv, expected) in cases {
        let reason = validate_two_sample_plan(PLAN, &argv).expect("argv mismatch must reject");
        assert!(reason.contains(expected), "{reason}");
    }
}

#[test]
fn manifest_two_sample_rows_run_under_statistical_filter_and_report_mismatches() {
    let csv = format!(
        "id,upstream_source,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,statistical_plan,source_license_note\n\
         row,src/stim/cmd/command_sample.test.cc,two-sample,stim sample,sample|--shots|20000|--seed|5,inputs/sample_noisy.stim,,0,empty,\"{PLAN}\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse two-sample manifest");
    let row = manifest.rows.first().expect("two-sample row");
    let stim = independent_records(0x5eed_0009, &[0.5, 0.1, 0.0, 1.0]);
    let stab = records_with_column_hits(&stim, 3, 0);

    assert!(RunFilter::Statistical.matches(row));
    assert!(!RunFilter::Exact.matches(row));
    let error = compare_fixture(row, &process_output(&stim), &process_output(&stab))
        .expect_err("deterministic disagreement must fail the row");
    assert!(
        matches!(
            &error,
            FixtureError::ComparatorMismatch {
                comparator: "two-sample",
                ..
            }
        ),
        "{error}"
    );
    compare_fixture(row, &process_output(&stim), &process_output(&stim))
        .expect("identical samples must pass");
}

fn argv(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_string()).collect()
}

fn process_output(stdout: &[u8]) -> crate::ProcessOutput {
    crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: stdout.to_vec(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    }
}
