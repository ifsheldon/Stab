use rand::{Rng, RngExt as _};
use stab_algebra::PauliBasis;
use stab_model::Cancellation;

use super::error::{DetectionError, DetectionResult};
use super::try_vec_with_capacity;
use crate::work_poll::{NOISE_WORD_UNITS, WORD_OP_UNITS, WorkPoll};

mod clifford;
#[cfg(test)]
mod clifford_tests;
mod delivery;
mod helpers;
mod noise;
mod plan;
mod program;
mod window;
mod word;

use helpers::{TWO_QUBIT_FRAME_BASES, measurement_record_word, try_zero_words};
use noise::{
    FrameExecutionMode, for_each_set_lane, sample_categorical_masks,
    visit_sparse_categorical_events,
};

pub(crate) use delivery::{
    DeliveryFailure, DeliveryOutcome, FrameBatchSource, PendingLanes, Refill, sample_or_rewind,
};
pub(super) use plan::{
    DirectDetectorFramePlan, SweepCorrectionPlan, admit_combined_compiled_storage,
};
pub(crate) use plan::{
    FRAME_BATCH_SHOTS, FrameBatchState, LaneFrameBatch, frame_state_storage_bytes,
};
pub(crate) use program::{
    CircuitProgram, CliffordStep, ControlledPair, FrameGate, PauliProductMeasurement, ProgramEntry,
    ProgramInstruction, ProgramOptions, ReferenceClifford, lookback_distance,
};
pub(in crate::detection) use program::{TraversalEvent, visit_circuit};
#[cfg(test)]
pub(crate) use window::with_window_planes;
pub(crate) use window::{
    PlaneConsumer, PlaneWindow, SampledRecords, ShotRecordTransposer, WindowShape, window_planes,
};
pub(crate) use word::FrameWord;

/// The widest lane word: one 256-shot [`stab_bits::BitBlock`] per plane. Sessions at the
/// default [`crate::LaneWidth`] and every measurement-to-detection batch run on it.
pub(crate) type FrameLanes = stab_bits::BitBlock;

/// The lane word each [`crate::LaneWidth`] runs on, checked where both are defined.
const _: () = {
    assert!(<u64 as FrameWord>::LANE_COUNT == crate::LaneWidth::Lanes64.shots_per_batch());
    assert!(size_of::<u64>() == crate::LaneWidth::Lanes64.plane_bytes());
    assert!(<FrameLanes as FrameWord>::LANE_COUNT == crate::LaneWidth::Lanes256.shots_per_batch());
    assert!(size_of::<FrameLanes>() == crate::LaneWidth::Lanes256.plane_bytes());
    assert!(FRAME_BATCH_SHOTS == crate::LaneWidth::WIDEST.shots_per_batch());
};

/// Products with more terms than this are measured in polled chunks of terms.
const LONG_PRODUCT_TERMS: usize = 1 << 12;

/// Work units of `per_word` units on every 64-lane word of one `W` plane.
#[inline(always)]
const fn lane_units<W: FrameWord>(per_word: usize) -> usize {
    per_word * (W::LANE_COUNT / u64::BITS as usize)
}

#[derive(Debug)]
pub(in crate::detection) struct BitPlaneDetectionFrame<W: FrameWord> {
    xs: Vec<W>,
    zs: Vec<W>,
    /// The retained measurement planes: every plane of the batch, or the newest ones when the
    /// batch streams through a [`PlaneWindow`].
    pub(in crate::detection) measurements: Vec<W>,
    /// Absolute measurement index of `measurements[0]`.
    measurement_base: usize,
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
            measurement_base: 0,
            observables: try_zero_words(observable_count, "detection frame observable record")?,
            correlated_error_occurred: W::default(),
        })
    }

    fn reset(
        &mut self,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let units = lane_units::<W>(WORD_OP_UNITS);
        poll.for_chunks_mut(&mut self.xs, units, |part| {
            part.fill(W::default());
            Ok::<_, DetectionError>(())
        })?;
        poll.for_chunks_mut(&mut self.zs, units, |part| {
            for bit in part {
                *bit = mode.random_mask(rng, 0.5);
            }
            Ok::<_, DetectionError>(())
        })?;
        self.measurements.clear();
        self.measurement_base = 0;
        poll.for_chunks_mut(&mut self.observables, units, |part| {
            part.fill(W::default());
            Ok::<_, DetectionError>(())
        })?;
        self.correlated_error_occurred = W::default();
        Ok(())
    }

    fn execute_program(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        let mut cursor = program.cursor(cancellation);
        while let Some(instruction) = cursor.next_instruction()? {
            self.execute_instruction(instruction, rng, mode, cursor.poll())?;
        }
        Ok(())
    }

    /// Executes `program` like [`Self::execute_program`], handing the measurement planes to
    /// `window` whenever it fills and once at the end.
    ///
    /// It is a separate function so the materializing loop compiles exactly as before.
    #[inline(never)]
    fn execute_program_windowed(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        cancellation: &Cancellation,
        window: &mut PlaneWindow<'_, W>,
    ) -> DetectionResult<()> {
        let mut cursor = program.cursor(cancellation);
        while let Some(instruction) = cursor.next_instruction()? {
            self.execute_instruction(instruction, rng, mode, cursor.poll())?;
            if self.measurements.len() >= window.hand_over_at {
                self.measurement_base = window.hand_over(
                    self.measurement_base,
                    &mut self.measurements,
                    false,
                    cursor.poll(),
                )?;
            }
        }
        window.hand_over(
            self.measurement_base,
            &mut self.measurements,
            true,
            cursor.poll(),
        )?;
        Ok(())
    }

    /// Measurement planes recorded by the batch so far, retained or already handed over.
    fn recorded_measurements(&self) -> usize {
        self.measurement_base + self.measurements.len()
    }

    fn execute_instruction(
        &mut self,
        instruction: &ProgramInstruction,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let op = lane_units::<W>(WORD_OP_UNITS);
        let noise = lane_units::<W>(NOISE_WORD_UNITS);
        match instruction {
            ProgramInstruction::Clifford { frame, targets, .. } => match frame {
                FrameGate::Identity => Ok(()),
                FrameGate::Single(map) => poll.for_chunks(targets, op, 1, |_, part| {
                    self.apply_single_qubit_map(*map, part)
                }),
                FrameGate::Two(map) => poll.for_chunks(targets, 2 * op, 2, |_, part| {
                    self.apply_two_qubit_map(*map, part)
                }),
            },
            ProgramInstruction::Reset {
                basis: PauliBasis::Z,
                targets,
            } => poll.for_chunks(targets, op, 1, |_, part| {
                for &qubit in part {
                    self.set_x_word(qubit, W::default())?;
                    self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
                }
                Ok(())
            }),
            ProgramInstruction::Reset { basis, targets } => {
                poll.for_chunks(targets, op, 1, |_, part| {
                    for &qubit in part {
                        self.reset_qubit(qubit, *basis, rng, mode)?;
                    }
                    Ok(())
                })
            }
            ProgramInstruction::Measure {
                basis: PauliBasis::Z,
                reset,
                flip_probability,
                targets,
                ..
            } => self.measure_z(targets, *reset, *flip_probability, rng, mode, poll),
            ProgramInstruction::Measure {
                basis,
                reset,
                flip_probability,
                targets,
                ..
            } => poll.for_chunks(targets, op + noise, 1, |_, part| {
                for &qubit in part {
                    let result = self.measure_qubit_frame(qubit, *basis, rng, mode)?
                        ^ mode.random_mask(rng, *flip_probability);
                    self.measurements.push(result);
                    if *reset {
                        self.reset_qubit(qubit, *basis, rng, mode)?;
                    }
                }
                Ok(())
            }),
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
                poll.for_chunks(pairs, 2 * op + noise, 1, |_, part| {
                    for &[left, right] in part {
                        let terms = [(left, *basis), (right, *basis)];
                        let terms = if left == right { &[][..] } else { &terms[..] };
                        self.measure_pauli_product_terms(terms, *flip_probability, rng, mode)?;
                    }
                    Ok(())
                })
            }
            ProgramInstruction::MeasurePads {
                flip_probability,
                values,
            } => poll.for_chunks(values, noise, 1, |_, part| {
                for _ in part {
                    self.measurements
                        .push(mode.random_mask(rng, *flip_probability));
                }
                Ok(())
            }),
            ProgramInstruction::PauliProducts(measurement) => {
                self.measure_pauli_products(measurement, rng, mode, poll)
            }
            ProgramInstruction::SinglePauliNoise {
                probabilities,
                targets,
            } => self.apply_resolved_single_pauli_noise(targets, *probabilities, rng, mode, poll),
            ProgramInstruction::TwoQubitPauliNoise {
                probabilities,
                targets,
            } => {
                self.apply_resolved_two_qubit_pauli_noise(targets, **probabilities, rng, mode, poll)
            }
            ProgramInstruction::CorrelatedError {
                else_branch,
                probability,
                terms,
            } => self.apply_correlated_error(terms, *else_branch, *probability, rng, mode, poll),
            ProgramInstruction::HeraldedErase {
                probability,
                targets,
            } => self.apply_heralded_erase(targets, *probability, rng, mode, poll),
            ProgramInstruction::HeraldedPauliChannel {
                probabilities,
                targets,
            } => self.apply_heralded_pauli_channel(targets, *probabilities, rng, mode, poll),
            ProgramInstruction::ControlledPauli {
                basis,
                frame,
                pairs,
                ..
            } => poll.for_chunks(pairs, 2 * op, 1, |_, part| {
                self.apply_controlled_pauli(*basis, *frame, part, mode)
            }),
            ProgramInstruction::Observable { id, terms } => {
                poll.for_chunks(terms, op, 1, |_, part| {
                    self.record_pauli_observable(*id, part)
                })
            }
        }
    }

    fn measure_z(
        &mut self,
        targets: &[usize],
        reset: bool,
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let measurement_start = self.measurements.len();
        poll.for_chunks(targets, lane_units::<W>(WORD_OP_UNITS), 1, |_, part| {
            for &qubit in part {
                self.measurements.push(self.x_word(qubit)?);
                if reset {
                    self.set_x_word(qubit, W::default())?;
                }
                self.set_z_word(qubit, mode.random_mask(rng, 0.5))?;
            }
            Ok::<_, DetectionError>(())
        })?;
        if mode.samples_noise()
            && !visit_sparse_categorical_events(
                [probability],
                targets.len(),
                mode.active_mask(),
                rng,
                poll,
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
            poll.for_chunks(
                targets,
                lane_units::<W>(NOISE_WORD_UNITS),
                1,
                |start, part| {
                    let first = measurement_start + start;
                    let records = self
                        .measurements
                        .get_mut(first..first + part.len())
                        .ok_or_else(|| {
                            DetectionError::invalid_sampler_compilation(
                                "compiled Z measurement escaped its output record",
                            )
                        })?;
                    for measurement in records {
                        *measurement ^= mode.random_mask(rng, probability);
                    }
                    Ok::<_, DetectionError>(())
                },
            )?;
        }
        Ok(())
    }

    fn apply_resolved_single_pauli_noise(
        &mut self,
        targets: &[usize],
        probabilities: [f64; 3],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return Ok(());
        }
        if visit_sparse_categorical_events(
            probabilities,
            targets.len(),
            mode.active_mask(),
            rng,
            poll,
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
        self.apply_dense_single_pauli_noise(targets, probabilities, rng, mode, poll)
    }

    /// The per-target path of a single-qubit channel too dense for sparse events, kept out of
    /// line so the sparse path compiles as before.
    #[inline(never)]
    fn apply_dense_single_pauli_noise(
        &mut self,
        targets: &[usize],
        probabilities: [f64; 3],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        poll.for_chunks(
            targets,
            lane_units::<W>(2 * NOISE_WORD_UNITS),
            1,
            |_, part| {
                for &qubit in part {
                    let [x_mask, y_mask, z_mask] =
                        sample_categorical_masks(probabilities, mode.active_mask(), rng);
                    self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
                    self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
                    self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
                }
                Ok(())
            },
        )
    }

    fn apply_resolved_two_qubit_pauli_noise(
        &mut self,
        targets: &[usize],
        probabilities: [f64; 15],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
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
            poll,
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
        self.apply_dense_two_qubit_pauli_noise(pairs, probabilities, rng, mode, poll)
    }

    /// The per-pair path of a two-qubit channel too dense for sparse events, kept out of line
    /// so the sparse path compiles as before.
    #[inline(never)]
    fn apply_dense_two_qubit_pauli_noise(
        &mut self,
        pairs: &[[usize; 2]],
        probabilities: [f64; 15],
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        poll.for_chunks(
            pairs,
            lane_units::<W>(8 * NOISE_WORD_UNITS),
            1,
            |_, part| {
                for &[left, right] in part {
                    let masks = sample_categorical_masks(probabilities, mode.active_mask(), rng);
                    for ((left_basis, right_basis), mask) in
                        TWO_QUBIT_FRAME_BASES.into_iter().zip(masks)
                    {
                        if let Some(basis) = left_basis {
                            self.apply_pauli_mask(left, basis, mask)?;
                        }
                        if let Some(basis) = right_basis {
                            self.apply_pauli_mask(right, basis, mask)?;
                        }
                    }
                }
                Ok(())
            },
        )
    }

    fn measure_pauli_products(
        &mut self,
        measurement: &PauliProductMeasurement,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let op = lane_units::<W>(WORD_OP_UNITS);
        let noise = lane_units::<W>(NOISE_WORD_UNITS);
        for product in measurement.products() {
            let (terms, _) = product?;
            if terms.len() <= LONG_PRODUCT_TERMS {
                poll.charge(noise + terms.len() * op)?;
                self.measure_pauli_product_terms(terms, measurement.flip_probability(), rng, mode)?;
            } else {
                poll.charge(noise)?;
                self.measure_long_pauli_product(
                    terms,
                    measurement.flip_probability(),
                    rng,
                    mode,
                    poll,
                )?;
            }
        }
        Ok(())
    }

    /// Measures one product with more terms than one chunk charges, polling between chunks of
    /// its terms; the random draws and results equal [`Self::measure_pauli_product_terms`].
    #[inline(never)]
    fn measure_long_pauli_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        flip_probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        let op = lane_units::<W>(WORD_OP_UNITS);
        let mut result = mode.random_mask(rng, flip_probability);
        poll.for_chunks(terms, op, 1, |_, part| {
            for (qubit, basis) in part {
                result ^= self.frame_measurement_bit(*qubit, *basis)?;
            }
            Ok::<_, DetectionError>(())
        })?;
        self.measurements.push(result);
        let mask = mode.random_mask(rng, 0.5);
        poll.for_chunks(terms, op, 1, |_, part| {
            self.xor_measured_product(part, mask)
        })
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
        mode: FrameExecutionMode<'_, W>,
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
        mode: FrameExecutionMode<'_, W>,
    ) -> DetectionResult<()> {
        let mask = mode.random_mask(rng, 0.5);
        self.xor_measured_product(terms, mask)
    }

    fn xor_measured_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        mask: W,
    ) -> DetectionResult<()> {
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
        mode: FrameExecutionMode<'_, W>,
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
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        if !else_branch {
            self.correlated_error_occurred = W::default();
        }
        if !mode.samples_noise() {
            return Ok(());
        }
        let eligible = mode.active_mask() & !self.correlated_error_occurred;
        let occurred = mode.random_mask(rng, probability) & eligible;
        self.correlated_error_occurred |= occurred;
        poll.for_chunks(terms, lane_units::<W>(WORD_OP_UNITS), 1, |_, part| {
            for &(qubit, basis) in part {
                self.apply_pauli_mask(qubit, basis, occurred)?;
            }
            Ok(())
        })
    }

    fn apply_heralded_erase(
        &mut self,
        targets: &[usize],
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return self.record_silent_heralds(targets, poll);
        }
        poll.for_chunks(
            targets,
            lane_units::<W>(2 * NOISE_WORD_UNITS),
            1,
            |_, part| self.apply_heralded_erase_chunk(part, probability, rng, mode),
        )
    }

    /// Records the all-false heralds of a channel that sweep correction does not sample.
    fn record_silent_heralds(
        &mut self,
        targets: &[usize],
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        poll.for_chunks(targets, lane_units::<W>(WORD_OP_UNITS), 1, |_, part| {
            self.measurements
                .extend(std::iter::repeat_n(W::default(), part.len()));
            Ok(())
        })
    }

    fn apply_heralded_erase_chunk(
        &mut self,
        targets: &[usize],
        probability: f64,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
    ) -> DetectionResult<()> {
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
        mode: FrameExecutionMode<'_, W>,
        poll: &mut WorkPoll<'_>,
    ) -> DetectionResult<()> {
        if !mode.samples_noise() {
            return self.record_silent_heralds(targets, poll);
        }
        poll.for_chunks(
            targets,
            lane_units::<W>(2 * NOISE_WORD_UNITS),
            1,
            |_, part| {
                for &qubit in part {
                    let [identity_mask, x_mask, y_mask, z_mask] =
                        sample_categorical_masks(probabilities, mode.active_mask(), rng);
                    let occurred = identity_mask | x_mask | y_mask | z_mask;
                    self.measurements.push(occurred);
                    self.apply_pauli_mask(qubit, PauliBasis::X, x_mask)?;
                    self.apply_pauli_mask(qubit, PauliBasis::Y, y_mask)?;
                    self.apply_pauli_mask(qubit, PauliBasis::Z, z_mask)?;
                }
                Ok(())
            },
        )
    }

    fn reset_qubit(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
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
        mode: FrameExecutionMode<'_, W>,
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
        mode: FrameExecutionMode<'_, W>,
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
