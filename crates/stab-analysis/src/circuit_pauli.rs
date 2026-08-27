use stab_algebra::{PauliBasis, PauliString, Tableau};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, GateCategory, Pauli, QubitId, Target,
    advanced::{ControlledPauliTargetPair, classify_controlled_pauli_target_pair},
};

use crate::{
    AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation,
    circuit_to_tableau,
};

const MAX_NONUNITARY_PAULI_CONJUGATION_INSTRUCTIONS: u64 = 1_000_000;

/// Returns the Pauli observable after the circuit, when its value remains well defined.
///
/// Pure Clifford circuits, including compact repeats, use one tableau action. Measurement and
/// reset circuits follow Stim's conditional observable rules under an explicit expanded-work cap.
pub fn pauli_after_circuit(pauli: &PauliString, circuit: &Circuit) -> AnalysisResult<PauliString> {
    validate_compact_pauli_circuit(pauli.len(), circuit)?;
    if let Ok(tableau) = circuit_to_tableau(circuit, false, false, false) {
        return apply_tableau_at_width(pauli, &tableau, false);
    }

    admit_nonunitary_expanded_work(circuit)?;
    let mut result = pauli.clone();
    for instruction in circuit.iter_flattened_instructions() {
        apply_instruction_after(&mut result, instruction)?;
    }
    Ok(result)
}

/// Returns the Pauli observable before the circuit, when its value remains well defined.
pub fn pauli_before_circuit(pauli: &PauliString, circuit: &Circuit) -> AnalysisResult<PauliString> {
    validate_compact_pauli_circuit(pauli.len(), circuit)?;
    if let Ok(tableau) = circuit_to_tableau(circuit, false, false, false) {
        return apply_tableau_at_width(pauli, &tableau, true);
    }

    admit_nonunitary_expanded_work(circuit)?;
    let mut result = pauli.clone();
    for instruction in circuit.iter_flattened_instructions_reverse() {
        apply_instruction_before(&mut result, instruction)?;
    }
    Ok(result)
}

fn validate_compact_pauli_circuit(width: usize, circuit: &Circuit) -> AnalysisResult<()> {
    let mut pending = Vec::new();
    let mut items = circuit.items().iter();
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => {
                if no_effect_on_pauli(instruction) {
                    continue;
                }
                if matches!(
                    instruction.gate().category(),
                    GateCategory::Noise | GateCategory::HeraldedNoise
                ) {
                    return Err(unsupported_instruction(instruction, "across"));
                }
                for target in instruction.targets() {
                    if let Some(qubit) = target.qubit_id()
                        && qubit.get() as usize >= width
                    {
                        return Err(AnalysisError::invalid_tableau_conversion(format!(
                            "{} targets qubit {} outside Pauli width {width}",
                            instruction.gate().canonical_name(),
                            qubit.get()
                        )));
                    }
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                pending.push(items);
                items = repeat.body().items().iter();
            }
            None => {
                let Some(parent) = pending.pop() else {
                    return Ok(());
                };
                items = parent;
            }
        }
    }
}

fn admit_nonunitary_expanded_work(circuit: &Circuit) -> AnalysisResult<()> {
    let limit = u128::from(MAX_NONUNITARY_PAULI_CONJUGATION_INSTRUCTIONS);
    let mut count = 0_u128;
    let mut multiplier = 1_u128;
    let mut pending = Vec::new();
    let mut items = circuit.items().iter();
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(_)) => {
                count = count.saturating_add(multiplier);
                if count > limit {
                    return Err(ResourceLimitError::fixed_operation(
                        ResourceOperation::PauliConjugation,
                        ResourceKind::ExpandedOperations,
                        u64::try_from(count).unwrap_or(u64::MAX),
                        MAX_NONUNITARY_PAULI_CONJUGATION_INSTRUCTIONS,
                    )
                    .into());
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                pending.push((items, multiplier));
                multiplier = multiplier.saturating_mul(u128::from(repeat.repeat_count().get()));
                items = repeat.body().items().iter();
            }
            None => {
                let Some((parent, parent_multiplier)) = pending.pop() else {
                    return Ok(());
                };
                items = parent;
                multiplier = parent_multiplier;
            }
        }
    }
}

fn apply_instruction_after(
    pauli: &mut PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    let name = instruction.gate().canonical_name();
    if no_effect_on_pauli(instruction) {
        return Ok(());
    }
    match name {
        "R" | "RX" | "RY" | "MR" | "MRX" | "MRY" => check_reset_avoided(pauli, instruction),
        "M" | "MX" | "MY" => check_measurement_commutes(pauli, instruction),
        "MPP" => check_mpp_commutes(pauli, instruction),
        _ if instruction.gate().is_unitary() => {
            apply_unitary_instruction(pauli, instruction, false)
        }
        _ => Err(unsupported_instruction(instruction, "after")),
    }
}

fn apply_instruction_before(
    pauli: &mut PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    let name = instruction.gate().canonical_name();
    if no_effect_on_pauli(instruction) {
        return Ok(());
    }
    match name {
        "R" | "RX" | "RY" | "MR" | "MRX" | "MRY" => undo_reset(pauli, instruction),
        "M" | "MX" | "MY" => check_measurement_commutes(pauli, instruction),
        "MPP" => check_mpp_commutes(pauli, instruction),
        _ if instruction.gate().is_unitary() => apply_unitary_instruction(pauli, instruction, true),
        _ => Err(unsupported_instruction(instruction, "before")),
    }
}

fn no_effect_on_pauli(instruction: &CircuitInstruction) -> bool {
    matches!(
        instruction.gate().canonical_name(),
        "DETECTOR"
            | "OBSERVABLE_INCLUDE"
            | "TICK"
            | "QUBIT_COORDS"
            | "SHIFT_COORDS"
            | "MPAD"
            | "I"
            | "II"
            | "I_ERROR"
            | "II_ERROR"
    ) || instruction.gate().category() == GateCategory::Annotation
}

fn apply_unitary_instruction(
    pauli: &mut PauliString,
    instruction: &CircuitInstruction,
    inverse: bool,
) -> AnalysisResult<()> {
    if instruction.gate().category() == GateCategory::PauliProduct {
        let decomposed = crate::advanced::decomposed_single_instruction(instruction)
            .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))?;
        if inverse {
            for piece in decomposed.iter_flattened_instructions_reverse() {
                apply_unitary_instruction(pauli, piece, true)?;
            }
        } else {
            for piece in decomposed.iter_flattened_instructions() {
                apply_unitary_instruction(pauli, piece, false)?;
            }
        }
        return Ok(());
    }

    let gate_name = instruction.gate().canonical_name();
    let local = crate::gate_tableau(instruction.gate())?;
    let local = if inverse {
        local.inverse().map_err(map_tableau_error)?
    } else {
        local
    };
    let mut output_sign = pauli.sign();
    for group in instruction.target_groups() {
        if matches!(gate_name, "CX" | "CY" | "CZ" | "XCZ" | "YCZ") {
            match classify_controlled_pauli_target_pair(instruction.gate(), group) {
                ControlledPauliTargetPair::Quantum { first, second } => {
                    apply_local_tableau_group(pauli, &local, &[first, second], &mut output_sign)?;
                }
                ControlledPauliTargetPair::Classical { target, .. } => {
                    let conditional_basis = match gate_name {
                        "CX" | "XCZ" => PauliBasis::X,
                        "CY" | "YCZ" => PauliBasis::Y,
                        "CZ" => PauliBasis::Z,
                        _ => {
                            return Err(AnalysisError::invalid_tableau_conversion(
                                "controlled-Pauli classifier received a different gate",
                            ));
                        }
                    };
                    let target = target.get() as usize;
                    if anticommutes(pauli_basis(pauli, target)?, conditional_basis) {
                        return Err(AnalysisError::invalid_tableau_conversion(format!(
                            "Pauli observable is affected by classically controlled {gate_name} on qubit {target}"
                        )));
                    }
                }
                ControlledPauliTargetPair::ClassicalNoop { .. } => {}
                ControlledPauliTargetPair::Unsupported => {
                    return Err(AnalysisError::invalid_tableau_conversion(format!(
                        "{gate_name} has an unsupported controlled-Pauli target orientation"
                    )));
                }
            }
        } else {
            let targets = group
                .iter()
                .map(|target| {
                    target.qubit_id().ok_or_else(|| {
                        AnalysisError::invalid_tableau_conversion(format!(
                            "{gate_name} target {target} is not a qubit"
                        ))
                    })
                })
                .collect::<AnalysisResult<Vec<_>>>()?;
            apply_local_tableau_group(pauli, &local, &targets, &mut output_sign)?;
        }
    }
    if output_sign != pauli.sign() {
        *pauli = pauli.with_sign(output_sign);
    }
    Ok(())
}

fn apply_local_tableau_group(
    pauli: &mut PauliString,
    tableau: &Tableau,
    targets: &[QubitId],
    output_sign: &mut stab_algebra::PauliSign,
) -> AnalysisResult<()> {
    if tableau.len() != targets.len() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "gate tableau width {} does not match {} target(s)",
            tableau.len(),
            targets.len()
        )));
    }
    for (index, target) in targets.iter().enumerate() {
        if targets
            .iter()
            .take(index)
            .any(|previous| previous == target)
        {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "gate tableau target qubit {} is repeated",
                target.get()
            )));
        }
    }
    let local_input = PauliString::from_bases(
        stab_algebra::PauliSign::Plus,
        targets
            .iter()
            .map(|target| pauli_basis(pauli, target.get() as usize))
            .collect::<AnalysisResult<Vec<_>>>()?,
    )
    .map_err(map_tableau_error)?;
    let local_output = tableau.apply(&local_input).map_err(map_tableau_error)?;
    if local_output.sign().is_negative() {
        *output_sign = toggled_sign(*output_sign);
    }
    for (index, target) in targets.iter().enumerate() {
        let basis = local_output.get(index).ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(
                "local tableau output was narrower than its target group",
            )
        })?;
        pauli
            .set(target.get() as usize, basis)
            .map_err(map_tableau_error)?;
    }
    Ok(())
}

fn apply_tableau_at_width(
    pauli: &PauliString,
    tableau: &Tableau,
    inverse: bool,
) -> AnalysisResult<PauliString> {
    let tableau = if inverse {
        tableau.inverse().map_err(map_tableau_error)?
    } else {
        tableau.clone()
    };
    if tableau.len() <= pauli.len() {
        let prefix = PauliString::from_bases(
            pauli.sign(),
            (0..tableau.len()).map(|index| pauli.get(index).unwrap_or(PauliBasis::I)),
        )
        .map_err(map_tableau_error)?;
        let transformed = tableau.apply(&prefix).map_err(map_tableau_error)?;
        return PauliString::from_bases(
            transformed.sign(),
            (0..pauli.len()).map(|index| {
                if index < transformed.len() {
                    transformed.get(index).unwrap_or(PauliBasis::I)
                } else {
                    pauli.get(index).unwrap_or(PauliBasis::I)
                }
            }),
        )
        .map_err(map_tableau_error);
    }

    let padded = PauliString::from_bases(
        pauli.sign(),
        (0..tableau.len()).map(|index| pauli.get(index).unwrap_or(PauliBasis::I)),
    )
    .map_err(map_tableau_error)?;
    let output = tableau.apply(&padded).map_err(map_tableau_error)?;
    if (pauli.len()..output.len()).any(|index| output.get(index) != Some(PauliBasis::I)) {
        return Err(AnalysisError::invalid_tableau_conversion(
            "Pauli conjugation produced support outside the input width",
        ));
    }
    PauliString::from_bases(
        output.sign(),
        (0..pauli.len()).map(|index| output.get(index).unwrap_or(PauliBasis::I)),
    )
    .map_err(map_tableau_error)
}

fn check_reset_avoided(
    pauli: &PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    for target in instruction.targets() {
        let index = target_qubit_index(instruction, target)?;
        if pauli.get(index) != Some(PauliBasis::I) {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "Pauli observable touches reset {} at qubit {index}",
                instruction.gate().canonical_name()
            )));
        }
    }
    Ok(())
}

fn undo_reset(pauli: &mut PauliString, instruction: &CircuitInstruction) -> AnalysisResult<()> {
    let measured_basis = single_result_basis(instruction)?;
    for target in instruction.targets() {
        let index = target_qubit_index(instruction, target)?;
        if anticommutes(pauli_basis(pauli, index)?, measured_basis) {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "Pauli observable anticommutes with reset {} at qubit {index}",
                instruction.gate().canonical_name()
            )));
        }
    }
    for target in instruction.targets() {
        let index = target_qubit_index(instruction, target)?;
        pauli.set(index, PauliBasis::I).map_err(map_tableau_error)?;
    }
    Ok(())
}

fn check_measurement_commutes(
    pauli: &PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    let measured_basis = single_result_basis(instruction)?;
    for target in instruction.targets() {
        let index = target_qubit_index(instruction, target)?;
        if anticommutes(pauli_basis(pauli, index)?, measured_basis) {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "Pauli observable anticommutes with measurement {} at qubit {index}",
                instruction.gate().canonical_name()
            )));
        }
    }
    Ok(())
}

fn check_mpp_commutes(pauli: &PauliString, instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let mut group_anticommutes = false;
        for target in group {
            let Target::Pauli {
                pauli: measured,
                id,
                ..
            } = target
            else {
                continue;
            };
            let basis = match measured {
                Pauli::X => PauliBasis::X,
                Pauli::Y => PauliBasis::Y,
                Pauli::Z => PauliBasis::Z,
            };
            group_anticommutes ^= anticommutes(pauli_basis(pauli, id.get() as usize)?, basis);
        }
        if group_anticommutes {
            return Err(AnalysisError::invalid_tableau_conversion(
                "Pauli observable anticommutes with an MPP measurement",
            ));
        }
    }
    Ok(())
}

fn single_result_basis(instruction: &CircuitInstruction) -> AnalysisResult<PauliBasis> {
    match instruction.gate().canonical_name() {
        "M" | "R" | "MR" => Ok(PauliBasis::Z),
        "MX" | "RX" | "MRX" => Ok(PauliBasis::X),
        "MY" | "RY" | "MRY" => Ok(PauliBasis::Y),
        name => Err(AnalysisError::invalid_tableau_conversion(format!(
            "{name} does not have a single-result Pauli basis"
        ))),
    }
}

fn target_qubit_index(instruction: &CircuitInstruction, target: &Target) -> AnalysisResult<usize> {
    target
        .qubit_id()
        .map(|id| id.get() as usize)
        .ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(format!(
                "{} target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })
}

fn pauli_basis(pauli: &PauliString, index: usize) -> AnalysisResult<PauliBasis> {
    pauli.get(index).ok_or_else(|| {
        AnalysisError::invalid_tableau_conversion(format!(
            "qubit {index} is outside Pauli width {}",
            pauli.len()
        ))
    })
}

fn anticommutes(left: PauliBasis, right: PauliBasis) -> bool {
    (left.x_bit() & right.z_bit()) ^ (left.z_bit() & right.x_bit())
}

fn toggled_sign(sign: stab_algebra::PauliSign) -> stab_algebra::PauliSign {
    if sign.is_negative() {
        stab_algebra::PauliSign::Plus
    } else {
        stab_algebra::PauliSign::Minus
    }
}

fn unsupported_instruction(instruction: &CircuitInstruction, direction: &str) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(format!(
        "Pauli conjugation {direction} {} is not defined",
        instruction.gate().canonical_name()
    ))
}

fn map_tableau_error(error: stab_algebra::StabilizerError) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(error.to_string())
}
