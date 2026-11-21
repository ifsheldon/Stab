//! Error recording: turns each noise channel, noisy measurement, and correlated-error chain
//! into independent error classes over the tracker's current sensitivities.

use stab_model::{
    CircuitInstruction, DemTarget, GateKind, Pauli, PauliTerms, Probability, QubitId,
};

use super::ReverseFoldAnalyzer;
use super::local_decomposition::{
    locally_decompose_combinations, merge_indistinguishable_disjoint_probabilities,
};
use crate::circuit_to_dem::error_accumulator::{ErrorAccumulator, TagId};
use crate::circuit_to_dem::error_decomp::{
    depolarize1_independent_channel_probability, depolarize2_independent_channel_probability,
};
use crate::circuit_to_dem::try_disjoint_to_independent_xyz_errors;
use crate::instruction_shape;
use crate::target_xor_set::xor_merge_into;
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ThresholdExcess};

const OPERATION: AnalysisOperation = AnalysisOperation::CircuitToDetectorErrorModel;

impl ReverseFoldAnalyzer<'_> {
    pub(super) fn record_measurement_errors(
        &mut self,
        instruction: &CircuitInstruction,
        measurement_count: usize,
    ) -> AnalysisResult<()> {
        let Some(probability) = instruction.probability() else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let end = self.tracker.measurement_count();
        let start = end.checked_sub(measurement_count).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                format!(
                    "{} consumes more measurements than remain during folded error analysis",
                    instruction.gate().canonical_name()
                ),
            )
        })?;
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        for index in (start..end).rev() {
            let targets = self.tracker.record_targets_slice_at(index)?;
            add_independent_error(&mut self.errors, probability, targets, tag)?;
        }
        Ok(())
    }

    pub(super) fn record_single_pauli_errors(
        &mut self,
        instruction: &CircuitInstruction,
        pauli: Pauli,
    ) -> AnalysisResult<()> {
        let probability = instruction_shape::probability(instruction, OPERATION)?;
        let qubits = instruction_shape::qubits(instruction, OPERATION)?;
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        for qubit in qubits {
            self.tracker
                .error_sensitivity_into(qubit, pauli, &mut self.scratch.sensitivity)?;
            add_independent_error(
                &mut self.errors,
                probability,
                &self.scratch.sensitivity,
                tag,
            )?;
        }
        Ok(())
    }

    /// Records a reverse-encountered `E`, folding any stacked
    /// `ELSE_CORRELATED_ERROR` chain into disjoint components exactly like
    /// pinned Stim's `correlated_error_block` (error_analyzer.cc:784-809).
    pub(super) fn record_correlated_error_block(
        &mut self,
        leading: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        if self.stacked_else_correlated.is_empty() {
            let probability = instruction_shape::probability(leading, OPERATION)?;
            return self.record_correlated_error(leading, probability);
        }
        let mut stacked = std::mem::take(&mut self.stacked_else_correlated);
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(AnalysisError::NoiseRequiresApproximation {
                gate: GateKind::ELSE_CORRELATED_ERROR,
                excess: None,
            });
        };
        let mut remaining = 1.0_f64;
        for &instruction in std::iter::once(&leading).chain(stacked.iter().rev()) {
            let probability = instruction_shape::probability(instruction, OPERATION)?;
            let actual = probability.get() * remaining;
            remaining *= 1.0 - probability.get();
            // Both factors lie in [0, 1], so the component is a probability.
            let actual = Probability::try_new(actual).map_err(|_| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "ELSE_CORRELATED_ERROR chain produced an invalid component probability",
                )
            })?;
            if actual.get() > threshold.get() {
                return Err(AnalysisError::NoiseRequiresApproximation {
                    gate: GateKind::ELSE_CORRELATED_ERROR,
                    excess: Some(ThresholdExcess::new(actual, threshold)),
                });
            }
            self.record_correlated_error(instruction, actual)?;
        }
        stacked.clear();
        self.stacked_else_correlated = stacked;
        Ok(())
    }

    fn record_correlated_error(
        &mut self,
        instruction: &CircuitInstruction,
        probability: Probability,
    ) -> AnalysisResult<()> {
        self.composite_error_targets(instruction_shape::pauli_terms(instruction, OPERATION)?)?;
        let tag = self.errors.intern_tag(instruction.tag_bytes())?;
        add_independent_error(
            &mut self.errors,
            probability,
            self.scratch.composite.as_slice(),
            tag,
        )
    }

    /// Port of pinned Stim's `undo_HERALDED_ERASE` (error_analyzer.cc:321-340):
    /// each heralded record combines the herald symptom with the qubit's X and
    /// Z sensitivity regions under four equal disjoint erase channels.
    pub(super) fn record_heralded_erase(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let argument = instruction_shape::probability(instruction, OPERATION)?;
        self.validate_disjoint_threshold(GateKind::HERALDED_ERASE, &[argument], false)?;
        let channel = Probability::try_new(argument.get() * 0.25).map_err(|_| {
            AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                "HERALDED_ERASE channel probability is invalid",
            )
        })?;
        let identity = Probability::try_new((1.0 - argument.get()).max(0.0)).map_err(|_| {
            AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                "HERALDED_ERASE identity probability is invalid",
            )
        })?;
        self.record_heralded_channels(instruction, [identity, channel, channel, channel, channel])
    }

    /// Port of pinned Stim's `undo_HERALDED_PAULI_CHANNEL_1`
    /// (error_analyzer.cc:342-365) with the vendor slot order `hi, hz, hx, hy`.
    pub(super) fn record_heralded_pauli_channel1(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let probabilities = instruction_shape::probabilities::<4>(instruction, OPERATION)?;
        let [hi, hx, hy, hz] = probabilities;
        self.validate_disjoint_threshold(GateKind::HERALDED_PAULI_CHANNEL_1, &probabilities, true)?;
        let identity =
            Probability::try_new((1.0 - hi.get() - hx.get() - hy.get() - hz.get()).max(0.0))
                .map_err(|_| {
                    AnalysisError::internal(
                        AnalysisOperation::CircuitToDetectorErrorModel,
                        "HERALDED_PAULI_CHANNEL_1 identity probability is invalid",
                    )
                })?;
        self.record_heralded_channels(instruction, [identity, hi, hz, hx, hy])
    }

    /// Shared heralded-record recorder: `channels` holds the no-herald
    /// identity slot followed by the herald-only, herald-plus-Z-sensitivity,
    /// herald-plus-X-sensitivity, and herald-plus-both slots, matching the
    /// vendor basis order `{xs, zs, herald}`.
    fn record_heralded_channels(
        &mut self,
        instruction: &CircuitInstruction,
        channels: [Probability; 5],
    ) -> AnalysisResult<()> {
        let zero = Probability::ZERO;
        for qubit in instruction_shape::qubits(instruction, OPERATION)?.rev() {
            let herald = self.tracker.pop_record_sensitivity()?;
            self.load_bases(&[(qubit, Pauli::Z), (qubit, Pauli::X)])?;
            let herald_basis = self.scratch.bases.get_mut(2).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "heralded error basis slot is out of range",
                )
            })?;
            herald_basis.clear();
            herald_basis.extend_from_slice(herald.as_slice());
            self.record_error_combinations(
                &mut [
                    channels[0],
                    zero,
                    zero,
                    zero,
                    channels[1],
                    channels[2],
                    channels[3],
                    channels[4],
                ],
                3,
                true,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    pub(super) fn record_depolarize1(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let probability = instruction_shape::probability(instruction, OPERATION)?;
        let independent = depolarize1_independent_channel_probability(probability)?;
        for qubit in instruction_shape::qubits(instruction, OPERATION)? {
            self.load_bases(&[(qubit, Pauli::Z), (qubit, Pauli::X)])?;
            self.record_error_combinations(
                &mut [Probability::ZERO, independent, independent, independent],
                2,
                false,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    pub(super) fn record_depolarize2(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let probability = instruction_shape::probability(instruction, OPERATION)?;
        let independent = depolarize2_independent_channel_probability(probability)?;
        for [left, right] in instruction_shape::qubit_pairs(instruction, OPERATION)? {
            let mut probabilities = [independent; 16];
            let [first, ..] = &mut probabilities;
            *first = Probability::ZERO;
            self.load_bases(&[
                (left, Pauli::Z),
                (left, Pauli::X),
                (right, Pauli::Z),
                (right, Pauli::X),
            ])?;
            self.record_error_combinations(&mut probabilities, 4, false, instruction.tag_bytes())?;
        }
        Ok(())
    }

    /// Port of pinned Stim's `undo_PAULI_CHANNEL_1`
    /// (`error_analyzer.cc:901-929`), loading `{zs, xs}` (`X` then `Z` Pauli
    /// sensitivities) with slot order `{0, ix, iz, iy}` so local decomposition
    /// and fused probability merges follow the vendor order.
    pub(super) fn record_pauli_channel1(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let probabilities = instruction_shape::probabilities::<3>(instruction, OPERATION)?;
        let [x, y, z] = probabilities;
        let independent = try_disjoint_to_independent_xyz_errors(x, y, z)?;
        let (ix, iy, iz, is_disjoint) = if let Some(independent) = independent {
            (independent.x, independent.y, independent.z, false)
        } else {
            self.validate_disjoint_threshold(GateKind::PAULI_CHANNEL_1, &probabilities, true)?;
            (x, y, z, true)
        };
        let zero = Probability::ZERO;
        for qubit in instruction_shape::qubits(instruction, OPERATION)? {
            self.load_bases(&[(qubit, Pauli::X), (qubit, Pauli::Z)])?;
            self.record_error_combinations(
                &mut [zero, ix, iz, iy],
                2,
                is_disjoint,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    /// Port of pinned Stim's `undo_PAULI_CHANNEL_2`
    /// (`error_analyzer.cc:931-956`), mapping the 15 `IX..ZZ` arguments onto
    /// the `{zs[b], xs[b], zs[a], xs[a]}` basis masks via `pauli_xyz_to_xz`.
    pub(super) fn record_pauli_channel2(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let probabilities = instruction_shape::probabilities::<15>(instruction, OPERATION)?;
        // Pinned Stim validates before looking at the targets, so a target-less channel with two
        // or more non-zero components is refused without `approximate_disjoint_errors` too.
        self.validate_disjoint_threshold(GateKind::PAULI_CHANNEL_2, &probabilities, true)?;
        if instruction.targets().is_empty() {
            return Ok(());
        }
        let mut table = [Probability::ZERO; 16];
        for (index, probability) in probabilities.into_iter().enumerate() {
            let k = index + 1;
            let mask = pauli_xyz_to_xz(k & 3) | (pauli_xyz_to_xz((k >> 2) & 3) << 2);
            if let Some(slot) = table.get_mut(mask) {
                *slot = probability;
            }
        }
        for [left, right] in instruction_shape::qubit_pairs(instruction, OPERATION)? {
            let mut slot_probabilities = table;
            self.load_bases(&[
                (right, Pauli::X),
                (right, Pauli::Z),
                (left, Pauli::X),
                (left, Pauli::Z),
            ])?;
            self.record_error_combinations(
                &mut slot_probabilities,
                4,
                true,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    /// Port of pinned Stim's `check_can_approximate_disjoint` (`error_analyzer.cc:865-899`).
    ///
    /// With `allow_single_component`, a channel with at most one non-zero component is already an
    /// independent error, so it needs neither `approximate_disjoint_errors` nor the threshold.
    fn validate_disjoint_threshold(
        &self,
        gate: GateKind,
        probabilities: &[Probability],
        allow_single_component: bool,
    ) -> AnalysisResult<()> {
        if allow_single_component
            && probabilities
                .iter()
                .filter(|probability| probability.get() > 0.0)
                .nth(1)
                .is_none()
        {
            return Ok(());
        }
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(AnalysisError::NoiseRequiresApproximation { gate, excess: None });
        };
        if let Some(probability) = probabilities
            .iter()
            .find(|probability| probability.get() > threshold.get())
        {
            return Err(AnalysisError::NoiseRequiresApproximation {
                gate,
                excess: Some(ThresholdExcess::new(*probability, threshold)),
            });
        }
        Ok(())
    }

    /// Loads the symptom sets of single-qubit Pauli errors into the leading basis slots.
    fn load_bases(&mut self, errors: &[(QubitId, Pauli)]) -> AnalysisResult<()> {
        for (index, &(qubit, pauli)) in errors.iter().enumerate() {
            let basis = self.scratch.bases.get_mut(index).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error basis index is out of range",
                )
            })?;
            self.tracker.error_sensitivity_into(qubit, pauli, basis)?;
        }
        Ok(())
    }

    /// Records every combination of the first `basis_count` loaded basis errors, where
    /// `probabilities[mask]` belongs to the XOR of the bases selected by `mask`.
    fn record_error_combinations(
        &mut self,
        probabilities: &mut [Probability],
        basis_count: usize,
        probabilities_are_disjoint: bool,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let combination_count = 1_usize
            .checked_shl(u32::try_from(basis_count).map_err(|_| {
                AnalysisError::count_overflow(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error basis count does not fit u32",
                )
            })?)
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error combination count overflowed",
                )
            })?;
        if probabilities.len() != combination_count {
            return Err(AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                format!(
                    "composite error expected {combination_count} probabilities, got {}",
                    probabilities.len()
                ),
            ));
        }
        let bases = self.scratch.bases.get(..basis_count).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                "composite error basis index is out of range",
            )
        })?;
        let combinations = &mut self.scratch.combinations;
        if combinations.len() < combination_count {
            combinations.resize_with(combination_count, Vec::new);
        }
        let combinations = combinations.get_mut(..combination_count).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::CircuitToDetectorErrorModel,
                "composite error target index is out of range",
            )
        })?;
        if let Some(empty) = combinations.first_mut() {
            empty.clear();
        }
        for mask in 1..combination_count {
            let bit = mask.trailing_zeros() as usize;
            let previous = mask & (mask - 1);
            let basis = bases.get(bit).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error basis index is out of range",
                )
            })?;
            let (earlier, later) = combinations.split_at_mut(mask);
            let previous = earlier.get(previous).ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error predecessor is out of range",
                )
            })?;
            let slot = later.first_mut().ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::CircuitToDetectorErrorModel,
                    "composite error target index is out of range",
                )
            })?;
            xor_merge_into(previous, basis, slot);
        }

        if self.options.decompose_errors {
            locally_decompose_combinations(bases, combinations, &mut self.scratch.joined)?;
        }
        if probabilities_are_disjoint {
            merge_indistinguishable_disjoint_probabilities(combinations, probabilities)?;
        }
        // Pinned Stim chains every slot through the accumulated map
        // (`add_error` per combination, error_analyzer.cc:1658-1660); slots
        // sharing one symptom class therefore merge through the same fused
        // `fma(old, 1 - p, (1 - old) * p)` rounding as cross-instruction
        // merges. An earlier pre-combined `l + r - 2lr` subtotal here was a
        // black-box misreading of that contraction.
        let tag = self.errors.intern_tag(tag)?;
        for (probability, targets) in probabilities.iter().zip(combinations.iter()).skip(1) {
            add_independent_error(&mut self.errors, *probability, targets, tag)?;
        }
        Ok(())
    }

    /// Writes the XOR of the symptoms of a correlated error's Pauli targets into
    /// `scratch.composite`.
    ///
    /// Pinned Stim's correlated-error handlers consult only the Pauli X/Z bits, so combiner
    /// targets and inversion bits are ignored decoration (frame_simulator.inl:767-775).
    fn composite_error_targets(&mut self, terms: PauliTerms<'_>) -> AnalysisResult<()> {
        self.scratch.composite.clear();
        for term in terms {
            self.tracker.error_sensitivity_into(
                term.id,
                term.pauli,
                &mut self.scratch.sensitivity,
            )?;
            self.scratch
                .composite
                .xor_sorted(&self.scratch.sensitivity, &mut self.scratch.merge);
        }
        Ok(())
    }
}

const fn pauli_xyz_to_xz(xyz: usize) -> usize {
    xyz ^ (xyz >> 1)
}

pub(super) fn add_independent_error(
    errors: &mut ErrorAccumulator,
    probability: Probability,
    targets: &[DemTarget],
    tag: TagId,
) -> AnalysisResult<()> {
    if probability.get() == 0.0 || targets.is_empty() {
        return Ok(());
    }
    errors.merge_independent(targets, tag, probability)
}
