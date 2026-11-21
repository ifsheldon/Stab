use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, DemDetectorId, DemInstruction, DemItem, DemTarget,
    DetectorErrorModel, GateKind, Pauli, Probability,
};

use super::{
    ErrorAnalyzerOptions,
    decompose::decompose_accumulated_errors,
    error_accumulator::{ErrorAccumulator, TagId},
};
use crate::sparse_rev_frame_tracker::SparseReverseFrameTracker;
use crate::target_xor_set::TargetXorSet;
use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError,
    UnanalyzableNoise,
};

mod local_decomposition;
mod loop_folding;
mod noise;
mod output;
#[cfg(test)]
mod tests;

use noise::add_independent_error;
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
    // Folded and unrolled analysis both recurse once per repeat level, and the reversed model
    // they emit nests no deeper, so the shared nesting envelope is admitted before any work;
    // expanded work itself is not capped.
    super::budget::validate_analyzer_repeat_nesting(circuit)?;
    ReverseFoldAnalyzer::new(circuit, options)?.analyze(circuit)
}

/// Holds `'c`, the analyzed circuit, so the pending `ELSE_CORRELATED_ERROR` chain borrows its
/// instructions instead of copying them.
struct ReverseFoldAnalyzer<'c> {
    options: ErrorAnalyzerOptions,
    tracker: SparseReverseFrameTracker,
    reversed_model: DetectorErrorModel,
    errors: ErrorAccumulator,
    scratch: AnalyzerScratch,
    /// Conservative retained bytes of every item pushed into `reversed_model`.
    output_bytes: u64,
    /// ELSE_CORRELATED_ERROR instructions seen (in reverse) since the last
    /// unrelated instruction, awaiting their leading E (error_analyzer.cc:673).
    stacked_else_correlated: Vec<&'c CircuitInstruction>,
    /// Live bytes of the `--fold_loops` hare trackers held while this analyzer undoes a
    /// tortoise iteration, one per enclosing period search.
    held_probe_bytes: u64,
}

impl<'c> ReverseFoldAnalyzer<'c> {
    fn new(circuit: &Circuit, options: ErrorAnalyzerOptions) -> AnalysisResult<Self> {
        let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
            AnalysisError::count_overflow(
                AnalysisOperation::CircuitToDetectorErrorModel,
                "analyze_errors measurement count does not fit usize",
            )
        })?;
        let detector_count = circuit.count_detectors()?;
        let mut tracker = SparseReverseFrameTracker::new_for_error_analysis(
            circuit.simulated_qubit_count(),
            measurement_count,
            detector_count,
            options.allow_gauge_detectors,
        );
        tracker.set_cancellation(options.cancellation.clone());
        tracker.set_memory_budget(
            AnalysisOperation::CircuitToDetectorErrorModel,
            ResourceKind::MaterializedBytes,
            options.memory_budget,
        );
        let errors = ErrorAccumulator::with_budget(options.memory_budget);
        Ok(Self {
            options,
            tracker,
            reversed_model: DetectorErrorModel::new(),
            errors,
            scratch: AnalyzerScratch::default(),
            output_bytes: 0,
            stacked_else_correlated: Vec::new(),
            held_probe_bytes: 0,
        })
    }

    /// Gives the error accumulator and the tracker each the bytes the other side, the retained
    /// output, and any held fold-search hares hold, so either one refuses growth as soon as the
    /// total would not fit.
    fn sync_error_budget_base(&mut self) {
        let beside_both = self.output_bytes.saturating_add(self.held_probe_bytes);
        self.errors
            .set_base_bytes(beside_both.saturating_add(self.tracker.live_bytes()));
        self.tracker
            .set_base_bytes(beside_both.saturating_add(self.errors.admitted_bytes()));
    }

    /// Bytes held outside the tracker: retained output, error classes, and held hares.
    fn retained_bytes_beside_tracker(&self) -> u64 {
        self.output_bytes
            .saturating_add(self.errors.admitted_bytes())
            .saturating_add(self.held_probe_bytes)
    }

    /// Charges `bytes` of retained output against the caller's budget before retaining them.
    fn charge_output(&mut self, bytes: u64) -> AnalysisResult<()> {
        let budget = self.options.memory_budget;
        let next_output = self.output_bytes.saturating_add(bytes);
        let live = self
            .tracker
            .live_bytes()
            .saturating_add(self.errors.admitted_bytes())
            .saturating_add(self.held_probe_bytes);
        let total = next_output.saturating_add(live);
        if !budget.admits(u128::from(total)) {
            return Err(ResourceLimitError::memory_budget(
                AnalysisOperation::CircuitToDetectorErrorModel,
                ResourceKind::MaterializedBytes,
                total,
                budget,
            )
            .into());
        }
        self.output_bytes = next_output;
        self.sync_error_budget_base();
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

    fn analyze(mut self, circuit: &'c Circuit) -> AnalysisResult<DetectorErrorModel> {
        self.undo_circuit(circuit)?;
        self.tracker.undo_implicit_rz_at_start_of_circuit()?;
        // Vendor post_check_initialization reports start-of-circuit gauges untagged.
        self.collect_gauge_errors(TagId::UNTAGGED)?;
        self.charge_output(0)?;
        self.flush()?;
        let mut model = std::mem::take(&mut self.reversed_model);
        let mut base_detector_id = 0_u64;
        let mut seen = SeenTargets::new();
        unreverse_model(&mut model, &mut base_detector_id, &mut seen)?;
        Ok(model)
    }

    fn undo_circuit(&mut self, circuit: &'c Circuit) -> AnalysisResult<()> {
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

    fn undo_instruction(&mut self, instruction: &'c CircuitInstruction) -> AnalysisResult<()> {
        self.tracker.poll_cancellation()?;
        self.sync_error_budget_base();
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
                self.stacked_else_correlated.push(instruction);
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
                    stab_model::advanced::dem_detector_instruction_with_tag_bytes(
                        instruction.args().to_vec(),
                        DemDetectorId::try_new(self.tracker.detector_count())?,
                        instruction.tag_bytes(),
                    )?,
                )?;
            }
            GateKind::OBSERVABLE_INCLUDE => {
                self.tracker.undo_instruction(instruction)?;
                let observable = instruction.observable_id_argument()?.ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        "OBSERVABLE_INCLUDE is missing an observable id",
                    )
                })?;
                self.push_output_instruction(
                    stab_model::advanced::dem_logical_observable_instruction_with_tag_bytes(
                        observable,
                        instruction.tag_bytes(),
                    ),
                )?;
            }
            GateKind::SHIFT_COORDS => {
                self.push_output_instruction(
                    stab_model::advanced::dem_shift_detectors_instruction_with_tag_bytes(
                        instruction.args().to_vec(),
                        0,
                        instruction.tag_bytes(),
                    )?,
                )?;
            }
            GateKind::TICK
            | GateKind::QUBIT_COORDS
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
        if self.tracker.has_gauge_errors() {
            // Vendor check_for_gauge passes the collapsing instruction's tag to add_error for
            // resets, measurements, demolition measurements, pair measurements, and MPP alike.
            let tag = self.errors.intern_tag(instruction.tag_bytes())?;
            self.collect_gauge_errors(tag)?;
        }
        self.charge_output(0)
    }

    fn require_no_pending_else_chain(&self) -> AnalysisResult<()> {
        if self.stacked_else_correlated.is_empty() {
            Ok(())
        } else {
            Err(UnanalyzableNoise::UnprecededElseCorrelatedError.into())
        }
    }

    fn collect_gauge_errors(&mut self, tag: TagId) -> AnalysisResult<()> {
        if !self.tracker.has_gauge_errors() {
            return Ok(());
        }
        for targets in self.tracker.take_gauge_errors() {
            add_independent_error(
                &mut self.errors,
                Probability::try_new(0.5)?,
                targets.as_slice(),
                tag,
            )?;
        }
        Ok(())
    }

    fn flush(&mut self) -> AnalysisResult<()> {
        self.sync_error_budget_base();
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
        self.errors.clear(
            self.output_bytes
                .saturating_add(self.held_probe_bytes)
                .saturating_add(self.tracker.live_bytes()),
        );
        Ok(())
    }
}
