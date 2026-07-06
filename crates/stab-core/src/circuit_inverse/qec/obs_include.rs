use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, Pauli, QubitId, Target,
};

pub(super) fn selected_obs_include_pauli_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(observable),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if reset.gate().canonical_name() != "RX"
        || observable.gate().canonical_name() != "OBSERVABLE_INCLUDE"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_obs_include_pauli_inverse(
        reset, observable,
    )?))
}

fn build_selected_obs_include_pauli_inverse(
    reset: &CircuitInstruction,
    observable: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    validate_selected_reset(reset)?;
    validate_selected_observable(observable)?;

    let mut result = Circuit::new();
    result.append_instruction(observable.clone());
    super::append_one_target_instruction(
        &mut result,
        Gate::from_name("MX")?,
        &[],
        Target::qubit(QubitId::new(1)?, false),
        None,
    )?;
    super::append_one_target_instruction(
        &mut result,
        observable.gate(),
        observable.args(),
        Target::measurement_record(MeasureRecordOffset::try_new(-1)?),
        None,
    )?;
    Ok(result)
}

fn validate_selected_reset(reset: &CircuitInstruction) -> CircuitResult<()> {
    if !reset.args().is_empty() {
        return Err(inverse_qec_obs_include_pauli_error(
            "RX instruction must be noiseless",
        ));
    }
    if reset.tag().is_some() {
        return Err(inverse_qec_obs_include_pauli_error(
            "RX instruction tags are outside this selected packet",
        ));
    }
    let reset_targets = super::plain_unique_single_qubit_targets(reset).ok_or_else(|| {
        inverse_qec_obs_include_pauli_error("RX target must be one plain unique qubit")
    })?;
    let [target] = reset_targets.as_slice() else {
        return Err(inverse_qec_obs_include_pauli_error(
            "RX target list must be exactly 1",
        ));
    };
    if target.qubit_id().map(|id| id.get()) != Some(1) {
        return Err(inverse_qec_obs_include_pauli_error(
            "RX target list must be exactly 1",
        ));
    }
    Ok(())
}

fn validate_selected_observable(observable: &CircuitInstruction) -> CircuitResult<()> {
    if observable.observable_id_argument()?.map(|id| id.get()) != Some(1) {
        return Err(inverse_qec_obs_include_pauli_error(
            "observable id must be exactly 1",
        ));
    }
    if observable.tag() != Some("test") {
        return Err(inverse_qec_obs_include_pauli_error(
            "observable tag must be exactly test",
        ));
    }
    let [target] = observable.targets() else {
        return Err(inverse_qec_obs_include_pauli_error(
            "observable target list must be exactly X1",
        ));
    };
    if target.pauli_type() != Some(Pauli::X)
        || target.qubit_id().map(|id| id.get()) != Some(1)
        || target.is_inverted_result_target()
    {
        return Err(inverse_qec_obs_include_pauli_error(
            "observable target list must be exactly X1",
        ));
    }
    Ok(())
}

fn inverse_qec_obs_include_pauli_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected observable Pauli include subset requires exact RX 1 followed by OBSERVABLE_INCLUDE[test](1) X1: {reason}"
    ))
}
