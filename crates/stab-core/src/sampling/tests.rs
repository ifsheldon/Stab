#![allow(
    clippy::expect_used,
    reason = "sampling unit tests use direct fixture parsing assertions for compact diagnostics"
)]

use super::*;

fn samples(input: &str, shots: usize) -> Vec<Vec<bool>> {
    let circuit = Circuit::from_stim_str(input).expect("parse circuit");
    CompiledSampler::compile(&circuit)
        .expect("compile sampler")
        .sample_zero_one(shots)
}

#[test]
fn samples_m8_basic_measurements_as_zeroes() {
    assert_eq!(
        samples(
            include_str!("../../../../oracle/fixtures/inputs/sample_basic.stim"),
            2
        ),
        vec![vec![false, false], vec![false, false]]
    );
}

#[test]
fn samples_x_and_inverted_measurements_like_command_sample() {
    assert_eq!(samples("X 0\nM 0\n", 1), vec![vec![true]]);
    assert_eq!(samples("M !0\n", 1), vec![vec![true]]);
}

#[test]
fn samples_reset_and_measure_reset_deterministically() {
    assert_eq!(samples("X 0\nR 0\nM 0\n", 1), vec![vec![false]]);
    assert_eq!(samples("X 0\nMR 0\nMR 0\n", 1), vec![vec![true, false]]);
}

#[test]
fn samples_repeat_blocks_without_flattening_during_compilation() {
    assert_eq!(
        samples("REPEAT 2 {\n    X 0\n    M 0\n}\n", 1),
        vec![vec![true, false]]
    );
}

#[test]
fn samples_single_qubit_clifford_measurements() {
    assert_eq!(samples("H 0\nS 0\nS 0\nH 0\nM 0\n", 3), vec![vec![true]; 3]);

    let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let first = sampler.sample_zero_one_with_seed(1000, Some(5));
    let second = sampler.sample_zero_one_with_seed(1000, Some(5));
    assert_eq!(first, second);

    let hits = first.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (400..=600).contains(&hits),
        "expected roughly 500 H-basis measurement hits, got {hits}"
    );
}

#[test]
fn samples_x_and_y_basis_measurements_deterministically() {
    assert_eq!(samples("H 0\nMX 0\n", 1), vec![vec![false]]);
    assert_eq!(samples("X 0\nH 0\nMX 0\n", 1), vec![vec![true]]);
    assert_eq!(samples("H 0\nS 0\nMY 0\n", 1), vec![vec![false]]);
    assert_eq!(samples("H 0\nZ 0\nS 0\nMY 0\n", 1), vec![vec![true]]);
}

#[test]
fn random_basis_measurement_collapses_to_the_measured_basis() {
    let circuit = Circuit::from_stim_str("MX 0\nMX 0\nMY 1\nMY 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
        assert_eq!(shot.first(), shot.get(1));
        assert_eq!(shot.get(2), shot.get(3));
    }
}

#[test]
fn reset_and_measure_reset_use_their_measurement_basis() {
    assert_eq!(
        samples("RX 0\nMX 0\nRY 1\nMY 1\n", 1),
        vec![vec![false, false]]
    );

    let circuit = Circuit::from_stim_str("MRX 0\nMX 0\nMRY 1\nMY 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
        assert_eq!(
            shot.get(1),
            Some(&false),
            "MRX should reset to +X after reporting"
        );
        assert_eq!(
            shot.get(3),
            Some(&false),
            "MRY should reset to +Y after reporting"
        );
    }
}

#[test]
fn measurement_record_feedback_applies_local_paulis() {
    assert_eq!(
        samples("X 0\nM 0\nCX rec[-1] 1\nM 1\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("M 0\nCX rec[-1] 1\nM 1\n", 1),
        vec![vec![false, false]]
    );
    assert_eq!(
        samples("X 0\nM 0\nCY rec[-1] 1\nM 1\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("H 1\nX 0\nM 0\nCZ rec[-1] 1\nMX 1\n", 1),
        vec![vec![true, true]]
    );
}

#[test]
fn entangling_clifford_measurements_preserve_bell_correlations() {
    let circuit = Circuit::from_stim_str("H 0\nCX 0 1\nM 0 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    let hits = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    assert!(
        (400..=600).contains(&hits),
        "expected roughly balanced Bell-pair measurements, got {hits}"
    );
    assert!(
        shots
            .iter()
            .all(|shot| shot.first().copied() == shot.get(1).copied()),
        "Bell-pair measurements should be perfectly correlated"
    );
}

#[test]
fn entangling_measure_reset_collapses_then_resets_only_measured_qubit() {
    let circuit = Circuit::from_stim_str("H 0\nCX 0 1\nMR 0\nM 0 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    assert!(
        shots.iter().all(|shot| {
            shot.get(1) == Some(&false) && shot.first().copied() == shot.get(2).copied()
        }),
        "MR should record the Bell collapse, reset qubit 0, and leave qubit 1 collapsed"
    );
}

#[test]
fn qubit_cx_and_feedback_cx_can_coexist() {
    let circuit =
        Circuit::from_stim_str("H 0\nCX 0 1\nM 0\nCX rec[-1] 2\nM 1 2\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    assert!(
        shots.iter().all(|shot| {
            let Some(measured) = shot.first() else {
                return false;
            };
            shot.get(1) == Some(measured) && shot.get(2) == Some(measured)
        }),
        "qubit CX should create a Bell correlation and feedback CX should read the measurement record"
    );
}

#[test]
fn two_qubit_tableau_gates_act_on_stabilizer_frame() {
    assert_eq!(
        samples("X 0\nSWAP 0 1\nM 0 1\n", 1),
        vec![vec![false, true]]
    );
}

#[test]
fn pair_measurements_use_requested_product_basis() {
    assert_eq!(
        samples("RX 0 1\nMXX 0 1\nRY 0 1\nMYY 0 1\nR 0 1\nMZZ 0 1\n", 1),
        vec![vec![false, false, false]]
    );
}

#[test]
fn pair_measurement_inversions_flip_product_results() {
    for shot in samples("MXX 0 1 0 !1 !0 1 !0 !1\n", 100) {
        let first = shot.first().copied().expect("first MXX result");
        assert_eq!(shot, vec![first, !first, !first, first]);
    }
}

#[test]
fn mpp_measures_pauli_products_with_inversions() {
    assert_eq!(
        samples("H 0\nCX 0 1\nMPP X0*X1 Z0*Z1 !Y0*Y1\n", 1),
        vec![vec![false, false, false]]
    );
}

#[test]
fn heralded_pauli_channel_records_and_applies_local_paulis() {
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 0, 0, 0) 0\n", 1),
        vec![vec![]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(1, 0, 0, 0) 0\nM 0\n", 1),
        vec![vec![false]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 1, 0, 0) 0\nM 0\n", 1),
        vec![vec![true]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 0, 1, 0) 0\nM 0\n", 1),
        vec![vec![true]]
    );
    assert_eq!(
        samples("H 0\nHERALDED_PAULI_CHANNEL_1(0, 0, 0, 1) 0\nMX 0\n", 1),
        vec![vec![true]]
    );
    assert_eq!(
        samples(
            "HERALDED_PAULI_CHANNEL_1(0, 1, 0, 0) 0\nCX rec[-1] 1\nM 0 1\n",
            1
        ),
        vec![vec![true, true]]
    );
}

#[test]
fn heralded_erase_records_heralds_and_randomizes_state() {
    assert_eq!(
        samples("HERALDED_ERASE(0) 0 1\nM 0 1\n", 1),
        vec![vec![false, false]]
    );

    let circuit = Circuit::from_stim_str("HERALDED_ERASE(1) 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    let hits = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    assert!(
        (400..=600).contains(&hits),
        "expected roughly 500 heralded erase Z-basis hits, got {hits}"
    );
}

#[test]
fn heralded_pauli_channel_samples_disjoint_probabilities() {
    let circuit = Circuit::from_stim_str(
        "HERALDED_PAULI_CHANNEL_1(0.05, 0.10, 0.15, 0.25) 0\nCX rec[-1] 1\nM 0 1\n",
    )
    .expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    let heralds = shots
        .iter()
        .filter(|shot| shot.get(1) == Some(&true))
        .count();
    let hits = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    assert!(
        (465..=635).contains(&heralds),
        "expected roughly 550 heralded pauli heralds, got {heralds}"
    );
    assert!(
        (165..=335).contains(&hits),
        "expected roughly 250 heralded pauli Z-basis hits, got {hits}"
    );
}

#[test]
fn rejects_feedback_that_reads_missing_measurements() {
    let circuit = Circuit::from_stim_str("CX rec[-1] 0\n").expect("parse circuit");

    assert_eq!(
        CompiledSampler::compile(&circuit),
        Err(CircuitError::invalid_sampler_compilation(
            "measurement record target rec[-1] is not available while compiling CX feedback"
        ))
    );
}

#[test]
fn z_error_flips_x_basis_measurements_after_hadamards() {
    let circuit =
        Circuit::from_stim_str("H 0\nZ_ERROR(0.25) 0\nH 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
    let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (175..=325).contains(&hits),
        "expected roughly 250 Z-error X-basis hits, got {hits}"
    );
}

#[test]
fn writes_stim_text_sample_formats() {
    let circuit = Circuit::from_stim_str("X 2 3 5\nM 0 1 2 3 4 5\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(sampler.sample_bytes(1, SampleFormat::ZeroOne), b"001101\n");
    assert_eq!(sampler.sample_bytes(1, SampleFormat::B8), &[0x2c]);
    assert_eq!(
        sampler.sample_bytes(1, SampleFormat::R8),
        &[0x02, 0x00, 0x01, 0x00]
    );
    assert_eq!(sampler.sample_bytes(1, SampleFormat::Hits), b"2,3,5\n");
    assert_eq!(
        sampler.sample_bytes(1, SampleFormat::Dets),
        b"shot M2 M3 M5\n"
    );
    assert_eq!(
        sampler.sample_bytes(2, SampleFormat::Hits),
        b"2,3,5\n2,3,5\n"
    );
}

#[test]
fn writes_r8_samples_with_long_false_runs() {
    let circuit = Circuit::from_stim_str("X 1\nM 0 0 0 0 0 0 0 0 0 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(sampler.sample_bytes(1, SampleFormat::R8), &[0x09, 0x00]);

    let long_zero_circuit =
        Circuit::from_stim_str(&format!("MPAD {}\n", "0 ".repeat(260))).expect("parse circuit");
    let long_zero_sampler = CompiledSampler::compile(&long_zero_circuit).expect("compile sampler");
    assert_eq!(
        long_zero_sampler.sample_bytes(1, SampleFormat::R8),
        &[0xff, 0x05]
    );
}

#[test]
fn writes_ptb64_samples_in_measurement_major_shot_groups() {
    let circuit = Circuit::from_stim_str("X 1\nM 0 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(
        sampler
            .sample_ptb64_bytes_with_seed(64, Some(5))
            .expect("sample ptb64"),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
        ]
    );
}

#[test]
fn rejects_ptb64_shot_counts_that_are_not_multiple_of_64() {
    let circuit = Circuit::from_stim_str("M 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(
        sampler.sample_ptb64_bytes_with_seed(63, Some(5)),
        Err(CircuitError::invalid_sampler_compilation(
            "shots must be a multiple of 64 to use ptb64 format"
        ))
    );
}

#[test]
fn writes_b8_samples_with_per_shot_padding() {
    let circuit = Circuit::from_stim_str("X 0 8\nM 0 1 2 3 4 5 6 7 8\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(
        sampler.sample_bytes(2, SampleFormat::B8),
        &[0x01, 0x01, 0x01, 0x01]
    );
}

#[test]
fn seeded_x_error_sampling_is_reproducible_and_statistical() {
    let circuit = Circuit::from_stim_str("X_ERROR(0.25) 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    let first = sampler.sample_zero_one_with_seed(1000, Some(5));
    let second = sampler.sample_zero_one_with_seed(1000, Some(5));
    assert_eq!(first, second);

    let hits = first.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (175..=325).contains(&hits),
        "expected roughly 250 noisy hits, got {hits}"
    );
}

#[test]
fn z_and_identity_errors_do_not_flip_z_basis_measurements() {
    assert_eq!(
        samples("Z_ERROR(0.9) 0\nI_ERROR(0.8) 0\nM 0\n", 20),
        vec![vec![false]; 20]
    );
}

#[test]
fn depolarize1_flips_z_basis_measurements_with_x_or_y_probability() {
    let circuit = Circuit::from_stim_str("DEPOLARIZE1(0.3) 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
    let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (125..=275).contains(&hits),
        "expected roughly 200 depolarize1 Z-basis hits, got {hits}"
    );
}

#[test]
fn pauli_channel1_flips_z_basis_measurements_for_x_or_y_cases() {
    let circuit =
        Circuit::from_stim_str("PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
    let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (215..=385).contains(&hits),
        "expected roughly 300 pauli-channel1 Z-basis hits, got {hits}"
    );
}

#[test]
fn depolarize2_flips_z_basis_measurements_for_two_qubit_x_or_y_cases() {
    let circuit = Circuit::from_stim_str("DEPOLARIZE2(0.3) 0 1\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
    let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (95..=225).contains(&hits),
        "expected roughly 160 depolarize2 Z-basis hits, got {hits}"
    );
}

#[test]
fn pauli_channel2_uses_stim_probability_order_for_z_basis_toggles() {
    let circuit = Circuit::from_stim_str(
        "PAULI_CHANNEL_2(0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\nM 0 1\n",
    )
    .expect("parse circuit");

    assert_eq!(
        CompiledSampler::compile(&circuit)
            .expect("compile sampler")
            .sample_zero_one_with_seed(5, Some(5)),
        vec![vec![true, false]; 5]
    );
}
