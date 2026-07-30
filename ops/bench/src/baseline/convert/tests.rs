#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::{
    CONVERT_01_128, CONVERT_01_TO_B8_EXPECTED, CONVERT_B8_TO_01_EXPECTED,
    CONVERT_B8_TO_B8_WIDE_EXPECTED, CONVERT_B8_TO_DETS_EXPECTED, CONVERT_CIRCUIT_DL_OBS_EXPECTED,
    CONVERT_CIRCUIT_DL_PRIMARY_EXPECTED, CONVERT_DEM_DETS_TO_01_EXPECTED,
    CONVERT_DETS_TO_B8_EXPECTED, CONVERT_PTB64_TO_01_EXPECTED, ConvertExpectedOutput,
    ConvertOutput, ConvertWitnessExpectation, M9_MEASUREMENTS_TO_DETS_EXPECTED,
    ensure_convert_witness, reference_ptb64_from_01,
};
use crate::baseline::batch_sinks::OutputWitness;

#[test]
fn frozen_convert_witnesses_reject_same_length_wrong_content() {
    for (row_id, expected) in [
        ("m7-convert-01-to-b8", CONVERT_01_TO_B8_EXPECTED),
        ("m7-convert-b8-to-01", CONVERT_B8_TO_01_EXPECTED),
        ("m7-convert-b8-to-b8-wide", CONVERT_B8_TO_B8_WIDE_EXPECTED),
        ("m7-convert-dets-to-b8", CONVERT_DETS_TO_B8_EXPECTED),
        ("m7-convert-b8-to-dets", CONVERT_B8_TO_DETS_EXPECTED),
        ("m7-convert-ptb64-to-01", CONVERT_PTB64_TO_01_EXPECTED),
        ("m7-convert-dem-dets-to-01", CONVERT_DEM_DETS_TO_01_EXPECTED),
        (
            "m9-convert-measurements-dets",
            M9_MEASUREMENTS_TO_DETS_EXPECTED,
        ),
    ] {
        let actual = ConvertOutput {
            primary: vec![0_u8; expected.bytes],
            side: None,
        };
        let expectation = ConvertWitnessExpectation::primary_witness(expected);
        let error = ensure_convert_witness(row_id, expectation, actual)
            .expect_err("same-length wrong content must be rejected");
        assert!(error.to_string().contains("pinned Stim semantic witness"));
    }
}

#[test]
fn convert_side_output_witness_rejects_primary_and_observable_mutations() {
    let mut wrong_primary = vec![0_u8; CONVERT_CIRCUIT_DL_PRIMARY_EXPECTED.bytes];
    *wrong_primary.first_mut().expect("primary witness byte") = b's';
    let correct_obs = vec![0_u8; CONVERT_CIRCUIT_DL_OBS_EXPECTED.bytes];
    let expectation = ConvertWitnessExpectation {
        primary: ConvertExpectedOutput::Witness(OutputWitness::from_bytes(&wrong_primary)),
        side: Some(ConvertExpectedOutput::Witness(
            CONVERT_CIRCUIT_DL_OBS_EXPECTED,
        )),
    };
    let actual = ConvertOutput {
        primary: vec![0_u8; CONVERT_CIRCUIT_DL_PRIMARY_EXPECTED.bytes],
        side: Some(correct_obs),
    };
    let error = ensure_convert_witness("m7-convert-circuit-dl-obs-out", expectation, actual)
        .expect_err("primary mutation must be rejected");
    assert!(error.to_string().contains("primary"));

    let correct_primary = vec![0_u8; CONVERT_CIRCUIT_DL_PRIMARY_EXPECTED.bytes];
    let mut wrong_obs = vec![0_u8; CONVERT_CIRCUIT_DL_OBS_EXPECTED.bytes];
    *wrong_obs.first_mut().expect("observable witness byte") = b'o';
    let expectation = ConvertWitnessExpectation {
        primary: ConvertExpectedOutput::Witness(OutputWitness::from_bytes(&correct_primary)),
        side: Some(ConvertExpectedOutput::Witness(OutputWitness::from_bytes(
            &wrong_obs,
        ))),
    };
    let actual = ConvertOutput {
        primary: correct_primary,
        side: Some(vec![0_u8; CONVERT_CIRCUIT_DL_OBS_EXPECTED.bytes]),
    };
    let error = ensure_convert_witness("m7-convert-circuit-dl-obs-out", expectation, actual)
        .expect_err("observable mutation must be rejected");
    assert!(error.to_string().contains("observable"));
}

#[test]
fn independent_ptb64_reference_has_frozen_fixture_witness() {
    let expected =
        reference_ptb64_from_01(CONVERT_01_128, 128).expect("derive reference ptb64 bytes");
    assert_eq!(
        OutputWitness::from_bytes(&expected),
        OutputWitness::new(65_536, 0xdd99_b80c_f77c_a325)
    );
}

#[test]
fn independent_ptb64_reference_preserves_bit_and_shot_orientation() {
    let mut input = Vec::new();
    for shot in 0..64 {
        input.extend_from_slice(match shot {
            0 => b"10\n",
            1 => b"01\n",
            _ => b"00\n",
        });
    }

    let actual = reference_ptb64_from_01(&input, 2).expect("pack hand-computable records");
    let mut expected = Vec::new();
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.extend_from_slice(&2_u64.to_le_bytes());
    assert_eq!(actual, expected);
}

#[test]
fn independent_ptb64_reference_rejects_unterminated_or_partial_groups() {
    assert!(reference_ptb64_from_01(b"0", 1).is_err());
    assert!(reference_ptb64_from_01(b"0\n", 1).is_err());
}

#[test]
fn independent_ptb64_reference_rejects_same_length_wrong_content() {
    let expected =
        reference_ptb64_from_01(CONVERT_01_128, 128).expect("derive reference ptb64 bytes");
    let mut actual = expected.clone();
    *actual.first_mut().expect("ptb64 witness byte") ^= 1;
    assert_eq!(actual.len(), expected.len());

    let error = ensure_convert_witness(
        "m7-convert-01-to-ptb64",
        ConvertWitnessExpectation::primary_bytes(expected),
        ConvertOutput {
            primary: actual,
            side: None,
        },
    )
    .expect_err("same-length wrong ptb64 content must be rejected");
    assert!(error.to_string().contains("independent semantic reference"));
}
