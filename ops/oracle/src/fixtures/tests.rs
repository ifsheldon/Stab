use super::{
    ExpectedStdoutPolicy, FixtureComparator, FixtureManifest, FixturePathRequirement, Milestone,
    RunMode, is_recordable, run_core_fixture, run_direct_rust_fixture, validate_fixture_path,
};

const MANIFEST_CSV: &str = include_str!("../../../../oracle/fixtures/manifest.csv");
const HEADER: &str = "id,milestone,upstream_source,parity_mode,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,status,statistical_plan,source_license_note\n";

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
