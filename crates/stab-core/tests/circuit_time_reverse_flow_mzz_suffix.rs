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

    require_contains(
        &error,
        "requires selected measurement-rich circuit to satisfy flow 0",
        "unsatisfied flow error",
    )
}

#[test]
fn mzz_unitary_suffix_rejects_observable_terms() -> Result<(), String> {
    let input = circuit(
        "
        MZZ 0 1
        H 0
        CX 0 1
        S 1
    ",
    )?;
    let observable_flow = flow("X0*X1 -> X0*Z1 xor rec[-1] xor obs[0]")?;
    let error = match circuit_time_reversed_for_flows(&input, &[observable_flow]) {
        Ok(_) => return Err("observable flow unexpectedly reversed".to_owned()),
        Err(error) => error.to_string(),
    };

    require_contains(
        &error,
        "does not support observable terms in selected flow 0",
        "observable flow error",
    )
}

#[test]
fn mzz_unitary_suffix_rejects_unscoped_shapes() -> Result<(), String> {
    for circuit_text in [
        "MZZ(0.125) 0 1\nH 0\n",
        "MZZ 0 1 2 3\nH 0\n",
        "MZZ 0 1\nCX rec[-1] 0\n",
        "MZZ 0 1\nDETECTOR rec[-1]\n",
        "MZZ 0 1\nX_ERROR(0.125) 0\n",
        "MZZ 0 1\nREPEAT 2 {\n    H 0\n}\n",
    ] {
        let input = circuit(circuit_text)?;
        let flow = flow("Z0 -> Z0 xor rec[-1]")?;
        let error = match circuit_time_reversed_for_flows(&input, &[flow]) {
            Ok(_) => {
                return Err(format!(
                    "unscoped MZZ suffix shape succeeded: {circuit_text}"
                ));
            }
            Err(error) => error.to_string(),
        };

        require_contains(
            &error,
            "one noiseless plain MZZ group followed by plain-qubit unitary",
            circuit_text,
        )?;
    }
    Ok(())
}
