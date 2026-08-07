use std::collections::BTreeSet;

use stab_model::{DemTarget, Pauli, QubitId};

use crate::{AnalysisError, AnalysisResult};

use super::{
    Anticommutation, SparseReverseFrameTracker, TrackerBasis, TrackerLocation, toggle_targets,
};

impl SparseReverseFrameTracker {
    pub fn new_for_error_analysis(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: u64,
        allow_gauge_detectors: bool,
    ) -> Self {
        let mut tracker = Self::new(
            qubit_count,
            measurement_count,
            detector_count,
            !allow_gauge_detectors,
        );
        tracker.error_analysis_mode = true;
        tracker.eliminate_detector_gauges = allow_gauge_detectors;
        tracker
    }

    pub fn take_gauge_errors(&mut self) -> Vec<BTreeSet<DemTarget>> {
        std::mem::take(&mut self.gauge_errors)
    }

    pub fn error_sensitivity(
        &self,
        qubit: QubitId,
        pauli: Pauli,
    ) -> AnalysisResult<BTreeSet<DemTarget>> {
        self.anticommuting_sensitivity(qubit, TrackerBasis::from_pauli(pauli))
    }

    pub(super) fn check_measurement_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.check_gauge(qubit, basis, self.anticommuting_sensitivity(qubit, basis)?)
    }

    pub(super) fn check_product_measurement_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
    ) -> AnalysisResult<()> {
        let mut gauge = BTreeSet::new();
        for (qubit, basis) in terms {
            toggle_targets(
                &mut gauge,
                self.anticommuting_sensitivity(*qubit, *basis)?
                    .iter()
                    .copied(),
            );
        }
        self.check_product_gauge(terms, gauge)
    }

    pub(super) fn check_reset_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
    ) -> AnalysisResult<()> {
        self.check_gauge(qubit, basis, self.anticommuting_sensitivity(qubit, basis)?)
    }

    fn check_gauge(
        &mut self,
        qubit: QubitId,
        basis: TrackerBasis,
        gauge: BTreeSet<DemTarget>,
    ) -> AnalysisResult<()> {
        self.check_product_gauge(&[(qubit, basis)], gauge)
    }

    fn check_product_gauge(
        &mut self,
        terms: &[(QubitId, TrackerBasis)],
        gauge: BTreeSet<DemTarget>,
    ) -> AnalysisResult<()> {
        if gauge.is_empty() {
            return Ok(());
        }
        let has_observables = gauge
            .iter()
            .any(|target| matches!(target, DemTarget::LogicalObservable(_)));
        if self.eliminate_detector_gauges && !has_observables {
            self.eliminate_detector_gauge(&gauge);
            self.gauge_errors.push(gauge);
            return Ok(());
        }
        if self.fail_on_anticommute || self.eliminate_detector_gauges {
            if self.error_analysis_mode {
                // Vendor check_for_gauge (error_analyzer.cc:408-437): gauge
                // observables are never eliminated even with gauge detectors
                // allowed, and allowing gauge detectors suppresses only the
                // detector line of the failure message.
                let has_detectors = !self.eliminate_detector_gauges
                    && gauge
                        .iter()
                        .any(|target| matches!(target, DemTarget::RelativeDetector(_)));
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
        Ok(())
    }

    fn eliminate_detector_gauge(&mut self, gauge: &BTreeSet<DemTarget>) {
        let Some(pivot) = gauge
            .iter()
            .filter_map(|target| match target {
                DemTarget::RelativeDetector(detector) => Some((*detector, *target)),
                DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                    None
                }
            })
            .max_by_key(|(detector, _)| *detector)
            .map(|(_, target)| target)
        else {
            return;
        };
        for targets in self.xs.values_mut() {
            if targets.contains(&pivot) {
                toggle_targets(targets, gauge.iter().copied());
            }
        }
        for targets in self.zs.values_mut() {
            if targets.contains(&pivot) {
                toggle_targets(targets, gauge.iter().copied());
            }
        }
        for targets in self.rec_bits.values_mut() {
            if targets.contains(&pivot) {
                toggle_targets(targets, gauge.iter().copied());
            }
        }
        for targets in self.observable_effects.values_mut() {
            if targets.contains(&pivot) {
                toggle_targets(targets, gauge.iter().copied());
            }
        }
        self.xs.retain(|_, targets| !targets.is_empty());
        self.zs.retain(|_, targets| !targets.is_empty());
        self.rec_bits.retain(|_, targets| !targets.is_empty());
        self.observable_effects
            .retain(|_, targets| !targets.is_empty());
    }
}
