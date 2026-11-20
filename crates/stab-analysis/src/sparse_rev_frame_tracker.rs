use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::PauliBasis;
use stab_model::advanced::CancellationCountdown;
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, DemTarget, MemoryBudget, Pauli, QubitId, Target,
    TargetKind,
};

mod budget;
mod clifford_action;
mod error_analysis;
mod measurements;
mod pauli_product;
mod regions;
mod sets;
mod shifted_repeat;
mod unitary_repeat;

use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};
use crate::controlled_pauli::{ControlSlot, ControlledPauliForm, controlled_pauli_form};
use crate::target_xor_set::{TargetXorSet, xor_merge_into};
use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceOperation};
use budget::{TrackerMeter, recompute_live_bytes, set_entry_bytes};
use clifford_action::{single_qubit_action, two_qubit_action};
use pauli_product::{pauli_product_terms_reversed, qubits_reversed};
use sets::{
    TrackerScratch, buffer_growth_bytes, replace_qubit_set, toggle_qubit_set, xor_growth_bytes,
    xor_item_growth_bytes,
};
pub(crate) use unitary_repeat::is_supported_unitary_circuit as folds_unitary_repeat_body;

/// Returns the form of a gate the tracker undoes as a classically controllable Pauli.
fn tracked_controlled_pauli(
    instruction: &CircuitInstruction,
) -> AnalysisResult<ControlledPauliForm> {
    controlled_pauli_form(instruction.gate()).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!(
            "{} is not a supported sparse reverse feedback gate",
            instruction.gate().canonical_name()
        ))
    })
}

/// Sensitivity sets keyed by qubit, record index, or observable; absent keys are empty sets.
type TargetSets<K> = BTreeMap<K, TargetXorSet>;

fn tracker_basis(basis: PauliBasis) -> AnalysisResult<TrackerBasis> {
    TrackerBasis::from_pauli_basis(basis).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(
            "identity basis is not a reverse stabilizer transition",
        )
    })
}

/// `Clone` is implemented in `budget` so a clone's meter matches its own buffers.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SparseReverseFrameTracker {
    xs: TargetSets<QubitId>,
    zs: TargetSets<QubitId>,
    qubit_count: usize,
    rec_bits: TargetSets<usize>,
    measurement_count: usize,
    detector_count: u64,
    observable_effects: TargetSets<u64>,
    anticommutation_policy: AnticommutationPolicy,
    error_analysis_mode: bool,
    eliminate_detector_gauges: bool,
    gauge_errors: Vec<TargetXorSet>,
    anticommutations: BTreeSet<Anticommutation>,
    scratch: TrackerScratch,
    meter: TrackerMeter,
    cancellation: TrackerCancellation,
}

/// The owning operation's cancellation signal, polled once per undone instruction and repeat
/// iteration. It is not tracker state, so every value compares equal.
#[derive(Clone, Debug, Default)]
struct TrackerCancellation {
    signal: Cancellation,
    countdown: CancellationCountdown,
}

impl PartialEq for TrackerCancellation {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TrackerCancellation {}

impl SparseReverseFrameTracker {
    pub(crate) fn new(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: u64,
        anticommutation_policy: AnticommutationPolicy,
    ) -> Self {
        Self {
            xs: BTreeMap::new(),
            zs: BTreeMap::new(),
            qubit_count,
            rec_bits: BTreeMap::new(),
            measurement_count,
            detector_count,
            observable_effects: BTreeMap::new(),
            anticommutation_policy,
            error_analysis_mode: false,
            eliminate_detector_gauges: false,
            gauge_errors: Vec::new(),
            anticommutations: BTreeSet::new(),
            scratch: TrackerScratch::default(),
            meter: TrackerMeter::default(),
            cancellation: TrackerCancellation::default(),
        }
    }

    /// Bounds the tracker's live sensitivity sets against `budget`, reporting refusals as
    /// `(operation, resource)`.
    pub(crate) fn set_memory_budget(
        &mut self,
        operation: ResourceOperation,
        resource: ResourceKind,
        budget: MemoryBudget,
    ) {
        self.meter.set_policy(operation, resource, budget);
    }

    /// Records bytes concurrently retained outside this tracker by the owning operation.
    pub(crate) fn set_base_bytes(&mut self, base_bytes: u64) {
        self.meter.base_bytes = base_bytes;
    }

    /// Bytes currently retained across the tracker's sensitivity maps, gauge sets,
    /// anticommutation records, recycled-set pool, and merge buffers, counted by capacity.
    pub(crate) fn live_bytes(&self) -> u64 {
        self.meter
            .live_bytes
            .saturating_add(self.scratch.retained_bytes())
    }

    /// Fails when the tracker's live state exceeds its configured memory budget.
    #[inline]
    pub(crate) fn check_live_budget(&self) -> AnalysisResult<()> {
        self.meter.admit(self.scratch.retained_bytes())
    }

    /// Fails when the tracker's live state plus `extra_bytes` exceeds its configured budget.
    pub(crate) fn check_live_budget_with_extra(&self, extra_bytes: u64) -> AnalysisResult<()> {
        self.meter
            .admit(self.scratch.retained_bytes().saturating_add(extra_bytes))
    }

    /// Runs `work` with `held` bytes of an operand taken out of the scratch or a map admitted
    /// alongside every growth it makes.
    #[inline]
    fn holding<T>(
        &mut self,
        held: u64,
        work: impl FnOnce(&mut Self) -> AnalysisResult<T>,
    ) -> AnalysisResult<T> {
        let previous = self.meter.held_bytes;
        self.meter.held_bytes = previous.saturating_add(held);
        let result = work(self);
        self.meter.held_bytes = previous;
        result
    }

    pub(super) fn sync_live_bytes(&mut self) {
        self.meter.live_bytes = recompute_live_bytes(self);
    }

    /// Polls `cancellation` while this tracker undoes instructions and repeat iterations.
    pub(crate) fn set_cancellation(&mut self, cancellation: Cancellation) {
        self.cancellation.signal = cancellation;
    }

    /// Fails when cancellation has been requested, for work that holds the tracker immutably.
    pub(crate) fn check_cancelled(&self) -> AnalysisResult<()> {
        Ok(self.cancellation.signal.check()?)
    }

    pub(crate) fn cancellation_signal(&self) -> &Cancellation {
        &self.cancellation.signal
    }

    /// Counts one bounded unit of reverse work, failing once cancellation is observed.
    #[inline]
    pub(crate) fn poll_cancellation(&mut self) -> AnalysisResult<()> {
        let TrackerCancellation { signal, countdown } = &mut self.cancellation;
        Ok(countdown.tick(signal)?)
    }

    pub(crate) fn measurement_count(&self) -> usize {
        self.measurement_count
    }

    pub(crate) fn detector_count(&self) -> u64 {
        self.detector_count
    }

    pub(crate) fn is_shifted_copy(&self, other: &Self) -> bool {
        shifted_repeat::is_shifted_copy(self, other)
    }

    pub(crate) fn shift_counts(
        &mut self,
        measurement_offset: i128,
        detector_offset: i128,
    ) -> AnalysisResult<()> {
        shifted_repeat::shift(self, measurement_offset, detector_offset)
    }

    pub(crate) fn undo_circuit(&mut self, circuit: &Circuit) -> AnalysisResult<()> {
        self.undo_circuit_with_gauge_output(circuit, GaugeOutputPolicy::Preserve)
    }

    pub(crate) fn undo_repeated_circuit(
        &mut self,
        body: &Circuit,
        repetitions: u64,
    ) -> AnalysisResult<()> {
        shifted_repeat::undo_loop_with_gauge_output(
            self,
            body,
            repetitions,
            GaugeOutputPolicy::Preserve,
        )
    }

    pub(crate) fn undo_circuit_for_analyzer_probe(
        &mut self,
        circuit: &Circuit,
    ) -> AnalysisResult<()> {
        self.undo_circuit_with_gauge_output(circuit, GaugeOutputPolicy::Discard)
    }

    fn undo_circuit_with_gauge_output(
        &mut self,
        circuit: &Circuit,
        gauge_output: GaugeOutputPolicy,
    ) -> AnalysisResult<()> {
        shifted_repeat::undo_circuit_with_gauge_output(self, circuit, gauge_output)
    }

    pub(crate) fn toggle_pauli_target(
        &mut self,
        qubit: QubitId,
        basis: PauliBasis,
        target: DemTarget,
    ) -> AnalysisResult<()> {
        let Some(basis) = TrackerBasis::from_pauli_basis(basis) else {
            return Ok(());
        };
        self.toggle_product_sensitivity(&[(qubit, basis)], &[target])
    }

    pub(crate) fn toggle_record_target_absolute(
        &mut self,
        index: usize,
        target: DemTarget,
    ) -> AnalysisResult<()> {
        if index >= self.measurement_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "measurement record index {index} is outside the sparse reverse tracker history"
            )));
        }
        self.toggle_record_target(index, target)
    }

    pub(crate) fn toggle_observable_effect(
        &mut self,
        observable: u32,
        target: DemTarget,
    ) -> AnalysisResult<()> {
        let key = u64::from(observable);
        let existing = self.observable_effects.get(&key);
        self.scratch
            .admit_growth(&self.meter, xor_item_growth_bytes(existing))?;
        let before = set_entry_bytes(existing);
        let set = self.observable_effects.entry(key).or_default();
        set.xor_item(target);
        let after = set_entry_bytes(Some(set));
        self.meter.adjust(before, after);
        Ok(())
    }

    pub(crate) fn pauli_targets_at(&self, qubit: QubitId) -> AnalysisResult<BTreeSet<DemTarget>> {
        Ok(self
            .xs_for(qubit)?
            .iter()
            .chain(self.zs_for(qubit)?)
            .copied()
            .collect())
    }

    pub(crate) fn record_targets_at(&self, index: usize) -> AnalysisResult<BTreeSet<DemTarget>> {
        Ok(self
            .record_targets_slice_at(index)?
            .iter()
            .copied()
            .collect())
    }

    /// Borrows the sorted sensitivity of one measurement record.
    pub(crate) fn record_targets_slice_at(&self, index: usize) -> AnalysisResult<&[DemTarget]> {
        if index >= self.measurement_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "measurement record index {index} is outside the sparse reverse tracker history"
            )));
        }
        Ok(sorted_slice(self.rec_bits.get(&index)))
    }

    pub(crate) fn active_targets(&self) -> BTreeSet<DemTarget> {
        let mut result = BTreeSet::new();
        for targets in self.xs.values().chain(self.zs.values()) {
            result.extend(targets);
        }
        for targets in self.rec_bits.values() {
            result.extend(targets);
        }
        result
    }

    pub(crate) fn target_anticommuted(&self, target: DemTarget) -> bool {
        self.anticommutations
            .iter()
            .any(|anticommutation| anticommutation.target == target)
    }

    pub(crate) fn undo_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        self.poll_cancellation()?;
        let result = match reverse_flow_transition(instruction) {
            ReverseFlowTransition::Measurement(basis) => {
                self.undo_measurements(instruction, tracker_basis(basis)?)
            }
            ReverseFlowTransition::Reset(basis) => {
                self.undo_resets(instruction, tracker_basis(basis)?)
            }
            ReverseFlowTransition::MeasureReset(basis) => {
                self.undo_measure_resets(instruction, tracker_basis(basis)?)
            }
            ReverseFlowTransition::PairMeasurement(basis) => {
                self.undo_pair_measurements(instruction, tracker_basis(basis)?)
            }
            ReverseFlowTransition::PauliProductMeasurement => {
                self.undo_pauli_product_measurements(instruction)
            }
            ReverseFlowTransition::MeasurementPad => self.undo_measurement_pads(instruction),
            ReverseFlowTransition::HeraldedMeasurement => {
                self.undo_heralded_measurements(instruction)
            }
            ReverseFlowTransition::PauliProductUnitary => self.undo_spp(instruction),
            ReverseFlowTransition::ControlledPauli(_) => {
                if tracked_controlled_pauli(instruction)?.slot != ControlSlot::Second
                    || instruction
                        .targets()
                        .iter()
                        .any(Target::is_classical_bit_target)
                {
                    self.undo_controlled_pauli(instruction)
                } else {
                    self.undo_two_qubit_tableau(instruction)
                }
            }
            ReverseFlowTransition::Detector => self.undo_detector(instruction),
            ReverseFlowTransition::Observable => self.undo_observable_include(instruction),
            ReverseFlowTransition::Tableau => {
                if instruction.gate().is_two_qubit_gate() {
                    self.undo_two_qubit_tableau(instruction)
                } else {
                    let action = single_qubit_action(instruction.gate())?;
                    for qubit in qubits_reversed(instruction) {
                        self.apply_single_qubit_action(action, qubit?)?;
                        self.check_live_budget()?;
                    }
                    Ok(())
                }
            }
            ReverseFlowTransition::SweepControlledPauliNoop | ReverseFlowTransition::Ignored => {
                Ok(())
            }
            ReverseFlowTransition::Unsupported => {
                Err(AnalysisError::invalid_detector_error_model(format!(
                    "sparse reverse frame tracker does not support gate {}",
                    instruction.gate().canonical_name()
                )))
            }
        };
        result?;
        self.check_live_budget()
    }

    pub(crate) fn feedback_sensitivity(
        &self,
        qubit: QubitId,
        feedback: Pauli,
    ) -> AnalysisResult<BTreeSet<DemTarget>> {
        let mut sensitivity = Vec::new();
        self.anticommuting_sensitivity_into(
            qubit,
            TrackerBasis::from_pauli(feedback),
            &mut sensitivity,
        )?;
        Ok(sensitivity.into_iter().collect())
    }

    pub(crate) fn absolute_record_index_from_offset(&self, offset: i32) -> AnalysisResult<usize> {
        self.record_index_from_offset(offset)
    }

    pub(crate) fn undo_implicit_rz_at_start_of_circuit(&mut self) -> AnalysisResult<()> {
        let active_qubits = self
            .xs
            .keys()
            .chain(self.zs.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        for qubit in active_qubits {
            self.check_reset_gauge(qubit, TrackerBasis::Z)?;
        }
        self.check_live_budget()
    }

    fn undo_spp(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        for terms in pauli_product_terms_reversed(instruction)? {
            let sensitivity = self.product_anticommuting_sensitivity(&terms)?;
            // SPP and SPP_DAG differ only by phase signs on anticommuting Paulis; this tracker
            // stores unsigned detector and observable regions, so both gates share propagation.
            let toggled = self.holding(held_set_bytes(&sensitivity), |tracker| {
                tracker.toggle_product_sensitivity(&terms, sensitivity.as_slice())
            });
            self.scratch.recycle(sensitivity);
            toggled?;
            self.check_live_budget()?;
        }
        Ok(())
    }

    fn undo_controlled_pauli(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let form = tracked_controlled_pauli(instruction)?;
        for group in instruction.target_groups().into_iter().rev() {
            let gate_name = instruction.gate().canonical_name();
            let [first, second] = group else {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during sparse reverse tracking",
                )));
            };
            let (control, target) = match form.slot {
                ControlSlot::Second => (second, first),
                ControlSlot::First | ControlSlot::Either => (first, second),
            };
            if let (Some(left), Some(right)) = (first.qubit_id(), second.qubit_id()) {
                if form.slot != ControlSlot::Second {
                    self.undo_quantum_controlled_pauli(instruction, control, target)?;
                } else {
                    let action = two_qubit_action(instruction.gate())?;
                    self.apply_two_qubit_action(action, left, right)?;
                }
            } else if form.slot == ControlSlot::Either {
                // A classical bit in either CZ slot Z-controls the other
                // slot; two classical bits have no effect (undo_ZCZ_single).
                if target.qubit_id().is_some() {
                    self.undo_classical_pauli(instruction, control, target)?;
                } else if control.qubit_id().is_some() {
                    self.undo_classical_pauli(instruction, target, control)?;
                }
            } else if target.qubit_id().is_none() {
                // Vendor undo_ZC{X,Y}_single throws whenever the Pauli slot
                // is not a qubit; the analyzer surface pins that rejection.
                // The flow solver and checker keep the pinned PFM-B4 contract
                // of ignoring classical-control groups instead, so only
                // error-analysis mode rejects here.
                if self.error_analysis_mode {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "{gate_name} target {target} is not a qubit"
                    )));
                }
            } else {
                self.undo_classical_pauli(instruction, control, target)?;
            }
            self.check_live_budget()?;
        }
        Ok(())
    }

    /// A classical control either has no effect on error propagation (sweep
    /// bits) or toggles the record's sensitivity by the target qubit's
    /// flipped bases (measurement records); vendor `undo_classical_pauli`.
    fn undo_classical_pauli(
        &mut self,
        instruction: &CircuitInstruction,
        control: &Target,
        target: &Target,
    ) -> AnalysisResult<()> {
        if control.is_sweep_bit_target() {
            return Ok(());
        }
        self.undo_classical_feedback(instruction, control, target)
    }

    fn undo_classical_feedback(
        &mut self,
        instruction: &CircuitInstruction,
        control: &Target,
        target: &Target,
    ) -> AnalysisResult<()> {
        let record = control.measurement_record_offset().ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} classical control {control} is not a measurement record",
                instruction.gate().canonical_name()
            ))
        })?;
        let qubit = target.qubit_id().ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} feedback target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        let basis = TrackerBasis::from_pauli(tracked_controlled_pauli(instruction)?.pauli);
        let mut sensitivity = self.scratch.take_operand();
        self.anticommuting_sensitivity_into(qubit, basis, &mut sensitivity)?;
        let index = self.record_index_from_offset(record.get())?;
        let toggled = self.holding(budget::buffer_bytes(sensitivity.capacity()), |tracker| {
            tracker.toggle_record_sensitivity(index, &sensitivity)
        });
        self.scratch.operand = sensitivity;
        toggled
    }

    fn undo_quantum_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
        control: &Target,
        target: &Target,
    ) -> AnalysisResult<()> {
        let control = control.qubit_id().ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} control target {control} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        let target = target.qubit_id().ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "{} target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        let target_basis = match controlled_pauli_form(instruction.gate()) {
            Some(form) if form.slot != ControlSlot::Second => TrackerBasis::from_pauli(form.pauli),
            Some(_) | None => {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{} sparse reverse tracking is not implemented for qubit-qubit controls",
                    instruction.gate().canonical_name()
                )));
            }
        };
        // Undoing a controlled Pauli moves the target's anticommuting sensitivity onto the
        // control's Z slot, then the control's X sensitivity onto the target's Pauli slots.
        // Each operand is copied before the toggle that could alias it.
        let mut operand = self.scratch.take_operand();
        self.anticommuting_sensitivity_into(target, target_basis, &mut operand)?;
        self.holding(budget::buffer_bytes(operand.capacity()), |tracker| {
            tracker.toggle_zs(control, &operand)
        })?;
        let control_xs = self.xs_for(control)?;
        self.scratch.admit_growth(
            &self.meter,
            buffer_growth_bytes(operand.capacity(), control_xs.len())
                .saturating_add(budget::buffer_bytes(operand.capacity())),
        )?;
        operand.clear();
        operand.extend_from_slice(control_xs);
        self.holding(budget::buffer_bytes(operand.capacity()), |tracker| {
            tracker.toggle_product_sensitivity(&[(target, target_basis)], &operand)
        })?;
        self.scratch.operand = operand;
        Ok(())
    }

    fn undo_two_qubit_tableau(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let gate_name = instruction.gate().canonical_name();
        let action = two_qubit_action(instruction.gate())?;
        for group in instruction.target_groups().into_iter().rev() {
            let [left, right] = group else {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during sparse reverse tracking"
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{gate_name} target {left} is not a qubit"
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{gate_name} target {right} is not a qubit"
                ))
            })?;
            self.apply_two_qubit_action(action, left, right)?;
            self.check_live_budget()?;
        }
        Ok(())
    }

    fn undo_detector(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.detector_count = self.detector_count.checked_sub(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detector count underflowed during sparse reverse tracking",
            )
        })?;
        let detector = DemTarget::relative_detector(self.detector_count)?;
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            let index = self.record_index_from_offset(offset.get())?;
            let existing = self.rec_bits.get(&index);
            self.scratch
                .admit_growth(&self.meter, xor_item_growth_bytes(existing))?;
            let before = set_entry_bytes(existing);
            let set = self.rec_bits.entry(index).or_default();
            set.xor_item(detector);
            let after = set_entry_bytes(Some(set));
            self.meter.adjust(before, after);
            self.check_live_budget()?;
        }
        Ok(())
    }

    fn undo_observable_include(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "OBSERVABLE_INCLUDE is missing an observable id",
            )
        })?;
        let mut sensitivity = self.scratch.take_operand();
        let effects = self.observable_effects.get(&observable.get());
        self.scratch.admit_growth(
            &self.meter,
            buffer_growth_bytes(sensitivity.capacity(), effects.map_or(1, TargetXorSet::len)),
        )?;
        match effects {
            Some(effects) => sensitivity.extend_from_slice(effects.as_slice()),
            None => sensitivity.push(DemTarget::logical_observable(observable.get())?),
        }
        let held = budget::buffer_bytes(sensitivity.capacity());
        let included = self.holding(held, |tracker| {
            tracker.include_observable_sensitivity(instruction, &sensitivity)
        });
        self.scratch.operand = sensitivity;
        included
    }

    /// XORs an observable's sensitivity into each record and Pauli target of its include.
    fn include_observable_sensitivity(
        &mut self,
        instruction: &CircuitInstruction,
        sensitivity: &[DemTarget],
    ) -> AnalysisResult<()> {
        for target in instruction.targets() {
            match target.kind() {
                TargetKind::MeasurementRecord { offset } => {
                    let index = self.record_index_from_offset(offset.get())?;
                    let existing = self.rec_bits.get(&index);
                    let growth = xor_growth_bytes(
                        existing,
                        sensitivity.len(),
                        0,
                        self.scratch.merge.capacity(),
                    );
                    self.scratch.admit_growth(&self.meter, growth)?;
                    let before = set_entry_bytes(existing);
                    let set = self.rec_bits.entry(index).or_default();
                    set.xor_sorted(sensitivity, &mut self.scratch.merge);
                    let after = set_entry_bytes(Some(set));
                    self.meter.adjust(before, after);
                }
                TargetKind::Pauli { pauli, id, .. } => {
                    self.toggle_product_sensitivity(
                        &[(id, TrackerBasis::from_pauli(pauli))],
                        sensitivity,
                    )?;
                }
                TargetKind::Qubit { .. } | TargetKind::SweepBit { .. } | TargetKind::Combiner => {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "OBSERVABLE_INCLUDE target {target} is not a measurement record or Pauli target"
                    )));
                }
            }
            self.check_live_budget()?;
        }
        Ok(())
    }

    /// Writes the targets whose tracked Pauli anticommutes with `basis` on `qubit` into `out`.
    fn anticommuting_sensitivity_into(
        &self,
        qubit: QubitId,
        basis: TrackerBasis,
        out: &mut Vec<DemTarget>,
    ) -> AnalysisResult<()> {
        let xs = self.xs_for(qubit)?;
        let zs = self.zs_for(qubit)?;
        let needed = match basis {
            TrackerBasis::X => zs.len(),
            TrackerBasis::Y => xs.len().saturating_add(zs.len()),
            TrackerBasis::Z => xs.len(),
        };
        let growth = buffer_growth_bytes(out.capacity(), needed);
        if growth > 0 {
            // `out` is usually the operand, out of the scratch while it is filled.
            self.scratch.admit_growth(
                &self.meter,
                growth.saturating_add(budget::buffer_bytes(out.capacity())),
            )?;
        }
        match basis {
            TrackerBasis::X => {
                out.clear();
                out.extend_from_slice(zs);
            }
            TrackerBasis::Y => xor_merge_into(xs, zs, out),
            TrackerBasis::Z => {
                out.clear();
                out.extend_from_slice(xs);
            }
        }
        Ok(())
    }

    /// XOR of the anticommuting sensitivities of every term of a Pauli product.
    fn product_anticommuting_sensitivity(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
    ) -> AnalysisResult<TargetXorSet> {
        // The product and every partial product stay within the sum of the terms' sensitivities.
        let mut needed = 0_usize;
        for (qubit, _) in terms {
            needed = needed
                .saturating_add(self.xs_for(*qubit)?.len())
                .saturating_add(self.zs_for(*qubit)?.len());
        }
        // The product and the merge buffer trade storage, so each may grow to hold the product.
        let growth = sets::unknown_buffer_growth_bytes(needed).saturating_mul(2);
        self.scratch.admit_growth(&self.meter, growth)?;
        let mut product = self.scratch.take_set();
        let mut operand = self.scratch.take_operand();
        for (qubit, basis) in terms {
            self.anticommuting_sensitivity_into(*qubit, *basis, &mut operand)?;
            product.xor_sorted(&operand, &mut self.scratch.merge);
        }
        self.scratch.operand = operand;
        Ok(product)
    }

    fn toggle_product_sensitivity(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
        sensitivity: &[DemTarget],
    ) -> AnalysisResult<()> {
        for (qubit, basis) in terms {
            match basis {
                TrackerBasis::X => self.toggle_xs(*qubit, sensitivity)?,
                TrackerBasis::Y => {
                    self.toggle_xs(*qubit, sensitivity)?;
                    self.toggle_zs(*qubit, sensitivity)?;
                }
                TrackerBasis::Z => self.toggle_zs(*qubit, sensitivity)?,
            }
        }
        Ok(())
    }

    /// Toggles a record's sensitivity, dropping the entry when it cancels to empty.
    ///
    /// An empty `sensitivity` leaves the record untouched, like toggling no targets one by one.
    fn toggle_record_sensitivity(
        &mut self,
        index: usize,
        sensitivity: &[DemTarget],
    ) -> AnalysisResult<()> {
        if sensitivity.is_empty() {
            return Ok(());
        }
        let existing = self.rec_bits.get(&index);
        self.scratch.admit_growth(
            &self.meter,
            xor_growth_bytes(
                existing,
                sensitivity.len(),
                0,
                self.scratch.merge.capacity(),
            ),
        )?;
        let before = set_entry_bytes(existing);
        let targets = self.rec_bits.entry(index).or_default();
        targets.xor_sorted(sensitivity, &mut self.scratch.merge);
        if targets.is_empty()
            && let Some(removed) = self.rec_bits.remove(&index)
        {
            self.scratch.recycle(removed);
        }
        let after = set_entry_bytes(self.rec_bits.get(&index));
        self.meter.adjust(before, after);
        Ok(())
    }

    fn toggle_record_target(&mut self, index: usize, target: DemTarget) -> AnalysisResult<()> {
        self.toggle_record_sensitivity(index, &[target])
    }

    fn clear_qubit(&mut self, qubit: QubitId) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        for removed in [self.xs.remove(&qubit), self.zs.remove(&qubit)]
            .into_iter()
            .flatten()
        {
            self.meter.adjust(set_entry_bytes(Some(&removed)), 0);
            self.scratch.recycle(removed);
        }
        Ok(())
    }

    fn xs_for(&self, qubit: QubitId) -> AnalysisResult<&[DemTarget]> {
        self.validate_qubit(qubit)?;
        Ok(sorted_slice(self.xs.get(&qubit)))
    }

    fn zs_for(&self, qubit: QubitId) -> AnalysisResult<&[DemTarget]> {
        self.validate_qubit(qubit)?;
        Ok(sorted_slice(self.zs.get(&qubit)))
    }

    fn toggle_xs(&mut self, qubit: QubitId, targets: &[DemTarget]) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        toggle_qubit_set(
            &mut self.xs,
            qubit,
            targets,
            &mut self.scratch,
            &mut self.meter,
        )
    }

    fn toggle_zs(&mut self, qubit: QubitId, targets: &[DemTarget]) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        toggle_qubit_set(
            &mut self.zs,
            qubit,
            targets,
            &mut self.scratch,
            &mut self.meter,
        )
    }

    fn ensure_measurements_available(
        &self,
        count: usize,
        gate: &'static str,
    ) -> AnalysisResult<()> {
        if self.measurement_count < count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "{gate} needs {count} prior measurement(s), but sparse reverse tracker only has {}",
                self.measurement_count
            )));
        }
        Ok(())
    }

    fn validate_qubit(&self, qubit: QubitId) -> AnalysisResult<()> {
        let index = qubit_index(qubit)?;
        if index >= self.qubit_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        }
        Ok(())
    }

    fn record_index_from_offset(&self, offset: i32) -> AnalysisResult<usize> {
        let measurement_count = i64::try_from(self.measurement_count).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "measurement count does not fit i64 during sparse reverse tracking",
            )
        })?;
        let index = measurement_count
            .checked_add(i64::from(offset))
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "measurement record offset overflowed during sparse reverse tracking",
                )
            })?;
        if index < 0 || index > measurement_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "measurement record offset rec[{offset}] is out of range"
            )));
        }
        usize::try_from(index).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "measurement record index does not fit usize during sparse reverse tracking",
            )
        })
    }
}

/// What the tracker does when a collapse anticommutes with a tracked detector or observable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnticommutationPolicy {
    /// Refuses the circuit with a validation error naming the anticommuting targets.
    Fail,
    /// Records the anticommutation and keeps going, for callers that report them.
    Record,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GaugeOutputPolicy {
    Preserve,
    Discard,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Anticommutation {
    target: DemTarget,
    location: TrackerLocation,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TrackerLocation {
    qubit: QubitId,
    basis: TrackerBasis,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TrackerBasis {
    X,
    Y,
    Z,
}

impl TrackerBasis {
    fn from_pauli_basis(basis: PauliBasis) -> Option<Self> {
        match basis {
            PauliBasis::I => None,
            PauliBasis::X => Some(Self::X),
            PauliBasis::Y => Some(Self::Y),
            PauliBasis::Z => Some(Self::Z),
        }
    }

    fn from_pauli(pauli: Pauli) -> Self {
        match pauli {
            Pauli::X => Self::X,
            Pauli::Y => Self::Y,
            Pauli::Z => Self::Z,
        }
    }
}

fn qubit_index(qubit: QubitId) -> AnalysisResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "qubit id {} does not fit usize during sparse reverse tracking",
            qubit.get()
        ))
    })
}

/// Bytes of a sensitivity set held outside the tracker's maps while it is merged.
fn held_set_bytes(set: &TargetXorSet) -> u64 {
    budget::buffer_bytes(set.capacity())
}

fn sorted_slice(set: Option<&TargetXorSet>) -> &[DemTarget] {
    set.map_or(&[], TargetXorSet::as_slice)
}

#[cfg(test)]
mod tests;
