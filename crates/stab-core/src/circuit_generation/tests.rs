use crate::{Circuit, CircuitResult, CodeDistance, Probability, RoundCount};

use super::{
    CircuitGenParams, append_begin_round_tick, append_measure, append_measure_reset, append_reset,
    append_unitary_1, append_unitary_2,
};

fn base_params() -> CircuitResult<CircuitGenParams> {
    CircuitGenParams::new(RoundCount::try_new(3)?, CodeDistance::try_new(5)?)
}

fn assert_circuit_text(circuit: &Circuit, expected: &str) {
    // Stab compares canonical printer output, so adjacent compatible instructions are fused.
    assert_eq!(circuit.to_stim_string(), expected);
}

#[test]
fn append_begin_round_tick_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_begin_round_tick(&params, &mut circuit, &[1, 2, 3])?;
    assert_circuit_text(&circuit, "TICK\n");

    let mut circuit = Circuit::new();
    let params = params.with_before_round_data_depolarization(Probability::try_new(0.125)?);
    append_begin_round_tick(&params, &mut circuit, &[1, 2, 3])?;
    assert_circuit_text(&circuit, "TICK\nDEPOLARIZE1(0.125) 1 2 3\n");

    Ok(())
}

#[test]
fn append_unitary_1_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_unitary_1(&params, &mut circuit, "H", &[2, 3, 5])?;
    assert_circuit_text(&circuit, "H 2 3 5\n");

    let mut circuit = Circuit::new();
    let params = params.with_after_clifford_depolarization(Probability::try_new(0.125)?);
    append_unitary_1(&params, &mut circuit, "H", &[2, 3, 5])?;
    assert_circuit_text(&circuit, "H 2 3 5\nDEPOLARIZE1(0.125) 2 3 5\n");

    Ok(())
}

#[test]
fn append_unitary_2_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_unitary_2(&params, &mut circuit, "CNOT", &[2, 3, 5, 7])?;
    assert_circuit_text(&circuit, "CX 2 3 5 7\n");

    let mut circuit = Circuit::new();
    let params = params.with_after_clifford_depolarization(Probability::try_new(0.125)?);
    append_unitary_2(&params, &mut circuit, "CNOT", &[2, 3, 5, 7])?;
    assert_circuit_text(&circuit, "CX 2 3 5 7\nDEPOLARIZE2(0.125) 2 3 5 7\n");

    Ok(())
}

#[test]
fn append_reset_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    append_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    assert_circuit_text(&circuit, "R 2 3 5 2 3 5\n");
    append_reset(&params, &mut circuit, &[1], 'X')?;
    append_reset(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(&circuit, "R 2 3 5 2 3 5\nRX 1\nRY 4\n");

    let mut circuit = Circuit::new();
    let params = params.with_after_reset_flip_probability(Probability::try_new(0.125)?);
    append_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    assert_circuit_text(&circuit, "R 2 3 5\nX_ERROR(0.125) 2 3 5\n");
    append_reset(&params, &mut circuit, &[1], 'X')?;
    append_reset(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(
        &circuit,
        concat!(
            "R 2 3 5\n",
            "X_ERROR(0.125) 2 3 5\n",
            "RX 1\n",
            "Z_ERROR(0.125) 1\n",
            "RY 4\n",
            "X_ERROR(0.125) 4\n",
        ),
    );

    Ok(())
}

#[test]
fn append_measure_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_measure(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    append_measure(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    assert_circuit_text(&circuit, "M 2 3 5 2 3 5\n");
    append_measure(&params, &mut circuit, &[1], 'X')?;
    append_measure(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(&circuit, "M 2 3 5 2 3 5\nMX 1\nMY 4\n");

    let mut circuit = Circuit::new();
    let params = params.with_before_measure_flip_probability(Probability::try_new(0.125)?);
    append_measure(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    assert_circuit_text(&circuit, "X_ERROR(0.125) 2 3 5\nM 2 3 5\n");
    append_measure(&params, &mut circuit, &[1], 'X')?;
    append_measure(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(
        &circuit,
        concat!(
            "X_ERROR(0.125) 2 3 5\n",
            "M 2 3 5\n",
            "Z_ERROR(0.125) 1\n",
            "MX 1\n",
            "X_ERROR(0.125) 4\n",
            "MY 4\n",
        ),
    );

    Ok(())
}

#[test]
fn append_measure_reset_matches_stim() -> CircuitResult<()> {
    // Adapted from Stim v1.16.0 src/stim/gen/circuit_gen_params.test.cc.
    let mut circuit = Circuit::new();
    let params = base_params()?;
    append_measure_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    append_measure_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    assert_circuit_text(&circuit, "MR 2 3 5 2 3 5\n");
    append_measure_reset(&params, &mut circuit, &[1], 'X')?;
    append_measure_reset(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(&circuit, "MR 2 3 5 2 3 5\nMRX 1\nMRY 4\n");

    let mut circuit = Circuit::new();
    let params = params
        .with_before_measure_flip_probability(Probability::try_new(0.125)?)
        .with_after_reset_flip_probability(Probability::try_new(0.25)?);
    append_measure_reset(&params, &mut circuit, &[2, 3, 5], 'Z')?;
    append_measure_reset(&params, &mut circuit, &[1], 'X')?;
    append_measure_reset(&params, &mut circuit, &[4], 'Y')?;
    assert_circuit_text(
        &circuit,
        concat!(
            "X_ERROR(0.125) 2 3 5\n",
            "MR 2 3 5\n",
            "X_ERROR(0.25) 2 3 5\n",
            "Z_ERROR(0.125) 1\n",
            "MRX 1\n",
            "Z_ERROR(0.25) 1\n",
            "X_ERROR(0.125) 4\n",
            "MRY 4\n",
            "X_ERROR(0.25) 4\n",
        ),
    );

    Ok(())
}
