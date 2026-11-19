use stab_model::{DemTarget, DemTargetKind, Pauli, QubitId};

use crate::target_xor_set::TargetXorSet;
use crate::{AnalysisError, AnalysisResult};

use super::{
    Anticommutation, AnticommutationPolicy, SparseReverseFrameTracker, TrackerBasis,
    TrackerLocation,
};

impl SparseReverseFrameTracker {
    pub(crate) fn new_for_error_analysis(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: u64,
        allow_gauge_detectors: bool,
    ) -> Self {
        let mut tracker = Self::new(
            qubit_count,
            measurement_count,
            detector_count,
            if allow_gauge_detectors {
                AnticommutationPolicy::Record
            } else {
                AnticommutationPolicy::Fail
            },
        );
        tracker.error_analysis_mode = true;
        tracker.eliminate_detector_gauges = allow_gauge_detectors;
        tracker
    }

    /// Returns gauge errors recorded since the last call; usually none.
    pub(crate) fn take_gauge_errors(&mut self) -> Vec<TargetXorSet> {
        let taken = std::mem::take(&mut self.gauge_errors);
        if !taken.is_empty() {
            self.sync_live_bytes();
        }
        taken
    }

    pub(crate) fn has_gauge_errors(&self) -> bool {
        !self.gauge_errors.is_empty()
    }

    /// Writes the sorted symptoms of a Pauli error on `qubit` into `out`.
    pub(crate) fn error_sensitivity_into(
        &self,
        qubit: QubitId,
        pauli: Pauli,
        out: &mut Vec<DemTarget>,
    ) -> AnalysisResult<()> {
        self.anticommuting_sensitivity_into(qubit, TrackerBasis::from_pauli(pauli), out)
    }

    pub(super) fn check_measurement_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.check_gauge(qubit, basis)
    }

    pub(super) fn check_product_measurement_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
    ) -> AnalysisResult<()> {
        let gauge = self.product_anticommuting_sensitivity(terms)?;
        if gauge.is_empty() {
            self.scratch.recycle(gauge);
            return Ok(());
        }
        self.check_product_gauge(terms, gauge)
    }

    pub(super) fn check_reset_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.check_gauge(qubit, basis)
    }

    fn check_gauge(&mut self, qubit: QubitId, basis: TrackerBasis) -> AnalysisResult<()> {
        let mut gauge = self.scratch.take_operand();
        self.anticommuting_sensitivity_into(qubit, basis, &mut gauge)?;
        if gauge.is_empty() {
            self.scratch.operand = gauge;
            return Ok(());
        }
        let mut owned = self.scratch.take_set();
        owned.assign_sorted(&gauge);
        self.scratch.operand = gauge;
        self.check_product_gauge(&[(qubit, basis)], owned)
    }

    fn check_product_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
        gauge: TargetXorSet,
    ) -> AnalysisResult<()> {
        if gauge.is_empty() {
            return Ok(());
        }
        let has_observables = gauge.iter().any(|target| target.is_observable());
        if self.eliminate_detector_gauges && !has_observables {
            self.eliminate_detector_gauge(&gauge)?;
            self.gauge_errors.push(gauge);
            self.sync_live_bytes();
            return Ok(());
        }
        if self.anticommutation_policy == AnticommutationPolicy::Fail
            || self.eliminate_detector_gauges
        {
            if self.error_analysis_mode {
                // Vendor check_for_gauge (error_analyzer.cc:408-437): gauge
                // observables are never eliminated even with gauge detectors
                // allowed, and allowing gauge detectors suppresses only the
                // detector line of the failure message.
                let has_detectors = !self.eliminate_detector_gauges
                    && gauge.iter().any(|target| target.is_detector());
                let mut message = String::new();
                if has_observables {
                    message.push_str("The circuit contains non-deterministic observables.\n");
                }
                if has_detectors {
                    message.push_str("The circuit contains non-deterministic detectors.\n");
                }
                message.push_str("The collapse anti-commuted with these detectors/observables:");
                for target in &gauge {
                    message.push_str("\n    ");
                    message.push_str(&target.to_string());
                }
                return Err(AnalysisError::invalid_detector_error_model(message));
            }
            let mut message = String::from("collapse anti-commuted with tracked targets:");
            for target in &gauge {
                message.push_str("\n    ");
                message.push_str(&target.to_string());
            }
            return Err(AnalysisError::invalid_detector_error_model(message));
        }
        self.check_live_budget_with_extra(super::budget::anticommutation_bytes(
            terms.len().saturating_mul(gauge.len()),
        ))?;
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
        self.sync_live_bytes();
        Ok(())
    }

    fn eliminate_detector_gauge(&mut self, gauge: &TargetXorSet) -> AnalysisResult<()> {
        let Some(pivot) = gauge
            .iter()
            .filter_map(|target| match target.kind() {
                DemTargetKind::RelativeDetector(detector) => Some((detector, *target)),
                DemTargetKind::LogicalObservable(_) | DemTargetKind::Separator => None,
            })
            .max_by_key(|(detector, _)| *detector)
            .map(|(_, target)| target)
        else {
            return Ok(());
        };
        // Vendor ErrorAnalyzer::remove_gauge (error_analyzer.cc:226-242) rewrites only the
        // qubit frames. Pending measurement-record sensitivities keep the eliminated pivot, so a
        // record merged later reintroduces it exactly as pinned Stim does.
        // Each rewritten set merges through the scratch buffer and keeps the merged storage, so
        // every one may grow to under twice its merged length.
        let growth = self
            .xs
            .values()
            .chain(self.zs.values())
            .filter(|targets| targets.contains(&pivot))
            .fold(0_u64, |sum, targets| {
                sum.saturating_add(super::sets::unknown_buffer_growth_bytes(
                    targets.len().saturating_add(gauge.len()),
                ))
            });
        self.scratch.admit_growth(&self.meter, growth)?;
        let merge = &mut self.scratch.merge;
        for targets in self.xs.values_mut().chain(self.zs.values_mut()) {
            if targets.contains(&pivot) {
                targets.xor_sorted(gauge.as_slice(), merge);
            }
        }
        self.xs.retain(|_, targets| !targets.is_empty());
        self.zs.retain(|_, targets| !targets.is_empty());
        Ok(())
    }
}
