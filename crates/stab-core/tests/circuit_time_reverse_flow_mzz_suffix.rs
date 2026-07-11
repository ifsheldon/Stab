use std::fmt::Debug;
use std::str::FromStr;

use stab_core::{Circuit, Flow, circuit_time_reversed_for_flows};

fn circuit(text: &str) -> Result<Circuit, String> {
    Circuit::from_stim_str(text).map_err(|error| error.to_string())
}

fn flow(text: &str) -> Result<Flow, String> {
    Flow::from_str(text).map_err(|error| error.to_string())
}

fn require_eq<T: Debug + PartialEq>(actual: &T, expected: &T, context: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{context}\nactual: {actual:?}\nexpected: {expected:?}"
        ))
    }
}

fn require_contains(haystack: &str, needle: &str, context: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("{context}\nmissing: {needle}\nactual: {haystack}"))
    }
}

#[test]
fn mzz_unitary_suffix_matches_pinned_stim_flow_through_h_cx_s() -> Result<(), String> {
    // Adapted from Stim v1.16.0 circuit_inverse_qec flow_through_mzz_h_cx_s coverage.
    let input = circuit(
        "
        MZZ 0 1
        H 0
        CX 0 1
        S 1
    ",
    )?;
    let flows = [
        flow("X0*X1 -> X0*Z1 xor rec[-1]")?,
        flow("X0*X1 -> Z0*Y1")?,
        flow("Z0 -> Z0*Z1 xor rec[-1]")?,
        flow("Z0 -> X0*Y1")?,
    ];

    let (actual_circuit, actual_flows) =
        circuit_time_reversed_for_flows(&input, &flows).map_err(|error| error.to_string())?;

    require_eq(
        &actual_circuit,
        &circuit(
            "
        S_DAG 1
        CX 0 1
        H 0
        MZZ 0 1
    ",
        )?,
        "reversed circuit",
    )?;
    require_eq(
        &actual_flows,
        &vec![
            flow("X0*Z1 -> X0*X1 xor rec[-1]")?,
            flow("Z0*Y1 -> X0*X1")?,
            flow("Z0*Z1 -> Z0 xor rec[-1]")?,
            flow("X0*Y1 -> Z0")?,
        ],
        "reversed flows",
    )
}

#[test]
fn mzz_unitary_suffix_rejects_unsatisfied_flows() -> Result<(), String> {
    let input = circuit(
        "
        MZZ 0 1
        H 0
        CX 0 1
        S 1
    ",
    )?;
    let wrong_flow = flow("X0*X1 -> X0*X1 xor rec[-1]")?;
    let error = match circuit_time_reversed_for_flows(&input, &[wrong_flow]) {
        Ok(_) => return Err("wrong suffix output unexpectedly satisfied".to_owned()),
        Err(error) => error.to_string(),
    };

    require_contains(&error, "anti-commuted", "unsatisfied flow error")
}

#[test]
fn mzz_unitary_suffix_ignores_input_observable_terms_like_stim() -> Result<(), String> {
    let input = circuit(
        "
        MZZ 0 1
        H 0
        CX 0 1
        S 1
    ",
    )?;
    let observable_flow = flow("X0*X1 -> X0*Z1 xor rec[-1] xor obs[0]")?;
    let (inverse, flows) = circuit_time_reversed_for_flows(&input, &[observable_flow])
        .map_err(|error| error.to_string())?;

    require_eq(
        &inverse,
        &circuit("S_DAG 1\nCX 0 1\nH 0\nMZZ 0 1\n")?,
        "observable-bearing reversed circuit",
    )?;
    require_eq(
        &flows,
        &vec![flow("X0*Z1 -> X0*X1 xor rec[-1]")?],
        "observable-bearing reversed flow",
    )
}

#[test]
fn mzz_unitary_suffix_rejects_feedback_and_duplicate_targets() -> Result<(), String> {
    for (circuit_text, flow_text, expected) in [
        (
            "MZZ 0 1\nCX rec[-1] 0\n",
            "Z0 -> Z0 xor rec[-1]",
            "feedback",
        ),
        (
            "MZZ 0 1 1 2\nH 0\n",
            "1 -> Z0*Z1 xor rec[-2]",
            "duplicate target qubit",
        ),
    ] {
        let error =
            match circuit_time_reversed_for_flows(&circuit(circuit_text)?, &[flow(flow_text)?]) {
                Ok(_) => return Err(format!("unsupported MZZ shape succeeded: {circuit_text}")),
                Err(error) => error.to_string(),
            };
        require_contains(&error, expected, circuit_text)?;
    }
    Ok(())
}
