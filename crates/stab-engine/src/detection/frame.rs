use rand::{Rng, RngExt as _};
use stab_algebra::{PauliBasis, PauliSign};
use stab_model::{Circuit, CircuitInstruction, CircuitItem, Gate, Pauli, Target};

use super::error::{DetectionError as CircuitError, DetectionResult as CircuitResult};
use super::{try_false_vec, try_vec_with_capacity};

mod helpers;
mod plan;

use helpers::{
    frame_bit, is_frame_bit_target, measurement_flip_probability, measurement_record_bit,
    pauli_basis, probability_list, qubit_index, sample_flip, sample_single_pauli,
    sample_two_qubit_pauli, set_frame_bit, single_probability_argument,
    unsupported_frame_instruction, unsupported_frame_target, xor_frame_bit, zero_probability_noise,
};

use plan::decomposed_frame_instruction;
pub(super) use plan::{
    DirectDetectorFramePlan, DirectDetectorFrameState, frame_conversion_plan_with_limits,
};

#[derive(Debug)]
struct ScalarDetectionFrame {
    xs: Vec<bool>,
    zs: Vec<bool>,
    measurements: Vec<bool>,
    detectors: Vec<bool>,
    observables: Vec<bool>,
    correlated_error_occurred: bool,
}

impl ScalarDetectionFrame {
    fn try_reusable(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> CircuitResult<Self> {
        Ok(Self {
            xs: try_false_vec(qubit_count, "detection frame X state")?,
            zs: try_false_vec(qubit_count, "detection frame Z state")?,
            measurements: try_vec_with_capacity(
                measurement_count,
                "detection frame measurement record",
            )?,
            detectors: try_vec_with_capacity(detector_count, "detection frame detector record")?,
            observables: try_false_vec(observable_count, "detection frame observable record")?,
            correlated_error_occurred: false,
        })
    }

    fn reset(&mut self, rng: &mut impl Rng) {
        self.xs.fill(false);
        for bit in &mut self.zs {
            *bit = rng.random_bool(0.5);
        }
        self.measurements.clear();
        self.detectors.clear();
        self.observables.fill(false);
        self.correlated_error_occurred = false;
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
            _ if stab_analysis::gate_has_tableau(instruction.gate()) => {
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
        match terms {
            [] => {}
            [(qubit, basis)] => self.randomize_measured_basis(*qubit, *basis, rng)?,
            _ => self.xor_random_measured_product(terms, rng)?,
        }
        Ok(())
    }

    /// Multiplies the frame by the measured product with probability one half.
    ///
    /// Measuring a Pauli product only defines the frame deviation modulo the whole product, so
    /// the collapse bit must land on every term together; randomizing a single term would
    /// multiply the deviation by a Pauli outside the measured stabilizer group and corrupt
    /// later commuting measurements.
    fn xor_random_measured_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        rng: &mut impl Rng,
    ) -> CircuitResult<()> {
        if !rng.random_bool(0.5) {
            return Ok(());
        }
        for (qubit, basis) in terms {
            match basis {
                PauliBasis::I => {}
                PauliBasis::X => self.xor_x_bit(*qubit, true)?,
                PauliBasis::Z => self.xor_z_bit(*qubit, true)?,
                PauliBasis::Y => {
                    self.xor_x_bit(*qubit, true)?;
                    self.xor_z_bit(*qubit, true)?;
                }
            }
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
        let tableau = stab_analysis::gate_tableau(gate)?;
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
        let input = stab_algebra::advanced::pauli_from_bases_unchecked(PauliSign::Plus, bases);
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
            // Pinned Stim consults only the Pauli X/Z bits here, so combiner
            // targets and inversion bits are ignored decoration
            // (frame_simulator.inl:767-775).
            if target.is_combiner() {
                continue;
            }
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
