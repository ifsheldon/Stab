//! Flow-row building blocks shared by the direct and composed generators.

use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString};
use stab_model::Target;

use crate::{AnalysisError, AnalysisResult};

use super::helpers::{
    internal_flow_error, pauli_basis, rows_matching, stabilizer_to_circuit_error,
};

pub(super) fn measured_pauli_product(
    gate_name: &'static str,
    qubit_count: usize,
    targets: &[Target],
) -> AnalysisResult<PauliString> {
    let mut product = stab_algebra::advanced::pauli_identity_unchecked(qubit_count);
    for target in targets {
        if target.is_combiner() {
            continue;
        }
        let qubit = target.qubit_id().ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} does not identify a qubit"
            ))
        })?;
        let pauli = target.pauli_type().ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} is not a Pauli target"
            ))
        })?;
        let term = single_pauli_with_sign(
            qubit_count,
            qubit.get() as usize,
            pauli_basis(pauli),
            if target.is_inverted_result_target() {
                PauliSign::Minus
            } else {
                PauliSign::Plus
            },
        );
        product = product.multiply_real(&term).map_err(|error| {
            AnalysisError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator Pauli product is not Hermitian: {error}"
            ))
        })?;
    }
    Ok(product)
}

pub(super) fn identity_flow_rows(qubit_count: usize) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(qubit_count.saturating_mul(2));
    for qubit in 0..qubit_count {
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::X),
            single_pauli(qubit_count, qubit, PauliBasis::X),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            single_pauli(qubit_count, qubit, PauliBasis::Z),
        ));
    }
    flows
}

pub(super) fn reverse_ordered_identity_flow_rows(qubit_count: usize) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(qubit_count.saturating_mul(2));
    for qubit in (0..qubit_count).rev() {
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::X),
            single_pauli(qubit_count, qubit, PauliBasis::X),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            single_pauli(qubit_count, qubit, PauliBasis::Z),
        ));
    }
    flows
}

pub(super) fn remove_single_anticommutations(
    flows: &mut Vec<Flow>,
    qubit: usize,
    basis: PauliBasis,
) -> AnalysisResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_single_measurement(flow.input(), qubit, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("single-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("single-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.swap_remove(pivot);
    Ok(())
}

pub(super) fn remove_pair_anticommutations(
    flows: &mut Vec<Flow>,
    left: usize,
    right: usize,
    basis: PauliBasis,
) -> AnalysisResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pair_measurement(flow.input(), left, right, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("pair-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("pair-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.swap_remove(pivot);
    Ok(())
}

pub(super) fn add_pauli_product_measurement_flow(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
    record_index: i32,
    qubit_count: usize,
) -> AnalysisResult<()> {
    if measured_product.has_no_pauli_terms() {
        flows.push(
            Flow::new(
                stab_algebra::advanced::pauli_identity_unchecked(qubit_count),
                stab_algebra::advanced::pauli_from_bases_unchecked(
                    measured_product.sign(),
                    vec![PauliBasis::I; qubit_count],
                ),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
        return Ok(());
    }
    remove_pauli_product_anticommutations(flows, measured_product)?;
    flows.push(
        Flow::new(
            measured_product.clone(),
            stab_algebra::advanced::pauli_identity_unchecked(qubit_count),
            [record_index],
            [],
        )
        .map_err(stabilizer_to_circuit_error)?,
    );
    Ok(())
}

pub(super) fn remove_pauli_product_anticommutations(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
) -> AnalysisResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pauli_product(flow.input(), measured_product)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows.get(pivot).cloned().ok_or_else(|| {
        internal_flow_error("Pauli-product measurement pivot row is out of bounds")
    })?;
    for &row in rest {
        let target = flows.get(row).ok_or_else(|| {
            internal_flow_error("Pauli-product measurement target row is out of bounds")
        })?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.swap_remove(pivot);
    Ok(())
}

pub(super) fn anticommutes_with_pauli_product(
    input: &PauliString,
    measured_product: &PauliString,
) -> bool {
    (0..input.len().max(measured_product.len())).fold(false, |anticommutes, qubit| {
        let input_basis = input.get(qubit).unwrap_or(PauliBasis::I);
        let measured_basis = measured_product.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes
            ^ (input_basis.x_bit() & measured_basis.z_bit())
            ^ (input_basis.z_bit() & measured_basis.x_bit())
    })
}

/// Undoes a single-qubit reset of `qubit` in `basis`: sacrifices the first flow whose input
/// anticommutes with the reset basis to fix the others, then clears `qubit` from every input.
///
/// One scan finds both sets, since the anticommuting flows are among those with a term on
/// `qubit` and the products leave every other flow's term there untouched. The sacrificed row is
/// swap-removed; row order is free here because the final canonical form depends only on the
/// span of the rows.
pub(super) fn undo_single_qubit_reset(
    flows: &mut Vec<Flow>,
    qubit: usize,
    basis: PauliBasis,
) -> AnalysisResult<()> {
    let mut touched = Vec::new();
    let mut anticommuting = Vec::new();
    for (row, flow) in flows.iter().enumerate() {
        let term = flow.input().get(qubit).unwrap_or(PauliBasis::I);
        if term == PauliBasis::I {
            continue;
        }
        touched.push(row);
        if (term.x_bit() & basis.z_bit()) ^ (term.z_bit() & basis.x_bit()) {
            anticommuting.push(row);
        }
    }
    let moved_row = flows.len().checked_sub(1);
    let removed = match anticommuting.split_first() {
        Some((&pivot, rest)) => {
            let pivot_flow = flows
                .get(pivot)
                .cloned()
                .ok_or_else(|| internal_flow_error("reset pivot row is out of bounds"))?;
            for &row in rest {
                let slot = flows
                    .get_mut(row)
                    .ok_or_else(|| internal_flow_error("reset target row is out of bounds"))?;
                *slot = slot
                    .multiply(&pivot_flow)
                    .map_err(stabilizer_to_circuit_error)?;
            }
            flows.swap_remove(pivot);
            Some(pivot)
        }
        None => None,
    };
    for row in touched {
        let row = match removed {
            Some(pivot) if row == pivot => continue,
            Some(pivot) if Some(row) == moved_row => pivot,
            _ => row,
        };
        let flow = flows
            .get_mut(row)
            .ok_or_else(|| internal_flow_error("reset cleared row is out of bounds"))?;
        flow.set_input_term(qubit, PauliBasis::I)
            .map_err(stabilizer_to_circuit_error)?;
    }
    Ok(())
}

pub(super) fn anticommutes_with_pair_measurement(
    input: &PauliString,
    left: usize,
    right: usize,
    measured_basis: PauliBasis,
) -> bool {
    let x = measured_basis.x_bit();
    let z = measured_basis.z_bit();
    let mut anticommutes = false;
    for qubit in [left, right] {
        let basis = input.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes ^= basis.x_bit() & z;
        anticommutes ^= basis.z_bit() & x;
    }
    anticommutes
}

pub(super) fn anticommutes_with_single_measurement(
    input: &PauliString,
    qubit: usize,
    measured_basis: PauliBasis,
) -> bool {
    let basis = input.get(qubit).unwrap_or(PauliBasis::I);
    (basis.x_bit() & measured_basis.z_bit()) ^ (basis.z_bit() & measured_basis.x_bit())
}

pub(super) fn pair_pauli(
    qubit_count: usize,
    left: usize,
    right: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    stab_algebra::advanced::pauli_from_bases_unchecked(
        sign,
        (0..qubit_count).map(|qubit| {
            if qubit == left || qubit == right {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

pub(super) fn single_pauli_with_sign(
    len: usize,
    index: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    stab_algebra::advanced::pauli_from_bases_unchecked(
        sign,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

pub(super) fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    stab_algebra::advanced::pauli_from_bases_unchecked(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}
