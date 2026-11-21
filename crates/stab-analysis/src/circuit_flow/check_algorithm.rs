//! Cost-based choice between the two algorithms that check Pauli-only flows of a unitary circuit.
//!
//! Both algorithms return the same public result; the choice is purely a performance decision.
//! The dense algorithm builds one Tableau of the whole circuit and applies it to every flow input.
//! The sparse algorithm tracks every flow output backwards through the circuit with the sparse
//! reverse frame tracker. Neither cost follows the qubit count alone: the sparse tracker is far
//! cheaper for few or low-weight flows at every width, and the dense Tableau is far cheaper for many
//! heavy flows from narrow widths up. The estimates below use only quantities read in one pass over
//! the top-level circuit items and the flows.

use stab_algebra::Flow;
use stab_model::{Circuit, CircuitItem};

/// Algorithm that checks Pauli-only flows of a unitary circuit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FlowCheckAlgorithm {
    /// Build one dense Tableau of the circuit and apply it to each flow input.
    DenseTableau,
    /// Track each flow output backwards with the sparse reverse frame tracker.
    SparseTracker,
}

/// Cheap size measures of a flow query, read before choosing an algorithm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FlowCheckCost {
    /// Simulated qubit count `n` of the circuit.
    qubits: f64,
    /// Top-level instructions `I`; repeat bodies are not expanded.
    instructions: f64,
    /// Gate targets `G` of the top-level instructions.
    gate_targets: f64,
    /// Number of flows `F`.
    flows: f64,
    /// Total Pauli weight of the flow inputs, `W_in`.
    input_weight: f64,
    /// Sum over flows of the larger of the input and output weights, `W_max`.
    tracked_weight: f64,
}

// Measured cost model, in nanoseconds on the AArch64 development host, with `w = ceil(n / 64)`:
//
//   dense  = DENSE_INSTRUCTION * I + DENSE_GATE * G * w + DENSE_INIT * n * w
//            + DENSE_APPLY * W_in * w + DENSE_FLOW * F * n
//   sparse = SPARSE_INSTRUCTION * I + SPARSE_GATE * G + SPARSE_TRACK * G * W_max / n
//            + SPARSE_SEED * W_max^2 / n + SPARSE_FLOW * F * n
//
// Dense: the TableauAppender rewrites `w` words per gate target after a per-instruction gate
// lookup, the identity allocation and final transpose cost `n * w`, `Tableau::apply` adds one row of
// `2w` words per input term, and every flow pays an `O(n)` pad-and-compare. Sparse: every gate
// target merges the sensitivity sets of its qubit, whose mean size is the tracked weight over the
// qubit count (a valid flow's tracked weight stays near the larger of its endpoint weights); seeding
// a flow term inserts into a sorted set of that mean size; and every flow pays an `O(n)` region
// extraction. Each estimate's coefficients were fitted, independently and without a constant term,
// by least squares of the relative error over the measured grid: surface-code, four-layer random,
// 32-layer 1D brickwork, and 128-layer random circuits of 16 to 4096 qubits, each with four
// single-qubit flows, all `2n` single-qubit flows, `8n` random single-qubit flows, and `n / 8`, `n`,
// and `8n` random full-width flows, timed best-of-N and pinned to one core. The coefficients come
// from the flow checker's grid. Choosing the smaller estimate picked an algorithm within 1.5x of
// the faster one in 237 of its 240 cells and in 236 of the 240 cells of time-reversal validation,
// and at worst 2.7x slower (four flows of a 384- to 1024-qubit deep random circuit, whose tracked
// weight stays high between the endpoints).
const DENSE_INSTRUCTION_NS: f64 = 889.0;
const DENSE_GATE_NS: f64 = 7.36;
const DENSE_INIT_NS: f64 = 26.1;
const DENSE_APPLY_NS: f64 = 2.36;
const DENSE_FLOW_NS: f64 = 3.70;
const SPARSE_INSTRUCTION_NS: f64 = 176.0;
const SPARSE_GATE_NS: f64 = 12.8;
const SPARSE_TRACK_NS: f64 = 9.96;
const SPARSE_SEED_NS: f64 = 1.68;
const SPARSE_FLOW_NS: f64 = 31.5;

impl FlowCheckCost {
    /// Reads the size measures of `flows` checked against `circuit`.
    pub(crate) fn measure(circuit: &Circuit, flows: &[Flow]) -> Self {
        let (instructions, gate_targets) =
            circuit
                .items()
                .iter()
                .fold(
                    (0_usize, 0_usize),
                    |(instructions, targets), item| match item {
                        CircuitItem::Instruction(instruction) => (
                            instructions.saturating_add(1),
                            targets.saturating_add(instruction.targets().len()),
                        ),
                        CircuitItem::RepeatBlock(_) => (instructions, targets),
                    },
                );
        let (input_weight, tracked_weight) =
            flows
                .iter()
                .fold((0_usize, 0_usize), |(input, tracked), flow| {
                    let flow_input = flow.input().weight();
                    let flow_tracked = flow_input.max(flow.output().weight());
                    (
                        input.saturating_add(flow_input),
                        tracked.saturating_add(flow_tracked),
                    )
                });
        Self {
            qubits: circuit.simulated_qubit_count() as f64,
            instructions: instructions as f64,
            gate_targets: gate_targets as f64,
            flows: flows.len() as f64,
            input_weight: input_weight as f64,
            tracked_weight: tracked_weight as f64,
        }
    }

    /// Estimated nanoseconds of the dense Tableau algorithm.
    pub(crate) fn dense_estimate(&self) -> f64 {
        let words = (self.qubits / 64.0).ceil();
        DENSE_INSTRUCTION_NS * self.instructions
            + DENSE_GATE_NS * self.gate_targets * words
            + DENSE_INIT_NS * self.qubits * words
            + DENSE_APPLY_NS * self.input_weight * words
            + DENSE_FLOW_NS * self.flows * self.qubits
    }

    /// Estimated nanoseconds of the sparse reverse tracker algorithm.
    pub(crate) fn sparse_estimate(&self) -> f64 {
        let mean_set_size = self.tracked_weight / self.qubits.max(1.0);
        SPARSE_INSTRUCTION_NS * self.instructions
            + SPARSE_GATE_NS * self.gate_targets
            + SPARSE_TRACK_NS * self.gate_targets * mean_set_size
            + SPARSE_SEED_NS * self.tracked_weight * mean_set_size
            + SPARSE_FLOW_NS * self.flows * self.qubits
    }

    /// Returns the algorithm with the lower estimate, preferring the sparse tracker on a tie.
    pub(crate) fn cheaper_algorithm(&self) -> FlowCheckAlgorithm {
        if self.dense_estimate() < self.sparse_estimate() {
            FlowCheckAlgorithm::DenseTableau
        } else {
            FlowCheckAlgorithm::SparseTracker
        }
    }
}

#[cfg(test)]
mod tests;
