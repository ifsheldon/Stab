use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemInstruction,
    DemRepeatBlock, DemTarget, DetectorErrorModel, Pauli, Probability, RepeatCount, Target,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
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

mod output;

use output::unreverse_model;

const MAX_LOOP_CYCLE_STEPS: u64 = 1_000_000;
const MAX_BOUNDED_REPEAT_UNROLL: u64 = 100_000;

type ErrorKey = (Vec<DemTarget>, Option<String>);

pub(super) fn try_analyze(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<Option<DetectorErrorModel>> {
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
        })
    }

    fn analyze(mut self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        self.undo_circuit(circuit)?;
        self.tracker.undo_implicit_rz_at_start_of_circuit()?;
        self.collect_gauge_errors()?;
        self.flush()?;
        let mut base_detector_id = 0_u64;
        let mut seen = BTreeSet::new();
        unreverse_model(&self.reversed_model, &mut base_detector_id, &mut seen)
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
        if !self.options.fold_loops {
            return self.undo_loop_by_unrolling(body, iterations);
        }

        let mut tortoise = self.tracker.clone();
        let mut hare = self.tracker.clone();
        let mut hare_iterations = 0_u64;
        let mut tortoise_iterations = 0_u64;
        let mut found_cycle = false;
        while hare_iterations < iterations && hare_iterations < MAX_LOOP_CYCLE_STEPS {
            hare.undo_circuit(body)?;
            hare.take_gauge_errors();
            hare_iterations = checked_add(hare_iterations, 1, "hare iteration")?;
            if hare.is_shifted_copy(&tortoise) {
                found_cycle = true;
                break;
            }
            if hare_iterations.is_multiple_of(2) {
                tortoise.undo_circuit(body)?;
                tortoise.take_gauge_errors();
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
            checked_product_u64(period, skipped_periods, "period skip")?,
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

fn locally_decompose_combinations(
    basis_errors: &[BTreeSet<DemTarget>],
    combinations: &mut [Vec<DemTarget>],
) -> CircuitResult<()> {
    let mut involved_detectors = BTreeMap::new();
    for basis in basis_errors {
        for target in basis {
            if let DemTarget::RelativeDetector(detector) = target {
                let next = involved_detectors.len();
                if !involved_detectors.contains_key(detector) {
                    if next >= 15 {
                        return Err(CircuitError::invalid_detector_error_model(
                            "an error case in a composite error exceeded 15 detector symptoms",
                        ));
                    }
                    involved_detectors.insert(*detector, next);
                }
            }
        }
    }

    let mut detector_masks = vec![0_u64; combinations.len()];
    for (slot, targets) in detector_masks.iter_mut().zip(combinations.iter()).skip(1) {
        let mut mask = 0_u64;
        for target in targets {
            if let DemTarget::RelativeDetector(detector) = target {
                let bit = involved_detectors.get(detector).copied().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "composite error detector has no local mask",
                    )
                })?;
                mask ^= 1_u64 << bit;
            }
        }
        *slot = mask;
    }

    let detector_counts = detector_masks
        .iter()
        .map(|mask| mask.count_ones())
        .collect::<Vec<_>>();
    let mut solved = vec![false; combinations.len()];
    let mut single_detector_union = 0_u64;
    for ((detector_count, detector_mask), solved_slot) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .skip(1)
    {
        if *detector_count == 1 {
            single_detector_union |= *detector_mask;
            *solved_slot = true;
        }
    }
    let mut irreducible_pairs = Vec::new();
    for (index, ((detector_count, detector_mask), solved_slot)) in detector_counts
        .iter()
        .zip(&detector_masks)
        .zip(&mut solved)
        .enumerate()
        .skip(1)
    {
        if *detector_count == 2 && *detector_mask & !single_detector_union != 0 {
            irreducible_pairs.push(index);
            *solved_slot = true;
        }
    }

    for goal_index in 1..combinations.len() {
        let detector_count = *indexed(
            &detector_counts,
            goal_index,
            "composite error detector count",
        )?;
        let is_solved = *indexed(&solved, goal_index, "composite error solved state")?;
        if detector_count == 0 || is_solved {
            continue;
        }
        let goal = *indexed(&detector_masks, goal_index, "composite error detector mask")?;
        let mut components = Vec::new();
        let mut remnants = if goal & !single_detector_union == 0 {
            goal
        } else {
            let mut contained_pair = None;
            for &pair in &irreducible_pairs {
                let pair_mask = *indexed(
                    &detector_masks,
                    pair,
                    "irreducible composite error pair mask",
                )?;
                if goal & pair_mask == pair_mask && goal & !(single_detector_union | pair_mask) == 0
                {
                    contained_pair = Some((pair, pair_mask));
                    break;
                }
            }
            if let Some((pair, pair_mask)) = contained_pair {
                components.push(
                    indexed(combinations, pair, "irreducible composite error component")?.clone(),
                );
                goal & !pair_mask
            } else if let Some((left, right)) = find_two_disjoint_pairs(
                goal,
                single_detector_union,
                &irreducible_pairs,
                &detector_masks,
            )? {
                let left_component =
                    indexed(combinations, left, "left composite error pair")?.clone();
                let right_component =
                    indexed(combinations, right, "right composite error pair")?.clone();
                if left_component <= right_component {
                    components.push(left_component);
                    components.push(right_component);
                } else {
                    components.push(right_component);
                    components.push(left_component);
                }
                let left_mask = *indexed(&detector_masks, left, "left composite error pair mask")?;
                let right_mask =
                    *indexed(&detector_masks, right, "right composite error pair mask")?;
                goal & !(left_mask | right_mask)
            } else {
                continue;
            }
        };

        while remnants != 0 {
            let mut single_match = None;
            for index in 1..combinations.len() {
                let detector_count = *indexed(
                    &detector_counts,
                    index,
                    "single composite error detector count",
                )?;
                let detector_mask = *indexed(
                    &detector_masks,
                    index,
                    "single composite error detector mask",
                )?;
                if detector_count == 1 && detector_mask & !remnants == 0 {
                    single_match = Some((index, detector_mask));
                    break;
                }
            }
            let Some((single, detector_mask)) = single_match else {
                return Err(CircuitError::invalid_detector_error_model(
                    "composite error local decomposition left an unsolved detector",
                ));
            };
            remnants &= !detector_mask;
            components
                .push(indexed(combinations, single, "single composite error component")?.clone());
        }
        *indexed_mut(
            combinations,
            goal_index,
            "decomposed composite error component",
        )? = join_error_components(&components);
    }
    Ok(())
}

fn find_two_disjoint_pairs(
    goal: u64,
    single_detector_union: u64,
    pairs: &[usize],
    masks: &[u64],
) -> CircuitResult<Option<(usize, usize)>> {
    for (position, &left) in pairs.iter().enumerate() {
        for &right in pairs.iter().skip(position + 1) {
            let left_mask = *indexed(masks, left, "left irreducible detector mask")?;
            let right_mask = *indexed(masks, right, "right irreducible detector mask")?;
            if left_mask & right_mask == 0
                && goal & !(single_detector_union | left_mask | right_mask) == 0
            {
                return Ok(Some((left, right)));
            }
        }
    }
    Ok(None)
}

fn join_error_components(components: &[Vec<DemTarget>]) -> Vec<DemTarget> {
    let mut joined = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            joined.push(DemTarget::separator());
        }
        joined.extend(component.iter().copied());
    }
    joined
}

fn merge_indistinguishable_disjoint_probabilities(
    targets: &[Vec<DemTarget>],
    probabilities: &mut [Probability],
) -> CircuitResult<()> {
    if targets.len() != probabilities.len() {
        return Err(CircuitError::invalid_detector_error_model(
            "disjoint probability and target table lengths differ",
        ));
    }
    for mask in 1..targets.len() {
        if !indexed(targets, mask, "disjoint target mask")?.is_empty() {
            continue;
        }
        for destination in 0..targets.len() {
            let source = destination ^ mask;
            if source > destination {
                let destination_probability = *indexed(
                    probabilities,
                    destination,
                    "disjoint destination probability",
                )?;
                let source_probability =
                    *indexed(probabilities, source, "disjoint source probability")?;
                *indexed_mut(
                    probabilities,
                    destination,
                    "disjoint destination probability",
                )? =
                    Probability::try_new(destination_probability.get() + source_probability.get())?;
                *indexed_mut(probabilities, source, "disjoint source probability")? =
                    Probability::try_new(0.0)?;
            }
        }
    }
    Ok(())
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

fn indexed<'a, T>(values: &'a [T], index: usize, context: &str) -> CircuitResult<&'a T> {
    values.get(index).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "folded analyzer {context} index {index} is out of range"
        ))
    })
}

fn indexed_mut<'a, T>(
    values: &'a mut [T],
    index: usize,
    context: &str,
) -> CircuitResult<&'a mut T> {
    values.get_mut(index).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "folded analyzer {context} index {index} is out of range"
        ))
    })
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
