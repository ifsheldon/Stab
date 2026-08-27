#![allow(
    clippy::expect_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use stab_analysis::circuit_without_tags;
use stab_model::Circuit;

#[test]
fn cq2_circuit_api_without_tags_is_recursive_and_non_mutating() {
    let original = Circuit::from_stim_str(
        "H[top] 0\nREPEAT[loop] 2 {\n    M[measure](0.125) 0\n    DETECTOR[det] rec[-1]\n}\n",
    )
    .expect("parse tagged circuit");
    let stripped = circuit_without_tags(&original);

    assert_eq!(
        stripped.to_string(),
        "H 0\nREPEAT 2 {\n    M(0.125) 0\n    DETECTOR rec[-1]\n}\n"
    );
    assert_eq!(circuit_without_tags(&original), stripped);
    assert!(original.to_string().contains("[loop]"));
    for tag in ["[top]", "[loop]", "[measure]", "[det]"] {
        assert!(!stripped.to_string().contains(tag));
    }

    let distinct_boundaries =
        Circuit::from_stim_str("H[first] 0\nH[second] 1\n").expect("parse tagged boundaries");
    assert_eq!(
        circuit_without_tags(&distinct_boundaries).to_string(),
        "H 0\nH 1\n",
        "removing tags must not fuse previously distinct instructions"
    );
}
