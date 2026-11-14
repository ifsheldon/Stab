use stab_algebra::{AlgebraMemoryBudget, StabilizerError, Tableau};
use stab_model::advanced::CancellationCountdown;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, GateCategory, MemoryBudget, QubitId,
    Target,
};

use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

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
/// cancellation signal is polled before every composition and once per instruction.
pub fn circuit_to_tableau_with_options(
    circuit: &Circuit,
    options: &CircuitToTableauOptions,
) -> AnalysisResult<Tableau> {
    options.cancellation.check()?;
    let num_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    let budget = AlgebraMemoryBudget::from_bytes(options.memory_budget.bytes());
    let mut result =
        Tableau::identity_with_budget(num_qubits, budget).map_err(map_tableau_error)?;
    let mut conversion = Conversion {
        options,
        countdown: CancellationCountdown::new(),
    };
    conversion.apply_circuit(circuit, &mut result)?;
    Ok(result)
}

struct Conversion<'a> {
    options: &'a CircuitToTableauOptions,
    countdown: CancellationCountdown,
}

impl Conversion<'_> {
    fn apply_circuit(&mut self, circuit: &Circuit, result: &mut Tableau) -> AnalysisResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.countdown.tick(&self.options.cancellation)?;
                    self.apply_instruction(instruction, result)?;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    // The body has the result's width, so it fits wherever the result does.
                    let mut body = identity_like(result)?;
                    self.apply_circuit(repeat.body(), &mut body)?;
                    if body.is_identity() {
                        continue;
                    }
                    let repeated = self.power(body, repeat.repeat_count().get())?;
                    if repeated.is_identity() {
                        continue;
                    }
                    *result = if result.is_identity() {
                        repeated
                    } else {
                        self.compose(result, &repeated)?
                    };
                }
            }
        }
        Ok(())
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
        result: &mut Tableau,
    ) -> AnalysisResult<()> {
        let gate = instruction.gate();
        let options = self.options;

        if !options.ignore_measurement && gate.produces_measurements() {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "measurement operation {}",
                gate.canonical_name()
            )));
        }
        if !options.ignore_reset && gate.is_reset() {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "reset operation {}",
                gate.canonical_name()
            )));
        }
        if !options.ignore_noise
            && gate.is_noisy()
            && instruction.args().iter().any(|argument| *argument > 0.0)
        {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "noisy operation {}",
                gate.canonical_name()
            )));
        }

        match gate.category() {
            GateCategory::Annotation
            | GateCategory::Collapsing
            | GateCategory::ControlFlow
            | GateCategory::HeraldedNoise
            | GateCategory::Noise
            | GateCategory::PairMeasurement => Ok(()),
            GateCategory::Controlled
            | GateCategory::HadamardLike
            | GateCategory::Pauli
            | GateCategory::Period3
            | GateCategory::Period4
            | GateCategory::ParityPhasing
            | GateCategory::Swap => {
                let local = crate::gate_tableau(gate)?;
                for group in instruction.target_groups() {
                    apply_unitary_group_to_tableau(gate.canonical_name(), &local, group, result)?;
                }
                Ok(())
            }
            GateCategory::PauliProduct if !gate.is_unitary() => Ok(()),
            GateCategory::PauliProduct => {
                let decomposed = crate::advanced::decomposed_single_instruction(instruction)
                    .map_err(|error| {
                        AnalysisError::invalid_tableau_conversion(error.to_string())
                    })?;
                self.apply_circuit(&decomposed, result)
            }
        }
    }
}

/// An identity Tableau of `tableau`'s width, which is already admitted.
fn identity_like(tableau: &Tableau) -> AnalysisResult<Tableau> {
    Tableau::identity_with_budget(tableau.len(), AlgebraMemoryBudget::from_bytes(u64::MAX))
        .map_err(map_tableau_error)
}

fn apply_unitary_group_to_tableau(
    gate_name: &str,
    local: &Tableau,
    targets: &[Target],
    result: &mut Tableau,
) -> AnalysisResult<()> {
    let target_ids = target_qubit_ids(gate_name, targets)?;
    if local.len() != target_ids.len() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "gate {gate_name} expected {} tableau targets but got {}",
            local.len(),
            target_ids.len()
        )));
    }
    let targets = target_ids
        .iter()
        .map(|target| target.get() as usize)
        .collect::<Vec<_>>();
    result.append(local, &targets).map_err(map_tableau_error)?;
    Ok(())
}

fn target_qubit_ids(gate_name: &str, targets: &[Target]) -> AnalysisResult<Vec<QubitId>> {
    targets
        .iter()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_tableau_conversion(format!(
                    "gate {gate_name} has non-qubit tableau target {target}"
                ))
            })
        })
        .collect()
}

/// Maps algebra failures, reporting a refused Tableau as a memory-budget refusal.
pub(crate) fn map_tableau_error(error: StabilizerError) -> AnalysisError {
    match error {
        StabilizerError::MemoryBudgetExceeded {
            requested_bytes,
            budget_bytes,
            ..
        } => ResourceLimitError::memory_budget(
            ResourceOperation::TableauConversion,
            ResourceKind::MaterializedBytes,
            u64::try_from(requested_bytes).unwrap_or(u64::MAX),
            budget_bytes,
        )
        .into(),
        other => AnalysisError::invalid_tableau_conversion(other.to_string()),
    }
}
