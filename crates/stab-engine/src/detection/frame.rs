use rand::{Rng, RngExt as _};
use stab_algebra::PauliBasis;
use stab_model::Cancellation;

use super::error::{DetectionError, DetectionResult};
use super::try_vec_with_capacity;

mod clifford;
#[cfg(test)]
mod clifford_tests;
mod helpers;
mod noise;
mod plan;
mod program;
mod word;

use helpers::{TWO_QUBIT_FRAME_BASES, measurement_record_word, try_zero_words};
pub(in crate::detection) use noise::batch_active_mask;
use noise::{
    FrameExecutionMode, for_each_set_lane, sample_categorical_masks,
    visit_sparse_categorical_events,
};

pub(super) use plan::{
    DetectorFrameState, DirectDetectorFramePlan, SweepCorrectionPlan,
    admit_combined_compiled_storage,
};
pub(crate) use plan::{PAULI_FRAME_BATCH_SHOTS, PauliFrameSamplingState};
pub(crate) use program::{
    CircuitProgram, CliffordStep, ControlledPair, FrameGate, PauliProductMeasurement, ProgramEntry,
    ProgramInstruction, ProgramOptions, ReferenceClifford,
};
pub(in crate::detection) use program::{TraversalEvent, visit_circuit};
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
        program: &CircuitProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        let mut cursor = program.cursor(cancellation);
        while let Some(instruction) = cursor.next_instruction()? {
            self.execute_instruction(instruction, rng, mode)?;
        }
        Ok(())
    }

    fn execute_instruction(
        &mut self,
        instruction: &ProgramInstruction,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        match instruction {
            ProgramInstruction::Clifford { frame, targets, .. } => match frame {
                FrameGate::Identity => Ok(()),
                FrameGate::Single(map) => self.apply_single_qubit_map(*map, targets),
                FrameGate::Two(map) => self.apply_two_qubit_map(*map, targets),
            },
            ProgramInstruction::Reset {
                basis: PauliBasis::Z,
                targets,
            } => {
                for &qubit in targets {
                    self.set_x_word(qubit, W::default())?;
                    self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
                }
                Ok(())
            }
            ProgramInstruction::Reset { basis, targets } => {
                for &qubit in targets {
                    self.reset_qubit(qubit, *basis, rng, mode)?;
                }
                Ok(())
            }
            ProgramInstruction::Measure {
                basis: PauliBasis::Z,
                reset,
                flip_probability,
                targets,
                ..
            } => self.measure_z(targets, *reset, *flip_probability, rng, mode),
            ProgramInstruction::Measure {
                basis,
                reset,
                flip_probability,
                targets,
                ..
            } => {
                for &qubit in targets {
                    let result = self.measure_qubit_frame(qubit, *basis, rng, mode)?
                        ^ mode.random_mask(rng, *flip_probability);
                    self.measurements.push(result);
                    if *reset {
                        self.reset_qubit(qubit, *basis, rng, mode)?;
                    }
                }
                Ok(())
            }
            ProgramInstruction::MeasurePairs {
                basis,
                flip_probability,
                targets,
                ..
            } => {
                let (pairs, unpaired) = targets.as_chunks::<2>();
                if !unpaired.is_empty() {
                    return Err(DetectionError::invalid_sampler_compilation(
                        "compiled pair measurement target list was not paired",
                    ));
                }
                for &[left, right] in pairs {
                    if left == right {
                        self.measure_pauli_product_terms(&[], *flip_probability, rng, mode)?;
                    } else {
                        self.measure_pauli_product_terms(
                            &[(left, *basis), (right, *basis)],
                            *flip_probability,
                            rng,
                            mode,
                        )?;
                    }
                }
                Ok(())
            }
            ProgramInstruction::MeasurePads {
                flip_probability,
                values,
            } => {
                for _ in values {
                    self.measurements
                        .push(mode.random_mask(rng, *flip_probability));
                }
                Ok(())
            }
            ProgramInstruction::PauliProducts(measurement) => {
                self.measure_pauli_products(measurement, rng, mode)
            }
            ProgramInstruction::SinglePauliNoise {
                probabilities,
                targets,
            } => self.apply_resolved_single_pauli_noise(targets, *probabilities, rng, mode),
            ProgramInstruction::TwoQubitPauliNoise {
                probabilities,
                targets,
            } => self.apply_resolved_two_qubit_pauli_noise(targets, **probabilities, rng, mode),
            ProgramInstruction::CorrelatedError {
                else_branch,
                probability,
                terms,
            } => self.apply_correlated_error(terms, *else_branch, *probability, rng, mode),
            ProgramInstruction::HeraldedErase {
                probability,
                targets,
            } => self.apply_heralded_erase(targets, *probability, rng, mode),
            ProgramInstruction::HeraldedPauliChannel {
                probabilities,
                targets,
            } => self.apply_heralded_pauli_channel(targets, *probabilities, rng, mode),
            ProgramInstruction::ControlledPauli {
                basis,
                frame,
                pairs,
                ..
            } => self.apply_controlled_pauli(*basis, *frame, pairs, mode),
            ProgramInstruction::Observable { id, terms } => {
                self.record_pauli_observable(*id, terms)
            }
        }
    }

    fn measure_z(
        &mut self,
        targets: &[usize],
        reset: bool,
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        let measurement_start = self.measurements.len();
        for &qubit in targets {
            self.measurements.push(self.x_word(qubit)?);
            if reset {
                self.set_x_word(qubit, W::default())?;
            }
            self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
        }
        if mode.samples_noise()
            && !visit_sparse_categorical_events(
                [probability],
                targets.len(),
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
                .take(targets.len())
            {
                *measurement ^= mode.random_mask(rng, probability);
            }
        }
        Ok(())
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
        let (pairs, unpaired) = targets.as_chunks::<2>();
        if !unpaired.is_empty() {
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
        for &[left, right] in pairs {
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

    fn measure_pauli_products(
        &mut self,
        measurement: &PauliProductMeasurement,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        for product in measurement.products() {
            let (terms, _) = product?;
            self.measure_pauli_product_terms(terms, measurement.flip_probability(), rng, mode)?;
        }
        Ok(())
    }

    fn record_pauli_observable(
        &mut self,
        observable_id: usize,
        terms: &[(usize, PauliBasis)],
    ) -> DetectionResult<()> {
        if self.observables.get(observable_id).is_none() {
            return Err(DetectionError::invalid_result_format(format!(
                "observable id {observable_id} was not initialized"
            )));
        }
        let mut bit = W::default();
        for &(qubit, basis) in terms {
            bit ^= self.frame_measurement_bit(qubit, basis)?;
        }
        let observable = self.observables.get_mut(observable_id).ok_or_else(|| {
            DetectionError::invalid_result_format(format!(
                "observable id {observable_id} was not initialized"
            ))
        })?;
        *observable ^= bit;
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
        basis: PauliBasis,
        map: clifford::TwoQubitFrameMap,
        pairs: &[ControlledPair],
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        for pair in pairs {
            match *pair {
                ControlledPair::Quantum { control, target } => {
                    self.apply_two_qubit_map(map, &[control, target])?;
                }
                ControlledPair::Record { offset, qubit } => {
                    let active = measurement_record_word(&self.measurements, offset)?;
                    self.apply_pauli_mask(qubit, basis, active)?;
                }
                ControlledPair::Sweep { id, qubit } => {
                    self.apply_pauli_mask(qubit, basis, mode.sweep_mask(id))?;
                }
            }
        }
        Ok(())
    }

    fn apply_correlated_error(
        &mut self,
        terms: &[(usize, PauliBasis)],
        else_branch: bool,
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !else_branch {
            self.correlated_error_occurred = W::default();
        }
        if !mode.samples_noise() {
            return Ok(());
        }
        let eligible = mode.active_mask::<W>() & !self.correlated_error_occurred;
        let occurred = mode.random_mask::<W>(rng, probability) & eligible;
        self.correlated_error_occurred |= occurred;
        for &(qubit, basis) in terms {
            self.apply_pauli_mask(qubit, basis, occurred)?;
        }
        Ok(())
    }

    fn apply_heralded_erase(
        &mut self,
        targets: &[usize],
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            self.measurements
                .extend(std::iter::repeat_n(W::default(), targets.len()));
            return Ok(());
        }
        for &qubit in targets {
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
        targets: &[usize],
        probabilities: [f64; 4],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            self.measurements
                .extend(std::iter::repeat_n(W::default(), targets.len()));
            return Ok(());
        }
        for &qubit in targets {
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
}
