use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::{FlexPauliString, PauliBasis, PauliPhase};
use stab_model::{Circuit, CircuitInstruction, CircuitItem, DemTarget, Pauli, QubitId, Target};

mod budget;
mod clifford_action;
mod error_analysis;
mod pauli_product;
mod shifted_recurrence;
mod shifted_repeat;
mod unitary_repeat;

use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};
use crate::target_xor_set::{TargetXorSet, xor_merge_into};
use crate::{AnalysisError, AnalysisResult};
pub(crate) use budget::{ReverseTrackerBudget, ReverseTrackerWorkBudget};
use clifford_action::{single_qubit_action, two_qubit_action};
use pauli_product::{pauli_product_measurement_terms_reversed, pauli_product_terms_reversed};
pub(crate) use shifted_recurrence::{ShiftedRecurrenceSearch, search_shifted_recurrence};

/// Sensitivity sets keyed by qubit, record index, or observable; absent keys are empty sets.
type TargetSets<K> = BTreeMap<K, TargetXorSet>;

/// Upper bound on recycled sensitivity buffers kept between operations.
const MAX_POOLED_SETS: usize = 32;

fn tracker_basis(basis: PauliBasis) -> AnalysisResult<TrackerBasis> {
    TrackerBasis::from_pauli_basis(basis).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(
            "identity basis is not a reverse stabilizer transition",
        )
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SparseReverseFrameTracker {
    xs: TargetSets<QubitId>,
    zs: TargetSets<QubitId>,
    qubit_count: usize,
    rec_bits: TargetSets<usize>,
    measurement_count: usize,
    detector_count: u64,
    observable_effects: TargetSets<u64>,
    fail_on_anticommute: bool,
    error_analysis_mode: bool,
    eliminate_detector_gauges: bool,
    gauge_errors: Vec<TargetXorSet>,
    anticommutations: BTreeSet<Anticommutation>,
    scratch: TrackerScratch,
}

/// Reused merge buffers. They hold no tracker state, so clones start empty and every
/// scratch value compares equal.
#[derive(Debug, Default)]
struct TrackerScratch {
    merge: Vec<DemTarget>,
    operand: Vec<DemTarget>,
    pool: Vec<TargetXorSet>,
}

impl TrackerScratch {
    fn take_set(&mut self) -> TargetXorSet {
        self.pool.pop().unwrap_or_default()
    }

    fn recycle(&mut self, mut set: TargetXorSet) {
        if self.pool.len() < MAX_POOLED_SETS {
            set.clear();
            self.pool.push(set);
        }
    }

    fn take_operand(&mut self) -> Vec<DemTarget> {
        let mut operand = std::mem::take(&mut self.operand);
        operand.clear();
        operand
    }
}

impl Clone for TrackerScratch {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for TrackerScratch {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for TrackerScratch {}

impl SparseReverseFrameTracker {
    pub(crate) fn new(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: u64,
        fail_on_anticommute: bool,
    ) -> Self {
        Self {
            xs: BTreeMap::new(),
            zs: BTreeMap::new(),
            qubit_count,
            rec_bits: BTreeMap::new(),
            measurement_count,
            detector_count,
            observable_effects: BTreeMap::new(),
            fail_on_anticommute,
            error_analysis_mode: false,
            eliminate_detector_gauges: false,
            gauge_errors: Vec::new(),
            anticommutations: BTreeSet::new(),
            scratch: TrackerScratch::default(),
        }
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
        let mut budget = ReverseTrackerBudget::Unlimited;
        self.undo_circuit_with_gauge_output(circuit, GaugeOutputPolicy::Preserve, &mut budget)
    }

    pub(crate) fn undo_repeated_circuit_with_budget(
        &mut self,
        body: &Circuit,
        repetitions: u64,
        budget: &mut dyn ReverseTrackerWorkBudget,
    ) -> AnalysisResult<()> {
        let mut budget = ReverseTrackerBudget::Metered(budget);
        shifted_repeat::undo_loop_with_gauge_output(
            self,
            body,
            repetitions,
            GaugeOutputPolicy::Preserve,
            &mut budget,
        )
    }

    pub(crate) fn undo_circuit_for_analyzer_probe(
        &mut self,
        circuit: &Circuit,
    ) -> AnalysisResult<()> {
        self.undo_circuit_with_gauge_output(
            circuit,
            GaugeOutputPolicy::Discard,
            &mut ReverseTrackerBudget::Unlimited,
        )
    }

    fn undo_circuit_with_gauge_output(
        &mut self,
        circuit: &Circuit,
        gauge_output: GaugeOutputPolicy,
        work_budget: &mut ReverseTrackerBudget<'_>,
    ) -> AnalysisResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    work_budget.admit_instruction(instruction)?;
                    self.undo_instruction(instruction)?;
                    if gauge_output == GaugeOutputPolicy::Discard {
                        self.take_gauge_errors();
                    }
                }
                CircuitItem::RepeatBlock(repeat) => {
                    if gauge_output == GaugeOutputPolicy::Preserve
                        && unitary_repeat::try_undo_supported_unitary_repeat(self, repeat)?
                    {
                        continue;
                    }
                    shifted_repeat::undo_loop_with_gauge_output(
                        self,
                        repeat.body(),
                        repeat.repeat_count().get(),
                        gauge_output,
                        work_budget,
                    )?;
                }
            }
        }
        Ok(())
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
        self.toggle_record_target(index, target);
        Ok(())
    }

    pub(crate) fn toggle_observable_effect(&mut self, observable: u32, target: DemTarget) {
        self.observable_effects
            .entry(u64::from(observable))
            .or_default()
            .xor_item(target);
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
        match reverse_flow_transition(instruction) {
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
                if matches!(instruction.gate().canonical_name(), "CX" | "CY" | "CZ")
                    || instruction
                        .targets()
                        .iter()
                        .any(Target::is_classical_bit_target)
                {
                    self.undo_controlled_pauli(instruction)
                } else {
                    self.undo_two_qubit_tableau(instruction, instruction.gate().canonical_name())
                }
            }
            ReverseFlowTransition::Detector => self.undo_detector(instruction),
            ReverseFlowTransition::Observable => self.undo_observable_include(instruction),
            ReverseFlowTransition::Tableau => {
                if instruction.gate().is_two_qubit_gate() {
                    self.undo_two_qubit_tableau(instruction, instruction.gate().canonical_name())
                } else {
                    let action = single_qubit_action(instruction.gate())?;
                    for qubit in qubits_reversed(instruction)? {
                        self.apply_single_qubit_action(action, qubit)?;
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
        }
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

    pub(crate) fn region_for_target(&self, target: DemTarget) -> AnalysisResult<FlexPauliString> {
        self.region_for_target_with_len(target, self.qubit_count)
    }

    pub(crate) fn compact_region_for_target(
        &self,
        target: DemTarget,
    ) -> AnalysisResult<FlexPauliString> {
        let len = self
            .xs
            .iter()
            .chain(&self.zs)
            .filter_map(|(qubit, targets)| targets.contains(&target).then_some(qubit.get()))
            .max()
            .map(|max_qubit| qubit_index(QubitId::new(max_qubit)?).map(|index| index + 1))
            .transpose()?
            .unwrap_or(0);
        self.region_for_target_with_len(target, len)
    }

    fn region_for_target_with_len(
        &self,
        target: DemTarget,
        len: usize,
    ) -> AnalysisResult<FlexPauliString> {
        let mut bases = vec![PauliBasis::I; len];
        for (qubit, xs) in &self.xs {
            if xs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "X-basis sensitivity qubit {} is outside its sparse region",
                        qubit.get()
                    ))
                })?;
                *basis = PauliBasis::X;
            }
        }
        for (qubit, zs) in &self.zs {
            if zs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "Z-basis sensitivity qubit {} is outside its sparse region",
                        qubit.get()
                    ))
                })?;
                *basis = match *basis {
                    PauliBasis::I => PauliBasis::Z,
                    PauliBasis::X => PauliBasis::Y,
                    actual => {
                        return Err(AnalysisError::invalid_detector_error_model(format!(
                            "unexpected {actual:?} basis while building sparse region for qubit {}",
                            qubit.get()
                        )));
                    }
                };
            }
        }
        FlexPauliString::from_phase_and_bases(PauliPhase::Plus, bases).map_err(|error| {
            AnalysisError::invalid_detector_error_model(format!(
                "failed to build detecting region for {target}: {error}"
            ))
        })
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
        Ok(())
    }

    fn undo_measure_resets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let qubits = qubits_reversed(instruction)?;
        self.ensure_measurements_available(qubits.len(), instruction.gate().canonical_name())?;
        for qubit in &qubits {
            self.check_reset_gauge(*qubit, basis)?;
        }
        for qubit in qubits {
            self.clear_qubit(qubit)?;
            self.undo_measurement_target(qubit, basis)?;
        }
        Ok(())
    }

    fn undo_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let qubits = qubits_reversed(instruction)?;
        self.ensure_measurements_available(qubits.len(), instruction.gate().canonical_name())?;
        for qubit in &qubits {
            self.check_measurement_gauge(*qubit, basis)?;
        }
        for qubit in qubits {
            self.undo_measurement_target(qubit, basis)?;
        }
        Ok(())
    }

    fn undo_pair_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let terms = pair_measurement_terms_reversed(instruction, basis)?;
        self.ensure_measurements_available(terms.len(), instruction.gate().canonical_name())?;
        for term in &terms {
            self.check_product_measurement_gauge(term)?;
        }
        for term in terms {
            let sensitivity = self.pop_record_sensitivity()?;
            self.toggle_product_sensitivity(&term, sensitivity.as_slice())?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    fn undo_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let terms = pauli_product_measurement_terms_reversed(instruction)?;
        self.ensure_measurements_available(terms.len(), instruction.gate().canonical_name())?;
        for term in &terms {
            self.check_product_measurement_gauge(term)?;
        }
        for term in terms {
            let sensitivity = self.pop_record_sensitivity()?;
            self.toggle_product_sensitivity(&term, sensitivity.as_slice())?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    fn undo_spp(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        for terms in pauli_product_terms_reversed(instruction)? {
            let sensitivity = self.product_anticommuting_sensitivity(&terms)?;
            // SPP and SPP_DAG differ only by phase signs on anticommuting Paulis; this tracker
            // stores unsigned detector and observable regions, so both gates share propagation.
            self.toggle_product_sensitivity(&terms, sensitivity.as_slice())?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    fn undo_measurement_target(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let sensitivity = self.pop_record_sensitivity()?;
        let result = self.toggle_product_sensitivity(&[(qubit, basis)], sensitivity.as_slice());
        self.scratch.recycle(sensitivity);
        result
    }

    fn undo_measurement_pads(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.ensure_measurements_available(instruction.targets().len(), "MPAD")?;
        for _ in instruction.targets().iter().rev() {
            let sensitivity = self.pop_record_sensitivity()?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    fn undo_heralded_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        self.ensure_measurements_available(
            instruction.targets().len(),
            instruction.gate().canonical_name(),
        )?;
        for _ in instruction.targets().iter().rev() {
            let sensitivity = self.pop_record_sensitivity()?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    pub(crate) fn pop_record_sensitivity(&mut self) -> AnalysisResult<TargetXorSet> {
        self.measurement_count = self.measurement_count.checked_sub(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "measurement count underflowed during sparse reverse tracking",
            )
        })?;
        Ok(self
            .rec_bits
            .remove(&self.measurement_count)
            .unwrap_or_default())
    }

    fn undo_resets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let qubits = qubits_reversed(instruction)?;
        for qubit in &qubits {
            self.check_reset_gauge(*qubit, basis)?;
        }
        for qubit in qubits {
            self.clear_qubit(qubit)?;
        }
        Ok(())
    }

    fn undo_controlled_pauli(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let gate_name = instruction.gate().canonical_name();
            let [first, second] = group else {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during sparse reverse tracking",
                )));
            };
            // Vendor orientation (sparse_rev_frame_tracker.cc,
            // undo_ZC{X,Y,Z}_single): CX/CY carry the classical-capable
            // control first, XCZ/YCZ carry the Pauli side first, and CZ
            // accepts a classical bit in either slot.
            let (control, target) = match gate_name {
                "XCZ" | "YCZ" => (second, first),
                _ => (first, second),
            };
            if let (Some(left), Some(right)) = (first.qubit_id(), second.qubit_id()) {
                if matches!(gate_name, "CX" | "CY" | "CZ") {
                    self.undo_quantum_controlled_pauli(instruction, control, target)?;
                } else {
                    let action = two_qubit_action(instruction.gate())?;
                    self.apply_two_qubit_action(action, left, right)?;
                }
            } else if gate_name == "CZ" {
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
        let basis = match instruction.gate().canonical_name() {
            "CX" | "XCZ" => TrackerBasis::X,
            "CY" | "YCZ" => TrackerBasis::Y,
            "CZ" => TrackerBasis::Z,
            name => {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{name} is not a supported sparse reverse feedback gate"
                )));
            }
        };
        let mut sensitivity = self.scratch.take_operand();
        self.anticommuting_sensitivity_into(qubit, basis, &mut sensitivity)?;
        let index = self.record_index_from_offset(record.get())?;
        self.toggle_record_sensitivity(index, &sensitivity);
        self.scratch.operand = sensitivity;
        Ok(())
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
        let target_basis = match instruction.gate().canonical_name() {
            "CX" => TrackerBasis::X,
            "CY" => TrackerBasis::Y,
            "CZ" => TrackerBasis::Z,
            name => {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{name} sparse reverse tracking is not implemented for qubit-qubit controls"
                )));
            }
        };
        // Undoing a controlled Pauli moves the target's anticommuting sensitivity onto the
        // control's Z slot, then the control's X sensitivity onto the target's Pauli slots.
        // Each operand is copied before the toggle that could alias it.
        let mut operand = self.scratch.take_operand();
        self.anticommuting_sensitivity_into(target, target_basis, &mut operand)?;
        self.toggle_zs(control, &operand)?;
        operand.clear();
        operand.extend_from_slice(self.xs_for(control)?);
        self.toggle_product_sensitivity(&[(target, target_basis)], &operand)?;
        self.scratch.operand = operand;
        Ok(())
    }

    fn undo_two_qubit_tableau(
        &mut self,
        instruction: &CircuitInstruction,
        gate_name: &'static str,
    ) -> AnalysisResult<()> {
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
            self.declared_record_targets(index).xor_item(detector);
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
        match self.observable_effects.get(&observable.get()) {
            Some(effects) => sensitivity.extend_from_slice(effects.as_slice()),
            None => sensitivity.push(DemTarget::logical_observable(observable.get())?),
        }
        for target in instruction.targets() {
            match target {
                Target::MeasurementRecord { offset } => {
                    let index = self.record_index_from_offset(offset.get())?;
                    self.rec_bits
                        .entry(index)
                        .or_default()
                        .xor_sorted(&sensitivity, &mut self.scratch.merge);
                }
                Target::Pauli { pauli, id, .. } => {
                    self.toggle_product_sensitivity(
                        &[(*id, TrackerBasis::from_pauli(*pauli))],
                        &sensitivity,
                    )?;
                }
                Target::Qubit { .. } | Target::SweepBit { .. } | Target::Combiner => {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "OBSERVABLE_INCLUDE target {target} is not a measurement record or Pauli target"
                    )));
                }
            }
        }
        self.scratch.operand = sensitivity;
        Ok(())
    }

    /// Writes the targets whose tracked Pauli anticommutes with `basis` on `qubit` into `out`.
    fn anticommuting_sensitivity_into(
        &self,
        qubit: QubitId,
        basis: TrackerBasis,
        out: &mut Vec<DemTarget>,
    ) -> AnalysisResult<()> {
        match basis {
            TrackerBasis::X => {
                out.clear();
                out.extend_from_slice(self.zs_for(qubit)?);
            }
            TrackerBasis::Y => xor_merge_into(self.xs_for(qubit)?, self.zs_for(qubit)?, out),
            TrackerBasis::Z => {
                out.clear();
                out.extend_from_slice(self.xs_for(qubit)?);
            }
        }
        Ok(())
    }

    /// XOR of the anticommuting sensitivities of every term of a Pauli product.
    fn product_anticommuting_sensitivity(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
    ) -> AnalysisResult<TargetXorSet> {
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
    fn toggle_record_sensitivity(&mut self, index: usize, sensitivity: &[DemTarget]) {
        if sensitivity.is_empty() {
            return;
        }
        let targets = self.rec_bits.entry(index).or_default();
        targets.xor_sorted(sensitivity, &mut self.scratch.merge);
        if targets.is_empty()
            && let Some(removed) = self.rec_bits.remove(&index)
        {
            self.scratch.recycle(removed);
        }
    }

    /// Record sensitivity touched by a `DETECTOR` or `OBSERVABLE_INCLUDE` declaration.
    ///
    /// Like pinned Stim's `rec_bits[index].xor_item(...)` (sparse_rev_frame_tracker.cc:837/850),
    /// the entry stays present even when duplicate record targets cancel it to empty; only undoing
    /// the measurement removes it. Entry presence is part of the shifted-copy comparison that picks
    /// the folded loop period, so dropping cancelled entries folds different iterations than Stim.
    fn declared_record_targets(&mut self, index: usize) -> &mut TargetXorSet {
        self.rec_bits.entry(index).or_default()
    }

    fn toggle_record_target(&mut self, index: usize, target: DemTarget) {
        self.toggle_record_sensitivity(index, &[target]);
    }

    fn clear_qubit(&mut self, qubit: QubitId) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        for removed in [self.xs.remove(&qubit), self.zs.remove(&qubit)]
            .into_iter()
            .flatten()
        {
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
        toggle_qubit_set(&mut self.xs, qubit, targets, &mut self.scratch);
        Ok(())
    }

    fn toggle_zs(&mut self, qubit: QubitId, targets: &[DemTarget]) -> AnalysisResult<()> {
        self.validate_qubit(qubit)?;
        toggle_qubit_set(&mut self.zs, qubit, targets, &mut self.scratch);
        Ok(())
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

fn qubits_reversed(instruction: &CircuitInstruction) -> AnalysisResult<Vec<QubitId>> {
    instruction
        .targets()
        .iter()
        .rev()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })
        })
        .collect()
}

fn pair_measurement_terms_reversed(
    instruction: &CircuitInstruction,
    basis: TrackerBasis,
) -> AnalysisResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction
        .target_groups()
        .into_iter()
        .rev()
        .map(|group| {
            let [left, right] = group else {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "{} expected paired targets during sparse reverse tracking",
                    instruction.gate().canonical_name()
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} target {left} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "{} target {right} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            Ok(vec![(left, basis), (right, basis)])
        })
        .collect()
}

fn qubit_index(qubit: QubitId) -> AnalysisResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "qubit id {} does not fit usize during sparse reverse tracking",
            qubit.get()
        ))
    })
}

fn sorted_slice(set: Option<&TargetXorSet>) -> &[DemTarget] {
    set.map_or(&[], TargetXorSet::as_slice)
}

/// XORs `targets` into one qubit's set, dropping the entry when it cancels to empty.
fn toggle_qubit_set(
    sets: &mut TargetSets<QubitId>,
    qubit: QubitId,
    targets: &[DemTarget],
    scratch: &mut TrackerScratch,
) {
    if targets.is_empty() {
        return;
    }
    let set = sets.entry(qubit).or_insert_with(|| scratch.take_set());
    set.xor_sorted(targets, &mut scratch.merge);
    if set.is_empty()
        && let Some(removed) = sets.remove(&qubit)
    {
        scratch.recycle(removed);
    }
}

/// Stores one qubit's recomputed set, recycling whichever buffer it displaces.
fn replace_qubit_set(
    sets: &mut TargetSets<QubitId>,
    qubit: QubitId,
    value: TargetXorSet,
    scratch: &mut TrackerScratch,
) {
    if value.is_empty() {
        if let Some(removed) = sets.remove(&qubit) {
            scratch.recycle(removed);
        }
        scratch.recycle(value);
    } else if let Some(replaced) = sets.insert(qubit, value) {
        scratch.recycle(replaced);
    }
}

#[cfg(test)]
mod tests;
