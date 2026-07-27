use std::borrow::Cow;

use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use super::{
    ConversionPlan, DetectionConversionLimits, DetectionConversionOutput, DetectionEventRecord,
    try_clone_detection_record, try_false_vec, try_reserve_detection_record_slots,
    try_vec_with_capacity,
};
use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate, Pauli, PauliBasis,
    PauliSign, PauliString, RepeatBlock, Target,
};

mod helpers;

use helpers::{
    frame_bit, is_frame_bit_target, is_frame_qubit_or_bit_target, measurement_flip_probability,
    measurement_record_bit, pauli_basis, probability_list, qubit_index, sample_flip,
    sample_single_pauli, sample_two_qubit_pauli, set_frame_bit, single_probability_argument,
    unsupported_frame_instruction, unsupported_frame_target, xor_frame_bit, zero_probability_noise,
};

struct AdmittedFrameConversion {
    plan: ConversionPlan,
}

impl AdmittedFrameConversion {
    fn admit(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<AdmittedFrameConversion> {
        let plan = ConversionPlan::from_visitor(limits, |plan| {
            append_frame_conversion_plan(circuit, plan)
        })?;
        Ok(Self { plan })
    }

    fn materialize_execution_circuit(&self, circuit: &Circuit) -> CircuitResult<Circuit> {
        let mut result = Circuit::new();
        append_frame_execution_circuit(circuit, &mut result)?;
        Ok(result)
    }
}

pub(super) fn frame_conversion_plan_with_limits(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> CircuitResult<ConversionPlan> {
    Ok(AdmittedFrameConversion::admit(circuit, limits)?.plan)
}

fn append_frame_conversion_plan(circuit: &Circuit, plan: &mut ConversionPlan) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                let decomposed = decomposed_frame_instruction(instruction)?;
                append_frame_conversion_plan(&decomposed, plan)?;
            }
            CircuitItem::Instruction(instruction) => {
                let Some(instruction) = frame_execution_instruction(instruction)? else {
                    continue;
                };
                validate_frame_detection_instruction(instruction.as_ref())?;
                plan.visit_instruction(instruction.as_ref())?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                plan.visit_repeated_body(repeat.repeat_count().get(), |plan| {
                    append_frame_conversion_plan(repeat.body(), plan)
                })?;
            }
        }
    }
    Ok(())
}

fn validate_frame_detection_instruction(instruction: &CircuitInstruction) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE"
        | "I_ERROR" | "II_ERROR" => Ok(()),
        "R"
        | "RX"
        | "RY"
        | "M"
        | "MX"
        | "MY"
        | "MR"
        | "MRX"
        | "MRY"
        | "MXX"
        | "MYY"
        | "MZZ"
        | "MPP"
        | "MPAD"
        | "X_ERROR"
        | "Y_ERROR"
        | "Z_ERROR"
        | "DEPOLARIZE1"
        | "DEPOLARIZE2"
        | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2"
        | "E"
        | "ELSE_CORRELATED_ERROR"
        | "HERALDED_ERASE"
        | "HERALDED_PAULI_CHANNEL_1" => Ok(()),
        "SPP" | "SPP_DAG" => Err(CircuitError::invalid_sampler_compilation(
            "frame detection must decompose SPP instructions before validation",
        )),
        "CX" | "CY" => validate_frame_controlled_pauli_targets(instruction),
        "CZ" => validate_frame_cz_targets(instruction),
        "XCZ" | "YCZ" => validate_frame_x_or_y_controlled_z_targets(instruction),
        _ if crate::analysis::gate_has_tableau(instruction.gate()) => Ok(()),
        _ if zero_probability_noise(instruction)? => Ok(()),
        name => Err(CircuitError::invalid_sampler_compilation(format!(
            "M9 detector frame subset does not support {name}"
        ))),
    }
}

fn decomposed_frame_instruction(instruction: &CircuitInstruction) -> CircuitResult<Circuit> {
    crate::analysis::decomposed_single_instruction(instruction).map_err(|error| {
        CircuitError::invalid_sampler_compilation(format!(
            "{} cannot be executed by frame detection via decomposition: {error}",
            instruction.gate().canonical_name()
        ))
    })
}

fn validate_frame_controlled_pauli_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        let [control, target] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if (control.qubit_id().is_some() || is_frame_bit_target(control))
            && target.qubit_id().is_some()
        {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

fn validate_frame_cz_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        let [left, right] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if is_frame_qubit_or_bit_target(left) && is_frame_qubit_or_bit_target(right) {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

fn validate_frame_x_or_y_controlled_z_targets(
    instruction: &CircuitInstruction,
) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        let [left, right] = target_group else {
            return Err(unsupported_frame_instruction(instruction));
        };
        if left.qubit_id().is_some() && right.qubit_id().is_some() {
            continue;
        }
        if left.qubit_id().is_some() && right.measurement_record_offset().is_some() {
            continue;
        }
        if left.qubit_id().is_some() && right.is_sweep_bit_target() {
            continue;
        }
        return Err(unsupported_frame_instruction(instruction));
    }
    Ok(())
}

pub(super) fn sample_detection_events_with_frame_and_limits(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
) -> CircuitResult<DetectionConversionOutput> {
    let admitted = AdmittedFrameConversion::admit(circuit, limits)?;
    admitted.plan.validate_detection_record_shot_count(shots)?;
    let executable = admitted.materialize_execution_circuit(circuit)?;
    let plan = &admitted.plan;
    let detector_count = plan.detector_terms.len();
    let observable_count = plan.observable_terms.len();
    let mut records = Vec::new();
    try_reserve_detection_record_slots(&mut records, shots)?;
    let mut rng = SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random));
    sample_detection_events_with_frame_plan(
        &executable,
        shots,
        plan,
        limits,
        &mut rng,
        |record| {
            records.push(try_clone_detection_record(record)?);
            Ok::<(), CircuitError>(())
        },
    )?;
    Ok(DetectionConversionOutput {
        records,
        detector_count,
        observable_count,
    })
}

pub(super) fn try_for_each_detection_event_with_frame_and_limits<E, F>(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
    limits: DetectionConversionLimits,
    mut visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    let admitted = AdmittedFrameConversion::admit(circuit, limits)?;
    let executable = admitted.materialize_execution_circuit(circuit)?;
    let plan = &admitted.plan;
    let mut rng = SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random));
    sample_detection_events_with_frame_plan(&executable, shots, plan, limits, &mut rng, |record| {
        visit(record)
    })
}

fn append_frame_execution_circuit(circuit: &Circuit, result: &mut Circuit) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if matches!(instruction.gate().canonical_name(), "SPP" | "SPP_DAG") =>
            {
                result.append_circuit(&decomposed_frame_instruction(instruction)?);
            }
            CircuitItem::Instruction(instruction) => {
                if let Some(instruction) = frame_execution_instruction(instruction)? {
                    result.append_instruction(instruction.into_owned());
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = Circuit::new();
                append_frame_execution_circuit(repeat.body(), &mut body)?;
                result.append_repeat_block(RepeatBlock::new_with_tag_bytes(
                    repeat.repeat_count(),
                    body,
                    repeat.tag_bytes(),
                ));
            }
        }
    }
    Ok(())
}

fn frame_execution_instruction<'a>(
    instruction: &'a CircuitInstruction,
) -> CircuitResult<Option<Cow<'a, CircuitInstruction>>> {
    if !matches!(instruction.gate().canonical_name(), "XCZ" | "YCZ") {
        return Ok(Some(Cow::Borrowed(instruction)));
    }

    let mut targets = Vec::new();
    let mut removed_sweep_target = false;
    for target_group in instruction.target_groups() {
        let [left, right] = target_group else {
            return Ok(Some(Cow::Borrowed(instruction)));
        };
        if left.qubit_id().is_some() && right.is_sweep_bit_target() {
            removed_sweep_target = true;
            continue;
        }
        targets.extend(target_group.iter().cloned());
    }
    if !removed_sweep_target {
        return Ok(Some(Cow::Borrowed(instruction)));
    }
    if targets.is_empty() {
        return Ok(None);
    }
    Ok(Some(Cow::Owned(CircuitInstruction::new_with_tag_bytes(
        instruction.gate(),
        instruction.args().to_vec(),
        targets,
        instruction.tag_bytes(),
    )?)))
}

fn sample_detection_events_with_frame_plan<E, F>(
    circuit: &Circuit,
    shots: usize,
    plan: &ConversionPlan,
    limits: DetectionConversionLimits,
    rng: &mut SmallRng,
    mut visit: F,
) -> Result<(), E>
where
    E: From<CircuitError>,
    F: FnMut(&DetectionEventRecord) -> Result<(), E>,
{
    for _ in 0..shots {
        let mut frame = ScalarDetectionFrame::try_new(
            circuit.count_qubits(),
            plan.measurement_count,
            plan.detector_terms.len(),
            plan.observable_terms.len(),
            rng,
        )?;
        frame.execute_circuit(circuit, limits.max_repeat_unroll(), rng)?;
        if frame.measurements.len() != plan.measurement_count {
            return Err(CircuitError::invalid_result_format(format!(
                "frame detection sampled {} measurement bits but expected {}",
                frame.measurements.len(),
                plan.measurement_count
            ))
            .into());
        }
        let record = DetectionEventRecord {
            detectors: frame.detectors,
            observables: frame.observables,
        };
        visit(&record)?;
    }
    Ok(())
}

struct ScalarDetectionFrame {
    xs: Vec<bool>,
    zs: Vec<bool>,
    measurements: Vec<bool>,
    detectors: Vec<bool>,
    observables: Vec<bool>,
    correlated_error_occurred: bool,
}

impl ScalarDetectionFrame {
    fn try_new(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
        rng: &mut impl Rng,
    ) -> CircuitResult<Self> {
        let xs = try_false_vec(qubit_count, "detection frame X state")?;
        let mut zs = try_false_vec(qubit_count, "detection frame Z state")?;
        let measurements =
            try_vec_with_capacity(measurement_count, "detection frame measurement record")?;
        let detectors = try_vec_with_capacity(detector_count, "detection frame detector record")?;
        let observables = try_false_vec(observable_count, "detection frame observable record")?;
        for bit in &mut zs {
            *bit = rng.random_bool(0.5);
        }
        Ok(Self {
            xs,
            zs,
            measurements,
            detectors,
            observables,
            correlated_error_occurred: false,
        })
    }

    fn execute_circuit(
        &mut self,
        circuit: &Circuit,
        max_repeat_unroll: u64,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.execute_instruction(instruction, max_repeat_unroll, rng)?
                }
                CircuitItem::RepeatBlock(repeat) => {
                    let repeat_count = repeat.repeat_count().get();
                    if repeat_count > max_repeat_unroll {
                        return Err(CircuitError::invalid_sampler_compilation(format!(
                            "frame detection currently supports repeat counts up to {max_repeat_unroll}, got {repeat_count}"
                        )));
                    }
                    for _ in 0..repeat_count {
                        self.execute_circuit(repeat.body(), max_repeat_unroll, rng)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        max_repeat_unroll: u64,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => Ok(()),
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "R" => self.reset_targets(instruction, PauliBasis::Z, rng),
            "RX" => self.reset_targets(instruction, PauliBasis::X, rng),
            "RY" => self.reset_targets(instruction, PauliBasis::Y, rng),
            "M" => self.measure_targets(instruction, PauliBasis::Z, false, rng),
            "MX" => self.measure_targets(instruction, PauliBasis::X, false, rng),
            "MY" => self.measure_targets(instruction, PauliBasis::Y, false, rng),
            "MR" => self.measure_targets(instruction, PauliBasis::Z, true, rng),
            "MRX" => self.measure_targets(instruction, PauliBasis::X, true, rng),
            "MRY" => self.measure_targets(instruction, PauliBasis::Y, true, rng),
            "MXX" => self.measure_pair_products(instruction, PauliBasis::X, rng),
            "MYY" => self.measure_pair_products(instruction, PauliBasis::Y, rng),
            "MZZ" => self.measure_pair_products(instruction, PauliBasis::Z, rng),
            "MPP" => self.measure_pauli_products(instruction, rng),
            "MPAD" => self.measure_pads(instruction, rng),
            "CX" => self.apply_controlled_or_feedback(instruction, PauliBasis::X),
            "CY" => self.apply_controlled_or_feedback(instruction, PauliBasis::Y),
            "CZ" => self.apply_cz_or_feedback(instruction),
            "XCZ" | "YCZ" => self.apply_x_or_y_controlled_z(instruction),
            "X_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [single_probability_argument(instruction)?.get(), 0.0, 0.0],
                rng,
            ),
            "Y_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [0.0, single_probability_argument(instruction)?.get(), 0.0],
                rng,
            ),
            "Z_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [0.0, 0.0, single_probability_argument(instruction)?.get()],
                rng,
            ),
            "I_ERROR" | "II_ERROR" => Ok(()),
            "DEPOLARIZE1" => {
                let probability = single_probability_argument(instruction)?.get() / 3.0;
                self.apply_single_pauli_noise(instruction, [probability; 3], rng)
            }
            "DEPOLARIZE2" => {
                let probability = single_probability_argument(instruction)?.get() / 15.0;
                self.apply_two_qubit_pauli_noise(instruction, [probability; 15], rng)
            }
            "PAULI_CHANNEL_1" => {
                let probabilities = probability_list::<3>(instruction)?;
                self.apply_single_pauli_noise(instruction, probabilities, rng)
            }
            "PAULI_CHANNEL_2" => {
                let probabilities = probability_list::<15>(instruction)?;
                self.apply_two_qubit_pauli_noise(instruction, probabilities, rng)
            }
            "E" => self.apply_correlated_error(instruction, false, rng),
            "ELSE_CORRELATED_ERROR" => self.apply_correlated_error(instruction, true, rng),
            "HERALDED_ERASE" => self.apply_heralded_erase(instruction, rng),
            "HERALDED_PAULI_CHANNEL_1" => self.apply_heralded_pauli_channel(instruction, rng),
            "SPP" | "SPP_DAG" => {
                self.execute_decomposed_instruction(instruction, max_repeat_unroll, rng)
            }
            _ if crate::analysis::gate_has_tableau(instruction.gate()) => {
                self.apply_tableau_instruction(instruction)
            }
            _ if zero_probability_noise(instruction)? => Ok(()),
            name => Err(CircuitError::invalid_sampler_compilation(format!(
                "M9 detector frame subset does not support {name}"
            ))),
        }
    }

    fn execute_decomposed_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        max_repeat_unroll: u64,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let decomposed = decomposed_frame_instruction(instruction)?;
        self.execute_circuit(&decomposed, max_repeat_unroll, rng)
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let mut bit = false;
        for target in instruction.targets() {
            let Some(offset) = target.measurement_record_offset() else {
                return Err(CircuitError::invalid_result_format(format!(
                    "DETECTOR target {target} is not a measurement record"
                )));
            };
            bit ^= measurement_record_bit(&self.measurements, offset)?;
        }
        self.detectors.push(bit);
        Ok(())
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction
            .observable_id_argument()?
            .ok_or_else(|| CircuitError::invalid_result_format("OBSERVABLE_INCLUDE missing id"))?;
        let observable_id = usize::try_from(observable.get()).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "observable id {} does not fit usize",
                observable.get()
            ))
        })?;
        if self.observables.get(observable_id).is_none() {
            return Err(CircuitError::invalid_result_format(format!(
                "observable id {observable_id} was not initialized"
            )));
        }
        let mut bit = false;
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                bit ^= measurement_record_bit(&self.measurements, offset)?;
            } else if target.is_pauli_target() {
                bit ^= self.pauli_target_frame_bit(target)?;
            } else {
                return Err(CircuitError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        if bit {
            let observable = self.observables.get_mut(observable_id).ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "observable id {observable_id} was not initialized"
                ))
            })?;
            *observable ^= true;
        }
        Ok(())
    }

    fn reset_targets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        for target in instruction.targets() {
            self.reset_qubit(qubit_index(instruction, target)?, basis, rng)?;
        }
        Ok(())
    }

    fn measure_targets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        reset: bool,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let result =
                self.measure_qubit_frame(qubit, basis, rng)? ^ sample_flip(flip_probability, rng);
            self.measurements.push(result);
            if reset {
                self.reset_qubit(qubit, basis, rng)?;
            }
        }
        Ok(())
    }

    fn measure_pads(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target in instruction.targets() {
            if target.qubit_id().is_none() {
                return Err(unsupported_frame_instruction(instruction));
            }
            self.measurements.push(sample_flip(flip_probability, rng));
        }
        Ok(())
    }

    fn measure_pair_products(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target_group in instruction.target_groups() {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            let raw_terms = vec![
                (qubit_index(instruction, left)?, basis, false),
                (qubit_index(instruction, right)?, basis, false),
            ];
            let (terms, _) = crate::sampling::pauli_product::normalize_terms(raw_terms, false)?;
            self.measure_pauli_product_terms(&terms, flip_probability, rng)?;
        }
        Ok(())
    }

    fn measure_pauli_products(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target_group in instruction.target_groups() {
            let mut raw_terms = Vec::new();
            for target in target_group {
                if target.is_combiner() {
                    continue;
                }
                let Some(pauli) = target.pauli_type() else {
                    return Err(unsupported_frame_instruction(instruction));
                };
                // Static inversion belongs to the reference sample. The frame record only stores flips.
                raw_terms.push((qubit_index(instruction, target)?, pauli_basis(pauli), false));
            }
            let (terms, _) = crate::sampling::pauli_product::normalize_terms(raw_terms, false)?;
            self.measure_pauli_product_terms(&terms, flip_probability, rng)?;
        }
        Ok(())
    }

    fn measure_pauli_product_terms(
        &mut self,
        terms: &[(usize, PauliBasis)],
        flip_probability: f64,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let mut result = sample_flip(flip_probability, rng);
        for (qubit, basis) in terms {
            result ^= self.frame_measurement_bit(*qubit, *basis)?;
        }
        self.measurements.push(result);
        if let Some((qubit, basis)) = terms.first() {
            self.randomize_measured_basis(*qubit, *basis, rng)?;
        }
        Ok(())
    }

    fn apply_controlled_or_feedback(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<()> {
        for target_group in instruction.target_groups() {
            let [control, target] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            if control.is_sweep_bit_target() {
                if target.qubit_id().is_some() {
                    // `detect` has no sweep input. Omitted sweep bits use all-false Stim semantics.
                    continue;
                }
                return Err(unsupported_frame_instruction(instruction));
            }
            if target.measurement_record_offset().is_some() || target.is_sweep_bit_target() {
                return Err(unsupported_frame_instruction(instruction));
            }
            if let Some(offset) = control.measurement_record_offset() {
                if measurement_record_bit(&self.measurements, offset)? {
                    self.apply_pauli(qubit_index(instruction, target)?, basis)?;
                }
            } else {
                self.apply_tableau_targets(instruction.gate(), target_group)?;
            }
        }
        Ok(())
    }

    fn apply_cz_or_feedback(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target_group in instruction.target_groups() {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            if is_frame_bit_target(left) && is_frame_bit_target(right) {
                continue;
            }
            if left.is_sweep_bit_target() && right.qubit_id().is_some() {
                // `detect` has no sweep input. Omitted sweep bits use all-false Stim semantics.
                continue;
            }
            if right.is_sweep_bit_target() && left.qubit_id().is_some() {
                // `detect` has no sweep input. Omitted sweep bits use all-false Stim semantics.
                continue;
            }
            match (
                left.measurement_record_offset(),
                right.measurement_record_offset(),
            ) {
                (Some(left_offset), None) => {
                    if measurement_record_bit(&self.measurements, left_offset)? {
                        self.apply_pauli(qubit_index(instruction, right)?, PauliBasis::Z)?;
                    }
                }
                (None, Some(right_offset)) => {
                    if measurement_record_bit(&self.measurements, right_offset)? {
                        self.apply_pauli(qubit_index(instruction, left)?, PauliBasis::Z)?;
                    }
                }
                (Some(_), Some(_)) => {}
                (None, None) => self.apply_tableau_targets(instruction.gate(), target_group)?,
            }
        }
        Ok(())
    }

    fn apply_x_or_y_controlled_z(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let feedback_basis = match instruction.gate().canonical_name() {
            "XCZ" => PauliBasis::X,
            "YCZ" => PauliBasis::Y,
            _ => return Err(unsupported_frame_instruction(instruction)),
        };
        for target_group in instruction.target_groups() {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            if left.qubit_id().is_some() && right.is_sweep_bit_target() {
                // `detect` has no sweep input. Omitted sweep bits use all-false Stim semantics.
                continue;
            }
            if let (Some(_), Some(offset)) = (left.qubit_id(), right.measurement_record_offset()) {
                if measurement_record_bit(&self.measurements, offset)? {
                    self.apply_pauli(qubit_index(instruction, left)?, feedback_basis)?;
                }
                continue;
            }
            if left.qubit_id().is_some() && right.qubit_id().is_some() {
                self.apply_tableau_targets(instruction.gate(), target_group)?;
                continue;
            }
            return Err(unsupported_frame_instruction(instruction));
        }
        Ok(())
    }

    fn apply_tableau_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target_group in instruction.target_groups() {
            self.apply_tableau_targets(instruction.gate(), target_group)?;
        }
        Ok(())
    }

    fn apply_tableau_targets(&mut self, gate: Gate, targets: &[Target]) -> CircuitResult<()> {
        let gate_name = gate.canonical_name();
        let tableau = crate::analysis::gate_tableau(gate)?;
        let qubits = targets
            .iter()
            .map(|target| {
                target
                    .qubit_id()
                    .ok_or_else(|| unsupported_frame_target(gate_name, target))
                    .and_then(|qubit| {
                        usize::try_from(qubit.get()).map_err(|_| {
                            CircuitError::invalid_sampler_compilation(format!(
                                "qubit target {} cannot fit in this platform's usize",
                                qubit.get()
                            ))
                        })
                    })
            })
            .collect::<CircuitResult<Vec<_>>>()?;
        if qubits.len() != tableau.len() {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "gate {gate_name} frame transform expected {} targets but got {}",
                tableau.len(),
                qubits.len()
            )));
        }
        let bases = qubits
            .iter()
            .map(|qubit| self.qubit_basis(*qubit))
            .collect::<CircuitResult<Vec<_>>>()?;
        let input = PauliString::from_bases_unchecked(PauliSign::Plus, bases);
        let output = tableau
            .apply(&input)
            .map_err(|error| CircuitError::invalid_sampler_compilation(error.to_string()))?;
        for (local_index, qubit) in qubits.into_iter().enumerate() {
            let basis = output.get(local_index).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "tableau frame transform changed output length",
                )
            })?;
            self.set_x_bit(qubit, basis.x_bit())?;
            self.set_z_bit(qubit, basis.z_bit())?;
        }
        Ok(())
    }

    fn apply_single_pauli_noise(
        &mut self,
        instruction: &CircuitInstruction,
        probabilities: [f64; 3],
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            if let Some(basis) = sample_single_pauli(probabilities, rng) {
                self.apply_pauli(qubit, basis)?;
            }
        }
        Ok(())
    }

    fn apply_two_qubit_pauli_noise(
        &mut self,
        instruction: &CircuitInstruction,
        probabilities: [f64; 15],
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        for target_group in instruction.target_groups() {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            let left = qubit_index(instruction, left)?;
            let right = qubit_index(instruction, right)?;
            if let Some((left_basis, right_basis)) = sample_two_qubit_pauli(probabilities, rng) {
                if let Some(basis) = left_basis {
                    self.apply_pauli(left, basis)?;
                }
                if let Some(basis) = right_basis {
                    self.apply_pauli(right, basis)?;
                }
            }
        }
        Ok(())
    }

    fn apply_correlated_error(
        &mut self,
        instruction: &CircuitInstruction,
        else_branch: bool,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        if else_branch && self.correlated_error_occurred {
            return Ok(());
        }
        if !else_branch {
            self.correlated_error_occurred = false;
        }
        if !sample_flip(single_probability_argument(instruction)?.get(), rng) {
            return Ok(());
        }
        self.correlated_error_occurred = true;
        for target in instruction.targets() {
            let Some(pauli) = target.pauli_type() else {
                return Err(unsupported_frame_instruction(instruction));
            };
            self.apply_pauli(qubit_index(instruction, target)?, pauli_basis(pauli))?;
        }
        Ok(())
    }

    fn apply_heralded_erase(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let probability = single_probability_argument(instruction)?.get();
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let occurred = sample_flip(probability, rng);
            self.measurements.push(occurred);
            if occurred {
                match rng.random::<u8>() & 3 {
                    1 => self.apply_pauli(qubit, PauliBasis::X)?,
                    2 => self.apply_pauli(qubit, PauliBasis::Z)?,
                    3 => self.apply_pauli(qubit, PauliBasis::Y)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn apply_heralded_pauli_channel(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        let probabilities = probability_list::<4>(instruction)?;
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let mut sampled_probability = rng.random::<f64>();
            let mut occurred = false;
            if sampled_probability < probabilities[0] {
                occurred = true;
            } else {
                sampled_probability -= probabilities[0];
                for (basis, probability) in [
                    (PauliBasis::X, probabilities[1]),
                    (PauliBasis::Y, probabilities[2]),
                    (PauliBasis::Z, probabilities[3]),
                ] {
                    if sampled_probability < probability {
                        occurred = true;
                        self.apply_pauli(qubit, basis)?;
                        break;
                    }
                    sampled_probability -= probability;
                }
            }
            self.measurements.push(occurred);
        }
        Ok(())
    }

    fn reset_qubit(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => {
                self.set_z_bit(qubit, false)?;
                self.set_x_bit(qubit, rng.random_bool(0.5))?;
            }
            PauliBasis::Y => {
                let bit = rng.random_bool(0.5);
                self.set_z_bit(qubit, bit)?;
                self.set_x_bit(qubit, bit)?;
            }
            PauliBasis::Z => {
                self.set_x_bit(qubit, false)?;
                self.set_z_bit(qubit, rng.random_bool(0.5))?;
            }
        }
        Ok(())
    }

    fn measure_qubit_frame(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
    ) -> CircuitResult<bool> {
        let result = self.frame_measurement_bit(qubit, basis)?;
        self.randomize_measured_basis(qubit, basis, rng)?;
        Ok(result)
    }

    fn frame_measurement_bit(&self, qubit: usize, basis: PauliBasis) -> CircuitResult<bool> {
        match basis {
            PauliBasis::I => Ok(false),
            PauliBasis::X => self.z_bit(qubit),
            PauliBasis::Y => Ok(self.x_bit(qubit)? ^ self.z_bit(qubit)?),
            PauliBasis::Z => self.x_bit(qubit),
        }
    }

    fn randomize_measured_basis(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => self.set_x_bit(qubit, rng.random_bool(0.5))?,
            PauliBasis::Y => {
                let result = self.x_bit(qubit)? ^ self.z_bit(qubit)?;
                let z = rng.random_bool(0.5);
                self.set_z_bit(qubit, z)?;
                self.set_x_bit(qubit, result ^ z)?;
            }
            PauliBasis::Z => self.set_z_bit(qubit, rng.random_bool(0.5))?,
        }
        Ok(())
    }

    fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) -> CircuitResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => self.xor_x_bit(qubit, true)?,
            PauliBasis::Y => {
                self.xor_x_bit(qubit, true)?;
                self.xor_z_bit(qubit, true)?;
            }
            PauliBasis::Z => self.xor_z_bit(qubit, true)?,
        }
        Ok(())
    }

    fn pauli_target_frame_bit(&self, target: &Target) -> CircuitResult<bool> {
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "OBSERVABLE_INCLUDE Pauli target {target} has no qubit id"
            ))
        })?;
        let qubit = usize::try_from(qubit.get()).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "qubit target {} cannot fit in this platform's usize",
                qubit.get()
            ))
        })?;
        match target.pauli_type() {
            Some(Pauli::X) => self.z_bit(qubit),
            Some(Pauli::Y) => Ok(self.x_bit(qubit)? ^ self.z_bit(qubit)?),
            Some(Pauli::Z) => self.x_bit(qubit),
            None => Err(CircuitError::invalid_result_format(format!(
                "OBSERVABLE_INCLUDE target {target} is not a Pauli target"
            ))),
        }
    }

    fn qubit_basis(&self, qubit: usize) -> CircuitResult<PauliBasis> {
        Ok(PauliBasis::from_xz(self.x_bit(qubit)?, self.z_bit(qubit)?))
    }

    fn x_bit(&self, qubit: usize) -> CircuitResult<bool> {
        frame_bit(&self.xs, qubit)
    }

    fn z_bit(&self, qubit: usize) -> CircuitResult<bool> {
        frame_bit(&self.zs, qubit)
    }

    fn set_x_bit(&mut self, qubit: usize, value: bool) -> CircuitResult<()> {
        set_frame_bit(&mut self.xs, qubit, value)
    }

    fn set_z_bit(&mut self, qubit: usize, value: bool) -> CircuitResult<()> {
        set_frame_bit(&mut self.zs, qubit, value)
    }

    fn xor_x_bit(&mut self, qubit: usize, value: bool) -> CircuitResult<()> {
        xor_frame_bit(&mut self.xs, qubit, value)
    }

    fn xor_z_bit(&mut self, qubit: usize, value: bool) -> CircuitResult<()> {
        xor_frame_bit(&mut self.zs, qubit, value)
    }
}
