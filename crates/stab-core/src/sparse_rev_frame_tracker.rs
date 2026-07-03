#![allow(
    dead_code,
    reason = "M10 lands sparse reverse tracker parity before the error matcher consumes this internal primitive"
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget,
    FlexPauliString, Pauli, PauliBasis, PauliPhase, QubitId, Target,
};

mod pauli_product;

use pauli_product::pauli_product_measurement_terms_reversed;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SparseReverseFrameTracker {
    xs: Vec<BTreeSet<DemTarget>>,
    zs: Vec<BTreeSet<DemTarget>>,
    rec_bits: BTreeMap<usize, BTreeSet<DemTarget>>,
    measurement_count: usize,
    detector_count: u64,
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
            xs: vec![BTreeSet::new(); qubit_count],
            zs: vec![BTreeSet::new(); qubit_count],
            rec_bits: BTreeMap::new(),
            measurement_count,
            detector_count,
            fail_on_anticommute,
            anticommutations: BTreeSet::new(),
        }
    }

    pub(crate) fn undo_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => self.undo_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    for _ in 0..repeat.repeat_count().get() {
                        self.undo_circuit(repeat.body())?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn undo_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "M" => self.undo_measurements(instruction, TrackerBasis::Z),
            "MX" => self.undo_measurements(instruction, TrackerBasis::X),
            "MY" => self.undo_measurements(instruction, TrackerBasis::Y),
            "MXX" => self.undo_pair_measurements(instruction, TrackerBasis::X),
            "MYY" => self.undo_pair_measurements(instruction, TrackerBasis::Y),
            "MZZ" => self.undo_pair_measurements(instruction, TrackerBasis::Z),
            "MPP" => self.undo_pauli_product_measurements(instruction),
            "MR" => self.undo_measure_resets(instruction, TrackerBasis::Z),
            "MRX" => self.undo_measure_resets(instruction, TrackerBasis::X),
            "MRY" => self.undo_measure_resets(instruction, TrackerBasis::Y),
            "MPAD" => self.undo_measurement_pads(instruction),
            "R" => self.undo_resets(instruction, TrackerBasis::Z),
            "RX" => self.undo_resets(instruction, TrackerBasis::X),
            "RY" => self.undo_resets(instruction, TrackerBasis::Y),
            "H" => self.undo_h(instruction),
            "H_XY" | "S" | "S_DAG" => self.undo_s_like(instruction),
            "C_XYZ" => self.undo_c_xyz(instruction),
            "CX" | "CY" | "CZ" => self.undo_controlled_pauli(instruction),
            "DETECTOR" => self.undo_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.undo_observable_include(instruction),
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
                self.undo_heralded_measurements(instruction)
            }
            _ => Ok(()),
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
        let bases = self.xs.iter().zip(&self.zs).map(|(xs, zs)| {
            match (xs.contains(&target), zs.contains(&target)) {
                (false, false) => PauliBasis::I,
                (true, false) => PauliBasis::X,
                (false, true) => PauliBasis::Z,
                (true, true) => PauliBasis::Y,
            }
        });
        FlexPauliString::from_phase_and_bases(PauliPhase::Plus, bases).map_err(|error| {
            CircuitError::invalid_detector_error_model(format!(
                "failed to build detecting region for {target}: {error}"
            ))
        })
    }

    pub(crate) fn undo_implicit_rz_at_start_of_circuit(&mut self) -> CircuitResult<()> {
        for index in 0..self.xs.len() {
            let qubit = QubitId::new(u32::try_from(index).map_err(|_| {
                CircuitError::invalid_detector_error_model(format!(
                    "qubit index {index} does not fit u32 during implicit start-state check"
                ))
            })?)?;
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

    fn undo_h(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for qubit in qubits_reversed(instruction)? {
            let index = self.checked_qubit_index(qubit)?;
            let xs = self.xs.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H target qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                ))
            })?;
            let zs = self.zs.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H target qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                ))
            })?;
            std::mem::swap(xs, zs);
        }
        Ok(())
    }

    fn undo_s_like(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for qubit in qubits_reversed(instruction)? {
            let xs = self.xs_for(qubit)?.clone();
            self.toggle_zs(qubit, &xs)?;
        }
        Ok(())
    }

    fn undo_c_xyz(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for qubit in qubits_reversed(instruction)? {
            let index = self.checked_qubit_index(qubit)?;
            let old_xs = self.xs_for(qubit)?.clone();
            let old_zs = self.zs_for(qubit)?.clone();
            let new_xs = old_zs.clone();
            let new_zs = xor_sets(&old_xs, &old_zs);
            let Some(xs) = self.xs.get_mut(index) else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "C_XYZ target qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                )));
            };
            *xs = new_xs;
            let Some(zs) = self.zs.get_mut(index) else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "C_XYZ target qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                )));
            };
            *zs = new_zs;
        }
        Ok(())
    }

    fn undo_controlled_pauli(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let [control, target] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired targets during sparse reverse tracking",
                    instruction.gate().canonical_name()
                )));
            };
            if control.is_measurement_record_target() {
                self.undo_classical_feedback(instruction, control, target)?;
            } else if target.is_measurement_record_target() {
                self.undo_classical_feedback(instruction, target, control)?;
            } else if control.is_sweep_bit_target() || target.is_sweep_bit_target() {
                // Sweep-controlled Paulis are preserved by the feedback-inlining
                // transform. The sparse tracker currently has no symbolic sweep
                // branch, so they do not affect fixed detector sensitivities here.
            } else {
                self.undo_quantum_controlled_pauli(instruction, control, target)?;
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
            "CX" => self.zs_for(qubit)?.clone(),
            "CY" => xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?),
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
            "CY" if self.xs_for(control)?.is_empty()
                && self.zs_for(control)?.is_empty()
                && self.xs_for(target)?.is_empty()
                && self.zs_for(target)?.is_empty() =>
            {
                Ok(())
            }
            name => Err(CircuitError::invalid_detector_error_model(format!(
                "{name} sparse reverse tracking is not implemented for qubit-qubit controls"
            ))),
        }
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
        let target = DemTarget::logical_observable(observable.get())?;
        let sensitivity = BTreeSet::from([target]);
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
        let index = self.checked_qubit_index(qubit)?;
        let Some(xs) = self.xs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "reset target qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        };
        xs.clear();
        let Some(zs) = self.zs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "reset target qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        };
        zs.clear();
        Ok(())
    }

    fn xs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.xs
            .get(self.checked_qubit_index(qubit)?)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                ))
            })
    }

    fn zs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.zs
            .get(self.checked_qubit_index(qubit)?)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "qubit {} is outside the sparse reverse tracker",
                    qubit.get()
                ))
            })
    }

    fn toggle_xs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        let index = self.checked_qubit_index(qubit)?;
        let Some(xs) = self.xs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        };
        toggle_targets(xs, targets.iter().copied());
        Ok(())
    }

    fn toggle_zs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        let index = self.checked_qubit_index(qubit)?;
        let Some(zs) = self.zs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        };
        toggle_targets(zs, targets.iter().copied());
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

    fn checked_qubit_index(&self, qubit: QubitId) -> CircuitResult<usize> {
        let index = qubit_index(qubit)?;
        if index >= self.xs.len() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the sparse reverse tracker",
                qubit.get()
            )));
        }
        Ok(index)
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

fn xor_sets(left: &BTreeSet<DemTarget>, right: &BTreeSet<DemTarget>) -> BTreeSet<DemTarget> {
    let mut result = left.clone();
    toggle_targets(&mut result, right.iter().copied());
    result
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
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used,
        reason = "unit tests use direct fixed-width tracker assertions for compact diagnostics"
    )]

    use crate::{Gate, MeasureRecordOffset, measurement_record_count};

    use super::*;

    fn tracker_from_pauli_text(text: &str) -> SparseReverseFrameTracker {
        let mut tracker = SparseReverseFrameTracker::new(text.len(), 0, 0, true);
        let sensitivity = BTreeSet::from([DemTarget::logical_observable(0).unwrap()]);
        for (index, character) in text.chars().enumerate() {
            let qubit = QubitId::new(u32::try_from(index).unwrap()).unwrap();
            match character {
                'I' => {}
                'X' => tracker.toggle_xs(qubit, &sensitivity).unwrap(),
                'Y' => {
                    tracker.toggle_xs(qubit, &sensitivity).unwrap();
                    tracker.toggle_zs(qubit, &sensitivity).unwrap();
                }
                'Z' => tracker.toggle_zs(qubit, &sensitivity).unwrap(),
                _ => panic!("unexpected Pauli text character {character}"),
            }
        }
        tracker
    }

    fn circuit(text: &str) -> Circuit {
        Circuit::from_stim_str(text).unwrap()
    }

    fn instruction(text: &str) -> CircuitInstruction {
        let parsed = circuit(text);
        let Some(CircuitItem::Instruction(instruction)) = parsed.items().first() else {
            panic!("expected one instruction in {text}");
        };
        instruction.clone()
    }

    fn q(id: u32) -> Target {
        Target::qubit(QubitId::new(id).unwrap(), false)
    }

    fn rec(offset: i32) -> Target {
        Target::measurement_record(MeasureRecordOffset::try_new(offset).unwrap())
    }

    fn single_pauli_set(id: u64) -> BTreeSet<DemTarget> {
        BTreeSet::from([DemTarget::logical_observable(id).unwrap()])
    }

    #[test]
    fn sparse_rev_frame_tracker_undo_tableau_h_subset() {
        for (input, expected) in [("I", "I"), ("X", "Z"), ("Y", "Y"), ("Z", "X")] {
            let mut actual = tracker_from_pauli_text(input);
            actual.undo_instruction(&instruction("H 0\n")).unwrap();
            assert_eq!(actual, tracker_from_pauli_text(expected));
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_undo_tableau_s_subset() {
        for (input, expected) in [("I", "I"), ("X", "Y"), ("Y", "X"), ("Z", "Z")] {
            let mut actual = tracker_from_pauli_text(input);
            actual.undo_instruction(&instruction("S 0\n")).unwrap();
            assert_eq!(actual, tracker_from_pauli_text(expected));
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_undo_tableau_c_xyz_subset() {
        for (input, expected) in [("I", "I"), ("X", "Z"), ("Y", "X"), ("Z", "Y")] {
            let mut actual = tracker_from_pauli_text(input);
            actual.undo_instruction(&instruction("C_XYZ 0\n")).unwrap();
            assert_eq!(actual, tracker_from_pauli_text(expected));
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_undo_tableau_cx_subset() {
        for (input, expected) in [
            ("II", "II"),
            ("IZ", "ZZ"),
            ("ZI", "ZI"),
            ("XI", "XX"),
            ("IX", "IX"),
            ("YY", "XZ"),
        ] {
            let mut actual = tracker_from_pauli_text(input);
            actual.undo_instruction(&instruction("CX 0 1\n")).unwrap();
            assert_eq!(actual, tracker_from_pauli_text(expected));
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_measurements_preserve_matching_basis() {
        for (gate, input) in [("MX", "XXX"), ("MY", "YYY"), ("M", "ZZZ")] {
            let mut actual = tracker_from_pauli_text(input);
            actual.measurement_count = 2;
            actual
                .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
                .unwrap();
            let mut expected = tracker_from_pauli_text(input);
            expected.measurement_count = 0;
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_measurements_reject_anticommuting_basis_without_mutation() {
        for (gate, input) in [("MX", "XIZ"), ("MY", "YIZ"), ("M", "YIZ")] {
            let mut actual = tracker_from_pauli_text(input);
            actual.measurement_count = 2;
            let before = actual.clone();
            let err = actual
                .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
                .unwrap_err();
            assert!(err.to_string().contains("anti-commuted"));
            assert_eq!(actual, before);
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_measure_resets_clear_then_move_feedback() {
        let mut actual = tracker_from_pauli_text("XXX");
        actual.measurement_count = 2;
        actual.undo_instruction(&instruction("MRX 0 2\n")).unwrap();
        let mut expected = tracker_from_pauli_text("IXI");
        expected.measurement_count = 0;
        assert_eq!(actual, expected);

        let mut actual = tracker_from_pauli_text("III");
        actual.measurement_count = 2;
        actual.rec_bits.insert(
            0,
            BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
        );
        actual.undo_instruction(&instruction("MRX 0 2\n")).unwrap();
        let mut expected = tracker_from_pauli_text("XII");
        expected.measurement_count = 0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_feedback_from_measurement_subset() {
        for (gate, expected_text) in [("MX", "XII"), ("MY", "YII"), ("M", "ZII")] {
            let mut actual = tracker_from_pauli_text("III");
            actual.measurement_count = 2;
            actual.rec_bits.insert(
                0,
                BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
            );
            actual
                .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
                .unwrap();
            let mut expected = tracker_from_pauli_text(expected_text);
            expected.measurement_count = 0;
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_feedback_into_measurement_subset() {
        let target = Gate::from_name("CX").unwrap();
        let cx = CircuitInstruction::new(target, Vec::new(), vec![rec(-5), q(0)], None).unwrap();
        let mut actual = tracker_from_pauli_text("ZII");
        actual.measurement_count = 12;
        actual.undo_instruction(&cx).unwrap();

        let mut expected = tracker_from_pauli_text("ZII");
        expected.measurement_count = 12;
        expected.rec_bits.insert(
            7,
            BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_pair_measurements_subset() {
        for (gate, expected_text) in [("MXX", "XXI"), ("MYY", "YYI"), ("MZZ", "ZZI")] {
            let mut actual = tracker_from_pauli_text("III");
            actual.measurement_count = 2;
            actual.rec_bits.insert(
                1,
                BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
            );
            actual
                .undo_instruction(&instruction(&format!("{gate} 0 1\n")))
                .unwrap();

            let mut expected = tracker_from_pauli_text(expected_text);
            expected.measurement_count = 1;
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn sparse_rev_frame_tracker_mpp_measurements_subset() {
        let mut actual = SparseReverseFrameTracker::new(6, 2, 0, true);
        actual.rec_bits.insert(0, single_pauli_set(0));
        actual.rec_bits.insert(1, single_pauli_set(1));
        actual
            .undo_instruction(&instruction("MPP X0*Y1*Z2 Z5\n"))
            .unwrap();

        let mut expected = tracker_from_pauli_text("XYZIIZ");
        expected.xs[0] = single_pauli_set(0);
        expected.xs[1] = single_pauli_set(0);
        expected.zs[1] = single_pauli_set(0);
        expected.zs[2] = single_pauli_set(0);
        expected.zs[5] = single_pauli_set(1);
        expected.measurement_count = 0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_rejects_anti_hermitian_mpp_products() {
        let mut actual = SparseReverseFrameTracker::new(1, 1, 0, true);
        let error = actual
            .undo_instruction(&instruction("MPP X0*Z0\n"))
            .unwrap_err();

        assert!(error.to_string().contains("anti-Hermitian"));
    }

    #[test]
    fn sparse_rev_frame_tracker_undo_circuit_feedback_subset() {
        let circuit = circuit(
            "
            MR 0
            CX rec[-1] 0
            M 0
            DETECTOR rec[-1]
            ",
        );
        let mut actual = SparseReverseFrameTracker::new(
            circuit.count_qubits(),
            measurement_record_count(&circuit).unwrap(),
            1,
            true,
        );
        actual.undo_circuit(&circuit).unwrap();

        let mut expected = SparseReverseFrameTracker::new(1, 0, 0, true);
        expected.zs[0].insert(DemTarget::relative_detector(0).unwrap());
        assert_eq!(actual, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_tracks_anticommutation_when_requested() {
        let circuit = circuit(
            "
            RX 0
            M 0
            DETECTOR rec[-1]
            ",
        );
        let mut tracker = SparseReverseFrameTracker::new(
            circuit.count_qubits(),
            measurement_record_count(&circuit).unwrap(),
            1,
            false,
        );
        tracker.undo_circuit(&circuit).unwrap();

        assert_eq!(
            tracker.anticommutations,
            BTreeSet::from([Anticommutation {
                target: DemTarget::relative_detector(0).unwrap(),
                location: TrackerLocation {
                    qubit: QubitId::new(0).unwrap(),
                    basis: TrackerBasis::X,
                },
            }])
        );
    }

    #[test]
    fn sparse_rev_frame_tracker_fails_anticommutation_by_default() {
        let circuit = circuit(
            "
            RX 0
            M 0
            DETECTOR rec[-1]
            ",
        );
        let mut tracker = SparseReverseFrameTracker::new(
            circuit.count_qubits(),
            measurement_record_count(&circuit).unwrap(),
            1,
            true,
        );
        assert!(tracker.undo_circuit(&circuit).is_err());
    }

    #[test]
    fn sparse_rev_frame_tracker_observable_include_paulis_subset() {
        let mut tracker = SparseReverseFrameTracker::new(4, 4, 4, true);
        tracker
            .undo_circuit(&circuit("OBSERVABLE_INCLUDE(5) X1 Y2 Z3 rec[-1]\n"))
            .unwrap();

        assert!(tracker.xs[0].is_empty());
        assert!(tracker.zs[0].is_empty());
        assert_eq!(tracker.xs[1], single_pauli_set(5));
        assert!(tracker.zs[1].is_empty());
        assert_eq!(tracker.xs[2], single_pauli_set(5));
        assert_eq!(tracker.zs[2], single_pauli_set(5));
        assert!(tracker.xs[3].is_empty());
        assert_eq!(tracker.zs[3], single_pauli_set(5));
        assert_eq!(tracker.rec_bits.get(&3), Some(&single_pauli_set(5)));
    }

    #[test]
    fn sparse_rev_frame_tracker_unrolls_repeat_blocks_for_now() {
        let circuit = circuit(
            "
            REPEAT 2 {
                M 0
                DETECTOR rec[-1]
            }
            ",
        );
        let mut tracker = SparseReverseFrameTracker::new(
            circuit.count_qubits(),
            measurement_record_count(&circuit).unwrap(),
            2,
            true,
        );
        tracker.undo_circuit(&circuit).unwrap();

        let mut expected = SparseReverseFrameTracker::new(1, 0, 0, true);
        expected.zs[0].insert(DemTarget::relative_detector(0).unwrap());
        expected.zs[0].insert(DemTarget::relative_detector(1).unwrap());
        assert_eq!(tracker, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_accepts_mpad_and_discards_record_sensitivity() {
        let mut actual = tracker_from_pauli_text("IIZ");
        actual.measurement_count = 2;
        actual.rec_bits.insert(
            1,
            BTreeSet::from([DemTarget::relative_detector(5).unwrap()]),
        );
        actual.undo_instruction(&instruction("MPAD 0\n")).unwrap();

        let mut expected = tracker_from_pauli_text("IIZ");
        expected.measurement_count = 1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn sparse_rev_frame_tracker_target_pauli_mapping_is_explicit() {
        assert_eq!(TrackerBasis::from_pauli(Pauli::X), TrackerBasis::X);
        assert_eq!(TrackerBasis::from_pauli(Pauli::Y), TrackerBasis::Y);
        assert_eq!(TrackerBasis::from_pauli(Pauli::Z), TrackerBasis::Z);
    }
}
