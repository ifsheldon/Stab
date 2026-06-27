use super::run_from;

mod channels;
mod parity_rows;

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
