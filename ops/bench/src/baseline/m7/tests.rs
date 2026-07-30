#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::{
    CONVERT_STIM_CANONICAL_EXPECTED, LEGACY_DISPATCH_EXPECTED, ensure_exact_bytes,
    ensure_legacy_dispatch_witness,
};
use crate::baseline::batch_sinks::OutputWitness;

#[test]
fn legacy_dispatch_rejects_same_width_wrong_content() {
    let wrong = vec![0_u8; LEGACY_DISPATCH_EXPECTED.bytes];
    let actual = OutputWitness::from_bytes(&wrong);
    assert_eq!(actual.bytes, LEGACY_DISPATCH_EXPECTED.bytes);
    assert_ne!(actual.digest, LEGACY_DISPATCH_EXPECTED.digest);

    let error = ensure_legacy_dispatch_witness("pf7-cli-legacy-dispatch-startup", actual)
        .expect_err("same-width output with the wrong circuit must be rejected");
    assert!(error.to_string().contains("pinned Stim v1.16.0"));
}

#[test]
fn canonical_stim_convert_rejects_same_length_wrong_content() {
    let mut actual = CONVERT_STIM_CANONICAL_EXPECTED.to_vec();
    *actual.first_mut().expect("canonical witness byte") ^= 1;
    assert_eq!(actual.len(), CONVERT_STIM_CANONICAL_EXPECTED.len());

    let error = ensure_exact_bytes(
        "m7-convert-stim-canonical",
        "canonical .stim conversion",
        CONVERT_STIM_CANONICAL_EXPECTED,
        &actual,
    )
    .expect_err("same-length canonical circuit mutation must be rejected");
    assert!(error.to_string().contains("byte output"));
}

#[test]
fn generator_exact_byte_witness_rejects_same_length_wrong_content() {
    let expected = b"# Generated repetition_code circuit.\nM 0\n";
    let mut actual = expected.to_vec();
    let target = actual
        .get_mut(expected.len() - 2)
        .expect("generator witness interior byte");
    *target = b'1';
    assert_eq!(actual.len(), expected.len());

    let error = ensure_exact_bytes(
        "m7-gen-repetition-d3-r3",
        "generator CLI output",
        expected,
        &actual,
    )
    .expect_err("same-length generator output mutation must be rejected");
    assert!(error.to_string().contains("pinned Stim v1.16.0"));
}
