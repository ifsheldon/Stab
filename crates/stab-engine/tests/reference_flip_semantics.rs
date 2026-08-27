#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "integration tests use deterministic valid circuits, exact assertions, and record indexes bounded by the asserted record count"
)]

//! Regression pins for Stim v1.16.0's noiseless reference-sample contract.
//!
//! Pinned Stim builds its reference sample from `aliased_noiseless_circuit`, which drops
//! result-flip probabilities before the reference run, so a `p == 1` measurement flip inverts
//! every sampled shot and fires detect/m2d detectors on every shot instead of never.
//! These tests fail against the pre-fix reference semantics that applied the flip
//! (docs/plans/post-review-remediation-plan.md, WS1).

mod support;

use stab_engine::ReferenceSampleMode;
use stab_model::Circuit;

use support::{SamplingFixture, convert_detection_records, sample_detections};

fn parse(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("test circuit should parse")
}

#[test]
fn measurement_flip_probabilities_never_reach_the_reference_sample() {
    for text in [
        "M(1) 0\n",
        "M(0.5) 0\n",
        "M(0) 0\n",
        "MR(1) 0\n",
        "MX(1) 0\n",
        "MPP(1) Z0\n",
    ] {
        let sampler = SamplingFixture::compile(&parse(text)).expect("compile sampler");
        assert_eq!(
            sampler.reference_sample().expect("reference sample"),
            vec![false],
            "{text:?}"
        );
    }

    // Static target inversion belongs to the noiseless circuit and stays in the reference.
    let inverted = SamplingFixture::compile(&parse("M(1) !0\n")).expect("compile sampler");
    assert_eq!(
        inverted.reference_sample().expect("reference sample"),
        vec![true]
    );
}

#[test]
fn certain_measurement_flips_fire_detect_detectors_on_every_shot() {
    // MR keeps this off the direct-Z fast path, whose reference bit was already noiseless, so
    // this pin discriminates the general-path regression; M covers the fast path itself.
    for text in ["M(1) 0\nDETECTOR rec[-1]\n", "MR(1) 0\nDETECTOR rec[-1]\n"] {
        let output = sample_detections(&parse(text), 64, Some(3)).expect("detect");
        assert_eq!(output.records.len(), 64);
        assert!(
            output
                .records
                .iter()
                .all(|record| record.detectors == vec![true]),
            "pinned Stim fires this detector on every shot: {text:?}"
        );
    }
}

#[test]
fn certain_measurement_flips_convert_like_pinned_stim_through_m2d() {
    for text in ["M(1) 0\nDETECTOR rec[-1]\n", "MR(1) 0\nDETECTOR rec[-1]\n"] {
        let circuit = parse(text);

        // A sampled shot of this circuit always records 1; against the noiseless reference (0)
        // the converted detector fires. A hypothetical 0 measurement must not fire it.
        let converted = convert_detection_records(
            &circuit,
            &[vec![true], vec![false]],
            None,
            ReferenceSampleMode::UseReferenceSample,
        )
        .expect("convert measurements");
        assert_eq!(converted.records[0].detectors, vec![true], "{text:?}");
        assert_eq!(converted.records[1].detectors, vec![false], "{text:?}");

        // Skipping the reference treats it as all-false and must agree here.
        let skipped = convert_detection_records(
            &circuit,
            &[vec![true]],
            None,
            ReferenceSampleMode::SkipReferenceSample,
        )
        .expect("convert measurements without reference");
        assert_eq!(skipped.records[0].detectors, vec![true], "{text:?}");
    }
}
