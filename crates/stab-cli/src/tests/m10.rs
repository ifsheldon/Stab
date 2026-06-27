use super::run_from;

#[test]
fn analyze_errors_basic_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_basic.stim").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m10_analyze_errors_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_fold_loops_repeat_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--fold_loops"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_fold_repeat.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m10_analyze_errors_fold_repeat.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_approx_pauli_channel1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--approximate_disjoint_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_approx_pauli_channel1.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_approx_pauli_channel1.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_depolarize1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_depolarize1.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m10_analyze_errors_depolarize1.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_measurement_flip_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_measurement_flip.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_measurement_flip.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_reset_clears_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_reset_clears_error.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_reset_clears_error.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn legacy_analyze_errors_alias_matches_subcommand() {
    let input = b"M 0\nDETECTOR rec[-1]\n";
    let mut legacy_stdout = Vec::new();
    let mut legacy_stderr = Vec::new();
    let legacy_status = run_from(
        ["stab", "--analyze_errors"],
        input.as_slice(),
        &mut legacy_stdout,
        &mut legacy_stderr,
    );
    let mut subcommand_stdout = Vec::new();
    let mut subcommand_stderr = Vec::new();
    let subcommand_status = run_from(
        ["stab", "analyze_errors"],
        input.as_slice(),
        &mut subcommand_stdout,
        &mut subcommand_stderr,
    );

    assert_eq!(legacy_status, 0);
    assert_eq!(subcommand_status, 0);
    assert_eq!(legacy_stdout, subcommand_stdout);
    assert_eq!(legacy_stderr, subcommand_stderr);
}

#[test]
fn analyze_errors_maps_simple_pauli_noise_to_dem_errors() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "error(0.25) D0\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_accepts_no_effect_flags_on_supported_circuits() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--fold_loops",
            "--allow_gauge_detectors",
            "--approximate_disjoint_errors",
        ],
        b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "detector D0\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}
