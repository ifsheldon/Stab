use stab_algebra::{PauliBasis, PauliString, StabilizerError, Tableau};
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, ControlledPauliTargetPair,
    GateCategory, GateKind, GateTargets, MemoryBudget, Pauli, PauliTarget, QubitId,
    advanced::CancellationCountdown,
};

use crate::instruction_shape;
use crate::{
    AnalysisError, AnalysisOperation, AnalysisRequestError, AnalysisResult,
    CircuitToTableauOptions, PauliCollapse, ResourceKind, ResourceLimitError,
    circuit_to_tableau_with_options,
};

/// Options for [`pauli_after_circuit_with_options`] and [`pauli_before_circuit_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PauliConjugationOptions {
    /// Budget for the circuit's Tableau and the conjugated Pauli string. A Tableau the budget
    /// refuses is not an error: conjugation then walks the expanded instructions instead.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per folded item, Tableau composition, and expanded instruction.
    pub cancellation: Cancellation,
}

/// Returns the Pauli observable after the circuit under the default memory budget.
///
/// See [`pauli_after_circuit_with_options`].
pub fn pauli_after_circuit(pauli: &PauliString, circuit: &Circuit) -> AnalysisResult<PauliString> {
    pauli_after_circuit_with_options(pauli, circuit, &PauliConjugationOptions::default())
}

/// Returns the Pauli observable after the circuit, when its value remains well defined.
///
/// Pure Clifford circuits, including compact repeats, use one tableau action. Measurement and
/// reset circuits, and circuits whose Tableau the budget refuses, follow Stim's conditional
/// observable rules over the expanded instructions, which can be as many as the repeat counts
/// multiply to; the cancellation signal bounds that walk.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the options' signal is observed,
/// [`AnalysisError::ResourceLimit`] when the conjugated string exceeds the budget, and
/// [`AnalysisError::PauliNotPreserved`] when the observable is not well defined after the
/// circuit.
pub fn pauli_after_circuit_with_options(
    pauli: &PauliString,
    circuit: &Circuit,
    options: &PauliConjugationOptions,
) -> AnalysisResult<PauliString> {
    if let Some(tableau) = conjugation_tableau(pauli, circuit, options)? {
        return apply_tableau_at_width(pauli, &tableau, false, options);
    }

    let mut result = pauli.clone();
    let mut countdown = CancellationCountdown::new();
    for instruction in circuit.iter_flattened_instructions() {
        countdown.tick(&options.cancellation)?;
        apply_instruction_after(&mut result, instruction)?;
    }
    Ok(result)
}

/// Returns the Pauli observable before the circuit under the default memory budget.
///
/// See [`pauli_before_circuit_with_options`].
pub fn pauli_before_circuit(pauli: &PauliString, circuit: &Circuit) -> AnalysisResult<PauliString> {
    pauli_before_circuit_with_options(pauli, circuit, &PauliConjugationOptions::default())
}

/// Returns the Pauli observable before the circuit, when its value remains well defined.
///
/// This walks the circuit backwards like [`pauli_after_circuit_with_options`] walks it forwards,
/// and fails the same way.
///
/// # Errors
///
/// Fails like [`pauli_after_circuit_with_options`].
pub fn pauli_before_circuit_with_options(
    pauli: &PauliString,
    circuit: &Circuit,
    options: &PauliConjugationOptions,
) -> AnalysisResult<PauliString> {
    if let Some(tableau) = conjugation_tableau(pauli, circuit, options)? {
        return apply_tableau_at_width(pauli, &tableau, true, options);
    }

    let mut result = pauli.clone();
    let mut countdown = CancellationCountdown::new();
    for instruction in circuit.iter_flattened_instructions_reverse() {
        countdown.tick(&options.cancellation)?;
        apply_instruction_before(&mut result, instruction)?;
    }
    Ok(result)
}

/// Validates the compact circuit and builds its Tableau when the circuit is unitary and the
/// budget admits it; `None` selects the expanded walk, and only cancellation stops here.
fn conjugation_tableau(
    pauli: &PauliString,
    circuit: &Circuit,
    options: &PauliConjugationOptions,
) -> AnalysisResult<Option<Tableau>> {
    options.cancellation.check()?;
    validate_compact_pauli_circuit(pauli.len(), circuit, &options.cancellation)?;
    let tableau_options = CircuitToTableauOptions {
        memory_budget: options.memory_budget,
        cancellation: options.cancellation.clone(),
        ..CircuitToTableauOptions::default()
    };
    match circuit_to_tableau_with_options(circuit, &tableau_options) {
        Ok(tableau) => Ok(Some(tableau)),
        Err(AnalysisError::Cancelled(cancelled)) => Err(cancelled.into()),
        Err(_) => Ok(None),
    }
}

fn validate_compact_pauli_circuit(
    width: usize,
    circuit: &Circuit,
    cancellation: &Cancellation,
) -> AnalysisResult<()> {
    let mut pending = Vec::new();
    let mut items = circuit.items().iter();
    let mut countdown = CancellationCountdown::new();
    loop {
        countdown.tick(cancellation)?;
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
                    // Stim refuses an instruction that targets qubits outside the Pauli string.
                    if let Some(qubit) = target.qubit_id()
                        && qubit.get() as usize >= width
                    {
                        return Err(AnalysisRequestError::PauliNarrowerThanCircuit {
                            gate: instruction.gate().kind(),
                            qubit,
                            width,
                        }
                        .into());
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
        let decomposed = crate::circuit_simplify::decomposed_single_instruction(instruction)
            .map_err(|error| {
                error.into_nested_pauli_product_error(AnalysisOperation::PauliConjugation)
            })?;
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
    match instruction.gate_targets() {
        GateTargets::Qubits(qubits) => {
            for qubit in qubits {
                apply_local_tableau_group(pauli, &local, &[qubit], &mut output_sign)?;
            }
        }
        GateTargets::QubitPairs(pairs) => {
            for pair in pairs {
                apply_local_tableau_group(pauli, &local, &pair, &mut output_sign)?;
            }
        }
        GateTargets::ControlledPairs(pairs) => {
            let conditional_basis = match instruction.gate().controlled_pauli() {
                Some(Pauli::X) => PauliBasis::X,
                Some(Pauli::Y) => PauliBasis::Y,
                Some(Pauli::Z) => PauliBasis::Z,
                None => {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::PauliConjugation,
                        format!("{gate_name} has controlled-Pauli targets but no controlled Pauli"),
                    ));
                }
            };
            for pair in pairs {
                match pair {
                    ControlledPauliTargetPair::Quantum { first, second } => {
                        apply_local_tableau_group(
                            pauli,
                            &local,
                            &[first, second],
                            &mut output_sign,
                        )?;
                    }
                    ControlledPauliTargetPair::Classical { target, .. } => {
                        if anticommutes(
                            pauli_basis(pauli, target.get() as usize)?,
                            conditional_basis,
                        ) {
                            return Err(AnalysisError::pauli_not_preserved(
                                instruction.gate().kind(),
                                PauliCollapse::ClassicallyControlled { qubit: target },
                            ));
                        }
                    }
                    ControlledPauliTargetPair::ClassicalNoop { .. } => {}
                    ControlledPauliTargetPair::Unsupported => {
                        return Err(AnalysisError::unsupported(
                            AnalysisOperation::PauliConjugation,
                            instruction.gate().kind(),
                            format!(
                                "{gate_name} has an unsupported controlled-Pauli target orientation"
                            ),
                        ));
                    }
                }
            }
        }
        GateTargets::None
        | GateTargets::MeasuredQubits(_)
        | GateTargets::MeasurementPads(_)
        | GateTargets::MeasuredQubitPairs(_)
        | GateTargets::Records(_)
        | GateTargets::ObservableTargets(_)
        | GateTargets::QubitCoords(_)
        | GateTargets::PauliProducts(_)
        | GateTargets::PauliTerms(_) => {
            return Err(AnalysisError::internal(
                AnalysisOperation::PauliConjugation,
                format!("{gate_name} is applied as a tableau but does not target qubits"),
            ));
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
        return Err(AnalysisError::internal(
            AnalysisOperation::PauliConjugation,
            format!(
                "gate tableau width {} does not match {} target(s)",
                tableau.len(),
                targets.len()
            ),
        ));
    }
    for (index, target) in targets.iter().enumerate() {
        if targets
            .iter()
            .take(index)
            .any(|previous| previous == target)
        {
            return Err(AnalysisError::internal(
                AnalysisOperation::PauliConjugation,
                format!("gate tableau target qubit {} is repeated", target.get()),
            ));
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
            AnalysisError::internal(
                AnalysisOperation::PauliConjugation,
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
    options: &PauliConjugationOptions,
) -> AnalysisResult<PauliString> {
    let budget = options.memory_budget;
    let tableau = if inverse {
        tableau.inverse().map_err(map_tableau_error)?
    } else {
        tableau.clone()
    };
    if tableau.len() <= pauli.len() {
        let prefix = PauliString::from_bases_with_budget(
            pauli.sign(),
            (0..tableau.len()).map(|index| pauli.get(index).unwrap_or(PauliBasis::I)),
            budget,
        )
        .map_err(map_tableau_error)?;
        let transformed = tableau.apply(&prefix).map_err(map_tableau_error)?;
        return PauliString::from_bases_with_budget(
            transformed.sign(),
            (0..pauli.len()).map(|index| {
                if index < transformed.len() {
                    transformed.get(index).unwrap_or(PauliBasis::I)
                } else {
                    pauli.get(index).unwrap_or(PauliBasis::I)
                }
            }),
            budget,
        )
        .map_err(map_tableau_error);
    }

    let padded = PauliString::from_bases_with_budget(
        pauli.sign(),
        (0..tableau.len()).map(|index| pauli.get(index).unwrap_or(PauliBasis::I)),
        budget,
    )
    .map_err(map_tableau_error)?;
    let output = tableau.apply(&padded).map_err(map_tableau_error)?;
    if (pauli.len()..output.len()).any(|index| output.get(index) != Some(PauliBasis::I)) {
        return Err(AnalysisError::internal(
            AnalysisOperation::PauliConjugation,
            "Pauli conjugation produced support outside the input width",
        ));
    }
    PauliString::from_bases_with_budget(
        output.sign(),
        (0..pauli.len()).map(|index| output.get(index).unwrap_or(PauliBasis::I)),
        budget,
    )
    .map_err(map_tableau_error)
}

fn check_reset_avoided(
    pauli: &PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    for qubit in instruction_shape::single_qubits(instruction, AnalysisOperation::PauliConjugation)?
    {
        if pauli.get(qubit.get() as usize) != Some(PauliBasis::I) {
            return Err(AnalysisError::pauli_not_preserved(
                instruction.gate().kind(),
                PauliCollapse::TouchesReset { qubit },
            ));
        }
    }
    Ok(())
}

fn undo_reset(pauli: &mut PauliString, instruction: &CircuitInstruction) -> AnalysisResult<()> {
    let measured_basis = single_result_basis(instruction)?;
    let qubits =
        instruction_shape::single_qubits(instruction, AnalysisOperation::PauliConjugation)?;
    for qubit in qubits.clone() {
        if anticommutes(pauli_basis(pauli, qubit.get() as usize)?, measured_basis) {
            return Err(AnalysisError::pauli_not_preserved(
                instruction.gate().kind(),
                PauliCollapse::AnticommutesWithReset { qubit },
            ));
        }
    }
    for qubit in qubits {
        pauli
            .set(qubit.get() as usize, PauliBasis::I)
            .map_err(map_tableau_error)?;
    }
    Ok(())
}

fn check_measurement_commutes(
    pauli: &PauliString,
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    let measured_basis = single_result_basis(instruction)?;
    for qubit in instruction_shape::single_qubits(instruction, AnalysisOperation::PauliConjugation)?
    {
        if anticommutes(pauli_basis(pauli, qubit.get() as usize)?, measured_basis) {
            return Err(AnalysisError::pauli_not_preserved(
                instruction.gate().kind(),
                PauliCollapse::AnticommutesWithMeasurement { qubit },
            ));
        }
    }
    Ok(())
}

fn check_mpp_commutes(pauli: &PauliString, instruction: &CircuitInstruction) -> AnalysisResult<()> {
    let products =
        instruction_shape::pauli_products(instruction, AnalysisOperation::PauliConjugation)?;
    for product in products {
        let mut group_anticommutes = false;
        for PauliTarget {
            pauli: measured,
            id,
            ..
        } in product
        {
            let basis = match measured {
                Pauli::X => PauliBasis::X,
                Pauli::Y => PauliBasis::Y,
                Pauli::Z => PauliBasis::Z,
            };
            group_anticommutes ^= anticommutes(pauli_basis(pauli, id.get() as usize)?, basis);
        }
        if group_anticommutes {
            return Err(AnalysisError::pauli_not_preserved(
                instruction.gate().kind(),
                PauliCollapse::AnticommutesWithProductMeasurement,
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
        None => Err(AnalysisError::internal(
            AnalysisOperation::PauliConjugation,
            format!(
                "{} does not have a single-result Pauli basis",
                instruction.gate().canonical_name()
            ),
        )),
    }
}

fn pauli_basis(pauli: &PauliString, index: usize) -> AnalysisResult<PauliBasis> {
    pauli.get(index).ok_or_else(|| {
        AnalysisError::internal(
            AnalysisOperation::PauliConjugation,
            format!("qubit {index} is outside Pauli width {}", pauli.len()),
        )
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
    AnalysisError::unsupported(
        AnalysisOperation::PauliConjugation,
        instruction.gate().kind(),
        format!(
            "Pauli conjugation {direction} {} is not defined",
            instruction.gate().canonical_name()
        ),
    )
}

/// Maps algebra failures, reporting a conjugated string the budget refuses as a resource error.
fn map_tableau_error(error: StabilizerError) -> AnalysisError {
    match error {
        StabilizerError::MemoryBudgetExceeded { exceeded, .. } => {
            ResourceLimitError::budget_refusal(
                AnalysisOperation::PauliConjugation,
                ResourceKind::MaterializedBytes,
                exceeded,
            )
            .into()
        }
        other => AnalysisError::internal(AnalysisOperation::PauliConjugation, other.to_string()),
    }
}
