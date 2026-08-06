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

#[test]
fn repeat_headers_accept_and_drop_parenthesized_arguments_like_stim() {
    // Pinned Stim lexes parens arguments for block gates and then discards
    // them (circuit.cc:213-218), probed against the v1.16.0 binary for each
    // spelling here.
    for (text, canonical) in [
        ("REPEAT(0.5) 3 {\nM 0\n}\n", "REPEAT 3 {\n    M 0\n}\n"),
        ("REPEAT() 2 {\nM 0\n}\n", "REPEAT 2 {\n    M 0\n}\n"),
        ("REPEAT(-1) 2 {\nM 0\n}\n", "REPEAT 2 {\n    M 0\n}\n"),
        (
            "REPEAT[t](1,2) 2 {\nM 0\n}\n",
            "REPEAT[t] 2 {\n    M 0\n}\n",
        ),
    ] {
        let circuit = Circuit::from_stim_str(text).expect("Stim-legal repeat header parses");
        assert_eq!(circuit.to_stim_string(), canonical, "{text:?}");
    }

    for rejected in ["REPEAT(abc) 2 {\nM 0\n}\n", "REPEAT(0.5)3 {\nM 0\n}\n"] {
        Circuit::from_stim_str(rejected)
            .expect_err("malformed repeat argument spellings must reject like Stim");
    }
}

#[test]
fn correlated_error_decorations_parse_and_reprint_like_stim() {
    // Pinned Stim consults only the Pauli X/Z bits of E and
    // ELSE_CORRELATED_ERROR targets, so combiners and inversion bits are
    // accepted decoration, and write_targets reprints them as stored
    // (gate_target.cc:214-226); probed per spelling against v1.16.0.
    for (text, canonical) in [
        ("E(0.1) X0*X1\n", "E(0.1) X0*X1\n"),
        ("E(0.1) X0 * X1\n", "E(0.1) X0*X1\n"),
        ("E(0.1) !X0\n", "E(0.1) !X0\n"),
        ("E(0.1) *X0\n", "E(0.1)*X0\n"),
        ("E(0.1)*X0\n", "E(0.1)*X0\n"),
        ("E(0.1) X0*\n", "E(0.1) X0*\n"),
        ("E(0.1) X0**X1\n", "E(0.1) X0**X1\n"),
        ("E(0.1) *\n", "E(0.1)*\n"),
        (
            "E(0.3) X0\nELSE_CORRELATED_ERROR(0.2) !Z0 * Z1\n",
            "E(0.3) X0\nELSE_CORRELATED_ERROR(0.2) !Z0*Z1\n",
        ),
    ] {
        let circuit = Circuit::from_stim_str(text).expect("decorated correlated error parses");
        assert_eq!(circuit.to_stim_string(), canonical, "{text:?}");
        let reparsed = Circuit::from_stim_str(&circuit.to_stim_string())
            .expect("printed decorated form must reparse");
        assert_eq!(reparsed, circuit, "{text:?}");
    }

    for rejected in ["X_ERROR(0.1) X0\n", "M*0\n", "MPP*Z0\n", "E(0.1) 0\n"] {
        Circuit::from_stim_str(rejected)
            .expect_err("decoration tolerance must stay scoped to correlated errors");
    }
}
