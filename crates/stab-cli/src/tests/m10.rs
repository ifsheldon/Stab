use super::run_from;
use std::ffi::OsString;
use tempfile::tempdir;

mod channels;
mod parity_rows;
mod pf7_cli;

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
fn analyze_errors_negative_zero_targets_match_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_negative_zero_targets.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_negative_zero_targets.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_negative_zero_fold_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--fold_loops"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_negative_zero_fold.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_negative_zero_fold.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_obs_include_pauli_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_obs_include_pauli.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_obs_include_pauli.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_obs_include_boundaries_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_obs_include_boundaries.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_obs_include_boundaries.stdout"
        )
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
fn analyze_errors_fold_loops_nested_repeat_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--fold_loops"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_fold_nested_repeat.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_fold_nested_repeat.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_sweep_controls_match_pf3_oracle() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\n\
          CX sweep[0] 0\n\
          CY sweep[1] 0\n\
          CZ sweep[2] 0\n\
          CZ 0 sweep[3]\n\
          CZ sweep[4] sweep[5]\n\
          XCZ 0 sweep[6]\n\
          YCZ 0 sweep[7]\n\
          M 1\n\
          CZ rec[-1] sweep[8]\n\
          CZ sweep[9] rec[-1]\n\
          M 2\n\
          CZ rec[-1] rec[-2]\n\
          M 0\n\
          DETECTOR rec[-1]\n"
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "error(0.25) D0\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_sweep_controls_reject_invalid_target_positions() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\nCX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let error = String::from_utf8(stderr).unwrap();
    assert!(
        error.contains("CX target sweep[0] is not a qubit"),
        "{error}"
    );

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\nM 0 1\nCY rec[-1] rec[-2]\nM 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let error = String::from_utf8(stderr).unwrap();
    assert!(
        error.contains("CY target rec[-2] is not a qubit"),
        "{error}"
    );

    for (gate, expected) in [
        ("XCZ", "XCZ target rec[-1] is not a qubit"),
        ("YCZ", "YCZ target rec[-1] is not a qubit"),
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let input =
            format!("X_ERROR(0.25) 0\nM 0 1\n{gate} rec[-1] rec[-2]\nM 0\nDETECTOR rec[-1]\n");
        let status = run_from(
            ["stab", "analyze_errors"],
            input.as_bytes(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 1);
        assert_eq!(String::from_utf8(stdout).unwrap(), "");
        let error = String::from_utf8(stderr).unwrap();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn analyze_errors_rejects_oversized_input_file_before_reading() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("oversized.stim");
    let file = std::fs::File::create(&input_path).expect("create oversized circuit input");
    file.set_len(64 * 1024 * 1024 + 1)
        .expect("mark oversized circuit input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("analyze_errors"),
        OsString::from("--in"),
        input_path.into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("analyze_errors input is too large; limit is 67108864 bytes")
    );
}

#[test]
fn analyze_errors_rejects_excessive_repeat_nesting() {
    let mut input = String::new();
    for _ in 0..257 {
        input.push_str("REPEAT 1 {\n");
    }
    input.push_str("TICK\n");
    for _ in 0..257 {
        input.push_str("}\n");
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        input.as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("repeat nesting exceeds current limit 256")
    );
}

#[test]
fn analyze_errors_path_io_reads_input_path_and_writes_output_path() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("input.stim");
    let output_path = dir.path().join("output.dem");
    std::fs::write(&input_path, "X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("write analyze_errors input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("analyze_errors"),
        OsString::from("--in"),
        input_path.into_os_string(),
        OsString::from("--out"),
        output_path.clone().into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"ignored stdin".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    assert_eq!(
        std::fs::read_to_string(output_path).expect("read analyze_errors output"),
        "error(0.25) D0\n"
    );
}

#[test]
fn analyze_errors_path_io_reports_input_and_output_path_errors() {
    let dir = tempdir().expect("tempdir");
    let missing_input = dir.path().join("missing.stim");
    let mut missing_stdout = Vec::new();
    let mut missing_stderr = Vec::new();
    let missing_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("analyze_errors"),
            OsString::from("--in"),
            missing_input.clone().into_os_string(),
        ],
        b"".as_slice(),
        &mut missing_stdout,
        &mut missing_stderr,
    );

    assert_eq!(missing_status, 1);
    assert_eq!(String::from_utf8(missing_stdout).unwrap(), "");
    let missing_error = String::from_utf8(missing_stderr).unwrap();
    assert!(missing_error.contains("failed to read"), "{missing_error}");
    assert!(missing_error.contains("missing.stim"), "{missing_error}");

    let unwritable_output = dir.path().join("missing-dir").join("output.dem");
    let mut output_stdout = Vec::new();
    let mut output_stderr = Vec::new();
    let output_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("analyze_errors"),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
        ],
        b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut output_stdout,
        &mut output_stderr,
    );

    assert_eq!(output_status, 1);
    assert_eq!(String::from_utf8(output_stdout).unwrap(), "");
    let output_error = String::from_utf8(output_stderr).unwrap();
    assert!(output_error.contains("failed to write"), "{output_error}");
    assert!(output_error.contains("output.dem"), "{output_error}");
}

#[test]
fn analyze_errors_path_io_opens_output_before_parsing_input() {
    let dir = tempdir().expect("tempdir");
    let unwritable_output = dir.path().join("missing-dir").join("output.dem");
    let mut output_stdout = Vec::new();
    let mut output_stderr = Vec::new();
    let output_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("analyze_errors"),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
        ],
        b"NOT_A_GATE\n".as_slice(),
        &mut output_stdout,
        &mut output_stderr,
    );

    assert_eq!(output_status, 1);
    assert_eq!(String::from_utf8(output_stdout).unwrap(), "");
    let output_error = String::from_utf8(output_stderr).unwrap();
    assert!(output_error.contains("failed to write"), "{output_error}");
    assert!(output_error.contains("output.dem"), "{output_error}");

    let truncated_output = dir.path().join("truncated.dem");
    std::fs::write(&truncated_output, "old output\n").expect("seed analyze_errors output");
    let mut parse_stdout = Vec::new();
    let mut parse_stderr = Vec::new();
    let parse_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("analyze_errors"),
            OsString::from("--out"),
            truncated_output.clone().into_os_string(),
        ],
        b"NOT_A_GATE\n".as_slice(),
        &mut parse_stdout,
        &mut parse_stderr,
    );

    assert_eq!(parse_status, 1);
    assert_eq!(String::from_utf8(parse_stdout).unwrap(), "");
    assert_ne!(String::from_utf8(parse_stderr).unwrap(), "");
    assert_eq!(
        std::fs::read_to_string(truncated_output).expect("read truncated analyze_errors output"),
        ""
    );
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
fn analyze_errors_noisy_basis_measurements_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_noisy_basis_measurements.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_noisy_basis_measurements.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_mpad_pair_measurements_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_mpad_pair_measurements.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_mpad_pair_measurements.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_mpp_product_measurements_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_mpp_product_measurements.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_mpp_product_measurements.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_heralded_erase_conditional_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "1",
        ],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_heralded_erase_conditional.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_heralded_erase_conditional.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_heralded_pauli_channel1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "1",
        ],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_heralded_pauli_channel1.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_heralded_pauli_channel1.stdout"
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
fn analyze_errors_identity_noise_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_identity_noise.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_identity_noise.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_h_propagates_pauli_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_h_propagates_pauli_error.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_h_propagates_pauli_error.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_cnot_propagates_pauli_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_cnot_propagates_pauli_error.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_cnot_propagates_pauli_error.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_swap_propagates_pauli_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_swap_propagates_pauli.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_swap_propagates_pauli.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_two_qubit_cliffords_match_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_two_qubit_cliffords.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_two_qubit_cliffords.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_hxy_propagates_pauli_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_hxy_propagates_pauli_error.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_hxy_propagates_pauli_error.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_correlated_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_correlated_error.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_correlated_error.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_pauli_channel1_product_measurements_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "1",
        ],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_product_measurements.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_pauli_channel1_product_measurements.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_decompose_fallback_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--decompose_errors"],
        include_bytes!("../../../../oracle/fixtures/inputs/analyze_errors_decompose_fallback.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_decompose_fallback.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_decompose_known_components_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--block_decompose_from_introducing_remnant_edges",
        ],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_decompose_known_components.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_decompose_known_components.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_allow_gauge_detectors_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--allow_gauge_detectors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_allow_gauge_detectors.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_allow_gauge_detectors.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_allow_gauge_detectors_hxy_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--allow_gauge_detectors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_allow_gauge_detectors_hxy.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_allow_gauge_detectors_hxy.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_gauge_detector_variants_match_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--allow_gauge_detectors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_gauge_detector_variants.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_gauge_detector_variants.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_rejects_gauge_detectors_by_default() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_allow_gauge_detectors.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("non-deterministic detectors"));
    assert!(stderr.contains("D0"));
    assert!(stderr.contains("D1"));
}

#[test]
fn analyze_errors_allow_gauge_detectors_still_rejects_gauge_observables() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--allow_gauge_detectors"],
        b"R 0\nH 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("non-deterministic observables"));
    assert!(stderr.contains("L0"));
}

#[test]
fn analyze_errors_block_decompose_rejects_remnant_edges() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--block_decompose_from_introducing_remnant_edges",
        ],
        b"X_ERROR(0.125) 0\nX_ERROR(0.375) 2\nM 0 1 2\nDETECTOR rec[-3] rec[-1]\nDETECTOR rec[-2] rec[-1]\nDETECTOR rec[-3] rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("Failed to decompose errors into graphlike components"));
    assert!(stderr.contains("block_decomposition_from_introducing_remnant_edges"));
}

#[test]
fn analyze_errors_ignore_decomposition_failures_emits_hyperedges() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--block_decompose_from_introducing_remnant_edges",
            "--ignore_decomposition_failures",
        ],
        b"X_ERROR(0.125) 0\nX_ERROR(0.375) 2\nM 0 1 2\nDETECTOR rec[-3] rec[-1]\nDETECTOR rec[-2] rec[-1]\nDETECTOR rec[-3] rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "error(0.375) D0 D1 D2\nerror(0.125) D0 D2\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_else_correlated_error_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--approximate_disjoint_errors"],
        include_bytes!(
            "../../../../oracle/fixtures/inputs/analyze_errors_else_correlated_error.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../oracle/fixtures/expected/m10_analyze_errors_else_correlated_error.stdout"
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
