use rand::{Rng, RngExt as _};
use stab_algebra::PauliBasis;
use stab_model::{
    CircuitInstruction, Gate, GateTargetGroupKind, Pauli, Target,
    advanced::{
        ClassicalControl, ControlledPauliTargetPair, classify_controlled_pauli_target_pair,
    },
};

use super::error::{DetectionError, DetectionResult};
use super::try_vec_with_capacity;

mod helpers;
mod noise;
mod plan;
mod program;
mod word;

use helpers::{
    TWO_QUBIT_FRAME_BASES, measurement_flip_probability, measurement_record_word, pauli_basis,
    probability_list, qubit_id_index, qubit_index, single_probability_argument, try_zero_words,
    unsupported_frame_instruction, unsupported_frame_target, zero_probability_noise,
};
pub(in crate::detection) use noise::batch_active_mask;
use noise::{
    FrameExecutionMode, for_each_set_lane, sample_categorical_masks,
    visit_sparse_categorical_events,
};

pub(super) use plan::{
    DetectorFrameState, DirectDetectorFramePlan, SweepCorrectionPlan,
    admit_combined_compiled_storage,
};
pub(crate) use plan::{PAULI_FRAME_BATCH_SHOTS, PauliFrameSamplingPlan, PauliFrameSamplingState};
use program::{
    FastFrameInstruction, FastFrameOperation, FrameInstruction, FrameProgram, FrameTableauTransform,
};
use word::FrameWord;

#[derive(Debug)]
pub(in crate::detection) struct BitPlaneDetectionFrame<W: FrameWord = u64> {
    xs: Vec<W>,
    zs: Vec<W>,
    pub(in crate::detection) measurements: Vec<W>,
    pub(in crate::detection) observables: Vec<W>,
    correlated_error_occurred: W,
}

impl<W: FrameWord> BitPlaneDetectionFrame<W> {
    pub(in crate::detection) fn try_reusable(
        qubit_count: usize,
        measurement_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            xs: try_zero_words(qubit_count, "detection frame X state")?,
            zs: try_zero_words(qubit_count, "detection frame Z state")?,
            measurements: try_vec_with_capacity(
                measurement_count,
                "detection frame measurement record",
            )?,
            observables: try_zero_words(observable_count, "detection frame observable record")?,
            correlated_error_occurred: W::default(),
        })
    }

    fn reset(&mut self, rng: &mut impl Rng, mode: FrameExecutionMode<'_>) {
        self.xs.fill(W::default());
        for bit in &mut self.zs {
            *bit = mode.random_mask(rng, 0.5);
        }
        self.measurements.clear();
        self.observables.fill(W::default());
        self.correlated_error_occurred = W::default();
    }

    fn execute_program(
        &mut self,
        program: &FrameProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let mut cursor = program.cursor();
        while let Some(step) = cursor.next_instruction()? {
            match step {
                FrameInstruction::Execute {
                    instruction,
                    tableau,
                } => self.execute_instruction(instruction, tableau, rng, mode)?,
                FrameInstruction::Fast(instruction) => {
                    self.execute_fast_instruction(instruction, rng, mode)?
                }
            }
        }
        Ok(())
    }

    fn execute_fast_instruction(
        &mut self,
        instruction: &FastFrameInstruction,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        match instruction.operation {
            FastFrameOperation::Hadamard => {
                for &qubit in &instruction.targets {
                    let x = self.x_word(qubit)?;
                    let z = self.z_word(qubit)?;
                    self.set_x_word(qubit, z)?;
                    self.set_z_word(qubit, x)?;
                }
                Ok(())
            }
            FastFrameOperation::ControlledNot => {
                for targets in instruction.targets.chunks_exact(2) {
                    let &[control, target] = targets else {
                        return Err(DetectionError::invalid_sampler_compilation(
                            "resolved CX target list was not paired",
                        ));
                    };
                    let control_x = self.x_word(control)?;
                    let target_z = self.z_word(target)?;
                    self.xor_x_word(target, control_x)?;
                    self.xor_z_word(control, target_z)?;
                }
                Ok(())
            }
            FastFrameOperation::Tableau => {
                let transform = instruction.tableau.ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled tableau operation omitted its transform",
                    )
                })?;
                let group_size = transform.target_count();
                if group_size == 0 {
                    return instruction.targets.is_empty().then_some(()).ok_or_else(|| {
                        DetectionError::invalid_sampler_compilation(
                            "compiled zero-width tableau retained qubit targets",
                        )
                    });
                }
                let groups = instruction.targets.chunks_exact(group_size);
                if !groups.remainder().is_empty() {
                    return Err(DetectionError::invalid_sampler_compilation(
                        "compiled tableau target list was not grouped",
                    ));
                }
                for targets in groups {
                    self.apply_resolved_tableau_targets(targets, transform)?;
                }
                Ok(())
            }
            FastFrameOperation::ResetZ => {
                for &qubit in &instruction.targets {
                    self.set_x_word(qubit, W::default())?;
                    self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
                }
                Ok(())
            }
            FastFrameOperation::MeasureZ | FastFrameOperation::MeasureResetZ => {
                let probability = instruction.probabilities.first().copied().ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled Z measurement omitted its flip probability",
                    )
                })?;
                let measurement_start = self.measurements.len();
                for &qubit in &instruction.targets {
                    self.measurements.push(self.x_word(qubit)?);
                    if instruction.operation == FastFrameOperation::MeasureResetZ {
                        self.set_x_word(qubit, W::default())?;
                    }
                    self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
                }
                if mode.samples_noise()
                    && !visit_sparse_categorical_events(
                        [probability],
                        instruction.targets.len(),
                        mode.active_mask(),
                        rng,
                        |target_index, _, lane_mask| {
                            let measurement = self
                                .measurements
                                .get_mut(measurement_start + target_index)
                                .ok_or_else(|| {
                                    DetectionError::invalid_sampler_compilation(
                                        "compiled Z measurement escaped its output record",
                                    )
                                })?;
                            *measurement ^= lane_mask;
                            Ok(())
                        },
                    )?
                {
                    for measurement in self
                        .measurements
                        .iter_mut()
                        .skip(measurement_start)
                        .take(instruction.targets.len())
                    {
                        *measurement ^= mode.random_mask(rng, probability);
                    }
                }
                Ok(())
            }
            FastFrameOperation::MeasureXX
            | FastFrameOperation::MeasureYY
            | FastFrameOperation::MeasureZZ => {
                let probability = instruction.probabilities.first().copied().ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled pair measurement omitted its flip probability",
                    )
                })?;
                let basis = match instruction.operation {
                    FastFrameOperation::MeasureXX => PauliBasis::X,
                    FastFrameOperation::MeasureYY => PauliBasis::Y,
                    FastFrameOperation::MeasureZZ => PauliBasis::Z,
                    _ => {
                        return Err(DetectionError::invalid_sampler_compilation(
                            "compiled pair measurement used a non-pair operation",
                        ));
                    }
                };
                let pairs = instruction.targets.chunks_exact(2);
                if !pairs.remainder().is_empty() {
                    return Err(DetectionError::invalid_sampler_compilation(
                        "compiled pair measurement target list was not paired",
                    ));
                }
                for pair in pairs {
                    let &[left, right] = pair else {
                        return Err(DetectionError::invalid_sampler_compilation(
                            "compiled pair measurement target list was not paired",
                        ));
                    };
                    if left == right {
                        self.measure_pauli_product_terms(&[], probability, rng, mode)?;
                    } else {
                        self.measure_pauli_product_terms(
                            &[(left, basis), (right, basis)],
                            probability,
                            rng,
                            mode,
                        )?;
                    }
                }
                Ok(())
            }
            FastFrameOperation::SinglePauliNoise => {
                let probabilities: [f64; 3] = instruction
                    .probabilities
                    .as_slice()
                    .try_into()
                    .map_err(|_| {
                        DetectionError::invalid_sampler_compilation(
                            "compiled single-qubit noise has the wrong probability count",
                        )
                    })?;
                self.apply_resolved_single_pauli_noise(
                    &instruction.targets,
                    probabilities,
                    rng,
                    mode,
                )
            }
            FastFrameOperation::TwoQubitPauliNoise => {
                let probabilities: [f64; 15] = instruction
                    .probabilities
                    .as_slice()
                    .try_into()
                    .map_err(|_| {
                        DetectionError::invalid_sampler_compilation(
                            "compiled two-qubit noise has the wrong probability count",
                        )
                    })?;
                self.apply_resolved_two_qubit_pauli_noise(
                    &instruction.targets,
                    probabilities,
                    rng,
                    mode,
                )
            }
        }
    }

    fn apply_resolved_single_pauli_noise(
        &mut self,
        targets: &[usize],
        probabilities: [f64; 3],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return Ok(());
        }
        if visit_sparse_categorical_events(
            probabilities,
            targets.len(),
            mode.active_mask(),
            rng,
            |target_index, category, lane_mask| {
                let qubit = targets.get(target_index).copied().ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled single-qubit noise target is out of range",
                    )
                })?;
                let basis = match category {
                    0 => PauliBasis::X,
                    1 => PauliBasis::Y,
                    2 => PauliBasis::Z,
                    _ => {
                        return Err(DetectionError::invalid_sampler_compilation(
                            "compiled single-qubit noise category is out of range",
                        ));
                    }
                };
                self.apply_pauli_mask(qubit, basis, lane_mask)
            },
        )? {
            return Ok(());
        }
        for &qubit in targets {
            let [x_mask, y_mask, z_mask] =
                sample_categorical_masks(probabilities, mode.active_mask(), rng);
            self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
        }
        Ok(())
    }

    fn apply_resolved_two_qubit_pauli_noise(
        &mut self,
        targets: &[usize],
        probabilities: [f64; 15],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return Ok(());
        }
        let pairs = targets.chunks_exact(2);
        if !pairs.remainder().is_empty() {
            return Err(DetectionError::invalid_sampler_compilation(
                "compiled two-qubit noise target list is not paired",
            ));
        }
        if visit_sparse_categorical_events(
            probabilities,
            pairs.len(),
            mode.active_mask(),
            rng,
            |target_index, category, lane_mask| {
                let start = target_index.checked_mul(2).ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled two-qubit noise target index overflowed",
                    )
                })?;
                let &[left, right] = targets.get(start..start + 2).ok_or_else(|| {
                    DetectionError::invalid_sampler_compilation(
                        "compiled two-qubit noise target is out of range",
                    )
                })?
                else {
                    return Err(DetectionError::invalid_sampler_compilation(
                        "compiled two-qubit noise target list is not paired",
                    ));
                };
                let (left_basis, right_basis) = TWO_QUBIT_FRAME_BASES
                    .get(category)
                    .copied()
                    .ok_or_else(|| {
                        DetectionError::invalid_sampler_compilation(
                            "compiled two-qubit noise category is out of range",
                        )
                    })?;
                if let Some(basis) = left_basis {
                    self.apply_pauli_mask(left, basis, lane_mask)?;
                }
                if let Some(basis) = right_basis {
                    self.apply_pauli_mask(right, basis, lane_mask)?;
                }
                Ok(())
            },
        )? {
            return Ok(());
        }
        for pair in targets.chunks_exact(2) {
            let &[left, right] = pair else {
                return Err(DetectionError::invalid_sampler_compilation(
                    "compiled two-qubit noise target list is not paired",
                ));
            };
            let masks = sample_categorical_masks(probabilities, mode.active_mask(), rng);
            for ((left_basis, right_basis), mask) in TWO_QUBIT_FRAME_BASES.into_iter().zip(masks) {
                if let Some(basis) = left_basis {
                    self.apply_pauli_mask(left, basis, mask)?;
                }
                if let Some(basis) = right_basis {
                    self.apply_pauli_mask(right, basis, mask)?;
                }
            }
        }
        Ok(())
    }

    fn execute_instruction(
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
        let mut bit = W::default();
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
        let observable = self.observables.get_mut(observable_id).ok_or_else(|| {
            DetectionError::invalid_result_format(format!(
                "observable id {observable_id} was not initialized"
            ))
        })?;
        *observable ^= bit;
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
                ^ mode.random_mask(rng, flip_probability);
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
                .push(mode.random_mask(rng, flip_probability));
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
        let mut result = mode.random_mask(rng, flip_probability);
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
        let mask = mode.random_mask(rng, 0.5);
        for (qubit, basis) in terms {
            match basis {
                PauliBasis::I => {}
                PauliBasis::X => self.xor_x_word(*qubit, mask)?,
                PauliBasis::Z => self.xor_z_word(*qubit, mask)?,
                PauliBasis::Y => {
                    self.xor_x_word(*qubit, mask)?;
                    self.xor_z_word(*qubit, mask)?;
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
                            measurement_record_word(&self.measurements, offset)?
                        }
                        ClassicalControl::Sweep(id) => mode.sweep_mask(id),
                    };
                    self.apply_pauli_mask(qubit_id_index(target)?, basis, active)?;
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
        let mut input_xs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        let mut input_zs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        for (((qubit_slot, x_slot), z_slot), target) in qubits
            .iter_mut()
            .zip(input_xs.iter_mut())
            .zip(input_zs.iter_mut())
            .zip(targets)
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
            *x_slot = self.x_word(qubit)?;
            *z_slot = self.z_word(qubit)?;
        }
        let admitted_xs = input_xs.get(..targets.len()).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau exceeded its inline target storage",
            )
        })?;
        let admitted_zs = input_zs.get(..targets.len()).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau exceeded its inline target storage",
            )
        })?;
        let (output_xs, output_zs) = transform
            .apply_word_planes(admitted_xs, admitted_zs)
            .ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled detector-frame tableau rejected its admitted target count",
                )
            })?;
        for (output_index, qubit) in qubits.iter().copied().take(targets.len()).enumerate() {
            let output_x = output_xs.get(output_index).copied().ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled detector-frame X output escaped inline storage",
                )
            })?;
            let output_z = output_zs.get(output_index).copied().ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled detector-frame Z output escaped inline storage",
                )
            })?;
            self.set_x_word(qubit, output_x)?;
            self.set_z_word(qubit, output_z)?;
        }
        Ok(())
    }

    fn apply_resolved_tableau_targets(
        &mut self,
        targets: &[usize],
        transform: FrameTableauTransform,
    ) -> DetectionResult<()> {
        const MAX_LOCAL_TABLEAU_QUBITS: usize = 8;
        if targets.len() != transform.target_count() {
            return Err(DetectionError::invalid_sampler_compilation(
                "compiled tableau transform rejected its resolved target count",
            ));
        }
        let mut input_xs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        let mut input_zs = [W::default(); MAX_LOCAL_TABLEAU_QUBITS];
        for ((x_slot, z_slot), &qubit) in input_xs.iter_mut().zip(input_zs.iter_mut()).zip(targets)
        {
            *x_slot = self.x_word(qubit)?;
            *z_slot = self.z_word(qubit)?;
        }
        let admitted_xs = input_xs.get(..targets.len()).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau exceeded its inline target storage",
            )
        })?;
        let admitted_zs = input_zs.get(..targets.len()).ok_or_else(|| {
            DetectionError::invalid_sampler_compilation(
                "compiled detector-frame tableau exceeded its inline target storage",
            )
        })?;
        let (output_xs, output_zs) = transform
            .apply_word_planes(admitted_xs, admitted_zs)
            .ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled tableau transform rejected resolved word planes",
                )
            })?;
        for (index, &qubit) in targets.iter().enumerate() {
            let output_x = output_xs.get(index).copied().ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled detector-frame X output escaped inline storage",
                )
            })?;
            let output_z = output_zs.get(index).copied().ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "compiled detector-frame Z output escaped inline storage",
                )
            })?;
            self.set_x_word(qubit, output_x)?;
            self.set_z_word(qubit, output_z)?;
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
        let targets = instruction.targets();
        if visit_sparse_categorical_events(
            probabilities,
            targets.len(),
            mode.active_mask(),
            rng,
            |target_index, category, lane_mask| {
                let target = targets
                    .get(target_index)
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                let qubit = qubit_index(instruction, target)?;
                let basis = match category {
                    0 => PauliBasis::X,
                    1 => PauliBasis::Y,
                    2 => PauliBasis::Z,
                    _ => return Err(unsupported_frame_instruction(instruction)),
                };
                self.apply_pauli_mask(qubit, basis, lane_mask)
            },
        )? {
            return Ok(());
        }
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let [x_mask, y_mask, z_mask] =
                sample_categorical_masks(probabilities, mode.active_mask(), rng);
            self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
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
        let target_groups = instruction.targets().chunks_exact(2);
        if !target_groups.remainder().is_empty() {
            return Err(unsupported_frame_instruction(instruction));
        }
        if visit_sparse_categorical_events(
            probabilities,
            target_groups.len(),
            mode.active_mask(),
            rng,
            |target_index, category, lane_mask| {
                let start = target_index
                    .checked_mul(2)
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                let end = start
                    .checked_add(2)
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                let target_group = instruction
                    .targets()
                    .get(start..end)
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                let [left, right] = target_group else {
                    return Err(unsupported_frame_instruction(instruction));
                };
                let left = qubit_index(instruction, left)?;
                let right = qubit_index(instruction, right)?;
                let (left_basis, right_basis) = TWO_QUBIT_FRAME_BASES
                    .get(category)
                    .copied()
                    .ok_or_else(|| unsupported_frame_instruction(instruction))?;
                if let Some(basis) = left_basis {
                    self.apply_pauli_mask(left, basis, lane_mask)?;
                }
                if let Some(basis) = right_basis {
                    self.apply_pauli_mask(right, basis, lane_mask)?;
                }
                Ok(())
            },
        )? {
            return Ok(());
        }
        for target_group in instruction.targets().chunks(2) {
            let [left, right] = target_group else {
                return Err(unsupported_frame_instruction(instruction));
            };
            let left = qubit_index(instruction, left)?;
            let right = qubit_index(instruction, right)?;
            let masks = sample_categorical_masks(probabilities, mode.active_mask(), rng);
            for ((left_basis, right_basis), mask) in TWO_QUBIT_FRAME_BASES.into_iter().zip(masks) {
                if let Some(basis) = left_basis {
                    self.apply_pauli_mask(left, basis, mask)?;
                }
                if let Some(basis) = right_basis {
                    self.apply_pauli_mask(right, basis, mask)?;
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
                self.correlated_error_occurred = W::default();
            }
            return Ok(());
        }
        if !else_branch {
            self.correlated_error_occurred = W::default();
        }
        let eligible = mode.active_mask::<W>() & !self.correlated_error_occurred;
        let occurred =
            mode.random_mask::<W>(rng, single_probability_argument(instruction)?.get()) & eligible;
        self.correlated_error_occurred |= occurred;
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
            self.apply_pauli_mask(
                qubit_index(instruction, target)?,
                pauli_basis(pauli),
                occurred,
            )?;
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
            self.measurements.extend(std::iter::repeat_n(
                W::default(),
                instruction.targets().len(),
            ));
            return Ok(());
        }
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let occurred = mode.random_mask(rng, probability);
            self.measurements.push(occurred);
            let mut x_mask = W::default();
            let mut y_mask = W::default();
            let mut z_mask = W::default();
            for_each_set_lane(occurred, |lane_mask| match rng.random::<u8>() & 3 {
                1 => x_mask |= lane_mask,
                2 => z_mask |= lane_mask,
                3 => y_mask |= lane_mask,
                _ => {}
            });
            self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
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
            self.measurements.extend(std::iter::repeat_n(
                W::default(),
                instruction.targets().len(),
            ));
            return Ok(());
        }
        for target in instruction.targets() {
            let qubit = qubit_index(instruction, target)?;
            let [identity_mask, x_mask, y_mask, z_mask] =
                sample_categorical_masks(probabilities, mode.active_mask(), rng);
            let occurred = identity_mask | x_mask | y_mask | z_mask;
            self.measurements.push(occurred);
            self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
            self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
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
                self.set_z_word(qubit, W::default())?;
                self.set_x_word(qubit, mode.random_mask(rng, 0.5))?;
            }
            PauliBasis::Y => {
                let bit = mode.random_mask(rng, 0.5);
                self.set_z_word(qubit, bit)?;
                self.set_x_word(qubit, bit)?;
            }
            PauliBasis::Z => {
                self.set_x_word(qubit, W::default())?;
                self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
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
    ) -> DetectionResult<W> {
        let result = self.frame_measurement_bit(qubit, basis)?;
        self.randomize_measured_basis(qubit, basis, rng, mode)?;
        Ok(result)
    }

    fn frame_measurement_bit(&self, qubit: usize, basis: PauliBasis) -> DetectionResult<W> {
        match basis {
            PauliBasis::I => Ok(W::default()),
            PauliBasis::X => self.z_word(qubit),
            PauliBasis::Y => Ok(self.x_word(qubit)? ^ self.z_word(qubit)?),
            PauliBasis::Z => self.x_word(qubit),
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
            PauliBasis::X => self.set_x_word(qubit, mode.random_mask(rng, 0.5))?,
            PauliBasis::Y => {
                let result = self.x_word(qubit)? ^ self.z_word(qubit)?;
                let z = mode.random_mask(rng, 0.5);
                self.set_z_word(qubit, z)?;
                self.set_x_word(qubit, result ^ z)?;
            }
            PauliBasis::Z => self.set_z_word(qubit, mode.random_mask(rng, 0.5))?,
        }
        Ok(())
    }

    fn apply_pauli_mask(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        mask: W,
    ) -> DetectionResult<()> {
        match basis {
            PauliBasis::I => {}
            PauliBasis::X => self.xor_x_word(qubit, mask)?,
            PauliBasis::Y => {
                self.xor_x_word(qubit, mask)?;
                self.xor_z_word(qubit, mask)?;
            }
            PauliBasis::Z => self.xor_z_word(qubit, mask)?,
        }
        Ok(())
    }

    fn pauli_target_frame_bit(&self, target: &Target) -> DetectionResult<W> {
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
            Some(Pauli::X) => self.z_word(qubit),
            Some(Pauli::Y) => Ok(self.x_word(qubit)? ^ self.z_word(qubit)?),
            Some(Pauli::Z) => self.x_word(qubit),
            None => Err(DetectionError::invalid_result_format(format!(
                "OBSERVABLE_INCLUDE target {target} is not a Pauli target"
            ))),
        }
    }
}
