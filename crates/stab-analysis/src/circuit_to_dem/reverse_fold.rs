use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, DemInstruction, DemInstructionKind, DemItem,
    DemRepeatCount, DemTarget, DetectorErrorModel, GateKind, Pauli, Probability, QubitId,
    RepeatCount, Target,
};

use super::{
    ErrorAnalyzerOptions,
    decompose::decompose_accumulated_errors,
    error_accumulator::{ErrorAccumulator, TagId},
    error_decomp::{
        depolarize1_independent_channel_probability, depolarize2_independent_channel_probability,
    },
    try_disjoint_to_independent_xyz_errors,
};
use crate::sparse_rev_frame_tracker::{
    ShiftedRecurrenceSearch, SparseReverseFrameTracker, search_shifted_recurrence,
};
use crate::target_xor_set::{TargetXorSet, xor_merge_into};
use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

mod local_decomposition;
mod output;
#[cfg(test)]
mod tests;

use local_decomposition::{
    locally_decompose_combinations, merge_indistinguishable_disjoint_probabilities,
};
use output::{SeenTargets, unreverse_model};

/// Composite channels combine at most four basis errors (two-qubit depolarization).
const MAX_COMBINATION_BASES: usize = 4;

/// Reused buffers for symptom sets built while recording errors.
#[derive(Debug, Default)]
struct AnalyzerScratch {
    sensitivity: Vec<DemTarget>,
    merge: Vec<DemTarget>,
    composite: TargetXorSet,
    bases: [Vec<DemTarget>; MAX_COMBINATION_BASES],
    combinations: Vec<Vec<DemTarget>>,
    joined: Vec<DemTarget>,
}

pub(super) fn try_analyze(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> AnalysisResult<DetectorErrorModel> {
    if !options.fold_loops {
        // Unrolled analysis recurses once per repeat level, so it admits the shared nesting
        // envelope before any work; expanded work itself is not capped.
        super::budget::validate_analyzer_repeat_nesting(circuit)?;
    }
    ReverseFoldAnalyzer::new(circuit, options)?.analyze(circuit)
}

struct ReverseFoldAnalyzer {
    options: ErrorAnalyzerOptions,
    tracker: SparseReverseFrameTracker,
    ticks_left: u64,
    reversed_model: DetectorErrorModel,
    errors: ErrorAccumulator,
    scratch: AnalyzerScratch,
    /// Conservative retained bytes of every item pushed into `reversed_model`.
    output_bytes: u64,
    /// ELSE_CORRELATED_ERROR instructions seen (in reverse) since the last
    /// unrelated instruction, awaiting their leading E (error_analyzer.cc:673).
    stacked_else_correlated: Vec<CircuitInstruction>,
}

impl ReverseFoldAnalyzer {
    fn new(circuit: &Circuit, options: ErrorAnalyzerOptions) -> AnalysisResult<Self> {
        let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "analyze_errors measurement count does not fit usize",
            )
        })?;
        let detector_count = circuit.count_detectors()?;
        let mut tracker = SparseReverseFrameTracker::new_for_error_analysis(
            stab_model::advanced::circuit_simulated_qubit_count(circuit),
            measurement_count,
            detector_count,
            options.allow_gauge_detectors,
        );
        tracker.set_cancellation(options.cancellation.clone());
        Ok(Self {
            options,
            tracker,
            ticks_left: circuit.count_ticks()?,
            reversed_model: DetectorErrorModel::new(),
            errors: ErrorAccumulator::new(),
            scratch: AnalyzerScratch::default(),
            output_bytes: 0,
            stacked_else_correlated: Vec::new(),
        })
    }

    /// Charges `bytes` of retained output against the caller's budget before retaining them.
    fn charge_output(&mut self, bytes: u64) -> AnalysisResult<()> {
        let budget = self.options.memory_budget;
        let total = self.output_bytes.saturating_add(bytes);
        if !budget.admits(u128::from(total)) {
            return Err(ResourceLimitError::memory_budget(
                ResourceOperation::CircuitToDetectorErrorModel,
                ResourceKind::MaterializedBytes,
                total,
                budget.bytes(),
            )
            .into());
        }
        self.output_bytes = total;
        Ok(())
    }

    fn push_output_instruction(&mut self, instruction: DemInstruction) -> AnalysisResult<()> {
        let bytes = (size_of::<DemItem>() as u64)
            .saturating_add(
                (instruction.targets().len() as u64).saturating_mul(size_of::<DemTarget>() as u64),
            )
            .saturating_add((instruction.args().len() as u64).saturating_mul(8))
            .saturating_add(instruction.tag_bytes().map_or(0, <[u8]>::len) as u64);
        self.charge_output(bytes)?;
        self.reversed_model.push_instruction(instruction);
        Ok(())
    }

    fn analyze(mut self, circuit: &Circuit) -> AnalysisResult<DetectorErrorModel> {
        self.undo_circuit(circuit)?;
        self.tracker.undo_implicit_rz_at_start_of_circuit()?;
        self.collect_gauge_errors()?;
        self.flush()?;
        let mut model = std::mem::take(&mut self.reversed_model);
        let mut base_detector_id = 0_u64;
        let mut seen = SeenTargets::new();
        unreverse_model(&mut model, &mut base_detector_id, &mut seen)?;
        Ok(model)
    }

    fn undo_circuit(&mut self, circuit: &Circuit) -> AnalysisResult<()> {
        self.tracker.poll_cancellation()?;
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.undo_instruction(instruction)?;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    self.require_no_pending_else_chain()?;
                    self.run_loop(repeat.body(), repeat.repeat_count(), repeat.tag_bytes())?;
                }
            }
        }
        // Pinned Stim scopes the pending chain to each circuit body and
        // rejects an ELSE stack left unconsumed at the body's start
        // (error_analyzer.cc:719-722).
        self.require_no_pending_else_chain()
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.tracker.poll_cancellation()?;
        let kind = instruction.gate().kind();
        if !matches!(kind, GateKind::ELSE_CORRELATED_ERROR | GateKind::E) {
            self.require_no_pending_else_chain()?;
        }
        match kind {
            GateKind::X_ERROR => self.record_single_pauli_errors(instruction, Pauli::X)?,
            GateKind::Y_ERROR => self.record_single_pauli_errors(instruction, Pauli::Y)?,
            GateKind::Z_ERROR => self.record_single_pauli_errors(instruction, Pauli::Z)?,
            GateKind::DEPOLARIZE1 => self.record_depolarize1(instruction)?,
            GateKind::DEPOLARIZE2 => self.record_depolarize2(instruction)?,
            GateKind::PAULI_CHANNEL_1 => self.record_pauli_channel1(instruction)?,
            GateKind::PAULI_CHANNEL_2 => self.record_pauli_channel2(instruction)?,
            GateKind::E => self.record_correlated_error_block(instruction)?,
            GateKind::ELSE_CORRELATED_ERROR => {
                self.stacked_else_correlated.push(instruction.clone());
            }
            GateKind::HERALDED_ERASE => self.record_heralded_erase(instruction)?,
            GateKind::HERALDED_PAULI_CHANNEL_1 => {
                self.record_heralded_pauli_channel1(instruction)?
            }
            GateKind::I_ERROR | GateKind::II_ERROR => {}
            GateKind::M
            | GateKind::MX
            | GateKind::MY
            | GateKind::MR
            | GateKind::MRX
            | GateKind::MRY => {
                self.record_measurement_errors(instruction, instruction.targets().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            GateKind::MXX | GateKind::MYY | GateKind::MZZ | GateKind::MPP => {
                self.record_measurement_errors(instruction, instruction.target_groups().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            GateKind::MPAD => {
                self.record_measurement_errors(instruction, instruction.targets().len())?;
                self.tracker.undo_instruction(instruction)?;
            }
            GateKind::DETECTOR => {
                self.tracker.undo_instruction(instruction)?;
                self.push_output_instruction(
                    stab_model::advanced::dem_instruction_with_tag_bytes(
                        DemInstructionKind::Detector,
                        instruction.args().to_vec(),
                        vec![DemTarget::relative_detector(self.tracker.detector_count())?],
                        instruction.tag_bytes(),
                    )?,
                )?;
            }
            GateKind::OBSERVABLE_INCLUDE => {
                self.tracker.undo_instruction(instruction)?;
                let observable = instruction.observable_id_argument()?.ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "OBSERVABLE_INCLUDE is missing an observable id",
                    )
                })?;
                self.push_output_instruction(
                    stab_model::advanced::dem_instruction_with_tag_bytes(
                        DemInstructionKind::LogicalObservable,
                        Vec::new(),
                        vec![DemTarget::logical_observable(observable.get())?],
                        instruction.tag_bytes(),
                    )?,
                )?;
            }
            GateKind::SHIFT_COORDS => {
                self.push_output_instruction(
                    stab_model::advanced::dem_instruction_with_tag_bytes(
                        DemInstructionKind::ShiftDetectors,
                        instruction.args().to_vec(),
                        vec![DemTarget::numeric(0)],
                        instruction.tag_bytes(),
                    )?,
                )?;
            }
            GateKind::TICK => {
                self.ticks_left = self.ticks_left.checked_sub(1).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "tick count underflowed during folded error analysis",
                    )
                })?;
            }
            GateKind::QUBIT_COORDS
            | GateKind::REPEAT
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
            | GateKind::ISWAP_DAG => self.tracker.undo_instruction(instruction)?,
        }
        self.collect_gauge_errors()
    }

    fn record_measurement_errors(
        &mut self,
        instruction: &CircuitInstruction,
        measurement_count: usize,
    ) -> AnalysisResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let end = self.tracker.measurement_count();
        let start = end.checked_sub(measurement_count).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} consumes more measurements than remain during folded error analysis",
                instruction.gate().canonical_name()
            ))
        })?;
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        for index in (start..end).rev() {
            let targets = self.tracker.record_targets_slice_at(index)?;
            add_independent_error(&mut self.errors, probability, targets, tag)?;
        }
        Ok(())
    }

    fn record_single_pauli_errors(
        &mut self,
        instruction: &CircuitInstruction,
        pauli: Pauli,
    ) -> AnalysisResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            self.tracker
                .error_sensitivity_into(qubit, pauli, &mut self.scratch.sensitivity)?;
            add_independent_error(
                &mut self.errors,
                probability,
                &self.scratch.sensitivity,
                tag,
            )?;
        }
        Ok(())
    }

    /// Records a reverse-encountered `E`, folding any stacked
    /// `ELSE_CORRELATED_ERROR` chain into disjoint components exactly like
    /// pinned Stim's `correlated_error_block` (error_analyzer.cc:784-809).
    fn record_correlated_error_block(
        &mut self,
        leading: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        if self.stacked_else_correlated.is_empty() {
            let Some(probability) = leading.probability_argument()? else {
                return Ok(());
            };
            return self.record_correlated_error(leading, probability);
        }
        let stacked = std::mem::take(&mut self.stacked_else_correlated);
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(AnalysisError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR requires approximate_disjoint_errors during error analysis",
            ));
        };
        let mut remaining = 1.0_f64;
        for instruction in std::iter::once(leading).chain(stacked.iter().rev()) {
            let Some(probability) = instruction.probability_argument()? else {
                continue;
            };
            let actual = probability.get() * remaining;
            remaining *= 1.0 - probability.get();
            if actual > threshold.get() {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "CORRELATED_ERROR/ELSE_CORRELATED_ERROR block has a component probability '{actual}' larger than the approximate_disjoint_errors threshold of '{}'.",
                    threshold.get()
                )));
            }
            let actual = Probability::try_new(actual).map_err(|_| {
                AnalysisError::invalid_detector_error_model(
                    "ELSE_CORRELATED_ERROR chain produced an invalid component probability",
                )
            })?;
            self.record_correlated_error(instruction, actual)?;
        }
        Ok(())
    }

    fn record_correlated_error(
        &mut self,
        instruction: &CircuitInstruction,
        probability: Probability,
    ) -> AnalysisResult<()> {
        self.composite_error_targets(instruction.targets())?;
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        add_independent_error(
            &mut self.errors,
            probability,
            self.scratch.composite.as_slice(),
            tag,
        )
    }

    fn require_no_pending_else_chain(&self) -> AnalysisResult<()> {
        if self.stacked_else_correlated.is_empty() {
            Ok(())
        } else {
            Err(AnalysisError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR wasn't preceded by ELSE_CORRELATED_ERROR or CORRELATED_ERROR (E)",
            ))
        }
    }

    /// Port of pinned Stim's `undo_HERALDED_ERASE` (error_analyzer.cc:321-340):
    /// each heralded record combines the herald symptom with the qubit's X and
    /// Z sensitivity regions under four equal disjoint erase channels.
    fn record_heralded_erase(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let Some(argument) = instruction.probability_argument()? else {
            return Ok(());
        };
        self.validate_disjoint_threshold("HERALDED_ERASE", &[argument])?;
        let channel = Probability::try_new(argument.get() * 0.25).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "HERALDED_ERASE channel probability is invalid",
            )
        })?;
        let identity = Probability::try_new((1.0 - argument.get()).max(0.0)).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "HERALDED_ERASE identity probability is invalid",
            )
        })?;
        self.record_heralded_channels(instruction, [identity, channel, channel, channel, channel])
    }

    /// Port of pinned Stim's `undo_HERALDED_PAULI_CHANNEL_1`
    /// (error_analyzer.cc:342-365) with the vendor slot order `hi, hz, hx, hy`.
    fn record_heralded_pauli_channel1(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [hi, hx, hy, hz] = probabilities.as_slice() else {
            return Err(AnalysisError::invalid_detector_error_model(
                "HERALDED_PAULI_CHANNEL_1 expects four probability arguments",
            ));
        };
        let specified = probabilities
            .iter()
            .filter(|probability| probability.get() > 0.0)
            .count();
        if specified > 1 {
            self.validate_disjoint_threshold("HERALDED_PAULI_CHANNEL_1", &probabilities)?;
        }
        let identity =
            Probability::try_new((1.0 - hi.get() - hx.get() - hy.get() - hz.get()).max(0.0))
                .map_err(|_| {
                    AnalysisError::invalid_detector_error_model(
                        "HERALDED_PAULI_CHANNEL_1 identity probability is invalid",
                    )
                })?;
        self.record_heralded_channels(instruction, [identity, *hi, *hz, *hx, *hy])
    }

    /// Shared heralded-record recorder: `channels` holds the no-herald
    /// identity slot followed by the herald-only, herald-plus-Z-sensitivity,
    /// herald-plus-X-sensitivity, and herald-plus-both slots, matching the
    /// vendor basis order `{xs, zs, herald}`.
    fn record_heralded_channels(
        &mut self,
        instruction: &CircuitInstruction,
        channels: [Probability; 5],
    ) -> AnalysisResult<()> {
        let zero = Probability::try_new(0.0).map_err(|_| {
            AnalysisError::invalid_detector_error_model("zero probability is invalid")
        })?;
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            let herald = self.tracker.pop_record_sensitivity()?;
            self.load_bases(&[(qubit, Pauli::Z), (qubit, Pauli::X)])?;
            let herald_basis = self.scratch.bases.get_mut(2).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "heralded error basis slot is out of range",
                )
            })?;
            herald_basis.clear();
            herald_basis.extend_from_slice(herald.as_slice());
            self.record_error_combinations(
                &mut [
                    channels[0],
                    zero,
                    zero,
                    zero,
                    channels[1],
                    channels[2],
                    channels[3],
                    channels[4],
                ],
                3,
                true,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    fn record_depolarize1(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let independent = depolarize1_independent_channel_probability(probability)?;
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "DEPOLARIZE1 target {target} is not a qubit"
                ))
            })?;
            self.load_bases(&[(qubit, Pauli::Z), (qubit, Pauli::X)])?;
            self.record_error_combinations(
                &mut [
                    Probability::try_new(0.0)?,
                    independent,
                    independent,
                    independent,
                ],
                2,
                false,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    fn record_depolarize2(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let independent = depolarize2_independent_channel_probability(probability)?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(AnalysisError::invalid_detector_error_model(
                    "DEPOLARIZE2 expected paired qubit targets",
                ));
            };
            let left = left.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "DEPOLARIZE2 left target is not a qubit",
                )
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "DEPOLARIZE2 right target is not a qubit",
                )
            })?;
            let mut probabilities = [independent; 16];
            let [first, ..] = &mut probabilities;
            *first = Probability::try_new(0.0)?;
            self.load_bases(&[
                (left, Pauli::Z),
                (left, Pauli::X),
                (right, Pauli::Z),
                (right, Pauli::X),
            ])?;
            self.record_error_combinations(&mut probabilities, 4, false, instruction.tag_bytes())?;
        }
        Ok(())
    }

    /// Port of pinned Stim's `undo_PAULI_CHANNEL_1`
    /// (`error_analyzer.cc:901-929`), loading `{zs, xs}` (`X` then `Z` Pauli
    /// sensitivities) with slot order `{0, ix, iz, iy}` so local decomposition
    /// and fused probability merges follow the vendor order.
    fn record_pauli_channel1(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [x, y, z] = probabilities.as_slice() else {
            return Err(AnalysisError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 expected three probabilities",
            ));
        };
        let independent = try_disjoint_to_independent_xyz_errors(*x, *y, *z)?;
        let (ix, iy, iz, is_disjoint) = if let Some(independent) = independent {
            (independent.x, independent.y, independent.z, false)
        } else {
            self.validate_disjoint_threshold("PAULI_CHANNEL_1", &probabilities)?;
            (*x, *y, *z, true)
        };
        let zero = Probability::try_new(0.0)?;
        for target in instruction.targets() {
            let qubit = target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model("PAULI_CHANNEL_1 target is not a qubit")
            })?;
            self.load_bases(&[(qubit, Pauli::X), (qubit, Pauli::Z)])?;
            self.record_error_combinations(
                &mut [zero, ix, iz, iy],
                2,
                is_disjoint,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    /// Port of pinned Stim's `undo_PAULI_CHANNEL_2`
    /// (`error_analyzer.cc:931-956`), mapping the 15 `IX..ZZ` arguments onto
    /// the `{zs[b], xs[b], zs[a], xs[a]}` basis masks via `pauli_xyz_to_xz`.
    fn record_pauli_channel2(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        if instruction.targets().is_empty() {
            return Ok(());
        }
        self.validate_disjoint_threshold("PAULI_CHANNEL_2", &probabilities)?;
        let probabilities: [Probability; 15] =
            probabilities.try_into().map_err(|values: Vec<_>| {
                AnalysisError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 expected 15 probabilities, got {}",
                    values.len()
                ))
            })?;
        let zero = Probability::try_new(0.0)?;
        let mut table = [zero; 16];
        for (index, probability) in probabilities.into_iter().enumerate() {
            let k = index + 1;
            let mask = pauli_xyz_to_xz(k & 3) | (pauli_xyz_to_xz((k >> 2) & 3) << 2);
            if let Some(slot) = table.get_mut(mask) {
                *slot = probability;
            }
        }
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(AnalysisError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 expected paired qubit targets",
                ));
            };
            let left = left.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 left target is not a qubit",
                )
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 right target is not a qubit",
                )
            })?;
            let mut slot_probabilities = table;
            self.load_bases(&[
                (right, Pauli::X),
                (right, Pauli::Z),
                (left, Pauli::X),
                (left, Pauli::Z),
            ])?;
            self.record_error_combinations(
                &mut slot_probabilities,
                4,
                true,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    fn validate_disjoint_threshold(
        &self,
        gate_name: &str,
        probabilities: &[Probability],
    ) -> AnalysisResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "{gate_name} requires approximate_disjoint_errors during error analysis"
            )));
        };
        if let Some(probability) = probabilities
            .iter()
            .find(|probability| probability.get() > threshold.get())
        {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "{gate_name} has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                probability.get(),
                threshold.get()
            )));
        }
        Ok(())
    }

    /// Loads the symptom sets of single-qubit Pauli errors into the leading basis slots.
    fn load_bases(&mut self, errors: &[(QubitId, Pauli)]) -> AnalysisResult<()> {
        for (index, &(qubit, pauli)) in errors.iter().enumerate() {
            let basis = self.scratch.bases.get_mut(index).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "composite error basis index is out of range",
                )
            })?;
            self.tracker.error_sensitivity_into(qubit, pauli, basis)?;
        }
        Ok(())
    }

    /// Records every combination of the first `basis_count` loaded basis errors, where
    /// `probabilities[mask]` belongs to the XOR of the bases selected by `mask`.
    fn record_error_combinations(
        &mut self,
        probabilities: &mut [Probability],
        basis_count: usize,
        probabilities_are_disjoint: bool,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let combination_count = 1_usize
            .checked_shl(u32::try_from(basis_count).map_err(|_| {
                AnalysisError::invalid_detector_error_model(
                    "composite error basis count does not fit u32",
                )
            })?)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "composite error combination count overflowed",
                )
            })?;
        if probabilities.len() != combination_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "composite error expected {combination_count} probabilities, got {}",
                probabilities.len()
            )));
        }
        let bases = self.scratch.bases.get(..basis_count).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "composite error basis index is out of range",
            )
        })?;
        let combinations = &mut self.scratch.combinations;
        if combinations.len() < combination_count {
            combinations.resize_with(combination_count, Vec::new);
        }
        let combinations = combinations.get_mut(..combination_count).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "composite error target index is out of range",
            )
        })?;
        if let Some(empty) = combinations.first_mut() {
            empty.clear();
        }
        for mask in 1..combination_count {
            let bit = mask.trailing_zeros() as usize;
            let previous = mask & (mask - 1);
            let basis = bases.get(bit).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "composite error basis index is out of range",
                )
            })?;
            let (earlier, later) = combinations.split_at_mut(mask);
            let previous = earlier.get(previous).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "composite error predecessor is out of range",
                )
            })?;
            let slot = later.first_mut().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "composite error target index is out of range",
                )
            })?;
            xor_merge_into(previous, basis, slot);
        }

        if self.options.decompose_errors {
            locally_decompose_combinations(bases, combinations, &mut self.scratch.joined)?;
        }
        if probabilities_are_disjoint {
            merge_indistinguishable_disjoint_probabilities(combinations, probabilities)?;
        }
        // Pinned Stim chains every slot through the accumulated map
        // (`add_error` per combination, error_analyzer.cc:1658-1660); slots
        // sharing one symptom class therefore merge through the same fused
        // `fma(old, 1 - p, (1 - old) * p)` rounding as cross-instruction
        // merges. An earlier pre-combined `l + r - 2lr` subtotal here was a
        // black-box misreading of that contraction.
        let tag = self.errors.intern_tag(tag)?;
        for (probability, targets) in probabilities.iter().zip(combinations.iter()).skip(1) {
            add_independent_error(&mut self.errors, *probability, targets, tag)?;
        }
        Ok(())
    }

    /// Writes the XOR of the symptoms of a correlated error's Pauli targets into
    /// `scratch.composite`.
    fn composite_error_targets(&mut self, targets: &[Target]) -> AnalysisResult<()> {
        self.scratch.composite.clear();
        for target in targets {
            // Pinned Stim's correlated-error handlers consult only the Pauli
            // X/Z bits, so combiner targets and inversion bits are ignored
            // decoration (frame_simulator.inl:767-775).
            if target.is_combiner() {
                continue;
            }
            let pauli = target.pauli_type().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "correlated error target {target} is not a Pauli target"
                ))
            })?;
            let qubit = target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "correlated error target {target} does not identify a qubit"
                ))
            })?;
            self.tracker
                .error_sensitivity_into(qubit, pauli, &mut self.scratch.sensitivity)?;
            self.scratch
                .composite
                .xor_sorted(&self.scratch.sensitivity, &mut self.scratch.merge);
        }
        Ok(())
    }

    fn collect_gauge_errors(&mut self) -> AnalysisResult<()> {
        if !self.tracker.has_gauge_errors() {
            return Ok(());
        }
        for targets in self.tracker.take_gauge_errors() {
            add_independent_error(
                &mut self.errors,
                Probability::try_new(0.5)?,
                targets.as_slice(),
                TagId::UNTAGGED,
            )?;
        }
        Ok(())
    }

    fn flush(&mut self) -> AnalysisResult<()> {
        if self.options.decompose_errors {
            decompose_accumulated_errors(
                &mut self.errors,
                self.options
                    .block_decomposition_from_introducing_remnant_edges,
                self.options.ignore_decomposition_failures,
            )?;
        }
        for class in self.errors.sorted_classes()?.into_iter().rev() {
            let probability = self.errors.probability(class)?;
            let targets = self.errors.targets(class)?;
            if probability.get() != 0.0 && !targets.is_empty() {
                self.push_output_instruction(
                    stab_model::advanced::dem_error_instruction_from_targets(
                        probability,
                        targets,
                        self.errors.tag_bytes(self.errors.tag(class)?),
                    )?,
                )?;
            }
        }
        self.errors.clear();
        Ok(())
    }

    fn run_loop(
        &mut self,
        body: &Circuit,
        repeat_count: RepeatCount,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let iterations = repeat_count.get();
        if iterations == 0 {
            return Ok(());
        }
        if !self.options.fold_loops {
            return self.undo_loop_by_unrolling(body, iterations);
        }

        let search = search_shifted_recurrence(&self.tracker, iterations, |probe| {
            probe.undo_circuit_for_analyzer_probe(body)
        })?;
        let recurrence = match search {
            ShiftedRecurrenceSearch::Found { recurrence } => *recurrence,
            ShiftedRecurrenceSearch::Exhausted => {
                return self.undo_loop_by_unrolling(body, iterations);
            }
        };
        let hare_iterations = recurrence.cycle_end_iterations;
        let mut tortoise_iterations = recurrence.transient_iterations;

        for _ in 0..tortoise_iterations {
            self.undo_circuit(body)?;
        }

        if hare_iterations < iterations {
            let period = hare_iterations
                .checked_sub(tortoise_iterations)
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "folded analyzer recurrence period underflowed",
                    )
                })?;
            if period == 0 {
                return Err(AnalysisError::invalid_detector_error_model(
                    "folded analyzer recurrence period was zero",
                ));
            }
            let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer repeat remainder underflowed",
                )
            })?;
            let period_repetitions = remaining / period;
            if period_repetitions > 1 {
                self.capture_repeated_period(
                    body,
                    recurrence.period,
                    period_repetitions,
                    &recurrence.cycle_end_state,
                    &mut tortoise_iterations,
                    tag,
                )?;
            }
        }

        let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
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
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let measurements_per_period = self
            .tracker
            .measurement_count()
            .checked_sub(hare.measurement_count())
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer measurement period underflowed",
                )
            })?;
        let detectors_per_period = self
            .tracker
            .detector_count()
            .checked_sub(hare.detector_count())
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer detector period underflowed",
                )
            })?;

        self.flush()?;
        let outer_reversed = std::mem::take(&mut self.reversed_model);
        let skipped_periods = period_repetitions.checked_sub(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "folded analyzer skipped period count underflowed",
            )
        })?;
        let folded_iterations =
            checked_product_u64(period, skipped_periods, "folded repeat iteration")?;
        let skipped_measurements =
            checked_product_usize(measurements_per_period, skipped_periods, "measurement skip")?;
        let skipped_detectors =
            checked_product_u64(detectors_per_period, skipped_periods, "detector skip")?;
        self.tracker.shift_counts(
            -i128::try_from(skipped_measurements).map_err(|_| {
                AnalysisError::invalid_detector_error_model(
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
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer nested detector shifts exceed the period delta",
                )
            })?;
        if remaining_shift > 0 {
            prepend_detector_shift(&mut period_body, remaining_shift)?;
        }

        self.reversed_model = outer_reversed;
        self.charge_output(size_of::<DemItem>() as u64)?;
        self.reversed_model.push_repeat_block(
            stab_model::advanced::dem_repeat_block_with_tag_bytes(
                DemRepeatCount::new(period_repetitions),
                period_body,
                tag,
            ),
        );
        Ok(())
    }

    fn undo_loop_by_unrolling(&mut self, body: &Circuit, iterations: u64) -> AnalysisResult<()> {
        for _ in 0..iterations {
            self.undo_circuit(body)?;
        }
        Ok(())
    }
}

const fn pauli_xyz_to_xz(xyz: usize) -> usize {
    xyz ^ (xyz >> 1)
}

fn add_independent_error(
    errors: &mut ErrorAccumulator,
    probability: Probability,
    targets: &[DemTarget],
    tag: TagId,
) -> AnalysisResult<()> {
    if probability.get() == 0.0 || targets.is_empty() {
        return Ok(());
    }
    errors.merge_independent(targets, tag, probability)
}

/// Adds `detector_shift` to the body's leading `shift_detectors`, or inserts one.
fn prepend_detector_shift(
    model: &mut DetectorErrorModel,
    detector_shift: u64,
) -> AnalysisResult<()> {
    let items = stab_model::advanced::dem_model_items_mut(model);
    if let Some(DemItem::Instruction(instruction)) = items.first_mut()
        && instruction.kind() == DemInstructionKind::ShiftDetectors
    {
        let combined = stab_model::advanced::dem_instruction_detector_shift(instruction)?
            .checked_add(detector_shift)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer detector shift overflowed",
                )
            })?;
        *instruction = stab_model::advanced::dem_instruction_with_tag_bytes(
            DemInstructionKind::ShiftDetectors,
            instruction.args().to_vec(),
            vec![DemTarget::numeric(combined)],
            instruction.tag_bytes(),
        )?;
        return Ok(());
    }
    items.insert(
        0,
        DemItem::Instruction(DemInstruction::shift_detectors(
            Vec::new(),
            detector_shift,
            None,
        )?),
    );
    Ok(())
}

fn checked_add(left: u64, right: u64, context: &str) -> AnalysisResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_u64(left: u64, right: u64, context: &str) -> AnalysisResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_usize(left: usize, right: u64, context: &str) -> AnalysisResult<usize> {
    left.checked_mul(usize::try_from(right).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "folded analyzer {context} does not fit usize"
        ))
    })?)
    .ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}
