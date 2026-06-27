use super::run_from;

fn assert_analyze_errors_matches(args: &[&str], input: &[u8], expected_stdout: &str) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), expected_stdout);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn analyze_errors_vacuous_observable_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_vacuous_observable.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_vacuous_observable.stdout"
        ),
    );
}

#[test]
fn analyze_errors_tagged_noise_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_tagged_noise.stim"),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_tagged_noise.stdout"
        ),
    );
}

#[test]
fn analyze_errors_coordinate_tracking_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_coordinate_tracking.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_coordinate_tracking.stdout"
        ),
    );
}

#[test]
fn analyze_errors_measure_reset_basis_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_measure_reset_basis.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_measure_reset_basis.stdout"
        ),
    );
}

#[test]
fn analyze_errors_period3_gates_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_period3_gates.stim"),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_period3_gates.stdout"
        ),
    );
}

#[test]
fn analyze_errors_controlled_pauli_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_controlled_pauli.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_controlled_pauli.stdout"
        ),
    );
}

#[test]
fn analyze_errors_composite_controlled_pauli_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors", "--decompose_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_composite_controlled_pauli.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_composite_controlled_pauli.stdout"
        ),
    );
}

#[test]
fn analyze_errors_measurement_feedback_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_measurement_feedback.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_measurement_feedback.stdout"
        ),
    );
}

#[test]
fn analyze_errors_multi_round_gauge_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors", "--allow_gauge_detectors"],
        include_bytes!(
            "../../../../../oracle/fixtures/inputs/analyze_errors_multi_round_gauge.stim"
        ),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_multi_round_gauge.stdout"
        ),
    );
}

#[test]
fn analyze_errors_tagged_repeat_matches_m10_oracle_golden() {
    assert_analyze_errors_matches(
        &["stab", "analyze_errors", "--fold_loops"],
        include_bytes!("../../../../../oracle/fixtures/inputs/analyze_errors_tagged_repeat.stim"),
        include_str!(
            "../../../../../oracle/fixtures/expected/m10_analyze_errors_tagged_repeat.stdout"
        ),
    );
}
