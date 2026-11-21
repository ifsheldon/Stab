use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use crate::AnalysisOperation;
use crate::resources::SatMaterializationResource;
use crate::{AnalysisError, AnalysisRequestError, AnalysisResult, ResourceLimitError};
use stab_model::advanced::{
    CancellationCountdown, DemRepeatSelection, DemTraversalState, FoldedDemBlock,
    FoldedDemTraversal, FoldedDemVisitor, shifted_targets,
};
use stab_model::{
    Cancellation, DemDetectorId, DemInstruction, DemInstructionView, DemRepeatBlock, DemTarget,
    DemTargetKind, DetectorErrorModel, ObservableId,
};

mod instance;
mod limits;

use instance::{BoolRef, Clause, MaxSatInstance, SatProblemMode, SatShape, validate_limit};
pub use limits::SatMaterializationLimits;

const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";

#[derive(Clone, Debug, PartialEq)]
struct FlattenedError {
    probability: f64,
    targets: Vec<DemTarget>,
}

#[derive(Clone, Debug)]
struct SatTargetIndex {
    detector_to_slot: BTreeMap<DemDetectorId, usize>,
    observable_to_slot: BTreeMap<ObservableId, usize>,
}

impl SatTargetIndex {
    fn from_errors(errors: &[FlattenedError], cancellation: &Cancellation) -> AnalysisResult<Self> {
        let mut detectors = BTreeSet::new();
        let mut observables = BTreeSet::new();
        let mut countdown = CancellationCountdown::new();
        for error in errors {
            countdown.tick(cancellation)?;
            for target in &error.targets {
                countdown.tick(cancellation)?;
                match target.kind() {
                    DemTargetKind::RelativeDetector(detector) => {
                        detectors.insert(detector);
                    }
                    DemTargetKind::LogicalObservable(observable) => {
                        observables.insert(observable);
                    }
                    DemTargetKind::Separator => {}
                }
            }
        }
        Ok(Self {
            detector_to_slot: detectors
                .into_iter()
                .enumerate()
                .map(|(slot, detector)| (detector, slot))
                .collect(),
            observable_to_slot: observables
                .into_iter()
                .enumerate()
                .map(|(slot, observable)| (observable, slot))
                .collect(),
        })
    }

    fn detector_slot(&self, detector: DemDetectorId) -> AnalysisResult<usize> {
        self.detector_to_slot
            .get(&detector)
            .copied()
            .ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::SatMaterialization,
                    format!(
                        "SAT detector target D{} has no compressed slot",
                        detector.get()
                    ),
                )
            })
    }

    fn observable_slot(&self, observable: ObservableId) -> AnalysisResult<usize> {
        self.observable_to_slot
            .get(&observable)
            .copied()
            .ok_or_else(|| {
                AnalysisError::internal(
                    AnalysisOperation::SatMaterialization,
                    format!(
                        "SAT observable target L{} has no compressed slot",
                        observable.get()
                    ),
                )
            })
    }
}

fn preflight_sat_shape(
    errors: &[FlattenedError],
    target_index: &SatTargetIndex,
    mode: SatProblemMode,
    limits: &SatMaterializationLimits,
) -> AnalysisResult<SatShape> {
    let mut seen_detectors = vec![false; target_index.detector_to_slot.len()];
    let mut seen_observables = vec![false; target_index.observable_to_slot.len()];
    let mut target_occurrences = 0usize;
    let mut xor_count = 0usize;
    let mut soft_clause_count = 0usize;

    let mut countdown = CancellationCountdown::new();
    for error in errors {
        countdown.tick(&limits.cancellation)?;
        if !error_participates_in_constraints(mode, error.probability) {
            continue;
        }
        if soft_clause_is_stored(mode, error.probability) {
            soft_clause_count = checked_sat_add(soft_clause_count, 1, "soft clause count")?;
        }
        for target in &error.targets {
            countdown.tick(&limits.cancellation)?;
            let seen = match target.kind() {
                DemTargetKind::RelativeDetector(detector) => {
                    let slot = target_index.detector_slot(detector)?;
                    seen_detectors.get_mut(slot).ok_or_else(|| {
                        AnalysisError::internal(
                            AnalysisOperation::SatMaterialization,
                            "SAT detector preflight slot is outside its state vector",
                        )
                    })?
                }
                DemTargetKind::LogicalObservable(observable) => {
                    let slot = target_index.observable_slot(observable)?;
                    seen_observables.get_mut(slot).ok_or_else(|| {
                        AnalysisError::internal(
                            AnalysisOperation::SatMaterialization,
                            "SAT observable preflight slot is outside its state vector",
                        )
                    })?
                }
                DemTargetKind::Separator => continue,
            };
            target_occurrences = checked_sat_add(target_occurrences, 1, "target occurrence count")?;
            if *seen {
                xor_count = checked_sat_add(xor_count, 1, "XOR count")?;
            } else {
                *seen = true;
            }
        }
    }

    let active_detectors = seen_detectors.iter().filter(|seen| **seen).count();
    let active_observables = seen_observables.iter().filter(|seen| **seen).count();
    let variables = checked_sat_add(errors.len(), xor_count, "variable count")?;
    let xor_clauses = checked_sat_product(xor_count, 4, "XOR clause count")?;
    let clauses = checked_sat_add(
        checked_sat_add(xor_clauses, soft_clause_count, "soft and XOR clause count")?,
        checked_sat_add(
            active_detectors,
            1,
            "hard detector and observable clause count",
        )?,
        "total clause count",
    )?;
    let clause_literals = checked_sat_add(
        checked_sat_add(
            checked_sat_product(xor_count, 12, "XOR clause literal count")?,
            soft_clause_count,
            "soft and XOR clause literal count",
        )?,
        checked_sat_add(
            active_detectors,
            active_observables,
            "hard target clause literal count",
        )?,
        "total clause literal count",
    )?;

    SatShape {
        error_mechanisms: errors.len(),
        target_occurrences,
        variables,
        clauses,
        clause_literals,
    }
    .validate(limits)
}

/// Generates an unweighted SAT problem under explicit traversal and materialization limits.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the limits' cancellation signal is observed and
/// [`AnalysisError::ResourceLimit`] when a limit is exceeded.
pub fn shortest_error_sat_problem(
    model: &DetectorErrorModel,
    limits: &SatMaterializationLimits,
) -> AnalysisResult<String> {
    sat_problem_as_wcnf_string(model, SatProblemMode::Unweighted, limits)
}

/// Generates a weighted SAT problem under explicit traversal and materialization limits.
///
/// # Errors
///
/// Fails like [`shortest_error_sat_problem`], and with [`AnalysisError::InvalidRequest`] when
/// `quantization` is zero.
pub fn likeliest_error_sat_problem(
    model: &DetectorErrorModel,
    quantization: u32,
    limits: &SatMaterializationLimits,
) -> AnalysisResult<String> {
    if quantization < 1 {
        return Err(AnalysisRequestError::ZeroSatQuantization.into());
    }
    sat_problem_as_wcnf_string(model, SatProblemMode::Weighted { quantization }, limits)
}

fn sat_problem_as_wcnf_string(
    model: &DetectorErrorModel,
    mode: SatProblemMode,
    limits: &SatMaterializationLimits,
) -> AnalysisResult<String> {
    limits.cancellation.check()?;
    if model.count_observables()? == 0 || model.count_errors()? == 0 {
        return unsat_wdimacs(limits);
    }
    let errors = flattened_error_instructions(model, limits)?;
    if errors.is_empty() {
        return unsat_wdimacs(limits);
    }
    let target_index = SatTargetIndex::from_errors(&errors, &limits.cancellation)?;
    if target_index.observable_to_slot.is_empty() {
        return unsat_wdimacs(limits);
    }
    let shape = preflight_sat_shape(&errors, &target_index, mode, limits)?;
    let mut instance = MaxSatInstance::with_shape(shape, limits)?;
    let mut errors_activated = Vec::new();
    errors_activated
        .try_reserve_exact(errors.len())
        .map_err(|_| {
            AnalysisError::allocation_failed(
                AnalysisOperation::SatMaterialization,
                format!(
                    "SAT problem generation cannot reserve {} error variables",
                    errors.len()
                ),
            )
        })?;
    let mut countdown = CancellationCountdown::new();
    for _ in &errors {
        countdown.tick(&limits.cancellation)?;
        errors_activated.push(instance.new_bool()?);
    }

    let mut detectors_activated = vec![BoolRef::false_ref(); target_index.detector_to_slot.len()];
    let mut observables_flipped = vec![BoolRef::false_ref(); target_index.observable_to_slot.len()];
    for (error_index, error) in errors.iter().enumerate() {
        countdown.tick(&limits.cancellation)?;
        let error_ref = errors_activated.get(error_index).copied().ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::SatMaterialization,
                "missing SAT error ref",
            )
        })?;
        if error_participates_in_constraints(mode, error.probability) {
            add_error_parity_terms(
                &mut instance,
                error_ref,
                &error.targets,
                &target_index,
                &mut detectors_activated,
                &mut observables_flipped,
                &limits.cancellation,
                &mut countdown,
            )?;
        }
        add_error_soft_clause(&mut instance, mode, error_ref, error.probability)?;
    }

    for detector in detectors_activated {
        countdown.tick(&limits.cancellation)?;
        if detector.variable_index().is_some() {
            instance.add_clause(Clause::hard(vec![detector.not()]))?;
        }
    }

    let mut observable_clause_vars = Vec::new();
    for observable in observables_flipped {
        countdown.tick(&limits.cancellation)?;
        if observable.variable_index().is_some() {
            observable_clause_vars.push(observable);
        }
    }
    instance.add_clause(Clause::hard(observable_clause_vars))?;
    instance.validate_shape(shape)?;
    instance.to_wdimacs(mode)
}

fn unsat_wdimacs(limits: &SatMaterializationLimits) -> AnalysisResult<String> {
    validate_limit(
        SatMaterializationResource::OutputBytes,
        UNSAT_WDIMACS.len(),
        limits.max_output_bytes,
    )?;
    Ok(UNSAT_WDIMACS.to_owned())
}

#[allow(
    clippy::too_many_arguments,
    reason = "threads the caller cancellation countdown alongside the SAT parity tables"
)]
fn add_error_parity_terms(
    instance: &mut MaxSatInstance,
    error_ref: BoolRef,
    targets: &[DemTarget],
    target_index: &SatTargetIndex,
    detectors_activated: &mut [BoolRef],
    observables_flipped: &mut [BoolRef],
    cancellation: &Cancellation,
    countdown: &mut CancellationCountdown,
) -> AnalysisResult<()> {
    for target in targets {
        countdown.tick(cancellation)?;
        match target.kind() {
            DemTargetKind::RelativeDetector(detector) => {
                let index = target_index.detector_slot(detector)?;
                let current = detectors_activated.get(index).copied().ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::SatMaterialization,
                        format!(
                            "SAT detector target D{} is outside the detector vector",
                            detector.get()
                        ),
                    )
                })?;
                let next = instance.xor(current, error_ref)?;
                let Some(slot) = detectors_activated.get_mut(index) else {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::SatMaterialization,
                        format!(
                            "SAT detector target D{} is outside the detector vector",
                            detector.get()
                        ),
                    ));
                };
                *slot = next;
            }
            DemTargetKind::LogicalObservable(observable) => {
                let index = target_index.observable_slot(observable)?;
                let current = observables_flipped.get(index).copied().ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::SatMaterialization,
                        format!(
                            "SAT observable target L{} is outside the observable vector",
                            observable.get()
                        ),
                    )
                })?;
                let next = instance.xor(current, error_ref)?;
                let Some(slot) = observables_flipped.get_mut(index) else {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::SatMaterialization,
                        format!(
                            "SAT observable target L{} is outside the observable vector",
                            observable.get()
                        ),
                    ));
                };
                *slot = next;
            }
            DemTargetKind::Separator => {}
        }
    }
    Ok(())
}

fn add_error_soft_clause(
    instance: &mut MaxSatInstance,
    mode: SatProblemMode,
    error_ref: BoolRef,
    probability: f64,
) -> AnalysisResult<()> {
    match mode {
        SatProblemMode::Unweighted => instance.add_clause(Clause::soft(error_ref.not(), 1.0)),
        SatProblemMode::Weighted { .. } => {
            if probability <= 0.0 {
                Ok(())
            } else if probability >= 1.0 {
                instance.add_clause(Clause::hard(vec![error_ref]))
            } else if probability < 0.5 {
                let weight = -(probability / (1.0 - probability)).ln();
                instance.add_clause(Clause::soft(error_ref.not(), weight))
            } else if probability == 0.5 {
                Ok(())
            } else {
                let weight = -((1.0 - probability) / probability).ln();
                instance.add_clause(Clause::soft(error_ref, weight))
            }
        }
    }
}

fn soft_clause_is_stored(mode: SatProblemMode, probability: f64) -> bool {
    match mode {
        SatProblemMode::Unweighted => true,
        SatProblemMode::Weighted { .. } => probability > 0.0 && probability != 0.5,
    }
}

fn error_participates_in_constraints(mode: SatProblemMode, probability: f64) -> bool {
    matches!(mode, SatProblemMode::Unweighted) || probability != 0.0
}

fn flattened_error_instructions(
    model: &DetectorErrorModel,
    limits: &SatMaterializationLimits,
) -> AnalysisResult<Vec<FlattenedError>> {
    let traversal = FoldedDemTraversal::new(model)?;
    traversal.validate_repeat_depth("SAT problem generation")?;
    // Complete traversal admission before reserving or mutating flattened error storage.
    let mut admission = SatErrorVisitor::new(limits, None);
    let _ = traversal.try_visit(&mut admission)?;
    let expected_counts = admission.counts;

    let mut errors = Vec::new();
    errors
        .try_reserve_exact(expected_counts.error_mechanisms)
        .map_err(|_| {
            AnalysisError::allocation_failed(
                AnalysisOperation::SatMaterialization,
                "SAT problem generation cannot allocate another error mechanism",
            )
        })?;
    let collected_counts = {
        let mut collector = SatErrorVisitor::new(limits, Some(&mut errors));
        let _ = traversal.try_visit(&mut collector)?;
        collector.counts
    };
    if collected_counts != expected_counts || errors.len() != expected_counts.error_mechanisms {
        return Err(AnalysisError::internal(
            AnalysisOperation::SatMaterialization,
            "SAT traversal shape changed between admission and materialization",
        ));
    }
    Ok(errors)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SatTraversalCounts {
    expanded_instructions: u64,
    error_mechanisms: usize,
    target_occurrences: usize,
}

struct SatErrorVisitor<'a> {
    limits: &'a SatMaterializationLimits,
    counts: SatTraversalCounts,
    errors: Option<&'a mut Vec<FlattenedError>>,
    countdown: CancellationCountdown,
}

impl SatErrorVisitor<'_> {
    fn new<'a>(
        limits: &'a SatMaterializationLimits,
        errors: Option<&'a mut Vec<FlattenedError>>,
    ) -> SatErrorVisitor<'a> {
        SatErrorVisitor {
            limits,
            counts: SatTraversalCounts::default(),
            errors,
            countdown: CancellationCountdown::new(),
        }
    }

    fn add_expanded_instruction(&mut self) -> AnalysisResult<()> {
        let next = self
            .counts
            .expanded_instructions
            .checked_add(1)
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::SatMaterialization,
                    "DEM SAT problem generation expanded instruction count overflowed",
                )
            })?;
        if next > self.limits.max_expanded_instructions {
            return Err(ResourceLimitError::sat_materialization(
                SatMaterializationResource::ExpandedInstructions,
                next,
                self.limits.max_expanded_instructions,
            )
            .into());
        }
        self.counts.expanded_instructions = next;
        Ok(())
    }

    fn push_error(
        &mut self,
        probability: f64,
        targets: &[DemTarget],
        detector_offset: u64,
    ) -> AnalysisResult<()> {
        let next_error_count = self.counts.error_mechanisms.checked_add(1).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::SatMaterialization,
                "SAT error mechanism count overflowed",
            )
        })?;
        if next_error_count > self.limits.max_error_mechanisms {
            return Err(ResourceLimitError::sat_materialization(
                SatMaterializationResource::ErrorMechanisms,
                sat_resource_amount(next_error_count, "SAT error mechanism count")?,
                sat_resource_amount(
                    self.limits.max_error_mechanisms,
                    "SAT error mechanism limit",
                )?,
            )
            .into());
        }
        let added_occurrences = targets
            .iter()
            .filter(|target| !target.is_separator())
            .count();
        let next_target_occurrences = self
            .counts
            .target_occurrences
            .checked_add(added_occurrences)
            .ok_or_else(|| {
                AnalysisError::count_overflow(
                    AnalysisOperation::SatMaterialization,
                    "SAT target occurrence count overflowed",
                )
            })?;
        if next_target_occurrences > self.limits.max_target_occurrences {
            return Err(ResourceLimitError::sat_materialization(
                SatMaterializationResource::TargetOccurrences,
                sat_resource_amount(next_target_occurrences, "SAT target occurrence count")?,
                sat_resource_amount(
                    self.limits.max_target_occurrences,
                    "SAT target occurrence limit",
                )?,
            )
            .into());
        }
        validate_shifted_target_ids(targets, detector_offset)?;
        if let Some(errors) = self.errors.as_deref_mut() {
            errors.push(FlattenedError {
                probability,
                targets: shifted_targets(targets, detector_offset)?,
            });
        }
        self.counts.error_mechanisms = next_error_count;
        self.counts.target_occurrences = next_target_occurrences;
        Ok(())
    }
}

fn validate_shifted_target_ids(targets: &[DemTarget], detector_offset: u64) -> AnalysisResult<()> {
    for target in targets {
        if let Some(detector) = target.detector() {
            let _ = detector.shifted(detector_offset)?;
        }
    }
    Ok(())
}

impl FoldedDemVisitor for SatErrorVisitor<'_> {
    type Error = AnalysisError;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> AnalysisResult<ControlFlow<()>> {
        self.countdown.tick(&self.limits.cancellation)?;
        match instruction.view() {
            DemInstructionView::Error {
                probability,
                targets,
            } => {
                self.add_expanded_instruction()?;
                self.push_error(probability.get(), targets, state.detector_offset())?;
            }
            DemInstructionView::ShiftDetectors { .. } => {
                self.add_expanded_instruction()?;
            }
            DemInstructionView::Detector { .. } | DemInstructionView::LogicalObservable { .. } => {}
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> AnalysisResult<DemRepeatSelection> {
        if body.summary().error_count()? == 0 {
            return Ok(DemRepeatSelection::Skip);
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: self.limits.max_repeat_iterations,
            context: "SAT problem generation",
        })
    }

    fn poll_cancellation(&mut self) -> AnalysisResult<()> {
        Ok(self.countdown.tick(&self.limits.cancellation)?)
    }

    fn repeat_expansion_limit_error(
        &mut self,
        _context: &'static str,
        actual: u64,
        limit: u64,
    ) -> AnalysisError {
        ResourceLimitError::sat_traversal_repeat_iterations("SAT problem generation", actual, limit)
            .into()
    }
}

fn sat_resource_amount(value: usize, context: &str) -> AnalysisResult<u64> {
    u64::try_from(value).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::SatMaterialization,
            format!("{context} does not fit resource diagnostics"),
        )
    })
}

fn checked_sat_add(left: usize, right: usize, context: &str) -> AnalysisResult<usize> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::SatMaterialization,
            format!("SAT {context} overflowed"),
        )
    })
}

fn checked_sat_product(left: usize, right: usize, context: &str) -> AnalysisResult<usize> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::SatMaterialization,
            format!("SAT {context} overflowed"),
        )
    })
}
