#![allow(
    clippy::expect_used,
    reason = "grammar remediation regressions extract required failure payloads directly"
)]

//! Regression pins for the WS3 `.stim` grammar repairs
//! (docs/plans/post-review-remediation-plan.md).
//!
//! Pinned Stim v1.16.0 rejects a braceless `REPEAT` at parse time ("Missing '{' at start of
//! REPEAT block", circuit.cc:218), tokenizes `*` as a combiner whether attached or spaced, and
//! rejects misplaced combiners at gate validation ("combiners ('*') that aren't between other
//! targets"). These tests fail against the pre-remediation grammar.

use stab_model::{Circuit, CircuitInstruction, Gate};

#[test]
fn bare_repeat_headers_are_rejected_with_missing_brace_diagnostics() {
    for text in ["REPEAT\n", "REPEAT[tag]\n", "H 0\nREPEAT\nM 0\n"] {
        let error = Circuit::from_stim_str(text).expect_err("braceless REPEAT must not parse");
        assert!(
            error.to_string().contains("missing '{' at start of REPEAT"),
            "{text:?}: {error}"
        );
    }
}

#[test]
fn block_only_gates_cannot_become_instructions() {
    let repeat = Gate::from_name("REPEAT").expect("REPEAT resolves for name lookup");
    let error = CircuitInstruction::new(repeat, Vec::new(), Vec::new(), None)
        .expect_err("control-flow gates must never validate as instructions");
    assert!(error.to_string().contains("instruction gate"), "{error}");
}

#[test]
fn stim_legal_combiner_spacings_parse_to_the_attached_form() {
    let attached = Circuit::from_stim_str("R 0 1\nMPP Z0*Z1\n").expect("attached form parses");
    for spaced in [
        "R 0 1\nMPP Z0 *Z1\n",
        "R 0 1\nMPP Z0* Z1\n",
        "R 0 1\nMPP Z0 * Z1\n",
    ] {
        let circuit = Circuit::from_stim_str(spaced).expect("Stim-legal spacing parses");
        assert_eq!(circuit, attached, "{spaced:?}");
    }
}

#[test]
fn misplaced_combiners_are_rejected_like_stim() {
    for text in ["MPP *Z0\n", "MPP Z0*\n", "MPP Z0**Z1\n", "MPP *\n"] {
        Circuit::from_stim_str(text).expect_err("misplaced combiners must not validate");
    }
}
