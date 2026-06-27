use super::{
    ExpectedStdoutPolicy, FixtureComparator, FixtureManifest, FixturePathRequirement, Milestone,
    RunFilter, RunMode, compare_fixture, is_recordable,
    outputs::{self, FixtureArgToken},
    run_core_fixture, run_direct_rust_fixture, statistical, validate_fixture_path,
};

const MANIFEST_CSV: &str = include_str!("../../../../oracle/fixtures/manifest.csv");
const HEADER: &str = "id,milestone,upstream_source,parity_mode,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,status,statistical_plan,source_license_note\n";

fn process_output(status: Option<i32>, stdout: &[u8], stderr: &[u8]) -> crate::ProcessOutput {
    crate::ProcessOutput {
        status,
        stdout: crate::CapturedOutput {
            bytes: stdout.to_vec(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: stderr.to_vec(),
            truncated: false,
        },
    }
}

fn observable_only_distribution_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(std::iter::repeat_n(b"000\n", 750).flatten());
    bytes.extend(std::iter::repeat_n(b"001\n", 250).flatten());
    bytes
}

#[test]
fn repository_fixture_manifest_passes_validation() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    manifest.check(&root).expect("manifest validation");
}

#[test]
fn fixture_manifest_has_implemented_smoke_cases() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let implemented = manifest
        .rows
        .iter()
        .filter(|row| row.status.as_str() == "implemented")
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();

    for id in [
        "smoke-help",
        "smoke-tiny-circuit",
        "m4-parser-basic",
        "m4-convert-canonical-print",
        "coverage-circuit-circuit-instruction",
        "coverage-circuit-gate-decomposition",
        "coverage-circuit-gate-target",
        "coverage-gates-gates",
        "coverage-util-bot-probability-util",
    ] {
        assert!(implemented.contains(&id), "{id}");
    }
}

#[test]
fn milestone_run_mode_parses_m4_filter() {
    let mode = RunMode::Milestone("M4".to_string());

    assert_eq!(mode.milestone_filter().unwrap(), Some(Milestone::M4));
}

#[test]
fn run_filter_flags_select_one_comparator_family() {
    assert_eq!(RunFilter::from_flags(false, false, false).unwrap(), None);
    assert_eq!(
        RunFilter::from_flags(true, false, false).unwrap(),
        Some(RunFilter::Exact)
    );
    assert_eq!(
        RunFilter::from_flags(false, true, false).unwrap(),
        Some(RunFilter::Statistical)
    );
    assert_eq!(
        RunFilter::from_flags(false, false, true).unwrap(),
        Some(RunFilter::Structural)
    );
    assert!(
        RunFilter::from_flags(true, true, false)
            .expect_err("conflicting filters should fail")
            .contains("choose at most one")
    );
    assert!(
        RunFilter::from_flags(false, true, true)
            .expect_err("conflicting filters should fail")
            .contains("choose at most one")
    );
}

#[test]
fn run_filter_matches_fixture_rows() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "m8-sample-basic")
        .expect("M8 exact row");
    let statistical_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "m8-sample-noisy-statistical")
        .expect("M8 statistical row");
    let structural_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "coverage-io-measure-record-writer")
        .expect("M8 structural exact-parity row");
    let structural_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "coverage-simulators-frame-simulator-util")
        .expect("M9 structural row");

    assert!(RunFilter::Exact.matches(exact_row));
    assert!(RunFilter::Exact.matches(structural_exact_row));
    assert!(!RunFilter::Exact.matches(statistical_row));
    assert!(RunFilter::Statistical.matches(statistical_row));
    assert!(!RunFilter::Statistical.matches(exact_row));
    assert!(RunFilter::Structural.matches(structural_row));
    assert!(RunFilter::Structural.matches(structural_exact_row));
    assert!(!RunFilter::Structural.matches(exact_row));
}

#[test]
fn m4_core_parse_print_rows_run_in_process() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    for id in ["m4-parser-basic", "m4-convert-canonical-print"] {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == id)
            .expect("M4 core row");
        let output = run_core_fixture(&root, row).expect("core fixture");

        assert_eq!(output.status, Some(0), "{id}");
    }
}

#[test]
fn m4_direct_rust_rows_run_cargo_tests() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    for id in [
        "coverage-circuit-circuit-instruction",
        "coverage-circuit-gate-decomposition",
        "coverage-circuit-gate-target",
        "coverage-gates-gates",
        "coverage-util-bot-probability-util",
    ] {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == id)
            .expect("M4 direct Rust row");
        let output = run_direct_rust_fixture(&root, row).expect("direct Rust fixture");

        assert_eq!(output.status, Some(0), "{id}");
    }
}

#[test]
fn core_exact_output_rows_are_not_recorded_from_stim_cli() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let core_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "m4-convert-canonical-print")
        .expect("M4 exact core row");
    let cli_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id == "smoke-tiny-circuit")
        .expect("M0 CLI exact row");

    assert!(!is_recordable(core_exact_row));
    assert!(is_recordable(cli_exact_row));
}

#[test]
fn exact_output_rows_have_expected_stdout_paths() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");

    for row in manifest
        .rows
        .iter()
        .filter(|row| row.comparator == FixtureComparator::ExactOutput)
        .filter(|row| row.status != super::FixtureStatus::ManifestOnly)
    {
        assert!(!row.expected_stdout_path.is_empty(), "{}", row.id);
    }
}

#[test]
fn repository_exact_output_files_exist() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    for row in manifest
        .rows
        .iter()
        .filter(|row| row.comparator == FixtureComparator::ExactOutput)
        .filter(|row| !row.expected_stdout_path.is_empty())
    {
        assert!(
            row.expected_stdout_file(&root).unwrap().is_file(),
            "{}",
            row.id
        );
    }
}

#[test]
fn repository_fixture_placeholder_files_exist() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    for row in &manifest.rows {
        for token in row.argv_tokens() {
            match outputs::parse_fixture_arg_token(&row.id, &token)
                .expect("parse fixture placeholder token")
            {
                Some(FixtureArgToken::Input(relative)) => {
                    let path = super::fixture_file(&root, relative).unwrap();
                    assert!(path.is_file(), "{} {}", row.id, relative);
                }
                Some(FixtureArgToken::Output(_relative))
                    if row.comparator == FixtureComparator::Statistical
                        && statistical::source_for_plan(&row.statistical_plan).unwrap()
                            == statistical::StatisticalSource::FixtureOutput => {}
                Some(FixtureArgToken::Output(relative)) => {
                    let path = super::fixture_file(&root, relative).unwrap();
                    assert!(path.is_file(), "{} {}", row.id, relative);
                }
                None => {}
            };
        }
    }
}

#[test]
fn fixture_output_placeholders_are_replaced_with_target_paths() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id == "m11-sample-dem-observable-output-exact")
        .expect("M11 fixture output row");

    let command = outputs::prepare_command(&root, row, "test").expect("prepare command");
    let output = command.outputs.first().expect("one fixture output");

    assert_eq!(command.outputs.len(), 1);
    assert_eq!(
        output.expected_relative,
        "expected/m11_sample_dem_observable_obs.stdout"
    );
    assert!(
        output.actual_path.starts_with(
            root.path
                .join("target")
                .join("oracle")
                .join("fixture-outputs")
        )
    );
    assert!(
        command
            .argv
            .iter()
            .any(|token| token.to_string_lossy().contains("fixture-outputs"))
    );
}

#[test]
fn fixture_input_placeholders_are_replaced_with_fixture_paths() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id == "m11-sample-dem-replay-side-outputs-exact")
        .expect("M11 fixture input row");

    let command = outputs::prepare_command(&root, row, "test").expect("prepare command");
    let expected_path = root
        .path
        .join("oracle")
        .join("fixtures")
        .join("inputs")
        .join("sample_dem_replay_errors.01");

    assert!(
        command
            .argv
            .iter()
            .any(|token| token == expected_path.as_os_str())
    );
}

#[test]
fn validation_rejects_cargo_test_row_without_cargo_arguments() {
    let csv = format!(
        "{HEADER}bad,M4,src/stim/circuit/gate_target.test.cc,structural,structural,cargo test,cargo-test,,,0,any,implemented,Run direct Rust gate target parity tests,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest.check(&root).expect_err("missing cargo args");

    assert!(
        error
            .to_string()
            .contains("bad cargo-test row has no cargo arguments")
    );
}

#[test]
fn validation_rejects_statistical_row_without_plan() {
    let csv = format!(
        "{HEADER}bad,M8,src/stim/cmd/command_sample.test.cc,statistical,statistical,stim sample,sample|--shots|10,inputs/sample_noisy.stim,,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest.check(&root).expect_err("missing plan should fail");

    assert!(
        error
            .to_string()
            .contains("comparator needs structural or statistical plan text")
    );
}

#[test]
fn statistical_plan_source_defaults_to_stdout() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert_eq!(
        statistical::source_for_plan(plan).unwrap(),
        statistical::StatisticalSource::Stdout
    );
}

#[test]
fn statistical_plan_source_accepts_fixture_output() {
    let plan = "sample_count=1000; fixed_seed=5; source=fixture_output; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert_eq!(
        statistical::source_for_plan(plan).unwrap(),
        statistical::StatisticalSource::FixtureOutput
    );
}

#[test]
fn statistical_plan_source_rejects_unknown_values() {
    let plan = "sample_count=1000; fixed_seed=5; source=sidecar; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::source_for_plan(plan)
            .expect_err("unknown source should fail")
            .contains("unknown statistical source")
    );
}

#[test]
fn statistical_fixture_output_placeholders_do_not_need_committed_expected_files() {
    let csv = format!(
        "{HEADER}good,M11,src/stim/simulators/dem_sampler.test.cc,statistical,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,implemented,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let fixture_root = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(fixture_root.path().join("expected")).expect("create expected dir");
    let row = manifest.rows.first().expect("one row");
    let mut violations = Vec::new();

    outputs::validate_row_tokens(
        row,
        fixture_root.path(),
        ExpectedStdoutPolicy::RequireExisting,
        &mut violations,
    );

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn statistical_fixture_output_source_requires_one_output_placeholder() {
    let csv = format!(
        "{HEADER}bad,M11,src/stim/simulators/dem_sampler.test.cc,statistical,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5,inputs/sample_dem_observable_only_noisy.dem,,0,empty,implemented,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let fixture_root = tempfile::tempdir().expect("tempdir");
    let row = manifest.rows.first().expect("one row");
    let mut violations = Vec::new();

    outputs::validate_row_tokens(
        row,
        fixture_root.path(),
        ExpectedStdoutPolicy::RequireExisting,
        &mut violations,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("requires exactly one fixture output placeholder"))
    );
}

#[test]
fn statistical_fixture_output_comparator_checks_stab_side_output() {
    let csv = format!(
        "{HEADER}good,M11,src/stim/simulators/dem_sampler.test.cc,statistical,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,implemented,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one row");
    let temp = tempfile::tempdir().expect("tempdir");
    let stim_path = temp.path().join("stim-side-output.stdout");
    let stab_path = temp.path().join("stab-side-output.stdout");
    std::fs::write(&stim_path, observable_only_distribution_bytes())
        .expect("write stim side output");
    std::fs::write(&stab_path, observable_only_distribution_bytes())
        .expect("write stab side output");
    let stim_output = outputs::FixtureOutput {
        expected_relative: "expected/missing-statistical.stdout".to_string(),
        actual_path: stim_path,
    };
    let stab_output = outputs::FixtureOutput {
        expected_relative: "expected/missing-statistical.stdout".to_string(),
        actual_path: stab_path,
    };

    outputs::compare_outputs(row, &[stim_output], &[stab_output]).expect("statistical side output");
}

#[test]
fn statistical_fixture_output_comparator_checks_stim_side_output() {
    let csv = format!(
        "{HEADER}good,M11,src/stim/simulators/dem_sampler.test.cc,statistical,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,implemented,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one row");
    let temp = tempfile::tempdir().expect("tempdir");
    let stim_path = temp.path().join("stim-side-output.stdout");
    let stab_path = temp.path().join("stab-side-output.stdout");
    std::fs::write(&stim_path, b"created by Stim\n").expect("write stim side output");
    std::fs::write(&stab_path, observable_only_distribution_bytes())
        .expect("write stab side output");
    let stim_output = outputs::FixtureOutput {
        expected_relative: "expected/missing-statistical.stdout".to_string(),
        actual_path: stim_path,
    };
    let stab_output = outputs::FixtureOutput {
        expected_relative: "expected/missing-statistical.stdout".to_string(),
        actual_path: stab_path,
    };
    let error = outputs::compare_outputs(row, &[stim_output], &[stab_output])
        .expect_err("invalid Stim statistical side output should fail");

    assert!(error.to_string().contains("Stim fixture output"));
}

#[test]
fn statistical_fixture_output_source_exact_compares_stdout() {
    let csv = format!(
        "{HEADER}good,M11,src/stim/simulators/dem_sampler.test.cc,statistical,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,implemented,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one row");
    let stim = process_output(Some(0), b"\n\n", b"");
    let stab = process_output(Some(0), b"00\n11\n", b"");
    let error = compare_fixture(row, &stim, &stab)
        .expect_err("statistical fixture-output row should still compare stdout");

    assert!(error.to_string().contains("stdout mismatch"));
}

#[test]
fn binomial_statistical_comparator_accepts_samples_within_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"1\n", 250).flatten());
    stdout.extend(std::iter::repeat_n(b"0\n", 750).flatten());

    assert_eq!(statistical::compare_statistical_plan(plan, &stdout), None);
}

#[test]
fn binomial_statistical_comparator_rejects_samples_outside_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"1\n", 500).flatten());
    stdout.extend(std::iter::repeat_n(b"0\n", 500).flatten());

    assert!(
        statistical::compare_statistical_plan(plan, &stdout)
            .expect("statistical rejection")
            .contains("outside 5 sigma")
    );
}

#[test]
fn binomial_statistical_comparator_rejects_non_bit_output() {
    let plan = "sample_count=1; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::compare_statistical_plan(plan, b"shot M0\n")
            .expect("statistical rejection")
            .contains("expected one 0/1 bit per shot")
    );
}

#[test]
fn binomial_statistical_comparator_rejects_sample_count_mismatch() {
    let plan = "sample_count=2; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::compare_statistical_plan(plan, b"0\n")
            .expect("statistical rejection")
            .contains("expected 2 samples")
    );
}

#[test]
fn bucket_statistical_comparator_accepts_samples_within_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate buckets 00=0.4,01=0.2,10=0.2,11=0.2 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"00\n", 400).flatten());
    stdout.extend(std::iter::repeat_n(b"01\n", 200).flatten());
    stdout.extend(std::iter::repeat_n(b"10\n", 200).flatten());
    stdout.extend(std::iter::repeat_n(b"11\n", 200).flatten());

    assert_eq!(statistical::compare_statistical_plan(plan, &stdout), None);
}

#[test]
fn bucket_statistical_comparator_rejects_unplanned_buckets() {
    let plan = "sample_count=1; fixed_seed=5; tolerate buckets 00=0.5,01=0.5 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::compare_statistical_plan(plan, b"10\n")
            .expect("statistical rejection")
            .contains("unplanned bucket")
    );
}

#[test]
fn bucket_statistical_comparator_rejects_samples_outside_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate buckets 00=0.4,01=0.2,10=0.2,11=0.2 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"00\n", 900).flatten());
    stdout.extend(std::iter::repeat_n(b"01\n", 100).flatten());

    assert!(
        statistical::compare_statistical_plan(plan, &stdout)
            .expect("statistical rejection")
            .contains("actual bucket")
    );
}

#[test]
fn bucket_statistical_plan_rejects_mixed_width_buckets() {
    let plan = "sample_count=1; fixed_seed=5; tolerate buckets 0=0.5,11=0.5 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::compare_statistical_plan(plan, b"0\n")
            .expect("statistical rejection")
            .contains("mixes bucket widths")
    );
}

#[test]
fn statistical_plan_rejects_unenforced_false_positive_claims() {
    let zero_budget = "sample_count=1; fixed_seed=5; tolerate binomial p=0.5 within 5 sigma; false_positive_rate<=0";
    assert!(
        statistical::compare_statistical_plan(zero_budget, b"0\n")
            .expect("zero false-positive budget should fail")
            .contains("must be positive")
    );

    let impossible_budget = "sample_count=1000; fixed_seed=5; tolerate buckets 00=0.4,01=0.2,10=0.2,11=0.2 within 5 sigma; false_positive_rate<=0.0000001";
    assert!(
        statistical::compare_statistical_plan(impossible_budget, b"00\n")
            .expect("overstated false-positive budget should fail")
            .contains("is tighter than the estimated")
    );
}

#[test]
fn validation_rejects_statistical_plan_that_disagrees_with_argv() {
    let csv = format!(
        "{HEADER}bad,M8,src/stim/cmd/command_sample.test.cc,statistical,statistical,stim sample,sample|--shots|10|--seed|5,inputs/sample_noisy.stim,,0,empty,red,sample_count=11; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check(&root)
        .expect_err("mismatched plan should fail");

    assert!(
        error
            .to_string()
            .contains("sample_count=11 does not match --shots 10")
    );
}

#[test]
fn validation_rejects_empty_argv_tokens() {
    let csv = format!(
        "{HEADER}bad,M8,src/stim/cmd/command_sample.test.cc,statistical,statistical,stim sample,sample||--shots|10,inputs/sample_noisy.stim,,0,empty,red,sample_count=10; fixed_seed=1; false_positive_rate<=0.001,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check(&root)
        .expect_err("empty argv token should fail");

    assert!(error.to_string().contains("has an empty argv token"));
}

#[test]
fn validation_rejects_unsafe_fixture_output_placeholder() {
    let csv = format!(
        "{HEADER}bad,M11,src/stim/cmd/command_sample_dem.test.cc,exact-output,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:../escape.stdout}},inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
        .expect_err("unsafe fixture output should fail");

    assert!(
        error
            .to_string()
            .contains("bad has unsafe fixture_output ../escape.stdout")
    );
}

#[test]
fn validation_rejects_unsafe_fixture_input_placeholder() {
    let csv = format!(
        "{HEADER}bad,M11,src/stim/cmd/command_sample_dem.test.cc,exact-output,exact-output,stim sample_dem,sample_dem|--replay_err_in|{{fixture_input:../escape.01}},inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
        .expect_err("unsafe fixture input should fail");

    assert!(
        error
            .to_string()
            .contains("bad has unsafe fixture_input ../escape.01")
    );
}

#[test]
fn validation_rejects_malformed_fixture_output_placeholder() {
    let csv = format!(
        "{HEADER}bad,M11,src/stim/cmd/command_sample_dem.test.cc,exact-output,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:expected/missing.stdout,inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
        .expect_err("malformed fixture output should fail");

    assert!(
        error
            .to_string()
            .contains("bad has malformed fixture output token")
    );
}

#[cfg(unix)]
#[test]
fn fixture_output_scratch_rejects_symlinked_parent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = crate::RepoRoot::resolve(temp.path()).expect("resolve temp root");
    let fixture_parent = temp.path().join("oracle").join("fixtures");
    std::fs::create_dir_all(fixture_parent.join("inputs")).expect("create fixture inputs");
    std::fs::create_dir_all(fixture_parent.join("expected")).expect("create fixture expected");
    std::fs::write(
        fixture_parent.join("inputs").join("sample.dem"),
        b"error(1) D0\n",
    )
    .expect("write input");
    std::fs::write(
        fixture_parent
            .join("expected")
            .join("sample_dem_observable_obs.stdout"),
        b"1\n",
    )
    .expect("write expected side output");
    std::fs::write(
        fixture_parent.join("expected").join("sample.stdout"),
        b"1\n",
    )
    .expect("write expected stdout");
    let oracle_target = temp.path().join("target").join("oracle");
    std::fs::create_dir_all(&oracle_target).expect("create oracle target");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).expect("create outside dir");
    std::os::unix::fs::symlink(&outside, oracle_target.join("fixture-outputs"))
        .expect("create scratch symlink");
    let csv = format!(
        "{HEADER}bad,M11,src/stim/cmd/command_sample_dem.test.cc,exact-output,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:expected/sample_dem_observable_obs.stdout}},inputs/sample.dem,expected/sample.stdout,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one fixture row");

    let error = outputs::prepare_command(&root, row, "test")
        .expect_err("symlinked scratch parent should fail");

    assert!(error.to_string().contains("scratch path contains symlink"));
}

#[test]
fn validation_requires_fixture_milestone_and_parity_to_match_matrix() {
    let csv = format!(
        "{HEADER}m7-convert-01-to-dets,M7,src/stim/cmd/command_convert.test.cc,exact-output,exact-output,stim convert 01 to dets,convert|--in_format=01|--out_format=dets,inputs/convert_measurements.01,expected/m7_convert_01_to_dets.stdout,0,empty,red,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let error = manifest
        .check(&root)
        .expect_err("M7 convert row must not satisfy M4 coverage");

    assert!(error.to_string().contains(
        "missing M2 fixture row for src/stim/cmd/command_convert.test.cc (M4/exact-output)"
    ));
}

#[test]
fn validation_requires_declared_expected_stdout_by_default() {
    let csv = format!(
        "{HEADER}bad,M4,src/stim/cmd/command_convert.test.cc,exact-output,exact-output,stab-core circuit parse print,core-circuit-parse-print,inputs/parser_basic.stim,expected/missing-golden.stdout,0,any,manifest-only,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    let error = manifest
        .check(&root)
        .expect_err("declared expected stdout should exist during validation");
    assert!(
        error
            .to_string()
            .contains("bad expected_stdout_path does not exist: expected/missing-golden.stdout")
    );

    let allow_missing_error = manifest
        .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
        .expect_err("synthetic manifest should still fail compatibility coverage");
    assert!(
        !allow_missing_error
            .to_string()
            .contains("bad expected_stdout_path does not exist: expected/missing-golden.stdout")
    );
}

#[cfg(unix)]
#[test]
fn fixture_path_validation_rejects_symlink_components() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fixture_root = temp.path().join("fixtures");
    std::fs::create_dir(&fixture_root).expect("create fixture root");
    let outside = temp.path().join("outside.stdout");
    std::fs::write(&outside, b"outside").expect("write outside file");
    std::os::unix::fs::symlink(&outside, fixture_root.join("link.stdout")).expect("create symlink");

    let error = validate_fixture_path(
        &fixture_root,
        "bad",
        "expected_stdout_path",
        "link.stdout",
        FixturePathRequirement::ExistingFileIfPresent,
    )
    .expect_err("symlink fixture path should fail");

    assert!(error.contains("contains symlink"));
}
