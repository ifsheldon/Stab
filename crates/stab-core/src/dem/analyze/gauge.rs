use std::collections::{BTreeMap, BTreeSet};

use crate::{Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, QubitId};

use super::effects::analyzer_paulis_anticommute;
use super::feedback::{ControlledPauliAction, controlled_pauli_action};
use super::mpp::pauli_product_terms;
use super::{AnalyzerBasis, AnalyzerPauli, DemTarget};

pub(super) fn find_gauge_errors(
    circuit: &Circuit,
    detector_terms_by_measurement: &BTreeMap<usize, Vec<u64>>,
    observable_terms_by_measurement: &BTreeMap<usize, Vec<u64>>,
    measurement_count: usize,
    qubit_count: usize,
    allow_gauge_detectors: bool,
) -> CircuitResult<Vec<Vec<DemTarget>>> {
    let mut tracker = GaugeTracker::new(
        detector_terms_by_measurement,
        observable_terms_by_measurement,
        measurement_count,
        qubit_count,
        allow_gauge_detectors,
    )?;
    tracker.undo_circuit(circuit)?;
    tracker.check_initial_resets()?;
    Ok(tracker.gauge_errors)
}

#[derive(Clone, Debug)]
struct GaugeTracker {
    xs: Vec<BTreeSet<DemTarget>>,
    zs: Vec<BTreeSet<DemTarget>>,
    rec_bits: BTreeMap<usize, BTreeSet<DemTarget>>,
    measurement_count: usize,
    allow_gauge_detectors: bool,
    gauge_errors: Vec<Vec<DemTarget>>,
}

impl GaugeTracker {
    fn new(
        detector_terms_by_measurement: &BTreeMap<usize, Vec<u64>>,
        observable_terms_by_measurement: &BTreeMap<usize, Vec<u64>>,
        measurement_count: usize,
        qubit_count: usize,
        allow_gauge_detectors: bool,
    ) -> CircuitResult<Self> {
        let mut rec_bits = BTreeMap::new();
        for (measurement, detectors) in detector_terms_by_measurement {
            for detector in detectors {
                toggle_target(
                    rec_bits.entry(*measurement).or_default(),
                    DemTarget::relative_detector(*detector)?,
                );
            }
        }
        for (measurement, observables) in observable_terms_by_measurement {
            for observable in observables {
                toggle_target(
                    rec_bits.entry(*measurement).or_default(),
                    DemTarget::logical_observable(*observable)?,
                );
            }
        }
        rec_bits.retain(|_, targets| !targets.is_empty());
        Ok(Self {
            xs: vec![BTreeSet::new(); qubit_count],
            zs: vec![BTreeSet::new(); qubit_count],
            rec_bits,
            measurement_count,
            allow_gauge_detectors,
            gauge_errors: Vec::new(),
        })
    }

    fn undo_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
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

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "M" => self.undo_measurements(instruction, AnalyzerBasis::Z),
            "MX" => self.undo_measurements(instruction, AnalyzerBasis::X),
            "MY" => self.undo_measurements(instruction, AnalyzerBasis::Y),
            "MXX" => self.undo_pair_measurements(instruction, AnalyzerBasis::X),
            "MYY" => self.undo_pair_measurements(instruction, AnalyzerBasis::Y),
            "MZZ" => self.undo_pair_measurements(instruction, AnalyzerBasis::Z),
            "MPP" => self.undo_pauli_product_measurements(instruction),
            "MR" => self.undo_measure_resets(instruction, AnalyzerBasis::Z),
            "MRX" => self.undo_measure_resets(instruction, AnalyzerBasis::X),
            "MRY" => self.undo_measure_resets(instruction, AnalyzerBasis::Y),
            "MPAD" => self.undo_measurement_pads(instruction),
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
                self.undo_heralded_measurements(instruction)
            }
            "R" => self.undo_resets(instruction, AnalyzerBasis::Z),
            "RX" => self.undo_resets(instruction, AnalyzerBasis::X),
            "RY" => self.undo_resets(instruction, AnalyzerBasis::Y),
            "H" => self.undo_h(instruction),
            "H_XY" => self.undo_h_xy(instruction),
            "CX" | "CY" | "CZ" | "XCX" | "XCY" | "XCZ" | "YCX" | "YCY" | "YCZ" => {
                self.undo_controlled_pauli(instruction)
            }
            "OBSERVABLE_INCLUDE" => self.undo_observable_include(instruction),
            _ => Ok(()),
        }
    }

    fn undo_measure_resets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            self.check_reset_gauge(qubit, basis)?;
            self.clear_qubit(qubit)?;
            self.undo_measurement_target(qubit, basis)?;
        }
        Ok(())
    }

    fn undo_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            self.undo_measurement_target(qubit, basis)?;
            self.check_measurement_gauge(qubit, basis)?;
        }
        Ok(())
    }

    fn undo_pair_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired qubit targets during gauge analysis",
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
            let sensitivity = self.pop_record_sensitivity()?;
            let terms = [(left, basis), (right, basis)];
            self.toggle_product_sensitivity(&terms, &sensitivity)?;
            self.check_product_measurement_gauge(&terms)?;
        }
        Ok(())
    }

    fn undo_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let terms = pauli_product_terms(instruction.gate().canonical_name(), group)?;
            let sensitivity = self.pop_record_sensitivity()?;
            self.toggle_product_sensitivity(&terms, &sensitivity)?;
            self.check_product_measurement_gauge(&terms)?;
        }
        Ok(())
    }

    fn undo_heralded_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        for _ in instruction.targets().iter().rev() {
            self.pop_record_sensitivity()?;
        }
        Ok(())
    }

    fn undo_measurement_target(
        &mut self,
        qubit: QubitId,
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        let sensitivity = self.pop_record_sensitivity()?;
        match basis {
            AnalyzerBasis::X => self.toggle_xs(qubit, &sensitivity)?,
            AnalyzerBasis::Y => {
                self.toggle_xs(qubit, &sensitivity)?;
                self.toggle_zs(qubit, &sensitivity)?;
            }
            AnalyzerBasis::Z => self.toggle_zs(qubit, &sensitivity)?,
        }
        Ok(())
    }

    fn undo_measurement_pads(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for _ in instruction.targets().iter().rev() {
            self.pop_record_sensitivity()?;
        }
        Ok(())
    }

    fn undo_observable_include(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            CircuitError::invalid_detector_error_model("OBSERVABLE_INCLUDE missing observable id")
        })?;
        let target = DemTarget::logical_observable(observable.get())?;
        let sensitivity = BTreeSet::from([target]);
        for target in instruction.targets().iter().rev() {
            let Some(pauli) = target.pauli_type() else {
                continue;
            };
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "OBSERVABLE_INCLUDE target {target} does not identify a qubit"
                ))
            })?;
            match AnalyzerBasis::from_pauli(pauli) {
                AnalyzerBasis::X => self.toggle_xs(qubit, &sensitivity)?,
                AnalyzerBasis::Y => {
                    self.toggle_xs(qubit, &sensitivity)?;
                    self.toggle_zs(qubit, &sensitivity)?;
                }
                AnalyzerBasis::Z => self.toggle_zs(qubit, &sensitivity)?,
            }
        }
        Ok(())
    }

    fn pop_record_sensitivity(&mut self) -> CircuitResult<BTreeSet<DemTarget>> {
        self.measurement_count = self.measurement_count.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "measurement count underflowed during gauge analysis",
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
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ))
            })?;
            self.check_reset_gauge(qubit, basis)?;
            self.clear_qubit(qubit)?;
        }
        Ok(())
    }

    fn check_measurement_gauge(
        &mut self,
        qubit: QubitId,
        basis: AnalyzerBasis,
    ) -> CircuitResult<()> {
        match basis {
            AnalyzerBasis::X => self.check_gauge(self.zs_for(qubit)?.clone()),
            AnalyzerBasis::Y => {
                self.check_gauge(xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?))
            }
            AnalyzerBasis::Z => self.check_gauge(self.xs_for(qubit)?.clone()),
        }
    }

    fn check_product_measurement_gauge(
        &mut self,
        terms: &[(QubitId, AnalyzerBasis)],
    ) -> CircuitResult<()> {
        let mut gauge = BTreeSet::new();
        for (qubit, basis) in terms {
            match basis {
                AnalyzerBasis::X => {
                    toggle_targets(&mut gauge, self.zs_for(*qubit)?.iter().copied())
                }
                AnalyzerBasis::Y => {
                    toggle_targets(&mut gauge, self.xs_for(*qubit)?.iter().copied());
                    toggle_targets(&mut gauge, self.zs_for(*qubit)?.iter().copied());
                }
                AnalyzerBasis::Z => {
                    toggle_targets(&mut gauge, self.xs_for(*qubit)?.iter().copied())
                }
            }
        }
        self.check_gauge(gauge)
    }

    fn check_reset_gauge(&mut self, qubit: QubitId, basis: AnalyzerBasis) -> CircuitResult<()> {
        match basis {
            AnalyzerBasis::X => self.check_gauge(self.zs_for(qubit)?.clone()),
            AnalyzerBasis::Y => {
                self.check_gauge(xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?))
            }
            AnalyzerBasis::Z => self.check_gauge(self.xs_for(qubit)?.clone()),
        }
    }

    fn check_initial_resets(&mut self) -> CircuitResult<()> {
        for qubit in 0..self.xs.len() {
            let gauge = self.xs.get(qubit).cloned().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "qubit {qubit} is outside the gauge tracker"
                ))
            })?;
            self.check_gauge(gauge)?;
        }
        Ok(())
    }

    fn check_gauge(&mut self, gauge: BTreeSet<DemTarget>) -> CircuitResult<()> {
        if gauge.is_empty() {
            return Ok(());
        }
        let has_observables = gauge
            .iter()
            .any(|target| matches!(target, DemTarget::LogicalObservable(_)));
        if self.allow_gauge_detectors && !has_observables {
            let targets = gauge.into_iter().collect::<Vec<_>>();
            self.remove_gauge(&targets);
            self.gauge_errors.push(targets);
            return Ok(());
        }

        let has_detectors = gauge
            .iter()
            .any(|target| matches!(target, DemTarget::RelativeDetector(_)));
        let mut message = String::new();
        if has_observables {
            message.push_str("The circuit contains non-deterministic observables.");
        }
        if has_detectors {
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str("The circuit contains non-deterministic detectors.");
        }
        message.push_str("\n\nThe collapse anti-commuted with these detectors/observables:");
        for target in &gauge {
            message.push_str("\n    ");
            message.push_str(&target.to_string());
        }
        Err(CircuitError::invalid_detector_error_model(message))
    }

    fn remove_gauge(&mut self, targets: &[DemTarget]) {
        let Some(max_target) = targets.last() else {
            return;
        };
        for sensitivity in self.xs.iter_mut().chain(self.zs.iter_mut()) {
            if sensitivity.contains(max_target) {
                toggle_targets(sensitivity, targets.iter().copied());
            }
        }
    }

    fn undo_h(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H target {target} is not a qubit"
                ))
            })?;
            let index = qubit_index(qubit)?;
            let xs = self.xs.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H target qubit {} is outside the gauge tracker",
                    qubit.get()
                ))
            })?;
            let zs = self.zs.get_mut(index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H target qubit {} is outside the gauge tracker",
                    qubit.get()
                ))
            })?;
            std::mem::swap(xs, zs);
        }
        Ok(())
    }

    fn undo_h_xy(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target in instruction.targets().iter().rev() {
            let qubit = target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "H_XY target {target} is not a qubit"
                ))
            })?;
            let xs = self.xs_for(qubit)?.clone();
            self.toggle_zs(qubit, &xs)?;
        }
        Ok(())
    }

    fn undo_controlled_pauli(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        for group in instruction.target_groups().into_iter().rev() {
            let [first, second] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during gauge analysis"
                )));
            };
            match controlled_pauli_action(gate_name, first, second)? {
                ControlledPauliAction::QuantumControlledPauli {
                    left,
                    right,
                    left_basis,
                    right_basis,
                } => {
                    self.undo_quantum_controlled_pauli(left, right, left_basis, right_basis)?;
                }
                ControlledPauliAction::MeasurementFeedback {
                    record_offset,
                    qubit,
                    pauli,
                } => self.undo_measurement_feedback(record_offset, qubit, pauli)?,
                ControlledPauliAction::NoEffect => {}
            }
        }
        Ok(())
    }

    fn undo_quantum_controlled_pauli(
        &mut self,
        left: QubitId,
        right: QubitId,
        left_basis: AnalyzerPauli,
        right_basis: AnalyzerPauli,
    ) -> CircuitResult<()> {
        let left_xs = self.xs_for(left)?.clone();
        let left_zs = self.zs_for(left)?.clone();
        let right_xs = self.xs_for(right)?.clone();
        let right_zs = self.zs_for(right)?.clone();
        self.toggle_if_anticommutes(AnalyzerPauli::X, left_basis, right, right_basis, &left_xs)?;
        self.toggle_if_anticommutes(AnalyzerPauli::Z, left_basis, right, right_basis, &left_zs)?;
        self.toggle_if_anticommutes(AnalyzerPauli::X, right_basis, left, left_basis, &right_xs)?;
        self.toggle_if_anticommutes(AnalyzerPauli::Z, right_basis, left, left_basis, &right_zs)?;
        Ok(())
    }

    fn toggle_if_anticommutes(
        &mut self,
        input_basis: AnalyzerPauli,
        control_basis: AnalyzerPauli,
        output_qubit: QubitId,
        output_basis: AnalyzerPauli,
        sensitivity: &BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
        if analyzer_paulis_anticommute(input_basis, control_basis) {
            self.toggle_pauli_sensitivity(output_qubit, output_basis, sensitivity)?;
        }
        Ok(())
    }

    fn undo_measurement_feedback(
        &mut self,
        record_offset: i32,
        qubit: QubitId,
        pauli: AnalyzerPauli,
    ) -> CircuitResult<()> {
        let sensitivity = match pauli {
            AnalyzerPauli::X => self.zs_for(qubit)?.clone(),
            AnalyzerPauli::Y => xor_sets(self.xs_for(qubit)?, self.zs_for(qubit)?),
            AnalyzerPauli::Z => self.xs_for(qubit)?.clone(),
        };
        let measurement = self.measurement_index_from_offset(record_offset)?;
        toggle_targets(
            self.rec_bits.entry(measurement).or_default(),
            sensitivity.iter().copied(),
        );
        if self
            .rec_bits
            .get(&measurement)
            .is_some_and(BTreeSet::is_empty)
        {
            self.rec_bits.remove(&measurement);
        }
        Ok(())
    }

    fn toggle_product_sensitivity(
        &mut self,
        terms: &[(QubitId, AnalyzerBasis)],
        sensitivity: &BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
        for (qubit, basis) in terms {
            match basis {
                AnalyzerBasis::X => self.toggle_xs(*qubit, sensitivity)?,
                AnalyzerBasis::Y => {
                    self.toggle_xs(*qubit, sensitivity)?;
                    self.toggle_zs(*qubit, sensitivity)?;
                }
                AnalyzerBasis::Z => self.toggle_zs(*qubit, sensitivity)?,
            }
        }
        Ok(())
    }

    fn toggle_pauli_sensitivity(
        &mut self,
        qubit: QubitId,
        pauli: AnalyzerPauli,
        sensitivity: &BTreeSet<DemTarget>,
    ) -> CircuitResult<()> {
        match pauli {
            AnalyzerPauli::X => self.toggle_xs(qubit, sensitivity)?,
            AnalyzerPauli::Y => {
                self.toggle_xs(qubit, sensitivity)?;
                self.toggle_zs(qubit, sensitivity)?;
            }
            AnalyzerPauli::Z => self.toggle_zs(qubit, sensitivity)?,
        }
        Ok(())
    }

    fn clear_qubit(&mut self, qubit: QubitId) -> CircuitResult<()> {
        let index = qubit_index(qubit)?;
        let Some(xs) = self.xs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "reset target qubit {} is outside the gauge tracker",
                qubit.get()
            )));
        };
        xs.clear();
        let Some(zs) = self.zs.get_mut(index) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "reset target qubit {} is outside the gauge tracker",
                qubit.get()
            )));
        };
        zs.clear();
        Ok(())
    }

    fn xs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.xs.get(qubit_index(qubit)?).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the gauge tracker",
                qubit.get()
            ))
        })
    }

    fn zs_for(&self, qubit: QubitId) -> CircuitResult<&BTreeSet<DemTarget>> {
        self.zs.get(qubit_index(qubit)?).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the gauge tracker",
                qubit.get()
            ))
        })
    }

    fn toggle_xs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        let Some(xs) = self.xs.get_mut(qubit_index(qubit)?) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the gauge tracker",
                qubit.get()
            )));
        };
        toggle_targets(xs, targets.iter().copied());
        Ok(())
    }

    fn toggle_zs(&mut self, qubit: QubitId, targets: &BTreeSet<DemTarget>) -> CircuitResult<()> {
        let Some(zs) = self.zs.get_mut(qubit_index(qubit)?) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "qubit {} is outside the gauge tracker",
                qubit.get()
            )));
        };
        toggle_targets(zs, targets.iter().copied());
        Ok(())
    }

    fn measurement_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let measurement_count = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement count does not fit i64 during gauge analysis",
            )
        })?;
        let index = measurement_count
            .checked_add(i64::from(offset))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "measurement offset overflowed during gauge analysis",
                )
            })?;
        if index < 0 || index >= measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record offset rec[{offset}] is out of range during gauge analysis"
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement index does not fit usize during gauge analysis",
            )
        })
    }
}

fn qubit_index(qubit: QubitId) -> CircuitResult<usize> {
    usize::try_from(qubit.get()).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "qubit id {} does not fit usize during gauge analysis",
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
