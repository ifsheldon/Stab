use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemObservableId, DemRepeatBlock, DemTarget,
    DetectorErrorModel, MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS, MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
    MAX_DEM_FLATTEN_REPEAT_UNROLL,
    traversal::{
        DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemTraversal,
        FoldedDemVisitor, shifted_targets,
    },
};
use crate::{CircuitError, CircuitResult};

mod instance;

use instance::{
    BoolRef, Clause, MAX_SAT_ERROR_MECHANISMS, MAX_SAT_TARGET_OCCURRENCES, MaxSatInstance,
    SatProblemMode, SatShape,
};

const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";

#[derive(Clone, Debug, PartialEq)]
struct FlattenedError {
    probability: f64,
    targets: Vec<DemTarget>,
}

#[derive(Clone, Debug)]
struct SatTargetIndex {
    detector_to_slot: BTreeMap<DemDetectorId, usize>,
    observable_to_slot: BTreeMap<DemObservableId, usize>,
}

impl SatTargetIndex {
    fn from_errors(errors: &[FlattenedError]) -> CircuitResult<Self> {
        let mut detectors = BTreeSet::new();
        let mut observables = BTreeSet::new();
        for error in errors {
            for target in &error.targets {
                match *target {
                    DemTarget::RelativeDetector(detector) => {
                        detectors.insert(detector);
                    }
                    DemTarget::LogicalObservable(observable) => {
                        observables.insert(observable);
                    }
                    DemTarget::Separator | DemTarget::Numeric(_) => {}
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

    fn detector_slot(&self, detector: DemDetectorId) -> CircuitResult<usize> {
        self.detector_to_slot
            .get(&detector)
            .copied()
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "SAT detector target D{} has no compressed slot",
                    detector.get()
                ))
            })
    }

    fn observable_slot(&self, observable: DemObservableId) -> CircuitResult<usize> {
        self.observable_to_slot
            .get(&observable)
            .copied()
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "SAT observable target L{} has no compressed slot",
                    observable.get()
                ))
            })
    }
}

fn preflight_sat_shape(
    errors: &[FlattenedError],
    target_index: &SatTargetIndex,
    mode: SatProblemMode,
) -> CircuitResult<SatShape> {
    let mut seen_detectors = vec![false; target_index.detector_to_slot.len()];
    let mut seen_observables = vec![false; target_index.observable_to_slot.len()];
    let mut target_occurrences = 0usize;
    let mut xor_count = 0usize;
    let mut soft_clause_count = 0usize;

    for error in errors {
        if soft_clause_is_stored(mode, error.probability) {
            soft_clause_count = checked_sat_add(soft_clause_count, 1, "soft clause count")?;
        }
        for target in &error.targets {
            let seen = match *target {
                DemTarget::RelativeDetector(detector) => {
                    let slot = target_index.detector_slot(detector)?;
                    seen_detectors.get_mut(slot).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "SAT detector preflight slot is outside its state vector",
                        )
                    })?
                }
                DemTarget::LogicalObservable(observable) => {
                    let slot = target_index.observable_slot(observable)?;
                    seen_observables.get_mut(slot).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "SAT observable preflight slot is outside its state vector",
                        )
                    })?
                }
                DemTarget::Separator | DemTarget::Numeric(_) => continue,
            };
            target_occurrences = checked_sat_add(target_occurrences, 1, "target occurrence count")?;
            if *seen {
                xor_count = checked_sat_add(xor_count, 1, "XOR count")?;
            } else {
                *seen = true;
            }
        }
    }

    let variables = checked_sat_add(errors.len(), xor_count, "variable count")?;
    let xor_clauses = checked_sat_product(xor_count, 4, "XOR clause count")?;
    let clauses = checked_sat_add(
        checked_sat_add(xor_clauses, soft_clause_count, "soft and XOR clause count")?,
        checked_sat_add(
            target_index.detector_to_slot.len(),
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
            target_index.detector_to_slot.len(),
            target_index.observable_to_slot.len(),
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
        output_bytes: 0,
    }
    .with_output_bound(mode)
}

pub fn shortest_error_sat_problem(model: &DetectorErrorModel) -> CircuitResult<String> {
    sat_problem_as_wcnf_string(model, SatProblemMode::Unweighted)
}

pub fn likeliest_error_sat_problem(
    model: &DetectorErrorModel,
    quantization: u32,
) -> CircuitResult<String> {
    if quantization < 1 {
        return Err(CircuitError::invalid_detector_error_model(
            "weighted SAT quantization must be at least 1",
        ));
    }
    sat_problem_as_wcnf_string(model, SatProblemMode::Weighted { quantization })
}

fn sat_problem_as_wcnf_string(
    model: &DetectorErrorModel,
    mode: SatProblemMode,
) -> CircuitResult<String> {
    if model.count_observables()? == 0 || model.count_errors()? == 0 {
        return Ok(UNSAT_WDIMACS.to_string());
    }
    let errors = flattened_error_instructions(model, mode)?;
    if errors.is_empty() {
        return Ok(UNSAT_WDIMACS.to_string());
    }
    let target_index = SatTargetIndex::from_errors(&errors)?;
    if target_index.observable_to_slot.is_empty() {
        return Ok(UNSAT_WDIMACS.to_string());
    }
    let shape = preflight_sat_shape(&errors, &target_index, mode)?;
    let mut instance = MaxSatInstance::with_shape(shape)?;
    let mut errors_activated = Vec::new();
    errors_activated
        .try_reserve_exact(errors.len())
        .map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "SAT problem generation cannot reserve {} error variables",
                errors.len()
            ))
        })?;
    for _ in &errors {
        errors_activated.push(instance.new_bool()?);
    }

    let mut detectors_activated = vec![BoolRef::false_ref(); target_index.detector_to_slot.len()];
    let mut observables_flipped = vec![BoolRef::false_ref(); target_index.observable_to_slot.len()];
    for (error_index, error) in errors.iter().enumerate() {
        let error_ref = errors_activated
            .get(error_index)
            .copied()
            .ok_or_else(|| CircuitError::invalid_detector_error_model("missing SAT error ref"))?;
        add_error_parity_terms(
            &mut instance,
            error_ref,
            &error.targets,
            &target_index,
            &mut detectors_activated,
            &mut observables_flipped,
        )?;
        add_error_soft_clause(&mut instance, mode, error_ref, error.probability)?;
    }

    for detector in detectors_activated {
        if detector.variable_index().is_some() {
            instance.add_clause(Clause::hard(vec![detector.not()]))?;
        }
    }

    let observable_clause_vars = observables_flipped
        .into_iter()
        .filter(|observable| observable.variable_index().is_some())
        .collect();
    instance.add_clause(Clause::hard(observable_clause_vars))?;
    instance.validate_shape(shape)?;
    instance.to_wdimacs(mode)
}

fn add_error_parity_terms(
    instance: &mut MaxSatInstance,
    error_ref: BoolRef,
    targets: &[DemTarget],
    target_index: &SatTargetIndex,
    detectors_activated: &mut [BoolRef],
    observables_flipped: &mut [BoolRef],
) -> CircuitResult<()> {
    for target in targets {
        match *target {
            DemTarget::RelativeDetector(detector) => {
                let index = target_index.detector_slot(detector)?;
                let current = detectors_activated.get(index).copied().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "SAT detector target D{} is outside the detector vector",
                        detector.get()
                    ))
                })?;
                let next = instance.xor(current, error_ref)?;
                let Some(slot) = detectors_activated.get_mut(index) else {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "SAT detector target D{} is outside the detector vector",
                        detector.get()
                    )));
                };
                *slot = next;
            }
            DemTarget::LogicalObservable(observable) => {
                let index = target_index.observable_slot(observable)?;
                let current = observables_flipped.get(index).copied().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "SAT observable target L{} is outside the observable vector",
                        observable.get()
                    ))
                })?;
                let next = instance.xor(current, error_ref)?;
                let Some(slot) = observables_flipped.get_mut(index) else {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "SAT observable target L{} is outside the observable vector",
                        observable.get()
                    )));
                };
                *slot = next;
            }
            DemTarget::Separator | DemTarget::Numeric(_) => {}
        }
    }
    Ok(())
}

fn add_error_soft_clause(
    instance: &mut MaxSatInstance,
    mode: SatProblemMode,
    error_ref: BoolRef,
    probability: f64,
) -> CircuitResult<()> {
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

fn flattened_error_instructions(
    model: &DetectorErrorModel,
    mode: SatProblemMode,
) -> CircuitResult<Vec<FlattenedError>> {
    let traversal = FoldedDemTraversal::new(model)?;
    traversal.validate_repeat_depth("SAT problem generation")?;
    let mut errors = Vec::new();
    let mut visitor = SatErrorVisitor {
        mode,
        expanded_instructions: 0,
        target_occurrences: 0,
        errors: &mut errors,
    };
    let _ = traversal.try_visit(&mut visitor)?;
    Ok(errors)
}

struct SatErrorVisitor<'a> {
    mode: SatProblemMode,
    expanded_instructions: u64,
    target_occurrences: usize,
    errors: &'a mut Vec<FlattenedError>,
}

impl SatErrorVisitor<'_> {
    fn add_expanded_instruction(&mut self) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "DEM SAT problem generation expanded instruction count overflowed",
                )
            })?;
        if self.expanded_instructions > MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM SAT problem generation currently supports at most {MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn push_error(
        &mut self,
        probability: f64,
        targets: &[DemTarget],
        detector_offset: u64,
    ) -> CircuitResult<()> {
        let next_error_count = self.errors.len().checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT error mechanism count overflowed")
        })?;
        if next_error_count > MAX_SAT_ERROR_MECHANISMS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "SAT problem generation currently supports at most {MAX_SAT_ERROR_MECHANISMS} error mechanisms, got at least {next_error_count}"
            )));
        }
        let added_occurrences = targets
            .iter()
            .filter(|target| {
                matches!(
                    target,
                    DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_)
                )
            })
            .count();
        let next_target_occurrences = self
            .target_occurrences
            .checked_add(added_occurrences)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("SAT target occurrence count overflowed")
            })?;
        if next_target_occurrences > MAX_SAT_TARGET_OCCURRENCES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "SAT problem generation currently supports at most {MAX_SAT_TARGET_OCCURRENCES} target occurrences, got at least {next_target_occurrences}"
            )));
        }
        self.errors.try_reserve(1).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "SAT problem generation cannot allocate another error mechanism",
            )
        })?;
        self.errors.push(FlattenedError {
            probability,
            targets: shifted_targets(targets, detector_offset)?,
        });
        self.target_occurrences = next_target_occurrences;
        Ok(())
    }
}

impl FoldedDemVisitor for SatErrorVisitor<'_> {
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>> {
        match instruction.kind() {
            DemInstructionKind::Error => {
                let probability = instruction.args().first().copied().ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "SAT error instruction is missing probability",
                    )
                })?;
                if !self.mode.includes_zero_probability_errors() && probability == 0.0 {
                    return Ok(ControlFlow::Continue(()));
                }
                let probability = match self.mode {
                    SatProblemMode::Unweighted => probability,
                    SatProblemMode::Weighted { .. } if state.folded_repeat_multiplicity() > 1 => {
                        weighted_repeat_map_probability(
                            probability,
                            state.folded_repeat_multiplicity(),
                        )
                    }
                    SatProblemMode::Weighted { .. } => probability,
                };
                if self.mode.includes_zero_probability_errors() || probability != 0.0 {
                    self.add_expanded_instruction()?;
                    self.push_error(probability, instruction.targets(), state.detector_offset())?;
                }
            }
            DemInstructionKind::ShiftDetectors => {
                if state.folded_repeat_multiplicity() == 1 {
                    self.add_expanded_instruction()?;
                }
            }
            DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        state: &DemTraversalState,
    ) -> CircuitResult<DemRepeatSelection> {
        let repeat_count = repeat.repeat_count().get();
        let body_shift = body.summary().detector_shift()?;
        let in_folded_repeat = state.folded_repeat_multiplicity() > 1;
        if body.summary().error_count()? == 0 {
            return Ok(DemRepeatSelection::Skip);
        }
        if !self.mode.includes_zero_probability_errors()
            && !body.summary().has_nonzero_probability_error()
        {
            return Ok(DemRepeatSelection::Skip);
        }
        if (in_folded_repeat || repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL)
            && body_shift == 0
            && (in_folded_repeat || !body.items().is_empty())
        {
            return Ok(DemRepeatSelection::FoldOnce);
        }
        if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM SAT problem generation currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
            context: "SAT problem generation",
        })
    }
}

fn weighted_repeat_map_probability(probability: f64, repeat_count: u64) -> f64 {
    if repeat_count == 0 || probability <= 0.0 {
        return 0.0;
    }
    if probability >= 1.0 {
        return if repeat_count.is_multiple_of(2) {
            0.0
        } else {
            1.0
        };
    }
    if probability == 0.5 {
        return 0.5;
    }
    if probability < 0.5 {
        return probability;
    }
    if repeat_count.is_multiple_of(2) {
        1.0 - probability
    } else {
        probability
    }
}

fn checked_sat_add(left: usize, right: usize, context: &str) -> CircuitResult<usize> {
    left.checked_add(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("SAT {context} overflowed"))
    })
}

fn checked_sat_product(left: usize, right: usize, context: &str) -> CircuitResult<usize> {
    left.checked_mul(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("SAT {context} overflowed"))
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic_in_result_fn,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::{
        MAX_SAT_TARGET_OCCURRENCES, SatErrorVisitor, SatProblemMode, likeliest_error_sat_problem,
        shortest_error_sat_problem,
    };
    use crate::{CircuitError, CircuitResult, DemTarget, DetectorErrorModel};

    const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";
    const TWO_ERROR_UNWEIGHTED_WDIMACS: &str = "\
p wcnf 3 8 9
1 -1 0
9 1 2 -3 0
9 1 -2 3 0
9 -1 2 3 0
9 -1 -2 -3 0
1 -2 0
9 -3 0
9 1 0
";

    fn dem(input: &str) -> CircuitResult<DetectorErrorModel> {
        DetectorErrorModel::from_dem_str(input)
    }

    #[test]
    fn sat_problem_shortest_no_error_is_unsatisfiable() -> CircuitResult<()> {
        assert_eq!(
            shortest_error_sat_problem(&DetectorErrorModel::new())?,
            UNSAT_WDIMACS
        );
        assert_eq!(
            shortest_error_sat_problem(&dem("error(0.1) D0")?)?,
            UNSAT_WDIMACS
        );
        assert_eq!(
            shortest_error_sat_problem(&dem("error(0.1)")?)?,
            UNSAT_WDIMACS
        );
        assert_eq!(shortest_error_sat_problem(&dem("")?)?, UNSAT_WDIMACS);
        Ok(())
    }

    #[test]
    fn sat_problem_shortest_single_observable_without_detectors_matches_stim() -> CircuitResult<()>
    {
        assert_eq!(
            shortest_error_sat_problem(&dem("error(0.1) L0")?)?,
            "p wcnf 1 2 3\n1 -1 0\n3 1 0\n"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_shortest_ignores_error_probabilities() -> CircuitResult<()> {
        for dem_text in [
            "error(0.1) D0 L0\nerror(0.1) D0\n",
            "error(1.0) D0 L0\nerror(0) D0\n",
            "error(0.5) D0 L0\nerror(0.999) D0\n",
            "error(0.001) D0 L0\nerror(0.999) D0\n",
            "error(0) D0 L0\nerror(0) D0\n",
            "error(0.5) D0 L0\nerror(0.5) D0\n",
        ] {
            assert_eq!(
                shortest_error_sat_problem(&dem(dem_text)?)?,
                TWO_ERROR_UNWEIGHTED_WDIMACS
            );
        }
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_no_error_is_unsatisfiable() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&DetectorErrorModel::new(), 10)?,
            UNSAT_WDIMACS
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_omits_zero_probability_error_variables() -> CircuitResult<()> {
        let with_zero = likeliest_error_sat_problem(
            &dem("error(0) D9 L3\nerror(0.1) D0\nerror(0.1) D0 L0\n")?,
            10,
        )?;
        let without_zero =
            likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 10)?;
        assert_eq!(with_zero, without_zero);
        assert!(
            with_zero.starts_with("p wcnf 3 "),
            "zero-probability errors should not allocate SAT variables: {with_zero}"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_skips_zero_probability_repeats() -> CircuitResult<()> {
        let model = dem("\
repeat 100001 {
    error(0) D1000000 L1000
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 L0
")?;
        let expected = likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 10)?;
        assert_eq!(likeliest_error_sat_problem(&model, 10)?, expected);

        let unweighted_error = match shortest_error_sat_problem(&model) {
            Ok(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "unweighted SAT unexpectedly accepted zero-probability repeat",
                ));
            }
            Err(error) => error.to_string(),
        };
        assert!(
            unweighted_error
                .contains("DEM SAT problem generation currently supports repeat counts up to"),
            "{unweighted_error}"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_skips_shifted_zero_probability_repeat_without_dense_allocation()
    -> CircuitResult<()> {
        let model = dem("\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 L0
")?;
        assert_eq!(
            likeliest_error_sat_problem(&model, 10)?,
            likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 10)?
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_compresses_sparse_observable_ids() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&dem("error(0.1) L1000001\n")?, 10)?,
            likeliest_error_sat_problem(&dem("error(0.1) L0\n")?, 10)?
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_balanced_probabilities_match_stim() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.1) D0\n")?, 10)?,
            "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 -2 0
81 -3 0
81 1 0
"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_large_probability_flips_soft_clause_sign() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.9) D0\n")?, 10)?,
            "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 2 0
81 -3 0
81 1 0
"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_half_probability_skips_soft_clause() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.5) D0\n")?, 10)?,
            "\
p wcnf 3 7 71
10 -1 0
71 1 2 -3 0
71 1 -2 3 0
71 -1 2 3 0
71 -1 -2 -3 0
71 -3 0
71 1 0
"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_header_counts_stored_clauses_like_stim() -> CircuitResult<()> {
        let wcnf = likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.49) D0\n")?, 1)?;
        let clause_count = wcnf_clause_count(&wcnf)?;
        assert_eq!(clause_count, 8, "{wcnf}");
        assert_eq!(wcnf.lines().skip(1).count(), 7, "{wcnf}");
        assert_eq!(
            wcnf,
            "p wcnf 3 8 9\n1 -1 0\n9 1 2 -3 0\n9 1 -2 3 0\n9 -1 2 3 0\n9 -1 -2 -3 0\n9 -3 0\n9 1 0\n"
        );
        Ok(())
    }

    #[test]
    fn sat_error_visitor_rejects_target_occurrences_before_materialization() -> CircuitResult<()> {
        let mut errors = Vec::new();
        let mut visitor = SatErrorVisitor {
            mode: SatProblemMode::Unweighted,
            expanded_instructions: 0,
            target_occurrences: MAX_SAT_TARGET_OCCURRENCES,
            errors: &mut errors,
        };
        let targets = [DemTarget::relative_detector(0)?];
        let error = visitor
            .push_error(0.1, &targets, 0)
            .expect_err("target occurrence beyond the cap");
        assert!(
            error
                .to_string()
                .contains("at most 500000 target occurrences")
        );
        assert!(visitor.errors.is_empty());
        Ok(())
    }

    fn wcnf_clause_count(wcnf: &str) -> CircuitResult<usize> {
        let header = wcnf.lines().next().ok_or_else(|| {
            CircuitError::invalid_detector_error_model("test WCNF output is missing a header")
        })?;
        let clause_count = header.split_whitespace().nth(3).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("test WCNF header is missing a clause count")
        })?;
        clause_count.parse::<usize>().map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "test WCNF header clause count is not numeric",
            )
        })
    }

    #[test]
    fn sat_problem_flattens_repeat_detector_offsets() -> CircuitResult<()> {
        let model = dem("\
repeat 2 {
    error(0.1) D0
    shift_detectors 1
}
error(0.1) D0 L0
")?;
        assert_eq!(
            shortest_error_sat_problem(&model)?,
            "\
p wcnf 3 7 8
1 -1 0
1 -2 0
1 -3 0
8 -1 0
8 -2 0
8 -3 0
8 3 0
"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_rejects_excessive_repeat_expansion() -> CircuitResult<()> {
        let model = dem("\
repeat 100001 {
    error(0.1) D0
    shift_detectors 1
}
error(0.1) D0 L0
")?;

        let error = match shortest_error_sat_problem(&model) {
            Ok(output) => {
                return Err(crate::CircuitError::invalid_detector_error_model(format!(
                    "SAT problem generation accepted hostile repeat expansion: {output}"
                )));
            }
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(
                "DEM SAT problem generation currently supports repeat counts up to 100000"
            ),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_weighted_quantization_must_be_positive() -> CircuitResult<()> {
        assert!(likeliest_error_sat_problem(&dem("error(0.1) L0")?, 0).is_err());
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_treats_deterministic_error_as_hard() -> CircuitResult<()> {
        assert_eq!(
            likeliest_error_sat_problem(&dem("error(1) L0")?, 10)?,
            "\
p wcnf 1 2 21
21 1 0
21 1 0
"
        );
        Ok(())
    }
}
