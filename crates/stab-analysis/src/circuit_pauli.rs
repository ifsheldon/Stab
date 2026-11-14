use stab_algebra::{PauliBasis, PauliString, Tableau};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, GateCategory, GateKind, Pauli, QubitId, Target,
    advanced::{ControlledPauliTargetPair, classify_controlled_pauli_target_pair},
};

use crate::{AnalysisError, AnalysisResult, circuit_to_tableau};

/// Returns the Pauli observable after the circuit, when its value remains well defined.
///
/// Pure Clifford circuits, including compact repeats, use one tableau action. Measurement and
/// reset circuits follow Stim's conditional observable rules under an explicit expanded-work cap.
pub fn pauli_after_circuit(pauli: &PauliString, circuit: &Circuit) -> AnalysisResult<PauliString> {
    validate_compact_pauli_circuit(pauli.len(), circuit)?;
    if let Ok(tableau) = circuit_to_tableau(circuit, false, false, false) {
        return apply_tableau_at_width(pauli, &tableau, false);
    }

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

fn apply_instruction_after(
    pauli: &mut PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    if no_effect_on_pauli(instruction) {
        return Ok(());
    }
    match instruction.gate().kind() {
        GateKind::R
        | GateKind::RX
        | GateKind::RY
        | GateKind::MR
        | GateKind::MRX
        | GateKind::MRY => check_reset_avoided(pauli, instruction),
        GateKind::M | GateKind::MX | GateKind::MY => check_measurement_commutes(pauli, instruction),
        GateKind::MPP => check_mpp_commutes(pauli, instruction),
        GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => apply_unitary_instruction(pauli, instruction, false),
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => Err(unsupported_instruction(instruction, "after")),
    }
}

fn apply_instruction_before(
    pauli: &mut PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    if no_effect_on_pauli(instruction) {
        return Ok(());
    }
    match instruction.gate().kind() {
        GateKind::R
        | GateKind::RX
        | GateKind::RY
        | GateKind::MR
        | GateKind::MRX
        | GateKind::MRY => undo_reset(pauli, instruction),
        GateKind::M | GateKind::MX | GateKind::MY => check_measurement_commutes(pauli, instruction),
        GateKind::MPP => check_mpp_commutes(pauli, instruction),
        GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => apply_unitary_instruction(pauli, instruction, true),
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => Err(unsupported_instruction(instruction, "before")),
    }
}

fn no_effect_on_pauli(instruction: &CircuitInstruction) -> bool {
    match instruction.gate().kind() {
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::MPAD
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::I
        | GateKind::II => true,
        GateKind::REPEAT
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => false,
    }
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
        if let Some(conditional_pauli) = instruction.gate().controlled_pauli() {
            match classify_controlled_pauli_target_pair(instruction.gate(), group) {
                ControlledPauliTargetPair::Quantum { first, second } => {
                    apply_local_tableau_group(pauli, &local, &[first, second], &mut output_sign)?;
                }
                ControlledPauliTargetPair::Classical { target, .. } => {
                    let conditional_basis = match conditional_pauli {
                        Pauli::X => PauliBasis::X,
                        Pauli::Y => PauliBasis::Y,
                        Pauli::Z => PauliBasis::Z,
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
    match instruction.gate().collapse_basis() {
        Some(Pauli::Z) => Ok(PauliBasis::Z),
        Some(Pauli::X) => Ok(PauliBasis::X),
        Some(Pauli::Y) => Ok(PauliBasis::Y),
        None => Err(AnalysisError::invalid_tableau_conversion(format!(
            "{} does not have a single-result Pauli basis",
            instruction.gate().canonical_name()
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
