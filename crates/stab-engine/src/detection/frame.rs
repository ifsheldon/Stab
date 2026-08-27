use rand::{Rng, RngExt as _};
use stab_algebra::PauliBasis;
use stab_model::{
    CircuitInstruction, Gate, GateTargetGroupKind, Pauli, Target,
    advanced::{
        ClassicalControl, ControlledPauliTargetPair, classify_controlled_pauli_target_pair,
    },
};

use super::error::{DetectionError, DetectionResult};
use super::{try_false_vec, try_vec_with_capacity};

mod helpers;
mod plan;
mod program;

use helpers::{
    frame_bit, measurement_flip_probability, measurement_record_bit, pauli_basis, probability_list,
    qubit_id_index, qubit_index, sample_flip, sample_single_pauli, sample_two_qubit_pauli,
    set_frame_bit, single_probability_argument, unsupported_frame_instruction,
    unsupported_frame_target, xor_frame_bit, zero_probability_noise,
};

pub(super) use plan::{
    DetectorFrameState, DirectDetectorFramePlan, SweepCorrectionPlan,
    admit_combined_compiled_storage,
};
use program::{FrameProgram, FrameTableauTransform};

#[derive(Clone, Copy)]
enum FrameExecutionMode<'a> {
    Sample,
    SweepCorrection(&'a [bool]),
}

impl FrameExecutionMode<'_> {
    fn random_bool(self, rng: &mut impl Rng, probability: f64) -> bool {
        match self {
            Self::Sample => rng.random_bool(probability),
            Self::SweepCorrection(_) => false,
        }
    }

    const fn samples_noise(self) -> bool {
        matches!(self, Self::Sample)
    }

    fn sweep_bit(self, id: u32) -> bool {
        match self {
            Self::Sample => false,
            Self::SweepCorrection(sweep_record) => usize::try_from(id)
                .ok()
                .and_then(|index| sweep_record.get(index))
                .copied()
                .unwrap_or(false),
        }
    }
}

#[derive(Debug)]
struct ScalarDetectionFrame {
    xs: Vec<bool>,
    zs: Vec<bool>,
    measurements: Vec<bool>,
    observables: Vec<bool>,
    correlated_error_occurred: bool,
}

impl ScalarDetectionFrame {
    fn try_reusable(
        qubit_count: usize,
        measurement_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            xs: try_false_vec(qubit_count, "detection frame X state")?,
            zs: try_false_vec(qubit_count, "detection frame Z state")?,
            measurements: try_vec_with_capacity(
                measurement_count,
                "detection frame measurement record",
            )?,
            observables: try_false_vec(observable_count, "detection frame observable record")?,
            correlated_error_occurred: false,
        })
    }

    fn reset(&mut self, rng: &mut impl Rng, mode: FrameExecutionMode<'_>) {
        self.xs.fill(false);
        for bit in &mut self.zs {
            *bit = mode.random_bool(rng, 0.5);
        }
        self.measurements.clear();
        self.observables.fill(false);
        self.correlated_error_occurred = false;
    }

    fn execute_program(
        &mut self,
        program: &FrameProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let mut cursor = program.cursor();
        while let Some(step) = cursor.next_instruction()? {
            self.execute_instruction(step.instruction, step.tableau, rng, mode)?;
        }
        Ok(())
    }

    pub(super) fn execute_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        tableau: Option<&FrameTableauTransform>,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        match instruction.gate().canonical_name() {
            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => Ok(()),
            "DETECTOR" => Ok(()),
            "OBSERVABLE_INCLUDE" => self.record_pauli_observable(instruction),
            "R" => self.reset_targets(instruction, PauliBasis::Z, rng, mode),
            "RX" => self.reset_targets(instruction, PauliBasis::X, rng, mode),
            "RY" => self.reset_targets(instruction, PauliBasis::Y, rng, mode),
            "M" => self.measure_targets(instruction, PauliBasis::Z, false, rng, mode),
            "MX" => self.measure_targets(instruction, PauliBasis::X, false, rng, mode),
            "MY" => self.measure_targets(instruction, PauliBasis::Y, false, rng, mode),
            "MR" => self.measure_targets(instruction, PauliBasis::Z, true, rng, mode),
            "MRX" => self.measure_targets(instruction, PauliBasis::X, true, rng, mode),
            "MRY" => self.measure_targets(instruction, PauliBasis::Y, true, rng, mode),
            "MXX" => self.measure_pair_products(instruction, PauliBasis::X, rng, mode),
            "MYY" => self.measure_pair_products(instruction, PauliBasis::Y, rng, mode),
            "MZZ" => self.measure_pair_products(instruction, PauliBasis::Z, rng, mode),
            "MPP" => self.measure_pauli_products(instruction, rng, mode),
            "MPAD" => self.measure_pads(instruction, rng, mode),
            "CX" | "CY" | "CZ" | "XCZ" | "YCZ" => {
                self.apply_controlled_pauli(instruction, tableau, mode)
            }
            "X_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [single_probability_argument(instruction)?.get(), 0.0, 0.0],
                rng,
                mode,
            ),
            "Y_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [0.0, single_probability_argument(instruction)?.get(), 0.0],
                rng,
                mode,
            ),
            "Z_ERROR" => self.apply_single_pauli_noise(
                instruction,
                [0.0, 0.0, single_probability_argument(instruction)?.get()],
                rng,
                mode,
            ),
            "I_ERROR" | "II_ERROR" => Ok(()),
            "DEPOLARIZE1" => {
                let probability = single_probability_argument(instruction)?.get() / 3.0;
                self.apply_single_pauli_noise(instruction, [probability; 3], rng, mode)
            }
            "DEPOLARIZE2" => {
                let probability = single_probability_argument(instruction)?.get() / 15.0;
                self.apply_two_qubit_pauli_noise(instruction, [probability; 15], rng, mode)
            }
            "PAULI_CHANNEL_1" => {
                let probabilities = probability_list::<3>(instruction)?;
                self.apply_single_pauli_noise(instruction, probabilities, rng, mode)
            }
            "PAULI_CHANNEL_2" => {
                let probabilities = probability_list::<15>(instruction)?;
                self.apply_two_qubit_pauli_noise(instruction, probabilities, rng, mode)
            }
            "E" => self.apply_correlated_error(instruction, false, rng, mode),
            "ELSE_CORRELATED_ERROR" => self.apply_correlated_error(instruction, true, rng, mode),
            "HERALDED_ERASE" => self.apply_heralded_erase(instruction, rng, mode),
            "HERALDED_PAULI_CHANNEL_1" => self.apply_heralded_pauli_channel(instruction, rng, mode),
            "SPP" | "SPP_DAG" => Err(DetectionError::invalid_sampler_compilation(
                "SPP reached detector-frame execution without compile-time lowering",
            )),
            _ if stab_analysis::gate_has_tableau(instruction.gate()) => {
                self.apply_tableau_instruction(instruction, tableau)
            }
            _ if zero_probability_noise(instruction)? => Ok(()),
            name => Err(DetectionError::invalid_sampler_compilation(format!(
                "detector frame execution does not support {name}"
            ))),
        }
    }

    fn record_pauli_observable(&mut self, instruction: &CircuitInstruction) -> DetectionResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            DetectionError::invalid_result_format("OBSERVABLE_INCLUDE missing id")
        })?;
        let observable_id = usize::try_from(observable.get()).map_err(|_| {
            DetectionError::invalid_result_format(format!(
                "observable id {} does not fit usize",
                observable.get()
            ))
        })?;
        if self.observables.get(observable_id).is_none() {
            return Err(DetectionError::invalid_result_format(format!(
                "observable id {observable_id} was not initialized"
            )));
        }
        let mut bit = false;
        for target in instruction.targets() {
            if target.measurement_record_offset().is_some() {
                continue;
            }
            if target.is_pauli_target() {
                bit ^= self.pauli_target_frame_bit(target)?;
            } else {
                return Err(DetectionError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        if bit {
            let observable = self.observables.get_mut(observable_id).ok_or_else(|| {
                DetectionError::invalid_result_format(format!(
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        for target in instruction.targets() {
            self.reset_qubit(qubit_index(instruction, target)?, basis, rng, mode)?;
        }
        Ok(())
    }

    fn measure_targets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        reset: bool,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let result = self.measure_qubit_frame(qubit, basis, rng, mode)?
                ^ mode.random_bool(rng, flip_probability);
            self.measurements.push(result);
            if reset {
                self.reset_qubit(qubit, basis, rng, mode)?;
            }
        }
        Ok(())
    }

    fn measure_pads(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target in instruction.targets() {
            if target.qubit_id().is_none() {
                return Err(unsupported_frame_instruction(instruction));
            }
            self.measurements
                .push(mode.random_bool(rng, flip_probability));
        }
        Ok(())
    }

    fn measure_pair_products(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let flip_probability = measurement_flip_probability(instruction)?;
        for target_group in instruction.targets().chunks(2) {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            let raw_terms = vec![
                (qubit_index(instruction, left)?, basis, false),
                (qubit_index(instruction, right)?, basis, false),
            ];
            let (terms, _) = crate::sampling::pauli_product::normalize_terms(raw_terms, false)?;
            self.measure_pauli_product_terms(&terms, flip_probability, rng, mode)?;
        }
        Ok(())
    }

    fn measure_pauli_products(
        &mut self,
        instruction: &CircuitInstruction,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
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
            self.measure_pauli_product_terms(&terms, flip_probability, rng, mode)?;
        }
        Ok(())
    }

    fn measure_pauli_product_terms(
        &mut self,
        terms: &[(usize, PauliBasis)],
        flip_probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let mut result = mode.random_bool(rng, flip_probability);
        for (qubit, basis) in terms {
            result ^= self.frame_measurement_bit(*qubit, *basis)?;
        }
        self.measurements.push(result);
        match terms {
            [] => {}
            [(qubit, basis)] => self.randomize_measured_basis(*qubit, *basis, rng, mode)?,
            _ => self.xor_random_measured_product(terms, rng, mode)?,
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.random_bool(rng, 0.5) {
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

    fn apply_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
        tableau: Option<&FrameTableauTransform>,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let basis = match instruction.gate().canonical_name() {
            "CX" | "XCZ" => PauliBasis::X,
            "CY" | "YCZ" => PauliBasis::Y,
            "CZ" => PauliBasis::Z,
            _ => return Err(unsupported_frame_instruction(instruction)),
        };
        for target_group in instruction.targets().chunks(2) {
            match classify_controlled_pauli_target_pair(instruction.gate(), target_group) {
                ControlledPauliTargetPair::Quantum { .. } => {
                    self.apply_tableau_targets(instruction.gate(), target_group, tableau)?;
                }
                ControlledPauliTargetPair::Classical { control, target } => {
                    let active = match control {
                        ClassicalControl::Record(offset) => {
                            measurement_record_bit(&self.measurements, offset)?
                        }
                        ClassicalControl::Sweep(id) => mode.sweep_bit(id),
                    };
                    if active {
                        self.apply_pauli(qubit_id_index(target)?, basis)?;
                    }
                }
                ControlledPauliTargetPair::ClassicalNoop { .. } => {}
                ControlledPauliTargetPair::Unsupported => {
                    return Err(unsupported_frame_instruction(instruction));
                }
            }
        }
        Ok(())
    }

    fn apply_tableau_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        tableau: Option<&FrameTableauTransform>,
    ) -> DetectionResult<()> {
        let targets = instruction.targets();
        let group_size = match instruction.gate().target_group_kind() {
            GateTargetGroupKind::Singles => 1,
            GateTargetGroupKind::Pairs => 2,
            GateTargetGroupKind::AllTargets if targets.is_empty() => return Ok(()),
            GateTargetGroupKind::AllTargets => targets.len(),
            GateTargetGroupKind::None if targets.is_empty() => return Ok(()),
            GateTargetGroupKind::None | GateTargetGroupKind::PauliProducts => {
                return Err(unsupported_frame_instruction(instruction));
            }
        };
        for target_group in targets.chunks(group_size) {
            self.apply_tableau_targets(instruction.gate(), target_group, tableau)?;
        }
        Ok(())
    }

    fn apply_tableau_targets(
        &mut self,
        gate: Gate,
        targets: &[Target],
        transform: Option<&FrameTableauTransform>,
    ) -> DetectionResult<()> {
        let gate_name = gate.canonical_name();
        const MAX_LOCAL_TABLEAU_QUBITS: usize = 8;
        let transform = transform.ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(format!(
                "gate {gate_name} has no compiled detector-frame tableau"
            ))
        })?;
        if targets.len() != transform.target_count() {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "gate {gate_name} frame transform expected {} targets but got {}",
                transform.target_count(),
                targets.len()
            )));
        }
        let mut qubits = [0_usize; MAX_LOCAL_TABLEAU_QUBITS];
        let mut input_bases = [PauliBasis::I; MAX_LOCAL_TABLEAU_QUBITS];
        for ((qubit_slot, basis_slot), target) in
            qubits.iter_mut().zip(input_bases.iter_mut()).zip(targets)
        {
            let qubit = target
                .qubit_id()
                .ok_or_else(|| unsupported_frame_target(gate_name, target))?;
            let qubit = usize::try_from(qubit.get()).map_err(|_| {
                DetectionError::invalid_sampler_compilation(format!(
                    "qubit target {} cannot fit in this platform's usize",
                    qubit.get()
                ))
            })?;
            *qubit_slot = qubit;
            *basis_slot = self.qubit_basis(qubit)?;
        }
        let admitted_bases = input_bases.get(..targets.len()).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau exceeded its inline target storage",
            )
        })?;
        let output = transform.output_mask(admitted_bases).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau rejected its admitted target count",
            )
        })?;
        for (output_index, qubit) in qubits.iter().copied().take(targets.len()).enumerate() {
            self.set_x_bit(qubit, ((output >> output_index) & 1) != 0)?;
            self.set_z_bit(
                qubit,
                ((output >> (MAX_LOCAL_TABLEAU_QUBITS + output_index)) & 1) != 0,
            )?;
        }
        Ok(())
    }

    fn apply_single_pauli_noise(
        &mut self,
        instruction: &CircuitInstruction,
        probabilities: [f64; 3],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return Ok(());
        }
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return Ok(());
        }
        for target_group in instruction.targets().chunks(2) {
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            if !else_branch {
                self.correlated_error_occurred = false;
            }
            return Ok(());
        }
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let probability = single_probability_argument(instruction)?.get();
        if !mode.samples_noise() {
            self.measurements
                .extend(std::iter::repeat_n(false, instruction.targets().len()));
            return Ok(());
        }
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let probabilities = probability_list::<4>(instruction)?;
        if !mode.samples_noise() {
            self.measurements
                .extend(std::iter::repeat_n(false, instruction.targets().len()));
            return Ok(());
        }
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => {
                self.set_z_bit(qubit, false)?;
                self.set_x_bit(qubit, mode.random_bool(rng, 0.5))?;
            }
            PauliBasis::Y => {
                let bit = mode.random_bool(rng, 0.5);
                self.set_z_bit(qubit, bit)?;
                self.set_x_bit(qubit, bit)?;
            }
            PauliBasis::Z => {
                self.set_x_bit(qubit, false)?;
                self.set_z_bit(qubit, mode.random_bool(rng, 0.5))?;
            }
        }
        Ok(())
    }

    fn measure_qubit_frame(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<bool> {
        let result = self.frame_measurement_bit(qubit, basis)?;
        self.randomize_measured_basis(qubit, basis, rng, mode)?;
        Ok(result)
    }

    fn frame_measurement_bit(&self, qubit: usize, basis: PauliBasis) -> DetectionResult<bool> {
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
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => self.set_x_bit(qubit, mode.random_bool(rng, 0.5))?,
            PauliBasis::Y => {
                let result = self.x_bit(qubit)? ^ self.z_bit(qubit)?;
                let z = mode.random_bool(rng, 0.5);
                self.set_z_bit(qubit, z)?;
                self.set_x_bit(qubit, result ^ z)?;
            }
            PauliBasis::Z => self.set_z_bit(qubit, mode.random_bool(rng, 0.5))?,
        }
        Ok(())
    }

    fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) -> DetectionResult<()> {
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

    fn pauli_target_frame_bit(&self, target: &Target) -> DetectionResult<bool> {
        let qubit = target.qubit_id().ok_or_else(|| {
            DetectionError::invalid_result_format(format!(
                "OBSERVABLE_INCLUDE Pauli target {target} has no qubit id"
            ))
        })?;
        let qubit = usize::try_from(qubit.get()).map_err(|_| {
            DetectionError::invalid_result_format(format!(
                "qubit target {} cannot fit in this platform's usize",
                qubit.get()
            ))
        })?;
        match target.pauli_type() {
            Some(Pauli::X) => self.z_bit(qubit),
            Some(Pauli::Y) => Ok(self.x_bit(qubit)? ^ self.z_bit(qubit)?),
            Some(Pauli::Z) => self.x_bit(qubit),
            None => Err(DetectionError::invalid_result_format(format!(
                "OBSERVABLE_INCLUDE target {target} is not a Pauli target"
            ))),
        }
    }

    fn qubit_basis(&self, qubit: usize) -> DetectionResult<PauliBasis> {
        Ok(PauliBasis::from_xz(self.x_bit(qubit)?, self.z_bit(qubit)?))
    }

    fn x_bit(&self, qubit: usize) -> DetectionResult<bool> {
        frame_bit(&self.xs, qubit)
    }

    fn z_bit(&self, qubit: usize) -> DetectionResult<bool> {
        frame_bit(&self.zs, qubit)
    }

    fn set_x_bit(&mut self, qubit: usize, value: bool) -> DetectionResult<()> {
        set_frame_bit(&mut self.xs, qubit, value)
    }

    fn set_z_bit(&mut self, qubit: usize, value: bool) -> DetectionResult<()> {
        set_frame_bit(&mut self.zs, qubit, value)
    }

    fn xor_x_bit(&mut self, qubit: usize, value: bool) -> DetectionResult<()> {
        xor_frame_bit(&mut self.xs, qubit, value)
    }

    fn xor_z_bit(&mut self, qubit: usize, value: bool) -> DetectionResult<()> {
        xor_frame_bit(&mut self.zs, qubit, value)
    }
}
