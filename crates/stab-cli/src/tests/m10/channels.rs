use super::run_from;

#[test]
fn analyze_errors_approx_pauli_channel1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors", "--approximate_disjoint_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_approx_pauli_channel1.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_approx_pauli_channel1.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_exact_pauli_channel1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_exact_pauli_channel1.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_exact_pauli_channel1.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_approx_pauli_channel1_accepts_numeric_threshold() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "0.5",
        ],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_approx_pauli_channel1.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_approx_pauli_channel1.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_pauli_channel1_two_qubit_clifford_matches_m10_oracle_golden() {
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
            "../../../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_two_qubit_clifford.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_pauli_channel1_two_qubit_clifford.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_pauli_channel1_controlled_pauli_matches_m10_oracle_golden() {
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
            "../../../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_controlled_pauli.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_pauli_channel1_controlled_pauli.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_approx_pauli_channel2_accepts_numeric_threshold() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "0.38",
        ],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_approx_pauli_channel2.stim"
        )
        .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_approx_pauli_channel2.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_heralded_erase_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "1",
        ],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_heralded_erase.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_heralded_erase.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_approx_pauli_channel1_rejects_low_numeric_threshold() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "0.3",
        ],
        b"R 0\nPAULI_CHANNEL_1(0, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("PAULI_CHANNEL_1"));
    assert!(stderr.contains("0.375"));
    assert!(stderr.contains("0.3"));
}

#[test]
fn analyze_errors_depolarize1_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_depolarize1.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_depolarize1.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_depolarize2_matches_m10_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "analyze_errors"],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_depolarize2.stim")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_depolarize2.stdout"
        )
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}
