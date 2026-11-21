//! Undoing measurements, resets, and measurement pads: each pops its record's sensitivity and
//! merges it back into the measured Paulis, checking for gauges in pinned Stim's target order.

use stab_model::{CircuitInstruction, QubitId};

use super::budget::set_entry_bytes;
use super::pauli_product::pauli_product_terms_reversed;
use super::{SparseReverseFrameTracker, TrackerBasis, held_set_bytes};
use crate::target_xor_set::TargetXorSet;
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, instruction_shape};

const OPERATION: AnalysisOperation = AnalysisOperation::ReverseFrameTracking;

impl SparseReverseFrameTracker {
    pub(super) fn undo_measure_resets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.ensure_measurements_available(
            instruction.targets().len(),
            instruction.gate().canonical_name(),
        )?;
        // Vendor ErrorAnalyzer::undo_MR* resolves each target completely (reset gauge, clear,
        // record merge) before the previous target; the post-merge measurement gauge check is
        // vacuous because the cleared frame only holds the measured basis.
        for qubit in instruction_shape::single_qubits(instruction, OPERATION)?.rev() {
            self.check_reset_gauge(qubit, basis)?;
            self.clear_qubit(qubit)?;
            self.undo_measurement_target(qubit, basis)?;
            self.check_live_budget()?;
        }
        Ok(())
    }

    pub(super) fn undo_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.ensure_measurements_available(
            instruction.targets().len(),
            instruction.gate().canonical_name(),
        )?;
        // Vendor ErrorAnalyzer::undo_M*_with_context merges each target's record before
        // checking that target for a gauge, one target at a time in reverse, so each check sees
        // the merged records and eliminated gauges of the targets after it.
        for qubit in instruction_shape::single_qubits(instruction, OPERATION)?.rev() {
            self.undo_measurement_target(qubit, basis)?;
            self.check_measurement_gauge(qubit, basis)?;
            self.check_live_budget()?;
        }
        Ok(())
    }

    pub(super) fn undo_pair_measurements(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let pairs = instruction_shape::measured_qubit_pairs(instruction, OPERATION)?;
        self.ensure_measurements_available(pairs.len(), instruction.gate().canonical_name())?;
        // Vendor undo_M{XX,YY,ZZ} resolve one pair at a time in reverse order, so a later
        // pair's record is merged before an earlier, possibly overlapping pair is checked.
        for [left, right] in pairs.rev() {
            self.undo_product_measurement_term(&[(left.id, basis), (right.id, basis)])?;
        }
        Ok(())
    }

    pub(super) fn undo_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let terms = pauli_product_terms_reversed(instruction)?;
        self.ensure_measurements_available(terms.len(), instruction.gate().canonical_name())?;
        // Vendor undo_MPP resolves one product at a time in reverse order, so a later product's
        // record is merged before an earlier, possibly anticommuting product is checked.
        for term in terms {
            self.undo_product_measurement_term(&term)?;
        }
        Ok(())
    }

    /// Merges one measured product's record into its Paulis, then checks it for a gauge.
    ///
    /// The merge cannot change the product's own anticommuting sensitivity, but it must
    /// precede the check so gauge elimination acts on the merged frame like vendor
    /// `check_for_gauge` after `undo_MZ_with_context`'s record merge.
    fn undo_product_measurement_term(
        &mut self,
        term: &[(QubitId, TrackerBasis)],
    ) -> AnalysisResult<()> {
        let sensitivity = self.pop_record_sensitivity()?;
        let merged = self.holding(held_set_bytes(&sensitivity), |tracker| {
            tracker.toggle_product_sensitivity(term, sensitivity.as_slice())
        });
        self.scratch.recycle(sensitivity);
        merged?;
        self.check_product_measurement_gauge(term)?;
        self.check_live_budget()
    }

    fn undo_measurement_target(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        let sensitivity = self.pop_record_sensitivity()?;
        let result = self.holding(held_set_bytes(&sensitivity), |tracker| {
            tracker.toggle_product_sensitivity(&[(qubit, basis)], sensitivity.as_slice())
        });
        self.scratch.recycle(sensitivity);
        result
    }

    pub(super) fn undo_measurement_pads(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        self.ensure_measurements_available(instruction.targets().len(), "MPAD")?;
        for _ in instruction.targets().iter().rev() {
            let sensitivity = self.pop_record_sensitivity()?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    pub(super) fn undo_heralded_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        self.ensure_measurements_available(
            instruction.targets().len(),
            instruction.gate().canonical_name(),
        )?;
        for _ in instruction.targets().iter().rev() {
            let sensitivity = self.pop_record_sensitivity()?;
            self.scratch.recycle(sensitivity);
        }
        Ok(())
    }

    pub(crate) fn pop_record_sensitivity(&mut self) -> AnalysisResult<TargetXorSet> {
        self.measurement_count = self.measurement_count.checked_sub(1).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::ReverseFrameTracking,
                "measurement count underflowed during sparse reverse tracking",
            )
        })?;
        let removed = self.rec_bits.remove(&self.measurement_count);
        self.meter.adjust(set_entry_bytes(removed.as_ref()), 0);
        Ok(removed.unwrap_or_default())
    }

    pub(super) fn undo_resets(
        &mut self,
        instruction: &CircuitInstruction,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        for qubit in instruction_shape::single_qubits(instruction, OPERATION)?.rev() {
            self.check_reset_gauge(qubit, basis)?;
            self.clear_qubit(qubit)?;
        }
        Ok(())
    }
}
