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

fn count_determined(input: &str, unknown_input: bool) -> u64 {
    let circuit = Circuit::from_stim_str(input).expect("parse circuit");
    count_determined_measurements(&circuit, unknown_input).expect("count determined measurements")
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
fn can_sample_against_zero_reference_sample() {
    let circuit = Circuit::from_stim_str("H 0\nS 0\nS 0\nH 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(
        sampler.sample_zero_one_with_seed_and_reference_mode(3, Some(5), false),
        vec![vec![true]; 3]
    );
    assert_eq!(
        sampler.sample_zero_one_with_seed_and_reference_mode(3, Some(5), true),
        vec![vec![false]; 3]
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
fn measurement_probability_arguments_flip_reported_results() {
    assert_eq!(samples("M(1) 0\n", 1), vec![vec![true]]);
    assert_eq!(samples("MPAD(1) 0 1\n", 1), vec![vec![true, false]]);

    let circuit = Circuit::from_stim_str("M(0.25) 0\nMPP(0.5) Z1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(4000, Some(5));

    let first_hits = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    let second_hits = shots
        .iter()
        .filter(|shot| shot.get(1) == Some(&true))
        .count();
    assert!(
        (800..=1200).contains(&first_hits),
        "expected roughly 1000 M(0.25) hits, got {first_hits}"
    );
    assert!(
        (1800..=2200).contains(&second_hits),
        "expected roughly 2000 MPP(0.5) hits, got {second_hits}"
    );
}

#[test]
fn anti_hermitian_mpp_products_are_rejected() {
    let circuit = Circuit::from_stim_str("MPP X0*Z0\n").expect("parse circuit");

    assert_eq!(
        CompiledSampler::compile(&circuit),
        Err(CircuitError::invalid_sampler_compilation(
            "MPP Pauli product is anti-Hermitian"
        ))
    );
}

#[test]
fn correlated_error_branches_match_stim_else_semantics() {
    assert_eq!(
        samples("E(1)\nELSE_CORRELATED_ERROR(1) X0\nM 0\n", 1),
        vec![vec![false]],
        "an empty successful correlated-error branch must suppress its ELSE branch"
    );
    assert_eq!(
        samples(
            "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(0) X1 X2\nELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
            1,
        ),
        vec![vec![false, false, false, false]]
    );
    assert_eq!(
        samples(
            "E(1) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\nE(1) X3 X4\nM 0 1 2 3 4\n",
            1,
        ),
        vec![vec![true, true, false, true, true]]
    );
    assert_eq!(
        samples(
            "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\nELSE_CORRELATED_ERROR(1) X2 X3\nM 0 1 2 3\n",
            1,
        ),
        vec![vec![false, true, true, false]]
    );
}

#[test]
fn correlated_error_samples_conditional_distribution() {
    let circuit = Circuit::from_stim_str(
        "CORRELATED_ERROR(0.5) X0\nELSE_CORRELATED_ERROR(0.25) X1\nELSE_CORRELATED_ERROR(0.75) X2\nM 0 1 2\n",
    )
    .expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(4000, Some(5));

    let mut hits = [0usize; 3];
    for shot in &shots {
        for (index, count) in hits.iter_mut().enumerate() {
            if shot.get(index) == Some(&true) {
                *count += 1;
            }
        }
    }
    let [first_hits, second_hits, third_hits] = hits;
    assert!(
        (1800..=2200).contains(&first_hits),
        "expected roughly 2000 first-branch hits, got {}",
        first_hits
    );
    assert!(
        (300..=700).contains(&second_hits),
        "expected roughly 500 second-branch hits, got {}",
        second_hits
    );
    assert!(
        (925..=1325).contains(&third_hits),
        "expected roughly 1125 third-branch hits, got {}",
        third_hits
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
fn seeded_sample_bytes_match_seeded_record_samples() {
    let circuit = Circuit::from_stim_str("H 0\nM 0\nM 0\nMPAD 0 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let records = sampler.sample_zero_one_with_seed(32, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(32, SampleFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, SampleFormat::ZeroOne)
    );
    assert_eq!(
        sampler.sample_bytes_with_seed(32, SampleFormat::B8, Some(5)),
        crate::result_formats::write_records(&records, SampleFormat::B8)
    );
}

#[test]
fn streaming_samples_match_seeded_record_samples() {
    let circuit = Circuit::from_stim_str("H 0\nM 0\nCX rec[-1] 1\nM 1\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let expected = sampler.sample_zero_one_with_seed(32, Some(5));
    let mut streamed = Vec::new();

    let result =
        sampler.for_each_sample_with_seed_and_reference_mode(32, Some(5), false, |record| {
            streamed.push(record.to_vec());
            Ok::<(), std::convert::Infallible>(())
        });

    match result {
        Ok(()) => {}
        Err(error) => match error {},
    }
    assert_eq!(streamed, expected);
}

#[test]
fn byte_sampling_measure_reset_uses_physical_result_for_reset() {
    let inverted_circuit = Circuit::from_stim_str("MR !0\nM 0\n").expect("parse circuit");
    let inverted_sampler = CompiledSampler::compile(&inverted_circuit).expect("compile sampler");
    assert_eq!(
        inverted_sampler.sample_bytes(1, SampleFormat::ZeroOne),
        b"10\n"
    );
    assert_eq!(inverted_sampler.sample_bytes(1, SampleFormat::B8), &[0x01]);

    let noisy_circuit = Circuit::from_stim_str("MR(1) 0\nM 0\n").expect("parse circuit");
    let noisy_sampler = CompiledSampler::compile(&noisy_circuit).expect("compile sampler");
    assert_eq!(
        noisy_sampler.sample_bytes_with_seed(1, SampleFormat::ZeroOne, Some(5)),
        b"10\n"
    );
    assert_eq!(
        noisy_sampler.sample_bytes_with_seed(1, SampleFormat::B8, Some(5)),
        &[0x01]
    );
}

#[test]
fn packed_sample_bytes_match_seeded_record_samples_for_surface_like_ops() {
    let circuit = Circuit::from_stim_str(
        "
        R 0 1 2 3
        H 0 2
        DEPOLARIZE1(0.001) 0 2
        CX 0 1 2 3
        DEPOLARIZE2(0.001) 0 1 2 3
        MR 0 2
        REPEAT 2 {
            H 0 2
            CX 0 1 2 3
            DEPOLARIZE2(0.001) 0 1 2 3
            H 0 2
            MR 0 2
        }
        M 1 3
        ",
    )
    .expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let records = sampler.sample_zero_one_with_seed(64, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(64, SampleFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, SampleFormat::ZeroOne)
    );
    assert_eq!(
        sampler.sample_bytes_with_seed(64, SampleFormat::B8, Some(5)),
        crate::result_formats::write_records(&records, SampleFormat::B8)
    );
}

#[test]
fn direct_noisy_z_measurement_bytes_match_seeded_record_samples() {
    let circuit = Circuit::from_stim_str("X_ERROR(0.25) 0\nM 0\n").expect("parse circuit");
    let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
    let records = sampler.sample_zero_one_with_seed(128, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(128, SampleFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, SampleFormat::ZeroOne)
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

#[test]
fn count_determined_measurements_matches_unknown_input_subset() {
    assert_eq!(count_determined("MZZ 0 1", false), 1);
    assert_eq!(count_determined("MZZ 0 1", true), 0);
    assert_eq!(count_determined("MPP Z0*Z1 X2*X3", false), 1);
    assert_eq!(count_determined("MPP Z0*Z1 X2*X3", true), 0);
    assert_eq!(
        count_determined(
            "
            MPP Z0*Z1 X2*X3
            TICK
            MPP Z0*Z1 X2*X3
            ",
            true,
        ),
        2
    );
    assert_eq!(
        count_determined(
            "
            MPP Z0*Z1 X2*X3
            TICK
            MPP Z0*Z1 X2*X3
            ",
            false,
        ),
        3
    );
}

#[test]
fn count_determined_measurements_matches_basis_measurement_subset() {
    for (input, expected) in [
        ("", 0),
        ("RX 0\nMX 0", 1),
        ("RX 0\nMRX 0", 1),
        ("RZ 0\nMX 0", 0),
        ("RZ 0\nMRX 0", 0),
        ("RY 0\nMY 0", 1),
        ("RY 0\nMRY 0", 1),
        ("RX 0\nMY 0", 0),
        ("RX 0\nMRY 0", 0),
        ("RZ 0\nMZ 0", 1),
        ("RZ 0\nMRZ 0", 1),
        ("RX 0\nMZ 0", 0),
        ("RX 0\nMRZ 0", 0),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_matches_pair_and_mpp_subset() {
    for (input, expected) in [
        ("RX 0 1\nMXX 0 1", 1),
        ("RY 0 1\nMXX 0 1", 0),
        ("RY 0 1\nMYY 0 1", 1),
        ("RX 0 1\nMYY 0 1", 0),
        ("RZ 0 1\nMZZ 0 1", 1),
        ("RY 0 1\nMZZ 0 1", 0),
        ("RX 0\nMPP X0", 1),
        ("RY 0\nMPP X0", 0),
        ("RY 0\nMPP Y0", 1),
        ("RX 0\nMPP Y0", 0),
        ("RZ 0\nMPP Z0", 1),
        ("RX 0\nMPP Z0", 0),
        ("RX 0\nRY 1\nRZ 2\nMPP X0*Y1*Z2", 1),
        ("RX 0\nRX 1\nRZ 2\nMPP X0*Y1*Z2", 0),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_matches_convergence_subset() {
    for (input, expected) in [
        ("MX 0 0", 1),
        ("MY 0 0", 1),
        ("RX 0\nMZ 0 0", 1),
        ("MRX 0 0", 1),
        ("MRY 0 0", 1),
        ("RX 0\nMRZ 0 0", 1),
        ("MXX 0 1 0 1", 1),
        ("MYY 0 1 0 1", 1),
        ("RX 0 1\nMZZ 0 1 0 1", 1),
        ("MXX 0 1\nMYY 0 1", 1),
        ("MPP X0*X1 Y0*Y1", 1),
        ("MPP X0*X1 X1*X2 !X0*X2", 1),
        ("REPEAT 3 {\nMPP X0*X1\n}", 2),
        ("MXX 0 1\nMX 0 1", 1),
        ("MYY 0 1\nMY 0 1", 1),
        ("RX 0 1\nMZZ 0 1\nMZ 0 1", 1),
        ("MPAD 1 0 1 0", 4),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}
