use stab_algebra::{PauliBasis, PauliSign, Tableau, TableauAppender};
use stab_model::{Circuit, Gate, GateKind, QubitId, Target};

use crate::{AnalysisError, AnalysisResult, gate_tableau};

struct SynthesisContext {
    remaining: TableauAppender,
    circuit: Circuit,
    pending_gate: Option<Gate>,
    pending_targets: Vec<Target>,
    qubit_targets: Vec<Target>,
    h_gate: Gate,
    h_tableau: Tableau,
    s_gate: Gate,
    s_tableau: Tableau,
    cx_gate: Gate,
    cx_tableau: Tableau,
}

impl SynthesisContext {
    fn new(tableau: &Tableau) -> AnalysisResult<Self> {
        let inverse = tableau.inverse().map_err(map_tableau_error)?;
        let num_qubits = inverse.len();
        let remaining = TableauAppender::new(inverse).map_err(map_tableau_error)?;
        let qubit_targets = (0..num_qubits)
            .map(qubit_target)
            .collect::<AnalysisResult<Vec<_>>>()?;
        let h_gate = GateKind::H.gate();
        let s_gate = GateKind::S.gate();
        let cx_gate = GateKind::CX.gate();
        Ok(Self {
            remaining,
            circuit: Circuit::new(),
            pending_gate: None,
            pending_targets: Vec::new(),
            qubit_targets,
            h_tableau: gate_tableau(h_gate)?,
            h_gate,
            s_tableau: gate_tableau(s_gate)?,
            s_gate,
            cx_tableau: gate_tableau(cx_gate)?,
            cx_gate,
        })
    }

    #[inline]
    fn x_code(&self, input: usize, output: usize) -> AnalysisResult<u8> {
        let basis = self
            .remaining
            .x_output_basis(input, output)
            .map_err(map_tableau_error)?;
        Ok(basis_code(basis))
    }

    #[inline]
    fn z_code(&self, input: usize, output: usize) -> AnalysisResult<u8> {
        let basis = self
            .remaining
            .z_output_basis(input, output)
            .map_err(map_tableau_error)?;
        Ok(basis_code(basis))
    }

    fn flush_pending(&mut self) -> AnalysisResult<()> {
        let Some(gate) = self.pending_gate.take() else {
            return Ok(());
        };
        let targets = std::mem::take(&mut self.pending_targets);
        self.circuit
            .append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
                gate,
                Vec::new(),
                targets,
                None,
            )?);
        Ok(())
    }

    fn push_gate_targets(&mut self, gate: Gate, targets: &[Target]) -> AnalysisResult<()> {
        if self.pending_gate != Some(gate) {
            self.flush_pending()?;
            self.pending_gate = Some(gate);
        }
        self.pending_targets.extend_from_slice(targets);
        Ok(())
    }

    fn apply_h(&mut self, qubit: usize) -> AnalysisResult<()> {
        self.remaining
            .append(&self.h_tableau, &[qubit])
            .map_err(map_tableau_error)?;
        let target = self.target(qubit)?;
        self.push_gate_targets(self.h_gate, &[target])
    }

    fn apply_s(&mut self, qubit: usize) -> AnalysisResult<()> {
        self.remaining
            .append(&self.s_tableau, &[qubit])
            .map_err(map_tableau_error)?;
        let target = self.target(qubit)?;
        self.push_gate_targets(self.s_gate, &[target])
    }

    fn apply_cx(&mut self, control: usize, target: usize) -> AnalysisResult<()> {
        self.remaining
            .append(&self.cx_tableau, &[control, target])
            .map_err(map_tableau_error)?;
        let first = self.target(control)?;
        let second = self.target(target)?;
        self.push_gate_targets(self.cx_gate, &[first, second])
    }

    #[inline]
    fn target(&self, qubit: usize) -> AnalysisResult<Target> {
        self.qubit_targets.get(qubit).cloned().ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(format!(
                "tableau synthesis qubit index {qubit} is outside width {}",
                self.qubit_targets.len()
            ))
        })
    }
}

#[inline]
const fn basis_code(basis: PauliBasis) -> u8 {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Z => 2,
        PauliBasis::Y => 3,
    }
}

/// Synthesizes a Clifford circuit using only `H`, `S`, and `CX` instructions.
///
/// The elimination follows Stim's canonical generator-pivot strategy, but the public contract is
/// semantic: converting the result back to a tableau reproduces `tableau` exactly.
pub fn tableau_to_circuit(tableau: &Tableau) -> AnalysisResult<Circuit> {
    if !tableau.satisfies_invariants().map_err(map_tableau_error)? {
        return Err(AnalysisError::invalid_tableau_conversion(
            "tableau synthesis requires canonical symplectic generators",
        ));
    }

    let mut ctx = SynthesisContext::new(tableau)?;
    let num_qubits = ctx.remaining.len();

    for column in 0..num_qubits {
        let pivot = (column..num_qubits)
            .find(|&row| {
                let x = ctx.x_code(column, row);
                let z = ctx.z_code(column, row);
                matches!((x, z), (Ok(x), Ok(z)) if x != 0 && z != 0 && x != z)
            })
            .ok_or_else(|| {
                AnalysisError::invalid_tableau_conversion(format!(
                    "tableau synthesis could not find a pivot for generator {column}"
                ))
            })?;

        if pivot != column {
            ctx.apply_cx(pivot, column)?;
            ctx.apply_cx(column, pivot)?;
            ctx.apply_cx(pivot, column)?;
        }

        if ctx.z_code(column, column)? == 3 {
            ctx.apply_s(column)?;
        }
        if ctx.z_code(column, column)? != 2 {
            ctx.apply_h(column)?;
        }
        if ctx.x_code(column, column)? != 1 {
            ctx.apply_s(column)?;
        }

        for row in column + 1..num_qubits {
            if ctx.x_code(column, row)? == 3 {
                ctx.apply_s(row)?;
            }
        }
        for row in column + 1..num_qubits {
            if ctx.x_code(column, row)? == 2 {
                ctx.apply_h(row)?;
            }
        }
        for row in column + 1..num_qubits {
            if ctx.x_code(column, row)? != 0 {
                ctx.apply_cx(column, row)?;
            }
        }

        for row in column + 1..num_qubits {
            if ctx.z_code(column, row)? == 3 {
                ctx.apply_s(row)?;
            }
        }
        for row in column + 1..num_qubits {
            if ctx.z_code(column, row)? == 1 {
                ctx.apply_h(row)?;
            }
        }
        for row in column + 1..num_qubits {
            if ctx.z_code(column, row)? != 0 {
                ctx.apply_cx(row, column)?;
            }
        }
    }

    let negative_zs = (0..num_qubits)
        .map(|column| {
            ctx.remaining
                .z_output_sign(column)
                .map(|sign| sign == PauliSign::Minus)
                .map_err(map_tableau_error)
        })
        .collect::<AnalysisResult<Vec<_>>>()?;
    for (column, negative) in negative_zs.iter().copied().enumerate() {
        if negative {
            ctx.apply_h(column)?;
        }
    }
    for (column, negative) in negative_zs.iter().copied().enumerate() {
        if negative {
            ctx.apply_s(column)?;
            ctx.apply_s(column)?;
        }
    }
    for (column, negative) in negative_zs.into_iter().enumerate() {
        if negative {
            ctx.apply_h(column)?;
        }
    }
    for column in 0..num_qubits {
        if ctx
            .remaining
            .x_output_sign(column)
            .map_err(map_tableau_error)?
            == PauliSign::Minus
        {
            ctx.apply_s(column)?;
            ctx.apply_s(column)?;
        }
    }

    ctx.flush_pending()?;
    if ctx.circuit.count_qubits() < num_qubits && num_qubits != 0 {
        ctx.apply_h(num_qubits - 1)?;
        ctx.apply_h(num_qubits - 1)?;
        ctx.flush_pending()?;
    }

    let final_tableau = ctx.remaining.finish().map_err(map_tableau_error)?;
    if !final_tableau.is_identity() {
        return Err(AnalysisError::invalid_tableau_conversion(
            "tableau synthesis did not eliminate every generator",
        ));
    }
    Ok(ctx.circuit)
}

fn qubit_target(index: usize) -> AnalysisResult<Target> {
    let index = u32::try_from(index).map_err(|_| {
        AnalysisError::invalid_tableau_conversion(format!(
            "tableau synthesis qubit index {index} does not fit in a Stim target"
        ))
    })?;
    Ok(Target::qubit(QubitId::new(index)?, false))
}

fn map_tableau_error(error: stab_algebra::StabilizerError) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(error.to_string())
}
