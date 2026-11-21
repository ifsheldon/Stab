use stab_algebra::{StabilizerError, Tableau, TableauAppender};
use stab_model::advanced::CancellationCountdown;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, GateCategory, MemoryBudget, Target,
};

use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, NonUnitaryReason, ResourceKind,
    ResourceLimitError,
};

/// Options for [`circuit_to_tableau_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CircuitToTableauOptions {
    /// Skip noise channels instead of refusing a circuit with nonzero noise.
    pub ignore_noise: bool,
    /// Skip measurements instead of refusing them.
    pub ignore_measurement: bool,
    /// Skip resets instead of refusing them.
    pub ignore_reset: bool,
    /// Budget for the Tableau and the repeat powers composed into it.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per instruction and before every Tableau composition.
    pub cancellation: Cancellation,
}

/// Converts a unitary circuit into its Tableau, admitted against the default memory budget.
///
/// See [`circuit_to_tableau_with_options`].
pub fn circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
) -> AnalysisResult<Tableau> {
    circuit_to_tableau_with_options(
        circuit,
        &CircuitToTableauOptions {
            ignore_noise,
            ignore_measurement,
            ignore_reset,
            ..CircuitToTableauOptions::default()
        },
    )
}

/// Converts a unitary circuit into its Tableau.
///
/// The Tableau's packed rows are admitted against the options' memory budget before
/// materialization, and every repeat body is raised to its repeat count by squaring, so a count up
/// to `2^63 - 1` costs at most 126 compositions of `O(n^3 / 64)` word operations each; the
/// cancellation signal is polled before every composition and once per instruction. Runs of gates
/// are appended through a [`TableauAppender`], so a one- or two-qubit gate costs `O(n / 64)` word
/// operations, plus one `O(n^2 / 64)` transpose around each repeat block.
pub fn circuit_to_tableau_with_options(
    circuit: &Circuit,
    options: &CircuitToTableauOptions,
) -> AnalysisResult<Tableau> {
    options.cancellation.check()?;
    let num_qubits = circuit.simulated_qubit_count();
    let budget = options.memory_budget;
    let result =
        TableauAppender::identity_with_budget(num_qubits, budget).map_err(map_tableau_error)?;
    let mut conversion = Conversion {
        options,
        countdown: CancellationCountdown::new(),
    };
    conversion
        .apply_circuit(circuit, result)?
        .finish()
        .map_err(map_tableau_error)
}

struct Conversion<'a> {
    options: &'a CircuitToTableauOptions,
    countdown: CancellationCountdown,
}

impl Conversion<'_> {
    fn apply_circuit(
        &mut self,
        circuit: &Circuit,
        mut result: TableauAppender,
    ) -> AnalysisResult<TableauAppender> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.countdown.tick(&self.options.cancellation)?;
                    result = self.apply_instruction(instruction, result)?;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    // The body has the result's width, so it fits wherever the result does.
                    let body = self
                        .apply_circuit(repeat.body(), identity_appender(result.len())?)?
                        .finish()
                        .map_err(map_tableau_error)?;
                    if body.is_identity() {
                        continue;
                    }
                    let repeated = self.power(body, repeat.repeat_count().get())?;
                    if repeated.is_identity() {
                        continue;
                    }
                    let prefix = result.finish().map_err(map_tableau_error)?;
                    let combined = if prefix.is_identity() {
                        repeated
                    } else {
                        self.compose(&prefix, &repeated)?
                    };
                    result = TableauAppender::new(combined).map_err(map_tableau_error)?;
                }
            }
        }
        Ok(result)
    }

    /// Raises `base` to `exponent` by squaring, polling cancellation before each composition.
    fn power(&self, base: Tableau, mut exponent: u64) -> AnalysisResult<Tableau> {
        let mut result: Option<Tableau> = None;
        let mut power = base;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = Some(match result {
                    None => power.clone(),
                    Some(accumulated) => self.compose(&accumulated, &power)?,
                });
            }
            exponent >>= 1;
            if exponent == 0 || power.is_identity() {
                break;
            }
            power = self.compose(&power, &power)?;
        }
        match result {
            Some(result) => Ok(result),
            None => identity_like(&power),
        }
    }

    fn compose(&self, first: &Tableau, second: &Tableau) -> AnalysisResult<Tableau> {
        self.options.cancellation.check()?;
        first.then(second).map_err(map_tableau_error)
    }

    fn apply_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        mut result: TableauAppender,
    ) -> AnalysisResult<TableauAppender> {
        let gate = instruction.gate();
        let options = self.options;

        if !options.ignore_measurement && gate.produces_measurements() {
            return Err(AnalysisError::non_unitary(
                AnalysisOperation::TableauConversion,
                gate.kind(),
                NonUnitaryReason::Measurement,
            ));
        }
        if !options.ignore_reset && gate.is_reset() {
            return Err(AnalysisError::non_unitary(
                AnalysisOperation::TableauConversion,
                gate.kind(),
                NonUnitaryReason::Reset,
            ));
        }
        if !options.ignore_noise
            && gate.is_noisy()
            && instruction.args().iter().any(|argument| *argument > 0.0)
        {
            return Err(AnalysisError::non_unitary(
                AnalysisOperation::TableauConversion,
                gate.kind(),
                NonUnitaryReason::Noise,
            ));
        }

        match gate.category() {
            GateCategory::Annotation
            | GateCategory::Collapsing
            | GateCategory::ControlFlow
            | GateCategory::HeraldedNoise
            | GateCategory::Noise
            | GateCategory::PairMeasurement => Ok(result),
            GateCategory::Controlled
            | GateCategory::HadamardLike
            | GateCategory::Pauli
            | GateCategory::Period3
            | GateCategory::Period4
            | GateCategory::ParityPhasing
            | GateCategory::Swap => {
                let local =
                    crate::gate::fixed_unitary_tableau(gate, AnalysisOperation::TableauConversion)?;
                for group in instruction.target_groups() {
                    apply_unitary_group_to_tableau(
                        gate.canonical_name(),
                        &local,
                        group,
                        &mut result,
                    )?;
                }
                Ok(result)
            }
            GateCategory::PauliProduct if !gate.is_unitary() => Ok(result),
            GateCategory::PauliProduct => {
                let decomposed = crate::circuit_simplify::decomposed_single_instruction(
                    instruction,
                )
                .map_err(|error| {
                    error.into_nested_pauli_product_error(AnalysisOperation::TableauConversion)
                })?;
                self.apply_circuit(&decomposed, result)
            }
        }
    }
}

/// An identity appender of an already admitted width.
fn identity_appender(num_qubits: usize) -> AnalysisResult<TableauAppender> {
    TableauAppender::identity_with_budget(num_qubits, MemoryBudget::UNLIMITED)
        .map_err(map_tableau_error)
}

/// An identity Tableau of `tableau`'s width, which is already admitted.
fn identity_like(tableau: &Tableau) -> AnalysisResult<Tableau> {
    Tableau::identity_with_budget(tableau.len(), MemoryBudget::UNLIMITED).map_err(map_tableau_error)
}

fn apply_unitary_group_to_tableau(
    gate_name: &str,
    local: &Tableau,
    targets: &[Target],
    result: &mut TableauAppender,
) -> AnalysisResult<()> {
    // One- and two-qubit groups, nearly every group, convert their targets on the stack.
    let mut narrow = [0_usize; 2];
    let wide;
    let target_ids: &[usize] = match narrow.get_mut(..targets.len()) {
        Some(slots) => {
            for (slot, target) in slots.iter_mut().zip(targets) {
                *slot = target_qubit_index(gate_name, target)?;
            }
            slots
        }
        None => {
            wide = targets
                .iter()
                .map(|target| target_qubit_index(gate_name, target))
                .collect::<AnalysisResult<Vec<_>>>()?;
            &wide
        }
    };
    if local.len() != target_ids.len() {
        return Err(AnalysisError::unsupported(
            AnalysisOperation::TableauConversion,
            None,
            format!(
                "gate {gate_name} expected {} tableau targets but got {}",
                local.len(),
                target_ids.len()
            ),
        ));
    }
    result.append(local, target_ids).map_err(map_tableau_error)
}

fn target_qubit_index(gate_name: &str, target: &Target) -> AnalysisResult<usize> {
    target
        .qubit_id()
        .map(|qubit| qubit.get() as usize)
        .ok_or_else(|| {
            AnalysisError::unsupported(
                AnalysisOperation::TableauConversion,
                None,
                format!("gate {gate_name} has non-qubit tableau target {target}"),
            )
        })
}

/// Maps algebra failures, reporting a refused Tableau as a memory-budget refusal.
pub(crate) fn map_tableau_error(error: StabilizerError) -> AnalysisError {
    match error {
        StabilizerError::MemoryBudgetExceeded { exceeded, .. } => {
            ResourceLimitError::budget_refusal(
                AnalysisOperation::TableauConversion,
                ResourceKind::MaterializedBytes,
                exceeded,
            )
            .into()
        }
        other => AnalysisError::internal(AnalysisOperation::TableauConversion, other.to_string()),
    }
}
