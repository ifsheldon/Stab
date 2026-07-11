use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemInstruction,
    DemRepeatBlock, DemTarget, DetectorErrorModel, Pauli, Probability, RepeatCount, Target,
    sparse_rev_frame_tracker::{AnalyzerProbeBudget, SparseReverseFrameTracker},
};

use super::{
    ErrorAnalyzerOptions,
    decompose::decompose_tagged_error_probabilities,
    effects::AnalyzerPauli,
    error_decomp::{
        depolarize1_independent_channel_probability, depolarize2_independent_channel_probability,
        pauli_channel2_components,
    },
    probabilities::{merge_disjoint_probability, merge_independent_probability},
    try_disjoint_to_independent_xyz_errors,
};

mod local_decomposition;
mod output;
#[cfg(test)]
mod tests;

use local_decomposition::{
    locally_decompose_combinations, merge_indistinguishable_disjoint_probabilities,
};
use output::unreverse_model;

#[cfg(not(test))]
const MAX_LOOP_CYCLE_STEPS: u64 = 1_000_000;
#[cfg(test)]
const MAX_LOOP_CYCLE_STEPS: u64 = 4_096;
const MAX_BOUNDED_REPEAT_UNROLL: u64 = 100_000;

type ErrorKey = (Vec<DemTarget>, Option<String>);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ReverseFoldDiagnostics {
    pub(super) recurrence_search_steps: u64,
    pub(super) recurrences_found: u64,
    pub(super) max_recurrence_period: u64,
    pub(super) represented_repeat_iterations: u64,
    pub(super) folded_repeat_iterations: u64,
    pub(super) max_boundary_entries: u64,
    pub(super) emitted_compact_dem_items: u64,
}

#[cfg(feature = "ops-contracts")]
impl From<ReverseFoldDiagnostics> for super::ErrorAnalyzerDiagnostics {
    fn from(diagnostics: ReverseFoldDiagnostics) -> Self {
        Self {
            used_reverse_fold: true,
            used_bounded_fallback: false,
            recurrence_search_steps: diagnostics.recurrence_search_steps,
            recurrences_found: diagnostics.recurrences_found,
            max_recurrence_period: diagnostics.max_recurrence_period,
            represented_repeat_iterations: diagnostics.represented_repeat_iterations,
            folded_repeat_iterations: diagnostics.folded_repeat_iterations,
            max_boundary_entries: diagnostics.max_boundary_entries,
            emitted_compact_dem_items: diagnostics.emitted_compact_dem_items,
        }
    }
}

pub(super) fn try_analyze(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<Option<DetectorErrorModel>> {
    if contains_unsupported_reverse_fold_instruction(circuit) {
        return Ok(None);
    }
    let (model, _) = ReverseFoldAnalyzer::new(circuit, options)?.analyze(circuit)?;
    Ok(Some(model))
}

#[cfg(feature = "ops-contracts")]
pub(super) fn try_analyze_with_diagnostics(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<Option<(DetectorErrorModel, ReverseFoldDiagnostics)>> {
    if contains_unsupported_reverse_fold_instruction(circuit) {
        return Ok(None);
    }
    ReverseFoldAnalyzer::new(circuit, options)?
        .analyze(circuit)
        .map(Some)
}

struct ReverseFoldAnalyzer {
    options: ErrorAnalyzerOptions,
    tracker: SparseReverseFrameTracker,
    ticks_left: u64,
    reversed_model: DetectorErrorModel,
    error_probabilities: BTreeMap<ErrorKey, Probability>,
    probe_budget: AnalyzerProbeBudget,
    diagnostics: ReverseFoldDiagnostics,
}

impl ReverseFoldAnalyzer {
    fn new(circuit: &Circuit, options: ErrorAnalyzerOptions) -> CircuitResult<Self> {
        let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "analyze_errors measurement count does not fit usize",
            )
        })?;
        let detector_count = circuit.count_detectors()?;
        Ok(Self {
            options,
            tracker: SparseReverseFrameTracker::new_for_error_analysis(
                circuit.count_qubits(),
                measurement_count,
                detector_count,
                options.allow_gauge_detectors,
            ),
            ticks_left: circuit.count_ticks()?,
            reversed_model: DetectorErrorModel::new(),
            error_probabilities: BTreeMap::new(),
            probe_budget: AnalyzerProbeBudget::new(MAX_LOOP_CYCLE_STEPS),
            diagnostics: ReverseFoldDiagnostics {
                represented_repeat_iterations: represented_repeat_iterations(circuit, 1),
                ..ReverseFoldDiagnostics::default()
            },
        })
    }

    fn analyze(
        mut self,
        circuit: &Circuit,
    ) -> CircuitResult<(DetectorErrorModel, ReverseFoldDiagnostics)> {
        self.undo_circuit(circuit)?;
        self.tracker.undo_implicit_rz_at_start_of_circuit()?;
        self.collect_gauge_errors()?;
        self.flush()?;
        let mut base_detector_id = 0_u64;
        let mut seen = BTreeSet::new();
        let model = unreverse_model(&self.reversed_model, &mut base_detector_id, &mut seen)?;
        self.diagnostics.emitted_compact_dem_items = compact_dem_item_count(&model);
        Ok((model, self.diagnostics))
    }

    fn undo_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.undo_instruction(instruction)?;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    self.run_loop(repeat.body(), repeat.repeat_count(), repeat.tag())?;
                }
            }
        }
        Ok(())
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        match gate_name {
            "X_ERROR" => self.record_single_pauli_errors(instruction, Pauli::X)?,
            "Y_ERROR" => self.record_single_pauli_errors(instruction, Pauli::Y)?,
            "Z_ERROR" => self.record_single_pauli_errors(instruction, Pauli::Z)?,
            "DEPOLARIZE1" => self.record_depolarize1(instruction)?,
            "DEPOLARIZE2" => self.record_depolarize2(instruction)?,
            "PAULI_CHANNEL_1" => self.record_pauli_channel1(instruction)?,
            "PAULI_CHANNEL_2" => self.record_pauli_channel2(instruction)?,
            "E" | "CORRELATED_ERROR" => self.record_correlated_error(instruction)?,
            "I_ERROR" | "II_ERROR" => {}
            "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => {
                self.record_measurement_errors(instruction, instruction.targets().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            "MXX" | "MYY" | "MZZ" | "MPP" => {
                self.record_measurement_errors(instruction, instruction.target_groups().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            "MPAD" => {
                self.record_measurement_errors(instruction, instruction.targets().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            "DETECTOR" => {
                self.tracker.undo_instruction(instruction)?;
                self.reversed_model
                    .push_instruction(DemInstruction::detector(
                        instruction.args().to_vec(),
                        DemTarget::relative_detector(self.tracker.detector_count())?,
                        instruction.tag().map(ToOwned::to_owned),
                    )?);
            }
            "OBSERVABLE_INCLUDE" => {
                self.tracker.undo_instruction(instruction)?;
                let observable = instruction.observable_id_argument()?.ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "OBSERVABLE_INCLUDE is missing an observable id",
                    )
                })?;
                self.reversed_model
                    .push_instruction(DemInstruction::logical_observable(
                        DemTarget::logical_observable(observable.get())?,
                        instruction.tag().map(ToOwned::to_owned),
                    )?);
            }
            "SHIFT_COORDS" => {
                self.reversed_model
                    .push_instruction(DemInstruction::shift_detectors(
                        instruction.args().to_vec(),
                        0,
                        instruction.tag().map(ToOwned::to_owned),
                    )?);
            }
            "TICK" => {
                self.ticks_left = self.ticks_left.checked_sub(1).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "tick count underflowed during folded error analysis",
                    )
                })?;
            }
            _ => self.tracker.undo_instruction(instruction)?,
        }
        self.collect_gauge_errors()
    }

    fn record_measurement_errors(
        &mut self,
        instruction: &CircuitInstruction,
        measurement_count: usize,
    ) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let end = self.tracker.measurement_count();
        let start = end.checked_sub(measurement_count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} consumes more measurements than remain during folded error analysis",
                instruction.gate().canonical_name()
            ))
        })?;
        for index in (start..end).rev() {
            let targets = self.tracker.record_targets_at(index)?;
            self.add_independent_error(
                probability,
                targets,
                instruction.tag().map(ToOwned::to_owned),
            )?;
        }
        Ok(())
    }

    fn record_single_pauli_errors(
        &mut self,
        instruction: &CircuitInstruction,
        pauli: Pauli,
    ) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            let targets = self.tracker.error_sensitivity(qubit, pauli)?;
            self.add_independent_error(
                probability,
                targets,
                instruction.tag().map(ToOwned::to_owned),
            )?;
        }
        Ok(())
    }

    fn record_correlated_error(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let targets = self.composite_error_targets(instruction.targets())?;
        self.add_independent_error(
            probability,
            targets,
            instruction.tag().map(ToOwned::to_owned),
        )
    }

    fn record_depolarize1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let independent = depolarize1_independent_channel_probability(probability)?;
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE1 target {target} is not a qubit"
                ))
            })?;
            self.record_error_combinations(
                vec![
                    Probability::try_new(0.0)?,
                    independent,
                    independent,
                    independent,
                ],
                vec![
                    self.tracker.error_sensitivity(qubit, Pauli::Z)?,
                    self.tracker.error_sensitivity(qubit, Pauli::X)?,
                ],
                false,
                instruction.tag(),
            )?;
        }
        Ok(())
    }

    fn record_depolarize2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let independent = depolarize2_independent_channel_probability(probability)?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 expected paired qubit targets",
                ));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model("DEPOLARIZE2 left target is not a qubit")
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 right target is not a qubit",
                )
            })?;
            let mut probabilities = vec![independent; 16];
            let first = probabilities.first_mut().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 probability table was unexpectedly empty",
                )
            })?;
            *first = Probability::try_new(0.0)?;
            self.record_error_combinations(
                probabilities,
                vec![
                    self.tracker.error_sensitivity(left, Pauli::Z)?,
                    self.tracker.error_sensitivity(left, Pauli::X)?,
                    self.tracker.error_sensitivity(right, Pauli::Z)?,
                    self.tracker.error_sensitivity(right, Pauli::X)?,
                ],
                false,
                instruction.tag(),
            )?;
        }
        Ok(())
    }

    fn record_pauli_channel1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [x, y, z] = probabilities.as_slice() else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 expected three probabilities",
            ));
        };
        let independent = try_disjoint_to_independent_xyz_errors(*x, *y, *z)?;
        if independent.is_none() {
            self.validate_disjoint_threshold("PAULI_CHANNEL_1", &probabilities)?;
        }
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model("PAULI_CHANNEL_1 target is not a qubit")
            })?;
            let components = if let Some(independent) = independent {
                [
                    (independent.x, Pauli::X),
                    (independent.y, Pauli::Y),
                    (independent.z, Pauli::Z),
                ]
            } else {
                [(*x, Pauli::X), (*y, Pauli::Y), (*z, Pauli::Z)]
            };
            if independent.is_some() {
                for (probability, pauli) in components {
                    self.add_independent_error(
                        probability,
                        self.tracker.error_sensitivity(qubit, pauli)?,
                        instruction.tag().map(ToOwned::to_owned),
                    )?;
                }
            } else {
                let disjoint = components
                    .into_iter()
                    .map(|(probability, pauli)| {
                        Ok((probability, self.tracker.error_sensitivity(qubit, pauli)?))
                    })
                    .collect::<CircuitResult<Vec<_>>>()?;
                self.add_disjoint_components(disjoint, instruction.tag())?;
            }
        }
        Ok(())
    }

    fn record_pauli_channel2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        self.validate_disjoint_threshold("PAULI_CHANNEL_2", &probabilities)?;
        let probabilities: [Probability; 15] =
            probabilities.try_into().map_err(|values: Vec<_>| {
                CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 expected 15 probabilities, got {}",
                    values.len()
                ))
            })?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 expected paired qubit targets",
                ));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 left target is not a qubit",
                )
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 right target is not a qubit",
                )
            })?;
            let disjoint = pauli_channel2_components(probabilities)
                .map(|(probability, left_pauli, right_pauli)| {
                    Ok((
                        probability,
                        self.paired_error_targets(
                            left_pauli.map(|pauli| (left, analyzer_pauli(pauli))),
                            right_pauli.map(|pauli| (right, analyzer_pauli(pauli))),
                        )?,
                    ))
                })
                .collect::<CircuitResult<Vec<_>>>()?;
            self.add_disjoint_components(disjoint, instruction.tag())?;
        }
        Ok(())
    }

    fn validate_disjoint_threshold(
        &self,
        gate_name: &str,
        probabilities: &[Probability],
    ) -> CircuitResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{gate_name} requires approximate_disjoint_errors during error analysis"
            )));
        };
        if let Some(probability) = probabilities
            .iter()
            .find(|probability| probability.get() > threshold.get())
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{gate_name} has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                probability.get(),
                threshold.get()
            )));
        }
        Ok(())
    }

    fn add_disjoint_components(
        &mut self,
        components: impl IntoIterator<Item = (Probability, BTreeSet<DemTarget>)>,
        tag: Option<&str>,
    ) -> CircuitResult<()> {
        let mut disjoint = BTreeMap::new();
        for (probability, targets) in components {
            if probability.get() == 0.0 || targets.is_empty() {
                continue;
            }
            merge_disjoint_probability(
                &mut disjoint,
                targets.into_iter().collect::<Vec<_>>(),
                probability,
            )?;
        }
        for (targets, probability) in disjoint {
            self.add_independent_error(
                probability,
                targets.into_iter().collect(),
                tag.map(ToOwned::to_owned),
            )?;
        }
        Ok(())
    }

    fn record_error_combinations(
        &mut self,
        mut probabilities: Vec<Probability>,
        basis_errors: Vec<BTreeSet<DemTarget>>,
        probabilities_are_disjoint: bool,
        tag: Option<&str>,
    ) -> CircuitResult<()> {
        let combination_count = 1_usize
            .checked_shl(u32::try_from(basis_errors.len()).map_err(|_| {
                CircuitError::invalid_detector_error_model(
                    "composite error basis count does not fit u32",
                )
            })?)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "composite error combination count overflowed",
                )
            })?;
        if probabilities.len() != combination_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "composite error expected {combination_count} probabilities, got {}",
                probabilities.len()
            )));
        }

        let mut targets = vec![BTreeSet::new(); combination_count];
        for mask in 1..combination_count {
            let bit = mask.trailing_zeros() as usize;
            let previous = mask & (mask - 1);
            let basis = basis_errors.get(bit).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "composite error basis index is out of range",
                )
            })?;
            let mut combined = targets.get(previous).cloned().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "composite error predecessor is out of range",
                )
            })?;
            toggle_targets(&mut combined, basis.clone());
            let slot = targets.get_mut(mask).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "composite error target index is out of range",
                )
            })?;
            *slot = combined;
        }

        let mut target_vectors = targets
            .into_iter()
            .map(|targets| targets.into_iter().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        if self.options.decompose_errors {
            locally_decompose_combinations(&basis_errors, &mut target_vectors)?;
        }
        if probabilities_are_disjoint {
            merge_indistinguishable_disjoint_probabilities(&target_vectors, &mut probabilities)?;
        }
        for (probability, targets) in probabilities.into_iter().zip(target_vectors).skip(1) {
            self.add_independent_error_targets(probability, targets, tag.map(ToOwned::to_owned))?;
        }
        Ok(())
    }

    fn composite_error_targets(&self, targets: &[Target]) -> CircuitResult<BTreeSet<DemTarget>> {
        let mut result = BTreeSet::new();
        for target in targets {
            let pauli = target.pauli_type().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "correlated error target {target} is not a Pauli target"
                ))
            })?;
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "correlated error target {target} does not identify a qubit"
                ))
            })?;
            toggle_targets(&mut result, self.tracker.error_sensitivity(qubit, pauli)?);
        }
        Ok(result)
    }

    fn paired_error_targets(
        &self,
        left: Option<(crate::QubitId, Pauli)>,
        right: Option<(crate::QubitId, Pauli)>,
    ) -> CircuitResult<BTreeSet<DemTarget>> {
        let mut result = BTreeSet::new();
        for (qubit, pauli) in left.into_iter().chain(right) {
            toggle_targets(&mut result, self.tracker.error_sensitivity(qubit, pauli)?);
        }
        Ok(result)
    }

    fn add_independent_error(
        &mut self,
        probability: Probability,
        targets: BTreeSet<DemTarget>,
        tag: Option<String>,
    ) -> CircuitResult<()> {
        self.add_independent_error_targets(probability, targets.into_iter().collect(), tag)
    }

    fn add_independent_error_targets(
        &mut self,
        probability: Probability,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> CircuitResult<()> {
        if probability.get() == 0.0 || targets.is_empty() {
            return Ok(());
        }
        merge_independent_probability(&mut self.error_probabilities, (targets, tag), probability)
    }

    fn collect_gauge_errors(&mut self) -> CircuitResult<()> {
        for targets in self.tracker.take_gauge_errors() {
            self.add_independent_error(Probability::try_new(0.5)?, targets, None)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> CircuitResult<()> {
        let mut probabilities = std::mem::take(&mut self.error_probabilities);
        if self.options.decompose_errors {
            probabilities = decompose_tagged_error_probabilities(
                probabilities,
                self.options
                    .block_decomposition_from_introducing_remnant_edges,
                self.options.ignore_decomposition_failures,
            )?;
        }
        for ((targets, tag), probability) in probabilities.into_iter().rev() {
            if probability.get() != 0.0 && !targets.is_empty() {
                self.reversed_model.push_instruction(DemInstruction::error(
                    probability,
                    targets,
                    tag,
                )?);
            }
        }
        Ok(())
    }

    fn run_loop(
        &mut self,
        body: &Circuit,
        repeat_count: RepeatCount,
        tag: Option<&str>,
    ) -> CircuitResult<()> {
        let iterations = repeat_count.get();
        if iterations == 0 {
            return Ok(());
        }
        self.observe_boundary_entries(self.tracker.boundary_entry_count());
        if !self.options.fold_loops {
            return self.undo_loop_by_unrolling(body, iterations);
        }

        let mut tortoise = self.tracker.clone();
        let mut hare = self.tracker.clone();
        let mut hare_iterations = 0_u64;
        let mut tortoise_iterations = 0_u64;
        let mut found_cycle = false;
        while hare_iterations < iterations && hare_iterations < MAX_LOOP_CYCLE_STEPS {
            hare.undo_circuit_for_analyzer_probe(body, &mut self.probe_budget)?;
            self.record_recurrence_probe(&hare);
            hare_iterations = checked_add(hare_iterations, 1, "hare iteration")?;
            if hare.is_shifted_copy(&tortoise) {
                found_cycle = true;
                break;
            }
            if hare_iterations.is_multiple_of(2) {
                tortoise.undo_circuit_for_analyzer_probe(body, &mut self.probe_budget)?;
                self.record_recurrence_probe(&tortoise);
                tortoise_iterations = checked_add(tortoise_iterations, 1, "tortoise iteration")?;
                if hare.is_shifted_copy(&tortoise) {
                    found_cycle = true;
                    break;
                }
            }
        }

        if !found_cycle {
            if iterations > MAX_BOUNDED_REPEAT_UNROLL {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors found no loop-state recurrence within {MAX_LOOP_CYCLE_STEPS} iterations for repeat count {iterations}"
                )));
            }
            return self.undo_loop_by_unrolling(body, iterations);
        }

        for _ in 0..tortoise_iterations {
            self.undo_circuit(body)?;
        }

        if hare_iterations < iterations {
            let period = hare_iterations
                .checked_sub(tortoise_iterations)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "folded analyzer recurrence period underflowed",
                    )
                })?;
            if period == 0 {
                return Err(CircuitError::invalid_detector_error_model(
                    "folded analyzer recurrence period was zero",
                ));
            }
            self.diagnostics.recurrences_found =
                self.diagnostics.recurrences_found.saturating_add(1);
            self.diagnostics.max_recurrence_period =
                self.diagnostics.max_recurrence_period.max(period);
            let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "folded analyzer repeat remainder underflowed",
                )
            })?;
            let period_repetitions = remaining / period;
            if period_repetitions > 1 {
                self.capture_repeated_period(
                    body,
                    period,
                    period_repetitions,
                    &hare,
                    &mut tortoise_iterations,
                    tag,
                )?;
            }
        }

        let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "folded analyzer repeat remainder underflowed",
            )
        })?;
        self.undo_loop_by_unrolling(body, remaining)
    }

    fn record_recurrence_probe(&mut self, tracker: &SparseReverseFrameTracker) {
        self.diagnostics.recurrence_search_steps = self.probe_budget.consumed_steps();
        self.observe_boundary_entries(tracker.boundary_entry_count())
    }

    fn observe_boundary_entries(&mut self, entries: usize) {
        let entries = u64::try_from(entries).unwrap_or(u64::MAX);
        self.diagnostics.max_boundary_entries = self.diagnostics.max_boundary_entries.max(entries);
    }

    fn capture_repeated_period(
        &mut self,
        body: &Circuit,
        period: u64,
        period_repetitions: u64,
        hare: &SparseReverseFrameTracker,
        tortoise_iterations: &mut u64,
        tag: Option<&str>,
    ) -> CircuitResult<()> {
        let measurements_per_period = self
            .tracker
            .measurement_count()
            .checked_sub(hare.measurement_count())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "folded analyzer measurement period underflowed",
                )
            })?;
        let detectors_per_period = self
            .tracker
            .detector_count()
            .checked_sub(hare.detector_count())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "folded analyzer detector period underflowed",
                )
            })?;

        self.flush()?;
        let outer_reversed = std::mem::take(&mut self.reversed_model);
        let skipped_periods = period_repetitions.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "folded analyzer skipped period count underflowed",
            )
        })?;
        let folded_iterations =
            checked_product_u64(period, skipped_periods, "folded repeat iteration")?;
        self.diagnostics.folded_repeat_iterations = self
            .diagnostics
            .folded_repeat_iterations
            .saturating_add(folded_iterations);
        let skipped_measurements =
            checked_product_usize(measurements_per_period, skipped_periods, "measurement skip")?;
        let skipped_detectors =
            checked_product_u64(detectors_per_period, skipped_periods, "detector skip")?;
        self.tracker.shift_counts(
            -i128::try_from(skipped_measurements).map_err(|_| {
                CircuitError::invalid_detector_error_model(
                    "folded analyzer measurement skip does not fit i128",
                )
            })?,
            -i128::from(skipped_detectors),
        )?;
        *tortoise_iterations = checked_add(
            *tortoise_iterations,
            folded_iterations,
            "tortoise skipped iteration",
        )?;

        for _ in 0..period {
            self.undo_circuit(body)?;
            *tortoise_iterations =
                checked_add(*tortoise_iterations, 1, "captured period iteration")?;
        }
        self.flush()?;
        let mut period_body = std::mem::take(&mut self.reversed_model);
        let nested_shift = period_body.total_detector_shift()?;
        let remaining_shift = detectors_per_period
            .checked_sub(nested_shift)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "folded analyzer nested detector shifts exceed the period delta",
                )
            })?;
        if remaining_shift > 0 {
            period_body = prepend_detector_shift(period_body, remaining_shift)?;
        }

        self.reversed_model = outer_reversed;
        self.reversed_model.push_repeat_block(DemRepeatBlock::new(
            RepeatCount::try_new(period_repetitions)?,
            period_body,
            tag.map(ToOwned::to_owned),
        ));
        Ok(())
    }

    fn undo_loop_by_unrolling(&mut self, body: &Circuit, iterations: u64) -> CircuitResult<()> {
        for _ in 0..iterations {
            self.undo_circuit(body)?;
        }
        Ok(())
    }
}

fn contains_unsupported_reverse_fold_instruction(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => matches!(
            instruction.gate().canonical_name(),
            "ELSE_CORRELATED_ERROR" | "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
        ),
        CircuitItem::RepeatBlock(repeat) => {
            contains_unsupported_reverse_fold_instruction(repeat.body())
        }
    })
}

fn represented_repeat_iterations(circuit: &Circuit, multiplier: u64) -> u64 {
    let mut total = 0_u64;
    for item in circuit.items() {
        let CircuitItem::RepeatBlock(repeat) = item else {
            continue;
        };
        let repeated = multiplier.saturating_mul(repeat.repeat_count().get());
        total = total.saturating_add(repeated);
        total = total.saturating_add(represented_repeat_iterations(repeat.body(), repeated));
    }
    total
}

fn analyzer_pauli(pauli: AnalyzerPauli) -> Pauli {
    match pauli {
        AnalyzerPauli::X => Pauli::X,
        AnalyzerPauli::Y => Pauli::Y,
        AnalyzerPauli::Z => Pauli::Z,
    }
}

fn toggle_targets(target: &mut BTreeSet<DemTarget>, values: BTreeSet<DemTarget>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}

fn prepend_detector_shift(
    model: DetectorErrorModel,
    detector_shift: u64,
) -> CircuitResult<DetectorErrorModel> {
    let mut result = DetectorErrorModel::new();
    let mut items = model.items().iter();
    match items.next() {
        Some(crate::DemItem::Instruction(instruction))
            if instruction.kind() == crate::DemInstructionKind::ShiftDetectors =>
        {
            let combined = instruction
                .detector_shift()?
                .checked_add(detector_shift)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "folded analyzer detector shift overflowed",
                    )
                })?;
            result.push_instruction(DemInstruction::shift_detectors(
                instruction.args().to_vec(),
                combined,
                instruction.tag().map(ToOwned::to_owned),
            )?);
        }
        Some(first) => {
            result.push_instruction(DemInstruction::shift_detectors(
                Vec::new(),
                detector_shift,
                None,
            )?);
            push_items(&mut result, std::slice::from_ref(first));
        }
        None => {
            result.push_instruction(DemInstruction::shift_detectors(
                Vec::new(),
                detector_shift,
                None,
            )?);
        }
    }
    for item in items {
        push_items(&mut result, std::slice::from_ref(item));
    }
    Ok(result)
}

fn push_items(target: &mut DetectorErrorModel, items: &[crate::DemItem]) {
    for item in items {
        match item {
            crate::DemItem::Instruction(instruction) => {
                target.push_instruction(instruction.clone());
            }
            crate::DemItem::RepeatBlock(repeat) => target.push_repeat_block(repeat.clone()),
        }
    }
}

pub(super) fn compact_dem_item_count(model: &DetectorErrorModel) -> u64 {
    let mut count = 0_u64;
    for item in model.items() {
        count = count.saturating_add(1);
        if let crate::DemItem::RepeatBlock(repeat) = item {
            count = count.saturating_add(compact_dem_item_count(repeat.body()));
        }
    }
    count
}

fn checked_add(left: u64, right: u64, context: &str) -> CircuitResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_u64(left: u64, right: u64, context: &str) -> CircuitResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_usize(left: usize, right: u64, context: &str) -> CircuitResult<usize> {
    left.checked_mul(usize::try_from(right).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "folded analyzer {context} does not fit usize"
        ))
    })?)
    .ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}
