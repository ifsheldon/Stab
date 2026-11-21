use std::assert_matches;

use super::{
    ExpectedStdoutPolicy, FixtureComparator, FixtureManifest, FixturePathRequirement,
    compare_fixture,
    hex_payload::decode as decode_hex_payload,
    is_recordable, outputs,
    row::{ArgToken, RowCommand},
    run_core_fixture, statistical, validate_fixture_path,
};

const MANIFEST_CSV: &str = include_str!("../../../../oracle/fixtures/manifest.csv");
const HEADER: &str = "id,upstream_source,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,statistical_plan,source_license_note\n";

#[test]
fn hex_fixture_payloads_preserve_arbitrary_bytes_and_reject_malformed_text() {
    assert_eq!(
        decode_hex_payload(b"00 ff\nA5 7e").expect("valid mixed-case hex"),
        vec![0x00, 0xff, 0xa5, 0x7e]
    );
    assert!(
        decode_hex_payload(b"abc")
            .expect_err("odd digit count must reject")
            .contains("even number")
    );
    assert!(
        decode_hex_payload(b"fg")
            .expect_err("non-hex digit must reject")
            .contains("0x67")
    );
}

/// Parses `plan` and compares `bytes`, reporting a parse failure as the comparison failure.
fn compare_plan(plan: &str, bytes: &[u8]) -> Option<String> {
    match statistical::StatisticalPlan::parse(plan) {
        Ok(plan) => plan.compare(bytes),
        Err(reason) => Some(reason),
    }
}

/// The manifest violation `csv` produces, whether at typed parsing or at path validation.
fn manifest_violation(csv: &str, policy: ExpectedStdoutPolicy) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    match FixtureManifest::from_csv(csv) {
        Ok(manifest) => manifest
            .check_with_expected_stdout_policy(&root, policy)
            .expect_err("manifest must be rejected")
            .to_string(),
        Err(error) => error.to_string(),
    }
}

fn process_output(status: Option<i32>, stdout: &[u8], stderr: &[u8]) -> crate::ProcessOutput {
    crate::ProcessOutput {
        status,
        stdout: crate::CapturedOutput {
            bytes: stdout.to_vec(),
        },
        stderr: crate::CapturedOutput {
            bytes: stderr.to_vec(),
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

#[cfg(unix)]
#[test]
fn fixture_manifest_rejects_symlinked_fixture_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("temporary repository");
    let outside = tempfile::tempdir().expect("outside fixture directory");
    std::fs::create_dir(temp.path().join("oracle")).expect("oracle directory");
    symlink(outside.path(), temp.path().join("oracle/fixtures")).expect("symlink fixture root");
    let root = crate::RepoRoot::resolve(temp.path()).expect("resolve repository");
    let manifest = FixtureManifest::from_csv(HEADER).expect("empty manifest");

    let error = manifest
        .check(&root)
        .expect_err("symlinked fixture root must fail closed");

    assert!(error.to_string().contains("source-owned directory tree"));
}

#[test]
fn fixture_input_read_enforces_aggregate_byte_limit() {
    let temp = tempfile::tempdir().expect("temporary repository");
    let fixture_root = temp.path().join("oracle/fixtures");
    std::fs::create_dir_all(fixture_root.join("inputs")).expect("fixture inputs");
    std::fs::write(
        fixture_root.join("inputs/oversized.json"),
        vec![b'x'; crate::process::OUTPUT_LIMIT_BYTES + 1],
    )
    .expect("oversized fixture input");
    let root = crate::RepoRoot::resolve(temp.path()).expect("resolve repository");

    let error = super::paths::read_fixture_file(&root, "inputs/oversized.json")
        .expect_err("oversized fixture input must fail closed");

    assert_matches!(error, super::FixtureError::FixtureFileTooLarge { .. });
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
            .find(|row| row.id.as_str() == id)
            .expect("M4 core row");
        let core = row.core().expect("core fixture kind");
        let output = run_core_fixture(&root, row, core).expect("core fixture");

        assert_eq!(output.status, Some(0), "{id}");
    }
}

#[test]
fn every_exact_row_with_a_golden_is_recordable_from_pinned_stim() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let core_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "m4-convert-canonical-print")
        .expect("M4 exact core row");
    let cli_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "smoke-tiny-circuit")
        .expect("M0 CLI exact row");
    let reverse_flow_exact_row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "pfm-b1-exact-flow-reverse-measurement")
        .expect("PFM-B1 pinned reverse-flow row");

    let core_structural_row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "m4-parser-basic")
        .expect("M4 structural core row");

    assert!(is_recordable(core_exact_row));
    assert!(is_recordable(cli_exact_row));
    assert!(is_recordable(reverse_flow_exact_row));
    assert!(!is_recordable(core_structural_row));
}

#[test]
fn successful_exact_output_rows_have_expected_stdout_paths() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");

    for row in manifest
        .rows
        .iter()
        .filter(|row| row.comparator == FixtureComparator::ExactOutput)
        .filter(|row| row.expected_status == 0)
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
        let RowCommand::Cli(argv) = &row.command else {
            continue;
        };
        let statistical_side_output = row
            .statistical_plan()
            .is_some_and(|plan| plan.source() == statistical::StatisticalSource::FixtureOutput);
        for token in argv.tokens() {
            match token {
                ArgToken::Input(relative) => {
                    let path = super::fixture_file(&root, relative).unwrap();
                    assert!(path.is_file(), "{} {}", row.id, relative);
                }
                ArgToken::Output(_) if statistical_side_output => {}
                ArgToken::Output(relative) => {
                    let path = super::fixture_file(&root, relative).unwrap();
                    assert!(path.is_file(), "{} {}", row.id, relative);
                }
                ArgToken::Literal(_) => {}
            }
        }
    }
}

#[test]
fn fixture_output_placeholders_use_owned_temporary_storage() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "m11-sample-dem-observable-output-exact")
        .expect("M11 fixture output row");

    let command = outputs::prepare_command(&root, row, "test").expect("prepare command");
    let output = command.outputs.first().expect("one fixture output");

    assert_eq!(command.outputs.len(), 1);
    assert_eq!(
        output.expected_relative,
        "expected/m11_sample_dem_observable_obs.stdout"
    );
    assert!(
        command
            .argv
            .iter()
            .any(|token| token == output.actual_path().as_os_str())
    );
    let run_dir = output
        .actual_path()
        .parent()
        .expect("fixture run directory")
        .to_path_buf();
    assert!(run_dir.is_dir());
    assert!(run_dir.starts_with(std::env::temp_dir()));
    std::fs::write(output.actual_path(), b"owned").expect("write side output");
    assert_eq!(
        outputs::read_actual_fixture_output(row, output).expect("read side output"),
        b"owned"
    );
    drop(command);
    assert!(!run_dir.exists(), "scratch run directory must be removed");
}

#[cfg(target_os = "linux")]
#[test]
fn descriptor_owned_fixture_output_rejects_a_child_symlink() {
    use std::os::unix::fs::symlink;

    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "m11-sample-dem-observable-output-exact")
        .expect("M11 fixture output row");
    let command = outputs::prepare_command(&root, row, "symlink-test").expect("prepare command");
    let output = command.outputs.first().expect("one fixture output");
    let outside = tempfile::NamedTempFile::new().expect("outside output");
    std::fs::write(outside.path(), b"outside").expect("outside bytes");
    symlink(outside.path(), output.actual_path()).expect("child output symlink");

    assert!(outputs::read_actual_fixture_output(row, output).is_err());
    assert_eq!(
        std::fs::read(outside.path()).expect("outside bytes"),
        b"outside"
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
        .find(|row| row.id.as_str() == "m11-sample-dem-replay-side-outputs-exact")
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
fn validation_rejects_statistical_row_without_plan() {
    let csv = format!(
        "{HEADER}bad,src/stim/cmd/command_sample.test.cc,statistical,stim sample,sample|--shots|10,inputs/sample_noisy.stim,,0,empty,,hand-authored\n"
    );

    assert!(
        manifest_violation(&csv, ExpectedStdoutPolicy::RequireExisting)
            .contains("comparator needs structural or statistical plan text")
    );
}

#[test]
fn statistical_plan_source_defaults_to_stdout() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert_eq!(
        statistical::StatisticalPlan::parse(plan).unwrap().source(),
        statistical::StatisticalSource::Stdout
    );
}

#[test]
fn statistical_plan_source_accepts_fixture_output() {
    let plan = "sample_count=1000; fixed_seed=5; source=fixture_output; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert_eq!(
        statistical::StatisticalPlan::parse(plan).unwrap().source(),
        statistical::StatisticalSource::FixtureOutput
    );
}

#[test]
fn statistical_plan_source_rejects_unknown_values() {
    let plan = "sample_count=1000; fixed_seed=5; source=sidecar; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        statistical::StatisticalPlan::parse(plan)
            .expect_err("unknown source should fail")
            .contains("unknown statistical source")
    );
}

#[test]
fn statistical_fixture_output_placeholders_do_not_need_committed_expected_files() {
    let csv = format!(
        "{HEADER}good,src/stim/simulators/dem_sampler.test.cc,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
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
        "{HEADER}bad,src/stim/simulators/dem_sampler.test.cc,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5,inputs/sample_dem_observable_only_noisy.dem,,0,empty,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
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
        "{HEADER}good,src/stim/simulators/dem_sampler.test.cc,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
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
    let stim_output =
        outputs::FixtureOutput::from_path("expected/missing-statistical.stdout", stim_path);
    let stab_output =
        outputs::FixtureOutput::from_path("expected/missing-statistical.stdout", stab_path);

    outputs::compare_outputs(row, &[stim_output], &[stab_output]).expect("statistical side output");
}

#[test]
fn statistical_fixture_output_comparator_checks_stim_side_output() {
    let csv = format!(
        "{HEADER}good,src/stim/simulators/dem_sampler.test.cc,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one row");
    let temp = tempfile::tempdir().expect("tempdir");
    let stim_path = temp.path().join("stim-side-output.stdout");
    let stab_path = temp.path().join("stab-side-output.stdout");
    std::fs::write(&stim_path, b"created by Stim\n").expect("write stim side output");
    std::fs::write(&stab_path, observable_only_distribution_bytes())
        .expect("write stab side output");
    let stim_output =
        outputs::FixtureOutput::from_path("expected/missing-statistical.stdout", stim_path);
    let stab_output =
        outputs::FixtureOutput::from_path("expected/missing-statistical.stdout", stab_path);
    let error = outputs::compare_outputs(row, &[stim_output], &[stab_output])
        .expect_err("invalid Stim statistical side output should fail");

    assert!(error.to_string().contains("Stim fixture output"));
}

#[test]
fn statistical_fixture_output_source_exact_compares_stdout() {
    let csv = format!(
        "{HEADER}good,src/stim/simulators/dem_sampler.test.cc,statistical,stim sample_dem,sample_dem|--shots|1000|--seed|5|--obs_out|{{fixture_output:expected/missing-statistical.stdout}},inputs/sample_dem_observable_only_noisy.dem,,0,empty,\"sample_count=1000; fixed_seed=5; source=fixture_output; tolerate buckets 000=0.75,001=0.25 within 5 sigma; false_positive_rate<=0.001\",hand-authored\n"
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
fn stdout_statistical_comparator_rejects_invalid_stim_distribution() {
    let csv = format!(
        "{HEADER}good,src/stim/cmd/command_sample.test.cc,statistical,stim sample,sample|--shots|1000|--seed|5,inputs/sample_noisy.stim,,0,empty,sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one row");
    let mut invalid = Vec::new();
    invalid.extend(std::iter::repeat_n(b"1\n", 500).flatten());
    invalid.extend(std::iter::repeat_n(b"0\n", 500).flatten());
    let mut valid = Vec::new();
    valid.extend(std::iter::repeat_n(b"1\n", 250).flatten());
    valid.extend(std::iter::repeat_n(b"0\n", 750).flatten());

    let error = compare_fixture(
        row,
        &process_output(Some(0), &invalid, b""),
        &process_output(Some(0), &valid, b""),
    )
    .expect_err("invalid Stim distribution must fail the fixture");

    assert!(error.to_string().contains("Stim stdout"));
}

#[test]
fn binomial_statistical_comparator_accepts_samples_within_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"1\n", 250).flatten());
    stdout.extend(std::iter::repeat_n(b"0\n", 750).flatten());

    assert_eq!(compare_plan(plan, &stdout), None);
}

#[test]
fn binomial_statistical_comparator_rejects_samples_outside_tolerance() {
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";
    let mut stdout = Vec::new();
    stdout.extend(std::iter::repeat_n(b"1\n", 500).flatten());
    stdout.extend(std::iter::repeat_n(b"0\n", 500).flatten());

    assert!(
        compare_plan(plan, &stdout)
            .expect("statistical rejection")
            .contains("is outside the accepted count")
    );
}

#[test]
fn binomial_statistical_comparator_rejects_non_bit_output() {
    let plan = "sample_count=1; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        compare_plan(plan, b"shot M0\n")
            .expect("statistical rejection")
            .contains("expected one 0/1 bit per shot")
    );
}

#[test]
fn binomial_statistical_comparator_rejects_sample_count_mismatch() {
    let plan = "sample_count=2; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        compare_plan(plan, b"0\n")
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

    assert_eq!(compare_plan(plan, &stdout), None);
}

#[test]
fn bucket_statistical_comparator_rejects_unplanned_buckets() {
    let plan = "sample_count=1; fixed_seed=5; tolerate buckets 00=0.5,01=0.5 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        compare_plan(plan, b"10\n")
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
        compare_plan(plan, &stdout)
            .expect("statistical rejection")
            .contains("actual bucket")
    );
}

#[test]
fn bucket_statistical_plan_rejects_mixed_width_buckets() {
    let plan = "sample_count=1; fixed_seed=5; tolerate buckets 0=0.5,11=0.5 within 5 sigma; false_positive_rate<=0.001";

    assert!(
        compare_plan(plan, b"0\n")
            .expect("statistical rejection")
            .contains("mixes bucket widths")
    );
}

#[test]
fn statistical_plan_rejects_unenforced_false_positive_claims() {
    let zero_budget = "sample_count=1; fixed_seed=5; tolerate binomial p=0.5 within 5 sigma; false_positive_rate<=0";
    assert!(
        compare_plan(zero_budget, b"0\n")
            .expect("zero false-positive budget should fail")
            .contains("must be positive")
    );

    let impossible_budget = "sample_count=1000; fixed_seed=5; tolerate buckets 00=0.4,01=0.2,10=0.2,11=0.2 within 5 sigma; false_positive_rate<=0.0000001";
    assert!(
        compare_plan(impossible_budget, b"00\n")
            .expect("overstated false-positive budget should fail")
            .contains("is tighter than the estimated")
    );
}

#[test]
fn validation_rejects_statistical_plan_that_disagrees_with_argv() {
    let csv = format!(
        "{HEADER}bad,src/stim/cmd/command_sample.test.cc,statistical,stim sample,sample|--shots|10|--seed|5,inputs/sample_noisy.stim,,0,empty,sample_count=11; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001,hand-authored\n"
    );

    assert!(
        manifest_violation(&csv, ExpectedStdoutPolicy::RequireExisting)
            .contains("sample_count=11 does not match --shots 10")
    );
}

#[test]
fn statistical_plans_reject_repeated_tokens_instead_of_keeping_the_last() {
    let base = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.001";
    for (token, expected) in [
        ("sample_count=10", "sample_count more than once"),
        ("fixed_seed=6", "fixed_seed more than once"),
        (
            "false_positive_rate<=0.5",
            "false_positive_rate more than once",
        ),
        (
            "tolerate binomial p=0.5 within 5 sigma",
            "tolerance more than once",
        ),
        (
            "tolerate buckets 0=0.5,1=0.5 within 5 sigma",
            "tolerance more than once",
        ),
    ] {
        let plan = format!("{base}; {token}");
        let reason =
            statistical::StatisticalPlan::parse(&plan).expect_err("duplicate token must reject");
        assert!(reason.contains(expected), "{token}: {reason}");
    }
}

#[test]
fn one_sample_budget_counts_both_checked_outputs() {
    // 5 sigma bounds one rate at ~5.7e-7: one output fits a 1e-6 budget, Stim plus Stab do not.
    let plan = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.000001";
    let reason = statistical::StatisticalPlan::parse(plan).expect_err("two outputs overspend");
    assert!(reason.contains("over 2 checked rate(s)"), "{reason}");

    let honest = "sample_count=1000; fixed_seed=5; tolerate binomial p=0.25 within 5 sigma; false_positive_rate<=0.000002";
    statistical::StatisticalPlan::parse(honest).expect("budget for both outputs");
}

#[test]
fn malformed_core_argv_is_rejected_at_load() {
    for (argv, expected) in [
        (
            "core-parse-print|junk",
            "core-parse-print takes no arguments",
        ),
        (
            "core-dem-parse-print|junk",
            "core-dem-parse-print takes no arguments",
        ),
        (
            "core-time-reverse-flows",
            "requires exactly one case id argument",
        ),
        (
            "core-time-reverse-flows|a|b",
            "requires exactly one case id argument",
        ),
    ] {
        let csv = format!(
            "{HEADER}bad,src/stim/circuit/circuit.test.cc,structural,stab-model parse print,{argv},inputs/parser_basic.stim,,0,any,structural plan,hand-authored\n"
        );
        let error = FixtureManifest::from_csv(&csv).expect_err("malformed core argv");
        assert!(error.to_string().contains(expected), "{argv}: {error}");
    }
}

#[test]
fn validation_rejects_empty_argv_tokens() {
    let csv = format!(
        "{HEADER}bad,src/stim/cmd/command_sample.test.cc,statistical,stim sample,sample||--shots|10,inputs/sample_noisy.stim,,0,empty,sample_count=10; fixed_seed=1; false_positive_rate<=0.001,hand-authored\n"
    );

    assert!(
        manifest_violation(&csv, ExpectedStdoutPolicy::RequireExisting)
            .contains("has an empty argv token")
    );
}

#[test]
fn validation_rejects_unsafe_fixture_output_placeholder() {
    let csv = format!(
        "{HEADER}bad,src/stim/cmd/command_sample_dem.test.cc,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:../escape.stdout}},inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,,hand-authored\n"
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
        "{HEADER}bad,src/stim/cmd/command_sample_dem.test.cc,exact-output,stim sample_dem,sample_dem|--replay_err_in|{{fixture_input:../escape.01}},inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,,hand-authored\n"
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
        "{HEADER}bad,src/stim/cmd/command_sample_dem.test.cc,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:expected/missing.stdout,inputs/sample_dem_deterministic.dem,expected/m11_sample_dem_deterministic.stdout,0,empty,,hand-authored\n"
    );

    assert!(
        manifest_violation(&csv, ExpectedStdoutPolicy::AllowMissing)
            .contains("bad has malformed fixture output token")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn fixture_output_scratch_ignores_repository_symlink() {
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
        "{HEADER}bad,src/stim/cmd/command_sample_dem.test.cc,exact-output,stim sample_dem,sample_dem|--obs_out|{{fixture_output:expected/sample_dem_observable_obs.stdout}},inputs/sample.dem,expected/sample.stdout,0,empty,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse manifest");
    let row = manifest.rows.first().expect("one fixture row");

    let command = outputs::prepare_command(&root, row, "test")
        .expect("repository scratch symlink is outside the descriptor-owned output path");
    let output = command.outputs.first().expect("one output");

    assert!(!output.actual_path().starts_with(&oracle_target));
    std::fs::write(output.actual_path(), b"inside").expect("write temporary output");
    assert_eq!(
        std::fs::read_dir(&outside)
            .expect("outside directory")
            .count(),
        0
    );
}

#[test]
fn child_fixture_output_read_enforces_descriptor_byte_limit() {
    let temp = tempfile::tempdir().expect("fixture output directory");
    let actual = temp.path().join("actual.bin");
    std::fs::write(&actual, vec![0_u8; 1024 * 1024 + 1]).expect("oversized output");
    let csv = format!(
        "{HEADER}output,src/stim/cmd/command_sample_dem.test.cc,exact-output,fixture output,sample_dem,inputs/sample.dem,expected/sample.stdout,0,empty,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse fixture row");
    let row = manifest.rows.first().expect("fixture row");
    let output = outputs::FixtureOutput::from_path("expected/side.bin", actual);

    let error = outputs::read_actual_fixture_output(row, &output)
        .expect_err("oversized child output must fail closed");

    assert_matches!(error, super::FixtureError::AuxiliaryOutputTooLarge { .. });
}

#[cfg(unix)]
#[test]
fn child_fixture_output_read_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("fixture output directory");
    let outside = temp.path().join("outside.bin");
    let actual = temp.path().join("actual.bin");
    std::fs::write(&outside, b"outside").expect("outside output");
    symlink(&outside, &actual).expect("symlink output");
    let csv = format!(
        "{HEADER}output,src/stim/cmd/command_sample_dem.test.cc,exact-output,fixture output,sample_dem,inputs/sample.dem,expected/sample.stdout,0,empty,,hand-authored\n"
    );
    let manifest = FixtureManifest::from_csv(&csv).expect("parse fixture row");
    let row = manifest.rows.first().expect("fixture row");
    let output = outputs::FixtureOutput::from_path("expected/side.bin", actual);

    assert!(outputs::read_actual_fixture_output(row, &output).is_err());
}

#[test]
fn validation_requires_declared_expected_stdout_by_default() {
    let csv = format!(
        "{HEADER}bad,src/stim/cmd/command_convert.test.cc,exact-output,stab-core circuit parse print,core-circuit-parse-print,inputs/parser_basic.stim,expected/missing-golden.stdout,0,any,,hand-authored\n"
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

    manifest
        .check_with_expected_stdout_policy(&root, ExpectedStdoutPolicy::AllowMissing)
        .expect("recording may create a declared golden that does not exist yet");
}

#[test]
fn validation_allows_failure_exact_rows_without_expected_stdout() {
    let manifest = FixtureManifest::from_csv(MANIFEST_CSV).expect("parse manifest");
    let row = manifest
        .rows
        .iter()
        .find(|row| row.id.as_str() == "m7-convert-bits-to-dets-reject")
        .expect("M7 rejection row");
    assert_eq!(row.expected_status, 1);
    assert!(row.expected_stdout_path.is_empty());

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let root = crate::RepoRoot::resolve(root).expect("resolve repo root");

    manifest
        .check(&root)
        .expect("nonzero exact rows can rely on status and stderr class");
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
