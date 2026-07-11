#![allow(
    dead_code,
    reason = "M10 lands sparse reverse tracker parity before the error matcher consumes this internal primitive"
)]
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget,
    FlexPauliString, Pauli, PauliBasis, PauliPhase, PauliSign, PauliString, QubitId,
    SingleQubitClifford, Tableau, Target,
};

mod pauli_product;
mod shifted_repeat;
mod unitary_repeat;

use pauli_product::{pauli_product_measurement_terms_reversed, pauli_product_terms_reversed};

use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};

static EMPTY_TARGETS: LazyLock<BTreeSet<DemTarget>> = LazyLock::new(BTreeSet::new);

fn tracker_basis(basis: PauliBasis) -> CircuitResult<TrackerBasis> {
    TrackerBasis::from_pauli_basis(basis).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "identity basis is not a reverse stabilizer transition",
        )
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SparseReverseFrameTracker {
    xs: BTreeMap<QubitId, BTreeSet<DemTarget>>,
    zs: BTreeMap<QubitId, BTreeSet<DemTarget>>,
    qubit_count: usize,
    rec_bits: BTreeMap<usize, BTreeSet<DemTarget>>,
    measurement_count: usize,
    detector_count: u64,
    observable_effects: BTreeMap<u64, BTreeSet<DemTarget>>,
    fail_on_anticommute: bool,
    anticommutations: BTreeSet<Anticommutation>,
}

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
            anticommutations: BTreeSet::new(),
        }
    }

    pub(crate) fn undo_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => self.undo_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    if unitary_repeat::try_undo_supported_unitary_repeat(self, repeat)? {
                        continue;
                    }
                    shifted_repeat::undo_loop(self, repeat.body(), repeat.repeat_count().get())?;
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
    ) -> CircuitResult<()> {
        let Some(basis) = TrackerBasis::from_pauli_basis(basis) else {
            return Ok(());
        };
        self.toggle_product_sensitivity(&[(qubit, basis)], &BTreeSet::from([target]))
    }

    pub(crate) fn toggle_record_target_absolute(
        &mut self,
        index: usize,
        target: DemTarget,
    ) -> CircuitResult<()> {
        if index >= self.measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record index {index} is outside the sparse reverse tracker history"
            )));
        }
        self.toggle_record_target(index, target);
        Ok(())
    }

    pub(crate) fn toggle_observable_effect(&mut self, observable: u32, target: DemTarget) {
        let effects = self
            .observable_effects
            .entry(u64::from(observable))
            .or_default();
        toggle_target(effects, target);
    }

    pub(crate) fn pauli_targets_at(&self, qubit: QubitId) -> CircuitResult<BTreeSet<DemTarget>> {
        Ok(self
            .xs_for(qubit)?
            .union(self.zs_for(qubit)?)
            .copied()
            .collect())
    }

    pub(crate) fn record_targets_at(&self, index: usize) -> CircuitResult<BTreeSet<DemTarget>> {
        if index >= self.measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record index {index} is outside the sparse reverse tracker history"
            )));
        }
        Ok(self.rec_bits.get(&index).cloned().unwrap_or_default())
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
    ) -> CircuitResult<()> {
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
                    let clifford =
                        SingleQubitClifford::from_gate(instruction.gate()).map_err(|_| {
                            CircuitError::invalid_detector_error_model(format!(
                                "sparse reverse frame tracker does not support tableau gate {}",
                                instruction.gate().canonical_name()
                            ))
                        })?;
                    self.undo_single_qubit_clifford(instruction, clifford)
                }
            }
            ReverseFlowTransition::SweepControlledPauliNoop | ReverseFlowTransition::Ignored => {
                Ok(())
            }
            ReverseFlowTransition::Unsupported => {
                Err(CircuitError::invalid_detector_error_model(format!(
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
    ) -> CircuitResult<BTreeSet<DemTarget>> {
        self.anticommuting_sensitivity(qubit, TrackerBasis::from_pauli(feedback))
    }

    pub(crate) fn absolute_record_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        self.record_index_from_offset(offset)
    }

    pub(crate) fn region_for_target(&self, target: DemTarget) -> CircuitResult<FlexPauliString> {
        self.region_for_target_with_len(target, self.qubit_count)
    }

    pub(crate) fn compact_region_for_target(
        &self,
        target: DemTarget,
    ) -> CircuitResult<FlexPauliString> {
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
    ) -> CircuitResult<FlexPauliString> {
        let mut bases = vec![PauliBasis::I; len];
        for (qubit, xs) in &self.xs {
            if xs.contains(&target) {
                let index = qubit_index(*qubit)?;
                let basis = bases.get_mut(index).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
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
                    CircuitError::invalid_detector_error_model(format!(
                        "Z-basis sensitivity qubit {} is outside its sparse region",
                        qubit.get()
                    ))
                })?;
                *basis = match *basis {
                    PauliBasis::I => PauliBasis::Z,
                    PauliBasis::X => PauliBasis::Y,
                    actual => {
                        return Err(CircuitError::invalid_detector_error_model(format!(
                            "unexpected {actual:?} basis while building sparse region for qubit {}",
                            qubit.get()
                        )));
                    }
                };
            }
        }
        FlexPauliString::from_phase_and_bases(PauliPhase::Plus, bases).map_err(|error| {
            CircuitError::invalid_detector_error_model(format!(
                "failed to build detecting region for {target}: {error}"
            ))
        })
    }

    pub(crate) fn undo_implicit_rz_at_start_of_circuit(&mut self) -> CircuitResult<()> {
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
    ) -> CircuitResult<()> {
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
    ) -> CircuitResult<()> {
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
    ) -> CircuitResult<()> {
        let terms = pair_measurement_terms_reversed(instruction, basis)?;
        self.ensure_measurements_available(terms.len(), instruction.gate().canonical_name())?;
        for term in &terms {
            self.check_product_measurement_gauge(term)?;
        }
        for term in terms {
            let sensitivity = self.pop_record_sensitivity()?;
            self.toggle_product_sensitivity(&term, &sensitivity)?;
        }
        Ok(())
    }

    fn undo_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let terms = pauli_product_measurement_terms_reversed(instruction)?;
        self.ensure_measurements_available(terms.len(), instruction.gate().canonical_name())?;
        for term in &terms {
            self.check_product_measurement_gauge(term)?;
        }
        for term in terms {
            let sensitivity = self.pop_record_sensitivity()?;
            self.toggle_product_sensitivity(&term, &sensitivity)?;
        }
        Ok(())
    }

    fn undo_spp(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for terms in pauli_product_terms_reversed(instruction)? {
            let mut sensitivity = BTreeSet::new();
            for (qubit, basis) in &terms {
                toggle_targets(
                    &mut sensitivity,
                    self.anticommuting_sensitivity(*qubit, *basis)?
                        .iter()
                        .copied(),
                );
            }
            // SPP and SPP_DAG differ only by phase signs on anticommuting Paulis; this tracker
            // stores unsigned detector and observable regions, so both gates share propagation.
            self.toggle_product_sensitivity(&terms, &sensitivity)?;
        }
        Ok(())
    }

    fn undo_measurement_target(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> CircuitResult<()> {
        let sensitivity = self.pop_record_sensitivity()?;
        self.toggle_product_sensitivity(&[(qubit, basis)], &sensitivity)
    }

    fn undo_measurement_pads(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.ensure_measurements_available(instruction.targets().len(), "MPAD")?;
        for _ in instruction.targets().iter().rev() {
            self.pop_record_sensitivity()?;
        }
        Ok(())
    }

    fn undo_heralded_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        self.ensure_measurements_available(
            instruction.targets().len(),
            instruction.gate().canonical_name(),
        )?;
        for _ in instruction.targets().iter().rev() {
            self.pop_record_sensitivity()?;
        }
        Ok(())
    }

    fn pop_record_sensitivity(&mut self) -> CircuitResult<BTreeSet<DemTarget>> {
        self.measurement_count = self.measurement_count.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
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
    ) -> CircuitResult<()> {
        let qubits = qubits_reversed(instruction)?;
        for qubit in &qubits {
            self.check_reset_gauge(*qubit, basis)?;
        }
        for qubit in qubits {
            self.clear_qubit(qubit)?;
        }
        Ok(())
    }

    fn undo_single_qubit_clifford(
        &mut self,
        instruction: &CircuitInstruction,
        clifford: SingleQubitClifford,
    ) -> CircuitResult<()> {
        let inverse = clifford.inverse().map_err(|error| {
            CircuitError::invalid_detector_error_model(format!(
                "failed to invert single-qubit Clifford {} during sparse reverse tracking: {error}",
                instruction.gate().canonical_name()
            ))
        })?;
        for qubit in qubits_reversed(instruction)? {
            let old_xs = self.xs_for(qubit)?.clone();
            let old_zs = self.zs_for(qubit)?.clone();
            let mut new_xs = BTreeSet::new();
            let mut new_zs = BTreeSet::new();
            for target in old_xs.union(&old_zs) {
                let old_basis = match (old_xs.contains(target), old_zs.contains(target)) {
                    (true, true) => PauliBasis::Y,
                    (true, false) => PauliBasis::X,
                    (false, true) => PauliBasis::Z,
                    (false, false) => PauliBasis::I,
                };
                let new_basis = inverse.apply_basis(old_basis).map_err(|error| {
                    CircuitError::invalid_detector_error_model(format!(
                        "failed to apply inverse single-qubit Clifford {} during sparse reverse tracking: {error}",
                        instruction.gate().canonical_name()
                    ))
                })?;
                if new_basis.x_bit() {
                    new_xs.insert(*target);
                }
                if new_basis.z_bit() {
                    new_zs.insert(*target);
                }
            }
            replace_qubit_set(&mut self.xs, qubit, new_xs);
            replace_qubit_set(&mut self.zs, qubit, new_zs);
        }
        Ok(())
    }

    fn undo_controlled_pauli(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let gate_name = instruction.gate().canonical_name();
            let [control, target] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during sparse reverse tracking",
                )));
            };
            if control.is_measurement_record_target() && target.qubit_id().is_some() {
                validate_feedback_record_position(gate_name, true)?;
                self.undo_classical_feedback(instruction, control, target)?;
            } else if target.is_measurement_record_target() && control.qubit_id().is_some() {
                validate_feedback_record_position(gate_name, false)?;
                self.undo_classical_feedback(instruction, target, control)?;
            } else if let (Some(left), Some(right)) = (control.qubit_id(), target.qubit_id()) {
                if matches!(gate_name, "CX" | "CY" | "CZ") {
                    self.undo_quantum_controlled_pauli(instruction, control, target)?;
                } else {
                    let inverse_tableau = two_qubit_inverse_tableau(instruction, gate_name)?;
                    self.undo_two_qubit_tableau_group(gate_name, &inverse_tableau, left, right)?;
                }
            }
        }
        Ok(())
    }

    fn undo_classical_feedback(
        &mut self,
        instruction: &CircuitInstruction,
        control: &Target,
        target: &Target,
    ) -> CircuitResult<()> {
        let record = control.measurement_record_offset().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} classical control {control} is not a measurement record",
                instruction.gate().canonical_name()
            ))
        })?;
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} feedback target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        let sensitivity = match instruction.gate().canonical_name() {
            "CX" | "XCZ" => self.zs_for(qubit)?.clone(),
            "CY" | "YCZ" => xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?),
            "CZ" => self.xs_for(qubit)?.clone(),
            name => {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{name} is not a supported sparse reverse feedback gate"
                )));
            }
        };
        let index = self.record_index_from_offset(record.get())?;
        self.toggle_record_sensitivity(index, &sensitivity);
        Ok(())
    }

    fn undo_quantum_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
        control: &Target,
        target: &Target,
    ) -> CircuitResult<()> {
        let control = control.qubit_id().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} control target {control} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        let target = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
        match instruction.gate().canonical_name() {
            "CX" => {
                let target_zs = self.zs_for(target)?.clone();
                self.toggle_zs(control, &target_zs)?;
                let control_xs = self.xs_for(control)?.clone();
                self.toggle_xs(target, &control_xs)?;
                Ok(())
            }
            "CZ" => {
                let target_xs = self.xs_for(target)?.clone();
                self.toggle_zs(control, &target_xs)?;
                let control_xs = self.xs_for(control)?.clone();
                self.toggle_zs(target, &control_xs)?;
                Ok(())
            }
            "CY" => {
                let target_anti_y = xor_sets(self.xs_for(target)?, self.zs_for(target)?);
                self.toggle_zs(control, &target_anti_y)?;
                let control_xs = self.xs_for(control)?.clone();
                self.toggle_xs(target, &control_xs)?;
                self.toggle_zs(target, &control_xs)?;
                Ok(())
            }
            name => Err(CircuitError::invalid_detector_error_model(format!(
                "{name} sparse reverse tracking is not implemented for qubit-qubit controls"
            ))),
        }
    }

    fn undo_two_qubit_tableau(
        &mut self,
        instruction: &CircuitInstruction,
        gate_name: &'static str,
    ) -> CircuitResult<()> {
        let inverse_tableau = two_qubit_inverse_tableau(instruction, gate_name)?;
        for group in instruction.target_groups().into_iter().rev() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during sparse reverse tracking"
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {left} is not a qubit"
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {right} is not a qubit"
                ))
            })?;
            self.undo_two_qubit_tableau_group(gate_name, &inverse_tableau, left, right)?;
        }
        Ok(())
    }

    fn undo_two_qubit_tableau_group(
        &mut self,
        gate_name: &'static str,
        inverse_tableau: &Tableau,
        left: QubitId,
        right: QubitId,
    ) -> CircuitResult<()> {
        self.validate_qubit(left)?;
        self.validate_qubit(right)?;
        let old_left_xs = self.xs_for(left)?.clone();
        let old_left_zs = self.zs_for(left)?.clone();
        let old_right_xs = self.xs_for(right)?.clone();
        let old_right_zs = self.zs_for(right)?.clone();
        let mut tracked_targets = BTreeSet::new();
        tracked_targets.extend(old_left_xs.iter().copied());
        tracked_targets.extend(old_left_zs.iter().copied());
        tracked_targets.extend(old_right_xs.iter().copied());
        tracked_targets.extend(old_right_zs.iter().copied());

        let mut new_left_xs = BTreeSet::new();
        let mut new_left_zs = BTreeSet::new();
        let mut new_right_xs = BTreeSet::new();
        let mut new_right_zs = BTreeSet::new();
        for target in tracked_targets {
            let input = PauliString::from_bases(
                PauliSign::Plus,
                [
                    basis_from_sets(&old_left_xs, &old_left_zs, target),
                    basis_from_sets(&old_right_xs, &old_right_zs, target),
                ],
            );
            let output = inverse_tableau.apply(&input).map_err(|error| {
                CircuitError::invalid_detector_error_model(format!(
                    "failed to apply inverse tableau for {gate_name} during sparse reverse tracking: {error}"
                ))
            })?;
            let left_basis = output.get(0).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "inverse tableau for {gate_name} did not return left output basis"
                ))
            })?;
            let right_basis = output.get(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "inverse tableau for {gate_name} did not return right output basis"
                ))
            })?;
            insert_basis_target(&mut new_left_xs, &mut new_left_zs, target, left_basis);
            insert_basis_target(&mut new_right_xs, &mut new_right_zs, target, right_basis);
        }
        replace_qubit_set(&mut self.xs, left, new_left_xs);
        replace_qubit_set(&mut self.zs, left, new_left_zs);
        replace_qubit_set(&mut self.xs, right, new_right_xs);
        replace_qubit_set(&mut self.zs, right, new_right_zs);
        Ok(())
    }

    fn undo_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.detector_count = self.detector_count.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "detector count underflowed during sparse reverse tracking",
            )
        })?;
        let detector = DemTarget::relative_detector(self.detector_count)?;
        for target in instruction.targets() {
            let offset = target.measurement_record_offset().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                ))
            })?;
            let index = self.record_index_from_offset(offset.get())?;
            self.toggle_record_target(index, detector);
        }
        Ok(())
    }

    fn undo_observable_include(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "OBSERVABLE_INCLUDE is missing an observable id",
            )
        })?;
        let sensitivity = self
            .observable_effects
            .get(&observable.get())
            .cloned()
            .unwrap_or(BTreeSet::from([DemTarget::logical_observable(
                observable.get(),
            )?]));
        for target in instruction.targets() {
            match target {
                Target::MeasurementRecord { offset } => {
                    let index = self.record_index_from_offset(offset.get())?;
                    self.toggle_record_sensitivity(index, &sensitivity);
                }
                Target::Pauli { pauli, id, .. } => {
                    self.toggle_product_sensitivity(
                        &[(*id, TrackerBasis::from_pauli(*pauli))],
                        &sensitivity,
                    )?;
                }
                Target::Qubit { .. } | Target::SweepBit { .. } | Target::Combiner => {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "OBSERVABLE_INCLUDE target {target} is not a measurement record or Pauli target"
                    )));
                }
            }
        }
        Ok(())
    }

    fn check_measurement_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> CircuitResult<()> {
        self.check_gauge(qubit, basis, self.anticommuting_sensitivity(qubit, basis)?)
    }

    fn check_product_measurement_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
    ) -> CircuitResult<()> {
        let mut gauge = BTreeSet::new();
        for (qubit, basis) in terms {
            toggle_targets(
                &mut gauge,
                self.anticommuting_sensitivity(*qubit, *basis)?
                    .iter()
                    .copied(),
            );
        }
        self.check_product_gauge(terms, gauge)
    }

    fn check_reset_gauge(&mut self, qubit: QubitId, basis: TrackerBasis) -> CircuitResult<()> {
        self.check_gauge(qubit, basis, self.anticommuting_sensitivity(qubit, basis)?)
    }

    fn check_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
        gauge: BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
        self.check_product_gauge(&[(qubit, basis)], gauge)
    }

    fn check_product_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
        gauge: BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
        if gauge.is_empty() {
            return Ok(());
        }
        if self.fail_on_anticommute {
            let mut message = String::from("collapse anti-commuted with tracked targets:");
            for target in &gauge {
                message.push_str("\n    ");
                message.push_str(&target.to_string());
            }
            return Err(CircuitError::invalid_detector_error_model(message));
        }
        for (qubit, basis) in terms {
            for target in &gauge {
                self.anticommutations.insert(Anticommutation {
                    target: *target,
                    location: TrackerLocation {
                        qubit: *qubit,
                        basis: *basis,
                    },
                });
            }
        }
        Ok(())
    }

    fn anticommuting_sensitivity(
        &self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> CircuitResult<BTreeSet<DemTarget>> {
        match basis {
            TrackerBasis::X => Ok(self.zs_for(qubit)?.clone()),
            TrackerBasis::Y => Ok(xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?)),
            TrackerBasis::Z => Ok(self.xs_for(qubit)?.clone()),
        }
    }

    fn toggle_product_sensitivity(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
        sensitivity: &BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
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

    fn toggle_record_sensitivity(&mut self, index: usize, sensitivity: &BTreeSet<DemTarget>) {
        for target in sensitivity {
            self.toggle_record_target(index, *target);
        }
    }

    fn toggle_record_target(&mut self, index: usize, target: DemTarget) {
        let targets = self.rec_bits.entry(index).or_default();
        toggle_target(targets, target);
        if targets.is_empty() {
            self.rec_bits.remove(&index);
        }
    }

    fn clear_qubit(&mut self, qubit: QubitId) -> CircuitResult<()> {
        self.validate_qubit(qubit)?;
        self.xs.remove(&qubit);
        self.zs.remove(&qubit);
        Ok(())
    }

    fn xs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.validate_qubit(qubit)?;
        Ok(self.xs.get(&qubit).unwrap_or(&EMPTY_TARGETS))
    }

    fn zs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.validate_qubit(qubit)?;
        Ok(self.zs.get(&qubit).unwrap_or(&EMPTY_TARGETS))
    }

    fn toggle_xs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        self.validate_qubit(qubit)?;
        let is_empty = {
            let xs = self.xs.entry(qubit).or_default();
            toggle_targets(xs, targets.iter().copied());
            xs.is_empty()
        };
        if is_empty {
            self.xs.remove(&qubit);
        }
        Ok(())
    }

    fn toggle_zs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        self.validate_qubit(qubit)?;
        let is_empty = {
            let zs = self.zs.entry(qubit).or_default();
            toggle_targets(zs, targets.iter().copied());
            zs.is_empty()
        };
        if is_empty {
            self.zs.remove(&qubit);
        }
        Ok(())
    }

    fn ensure_measurements_available(&self, count: usize, gate: &'static str) -> CircuitResult<()> {
        if self.measurement_count < count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{gate} needs {count} prior measurement(s), but sparse reverse tracker only has {}",
                self.measurement_count
            )));
        }
        Ok(())
    }

    fn validate_qubit(&self, qubit: QubitId) -> CircuitResult<()> {
        let index = qubit_index(qubit)?;
        if index >= self.qubit_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        }
        Ok(())
    }

    fn record_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let measurement_count = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement count does not fit i64 during sparse reverse tracking",
            )
        })?;
        let index = measurement_count
            .checked_add(i64::from(offset))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "measurement record offset overflowed during sparse reverse tracking",
                )
            })?;
        if index < 0 || index >= measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record offset rec[{offset}] is outside the sparse reverse tracker history"
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement record index does not fit usize during sparse reverse tracking",
            )
        })
    }
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

    fn from_pauli(pauli: crate::Pauli) -> Self {
        match pauli {
            crate::Pauli::X => Self::X,
            crate::Pauli::Y => Self::Y,
            crate::Pauli::Z => Self::Z,
        }
    }
}

fn qubits_reversed(instruction: &CircuitInstruction) -> CircuitResult<Vec<QubitId>> {
    instruction
        .targets()
        .iter()
        .rev()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
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
) -> CircuitResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction
        .target_groups()
        .into_iter()
        .rev()
        .map(|group| {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired targets during sparse reverse tracking",
                    instruction.gate().canonical_name()
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {left} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {right} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            Ok(vec![(left, basis), (right, basis)])
        })
        .collect()
}

fn qubit_index(qubit: QubitId) -> CircuitResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "qubit id {} does not fit usize during sparse reverse tracking",
            qubit.get()
        ))
    })
}

fn two_qubit_inverse_tableau(
    instruction: &CircuitInstruction,
    gate_name: &'static str,
) -> CircuitResult<Tableau> {
    let inverse = instruction.gate().inverse().ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "{gate_name} does not have a unitary inverse for sparse reverse tracking"
        ))
    })?;
    let inverse_tableau = inverse.tableau().map_err(|error| {
        CircuitError::invalid_detector_error_model(format!(
            "failed to load inverse tableau for {gate_name} during sparse reverse tracking: {error}"
        ))
    })?;
    if inverse_tableau.len() != 2 {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{gate_name} expected a two-qubit tableau during sparse reverse tracking"
        )));
    }
    Ok(inverse_tableau)
}

fn validate_feedback_record_position(gate_name: &str, record_is_first: bool) -> CircuitResult<()> {
    let valid = match gate_name {
        "CX" | "CY" => record_is_first,
        "XCZ" | "YCZ" => !record_is_first,
        "CZ" => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CircuitError::invalid_detector_error_model(format!(
            "{gate_name} does not support a measurement-record feedback target in this position"
        )))
    }
}

fn xor_sets(left: &BTreeSet<DemTarget>, right: &BTreeSet<DemTarget>) -> BTreeSet<DemTarget> {
    let mut result = left.clone();
    toggle_targets(&mut result, right.iter().copied());
    result
}

fn basis_from_sets(
    xs: &BTreeSet<DemTarget>,
    zs: &BTreeSet<DemTarget>,
    target: DemTarget,
) -> PauliBasis {
    PauliBasis::from_xz(xs.contains(&target), zs.contains(&target))
}

fn insert_basis_target(
    xs: &mut BTreeSet<DemTarget>,
    zs: &mut BTreeSet<DemTarget>,
    target: DemTarget,
    basis: PauliBasis,
) {
    if basis.x_bit() {
        xs.insert(target);
    }
    if basis.z_bit() {
        zs.insert(target);
    }
}

fn replace_qubit_set(
    sets: &mut BTreeMap<QubitId, BTreeSet<DemTarget>>,
    qubit: QubitId,
    value: BTreeSet<DemTarget>,
) {
    if value.is_empty() {
        sets.remove(&qubit);
    } else {
        sets.insert(qubit, value);
    }
}

fn toggle_targets(target: &mut BTreeSet<DemTarget>, values: impl Iterator<Item = DemTarget>) {
    for value in values {
        toggle_target(target, value);
    }
}

fn toggle_target(target: &mut BTreeSet<DemTarget>, value: DemTarget) {
    if !target.insert(value) {
        target.remove(&value);
    }
}

#[cfg(test)]
mod tests;
