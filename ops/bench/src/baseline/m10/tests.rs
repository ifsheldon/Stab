#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::{
    PF7_ANALYZE_DECOMPOSE_EXPECTED, analyzer_semantic_witness, ensure_analyzer_semantic_witness,
};

#[test]
fn analyzer_witness_rejects_same_width_wrong_content() {
    let actual = analyzer_semantic_witness("pf7-cli-analyze-errors-decompose", b"detector D1\n")
        .expect("derive mutated analyzer witness");
    assert_eq!(actual.bytes, PF7_ANALYZE_DECOMPOSE_EXPECTED.bytes);

    ensure_analyzer_semantic_witness(
        "pf7-cli-analyze-errors-decompose",
        PF7_ANALYZE_DECOMPOSE_EXPECTED,
        &actual,
    )
    .expect_err("same-width output with a different detector must be rejected");
}

#[test]
fn analyzer_witness_normalizes_only_insignificant_probability_printing() {
    let pinned = analyzer_semantic_witness(
        "pinned",
        b"error(0.002529202133333148701244130762688656) D0 D8\n",
    )
    .expect("derive pinned witness");
    let stab = analyzer_semantic_witness(
        "stab",
        b"error(0.002529202133333149134924999756890429) D0 D8\n",
    )
    .expect("derive Stab witness");
    let changed = analyzer_semantic_witness(
        "changed",
        b"error(0.102529202133333149134924999756890429) D0 D8\n",
    )
    .expect("derive materially changed witness");

    assert_eq!(pinned.digest, stab.digest);
    assert_ne!(pinned.digest, changed.digest);
}
