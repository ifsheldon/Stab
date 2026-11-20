use stab_algebra::{Flow, PauliBasis};
use stab_model::{Cancellation, Circuit, CircuitItem, MemoryBudget};

use crate::AnalysisOperation;
use crate::{AnalysisError, AnalysisResult};

mod canonicalize;
mod classification;
mod composed;
mod helpers;
mod resources;
mod rows;
mod simple;

use classification::{circuit_is_ignored_only, circuit_requires_reverse_flow_solver};
use composed::composed_measurement_flow_generators;
use helpers::{instruction_qubit_count, stabilizer_to_circuit_error};
use rows::{reverse_ordered_identity_flow_rows, single_pauli};
use simple::{
    measurement_pad_flows, simple_measure_reset_flows, simple_measurement_flows,
    simple_pair_measurement_flows, simple_pauli_product_measurement_flows, simple_reset_flows,
};

use super::transitions::{ReverseFlowTransition, reverse_flow_transition};
use resources::admit_flow_generator_rows;
pub(crate) use resources::measurement_rich_flow_generator_projected_bytes;

/// Options for [`circuit_flow_generators_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlowGeneratorOptions {
    /// Budget for the owned flow rows, projected before they are built, and for the Tableau of a
    /// unitary circuit.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per undone instruction of a measurement-rich circuit, whose compact
    /// repeats are expanded one iteration at a time.
    pub cancellation: Cancellation,
}

/// Returns signed stabilizer-flow generators for supported tableau and measurement-rich circuits.
///
/// Measurement-rich output is admitted by projected owned bytes, and compact repeats are traversed
/// directly instead of first cloning an expanded circuit; expanded work is not capped.
pub fn circuit_flow_generators(circuit: &Circuit) -> AnalysisResult<Vec<Flow>> {
    circuit_flow_generators_with_options(circuit, &FlowGeneratorOptions::default())
}

/// Returns signed stabilizer-flow generators, polling the options' cancellation signal.
pub fn circuit_flow_generators_with_options(
    circuit: &Circuit,
    options: &FlowGeneratorOptions,
) -> AnalysisResult<Vec<Flow>> {
    options.cancellation.check()?;
    let budget = options.memory_budget;
    if circuit_is_ignored_only(circuit) {
        let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
        admit_flow_generator_rows(qubit_count, 0, budget)?;
        return Ok(reverse_ordered_identity_flow_rows(qubit_count));
    }
    if circuit_requires_reverse_flow_solver(circuit) {
        return simple_measurement_rich_flow_generators(circuit, budget, &options.cancellation)?
            .ok_or_else(|| unsupported_flow_generator_error(circuit));
    }
    unitary_flow_generators(circuit, options)
}

fn unitary_flow_generators(
    circuit: &Circuit,
    options: &FlowGeneratorOptions,
) -> AnalysisResult<Vec<Flow>> {
    let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    admit_flow_generator_rows(qubit_count, 0, options.memory_budget)?;
    let tableau = crate::circuit_to_tableau_with_options(
        circuit,
        &crate::CircuitToTableauOptions {
            ignore_noise: true,
            memory_budget: options.memory_budget,
            cancellation: options.cancellation.clone(),
            ..crate::CircuitToTableauOptions::default()
        },
    )?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in (0..tableau.len()).rev() {
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_circuit_error)?,
        ));
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_circuit_error)?,
        ));
    }
    Ok(flows)
}

fn simple_measurement_rich_flow_generators(
    circuit: &Circuit,
    budget: MemoryBudget,
    cancellation: &Cancellation,
) -> AnalysisResult<Option<Vec<Flow>>> {
    if let [CircuitItem::Instruction(instruction)] = circuit.items() {
        let transition = reverse_flow_transition(instruction);
        let simple = match transition {
            ReverseFlowTransition::Measurement(basis) => {
                simple_measurement_flows(instruction, basis, budget)?
            }
            ReverseFlowTransition::Reset(basis) => simple_reset_flows(instruction, basis, budget)?,
            ReverseFlowTransition::MeasureReset(basis) => {
                simple_measure_reset_flows(instruction, basis, budget)?
            }
            ReverseFlowTransition::PairMeasurement(basis) => {
                simple_pair_measurement_flows(instruction, basis, budget)?
            }
            ReverseFlowTransition::PauliProductMeasurement => {
                simple_pauli_product_measurement_flows(instruction, budget)?
            }
            ReverseFlowTransition::MeasurementPad => {
                Some(measurement_pad_flows(instruction, budget)?)
            }
            ReverseFlowTransition::SweepControlledPauliNoop => {
                let qubit_count = instruction_qubit_count(instruction);
                admit_flow_generator_rows(qubit_count, 0, budget)?;
                Some(reverse_ordered_identity_flow_rows(qubit_count))
            }
            _ => None,
        };
        if simple.is_some() {
            return Ok(simple);
        }
    }
    composed_measurement_flow_generators(circuit, budget, cancellation)
}

fn unsupported_flow_generator_error(circuit: &Circuit) -> AnalysisError {
    AnalysisError::unsupported(
        AnalysisOperation::FlowGeneration,
        None,
        format!(
            "circuit_flow_generators cannot derive generators for the supplied gate or target shape; got {} top-level item(s)",
            circuit.items().len()
        ),
    )
}
