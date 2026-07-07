use super::{
    DemFlatteningBudget, DemInstruction, DemInstructionKind, DemItem, DemObservableId, DemTarget,
    DetectorErrorModel, MAX_DEM_FLATTEN_REPEAT_UNROLL, MAX_DEM_REPEAT_NESTING,
};
use crate::{CircuitError, CircuitResult};

const UNSAT_WDIMACS: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";
const MAX_SAT_DENSE_TARGET_COUNT: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SatProblemMode {
    Unweighted,
    Weighted { quantization: u32 },
}

impl SatProblemMode {
    fn includes_zero_probability_errors(self) -> bool {
        matches!(self, Self::Unweighted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoolAtom {
    Constant(bool),
    Variable(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoolRef {
    atom: BoolAtom,
    negated: bool,
}

impl BoolRef {
    fn false_ref() -> Self {
        Self {
            atom: BoolAtom::Constant(false),
            negated: false,
        }
    }

    fn variable(index: usize) -> Self {
        Self {
            atom: BoolAtom::Variable(index),
            negated: false,
        }
    }

    fn not(self) -> Self {
        Self {
            atom: self.atom,
            negated: !self.negated,
        }
    }

    fn constant_value(self) -> Option<bool> {
        match self.atom {
            BoolAtom::Constant(value) => Some(value ^ self.negated),
            BoolAtom::Variable(_) => None,
        }
    }

    fn variable_index(self) -> Option<usize> {
        match self.atom {
            BoolAtom::Variable(index) => Some(index),
            BoolAtom::Constant(_) => None,
        }
    }

    fn to_wdimacs_literal(self) -> CircuitResult<Option<String>> {
        let Some(index) = self.variable_index() else {
            return Ok(None);
        };
        let one_based = index.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT variable index overflowed")
        })?;
        if self.negated {
            Ok(Some(format!("-{one_based}")))
        } else {
            Ok(Some(one_based.to_string()))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ClauseWeight {
    Hard,
    Soft(f64),
}

#[derive(Clone, Debug, PartialEq)]
struct Clause {
    vars: Vec<BoolRef>,
    weight: ClauseWeight,
}

impl Clause {
    fn hard(vars: Vec<BoolRef>) -> Self {
        Self {
            vars,
            weight: ClauseWeight::Hard,
        }
    }

    fn soft(var: BoolRef, weight: f64) -> Self {
        Self {
            vars: vec![var],
            weight: ClauseWeight::Soft(weight),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct MaxSatInstance {
    num_variables: usize,
    max_weight: f64,
    clauses: Vec<Clause>,
}

impl MaxSatInstance {
    fn new_bool(&mut self) -> CircuitResult<BoolRef> {
        let variable = self.num_variables;
        self.num_variables = self.num_variables.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT variable count overflowed")
        })?;
        Ok(BoolRef::variable(variable))
    }

    fn add_clause(&mut self, clause: Clause) -> CircuitResult<()> {
        if let ClauseWeight::Soft(weight) = clause.weight {
            if !weight.is_finite() || weight <= 0.0 {
                return Err(CircuitError::invalid_detector_error_model(
                    "SAT soft clause weight must be finite and positive",
                ));
            }
            self.max_weight = self.max_weight.max(weight);
        }
        self.clauses.push(clause);
        Ok(())
    }

    fn xor(&mut self, left: BoolRef, right: BoolRef) -> CircuitResult<BoolRef> {
        match (left.constant_value(), right.constant_value()) {
            (Some(false), _) => return Ok(right),
            (Some(true), _) => return Ok(right.not()),
            (_, Some(false)) => return Ok(left),
            (_, Some(true)) => return Ok(left.not()),
            (None, None) => {}
        }

        let output = self.new_bool()?;
        self.add_clause(Clause::hard(vec![left, right, output.not()]))?;
        self.add_clause(Clause::hard(vec![left, right.not(), output]))?;
        self.add_clause(Clause::hard(vec![left.not(), right, output]))?;
        self.add_clause(Clause::hard(vec![left.not(), right.not(), output.not()]))?;
        Ok(output)
    }

    fn to_wdimacs(&self, mode: SatProblemMode) -> CircuitResult<String> {
        let emitted_clause_count = self.emitted_clause_count(mode)?;
        let top = self.top_weight(mode, emitted_clause_count)?;
        let mut out = String::new();
        out.push_str("p wcnf ");
        out.push_str(&self.num_variables.to_string());
        out.push(' ');
        out.push_str(&emitted_clause_count.to_string());
        out.push(' ');
        out.push_str(&top.to_string());
        out.push('\n');

        for clause in &self.clauses {
            let weight = self.quantized_weight(mode, top, &clause.weight)?;
            if weight == 0 {
                continue;
            }
            out.push_str(&weight.to_string());
            for var in &clause.vars {
                if let Some(literal) = var.to_wdimacs_literal()? {
                    out.push(' ');
                    out.push_str(&literal);
                }
            }
            out.push_str(" 0\n");
        }
        Ok(out)
    }

    fn emitted_clause_count(&self, mode: SatProblemMode) -> CircuitResult<usize> {
        let mut count = 0usize;
        for clause in &self.clauses {
            if self.clause_is_emitted(mode, clause)? {
                count = count.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("SAT clause count overflowed")
                })?;
            }
        }
        Ok(count)
    }

    fn clause_is_emitted(&self, mode: SatProblemMode, clause: &Clause) -> CircuitResult<bool> {
        match clause.weight {
            ClauseWeight::Hard => Ok(true),
            ClauseWeight::Soft(_) => Ok(self.quantized_weight(mode, 0, &clause.weight)? != 0),
        }
    }

    fn top_weight(
        &self,
        mode: SatProblemMode,
        emitted_clause_count: usize,
    ) -> CircuitResult<usize> {
        match mode {
            SatProblemMode::Unweighted => emitted_clause_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("unweighted SAT top weight overflowed")
            }),
            SatProblemMode::Weighted { quantization } => {
                let quantization = usize::try_from(quantization).map_err(|_| {
                    CircuitError::invalid_detector_error_model(
                        "weighted SAT quantization does not fit usize",
                    )
                })?;
                quantization
                    .checked_mul(emitted_clause_count)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "weighted SAT top weight overflowed",
                        )
                    })
            }
        }
    }

    fn quantized_weight(
        &self,
        mode: SatProblemMode,
        top: usize,
        weight: &ClauseWeight,
    ) -> CircuitResult<usize> {
        match weight {
            ClauseWeight::Hard => Ok(top),
            ClauseWeight::Soft(_) if matches!(mode, SatProblemMode::Unweighted) => Ok(1),
            ClauseWeight::Soft(weight) => {
                let SatProblemMode::Weighted { quantization } = mode else {
                    return Err(CircuitError::invalid_detector_error_model(
                        "unweighted SAT problem received weighted clause",
                    ));
                };
                if self.max_weight <= 0.0 {
                    return Err(CircuitError::invalid_detector_error_model(
                        "weighted SAT problem has no positive soft-clause weight",
                    ));
                }
                rounded_nonnegative_usize(*weight / self.max_weight * f64::from(quantization))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlattenedError {
    probability: f64,
    targets: Vec<DemTarget>,
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
    let errors = flattened_error_instructions(model, mode)?;
    if errors.is_empty() {
        return Ok(UNSAT_WDIMACS.to_string());
    }
    let (detector_count, observable_count) = flattened_error_target_counts(&errors)?;
    if observable_count == 0 {
        return Ok(UNSAT_WDIMACS.to_string());
    }
    validate_sat_dense_target_counts(detector_count, observable_count)?;

    let num_observables = usize::try_from(observable_count).map_err(|_| {
        CircuitError::invalid_detector_error_model("observable count does not fit usize")
    })?;
    let num_detectors = usize::try_from(detector_count).map_err(|_| {
        CircuitError::invalid_detector_error_model("detector count does not fit usize")
    })?;
    let mut instance = MaxSatInstance::default();
    let mut errors_activated = Vec::with_capacity(errors.len());
    for _ in &errors {
        errors_activated.push(instance.new_bool()?);
    }

    let mut detectors_activated = vec![BoolRef::false_ref(); num_detectors];
    let mut observables_flipped = vec![BoolRef::false_ref(); num_observables];
    for (error_index, error) in errors.iter().enumerate() {
        let error_ref = errors_activated
            .get(error_index)
            .copied()
            .ok_or_else(|| CircuitError::invalid_detector_error_model("missing SAT error ref"))?;
        add_error_parity_terms(
            &mut instance,
            error_ref,
            &error.targets,
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
    instance.to_wdimacs(mode)
}

fn validate_sat_dense_target_counts(
    detector_count: u64,
    observable_count: u64,
) -> CircuitResult<()> {
    if detector_count > MAX_SAT_DENSE_TARGET_COUNT {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "SAT problem generation currently supports at most {MAX_SAT_DENSE_TARGET_COUNT} effective detector nodes, got {detector_count}"
        )));
    }
    if observable_count > MAX_SAT_DENSE_TARGET_COUNT {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "SAT problem generation currently supports at most {MAX_SAT_DENSE_TARGET_COUNT} effective observable nodes, got {observable_count}"
        )));
    }
    Ok(())
}

fn add_error_parity_terms(
    instance: &mut MaxSatInstance,
    error_ref: BoolRef,
    targets: &[DemTarget],
    detectors_activated: &mut [BoolRef],
    observables_flipped: &mut [BoolRef],
) -> CircuitResult<()> {
    for target in targets {
        match *target {
            DemTarget::RelativeDetector(detector) => {
                let index = dem_detector_index(detector)?;
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
                let index = dem_observable_index(observable)?;
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

fn flattened_error_instructions(
    model: &DetectorErrorModel,
    mode: SatProblemMode,
) -> CircuitResult<Vec<FlattenedError>> {
    let mut errors = Vec::new();
    let mut budget = DemFlatteningBudget::default();
    collect_flattened_error_instructions(model, 0, mode, 0, &mut budget, &mut errors)?;
    Ok(errors)
}

fn collect_flattened_error_instructions(
    model: &DetectorErrorModel,
    mut detector_offset: u64,
    mode: SatProblemMode,
    depth: usize,
    budget: &mut DemFlatteningBudget,
    errors: &mut Vec<FlattenedError>,
) -> CircuitResult<u64> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "DEM SAT problem generation repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    budget.add_expanded_instructions(1, "SAT problem generation")?;
                    let probability = instruction.args().first().copied().ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "SAT error instruction is missing probability",
                        )
                    })?;
                    if mode.includes_zero_probability_errors() || probability != 0.0 {
                        errors.push(flatten_error(instruction, detector_offset)?);
                    }
                }
                DemInstructionKind::ShiftDetectors => {
                    budget.add_expanded_instructions(1, "SAT problem generation")?;
                    detector_offset = detector_offset
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "SAT detector offset overflowed",
                            )
                        })?;
                }
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
            },
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL
                    && let Some(folded_errors) = folded_selected_sat_repeat_errors(
                        repeat.body(),
                        repeat_count,
                        detector_offset,
                        mode,
                        depth + 1,
                    )?
                {
                    budget.add_expanded_instructions(
                        u64::try_from(folded_errors.len()).map_err(|_| {
                            CircuitError::invalid_detector_error_model(
                                "SAT folded repeat error count does not fit u64",
                            )
                        })?,
                        "SAT problem generation",
                    )?;
                    errors.extend(folded_errors);
                    let body_shift = repeat.body().total_detector_shift()?;
                    detector_offset = body_shift
                        .checked_mul(repeat_count)
                        .and_then(|shift| detector_offset.checked_add(shift))
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "SAT repeat detector offset overflowed",
                            )
                        })?;
                    continue;
                }
                if !mode.includes_zero_probability_errors()
                    && !repeat
                        .body()
                        .has_nonzero_probability_error("SAT problem generation")?
                {
                    let body_shift = repeat.body().total_detector_shift()?;
                    detector_offset = body_shift
                        .checked_mul(repeat.repeat_count().get())
                        .and_then(|shift| detector_offset.checked_add(shift))
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "SAT repeat detector offset overflowed",
                            )
                        })?;
                    continue;
                }
                if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "DEM SAT problem generation currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                budget.add_repeat_iterations(repeat_count, "SAT problem generation")?;
                for _ in 0..repeat_count {
                    detector_offset = collect_flattened_error_instructions(
                        repeat.body(),
                        detector_offset,
                        mode,
                        depth + 1,
                        budget,
                        errors,
                    )?;
                }
            }
        }
    }
    Ok(detector_offset)
}

fn folded_selected_sat_repeat_errors(
    body: &DetectorErrorModel,
    repeat_count: u64,
    detector_offset: u64,
    mode: SatProblemMode,
    depth: usize,
) -> CircuitResult<Option<Vec<FlattenedError>>> {
    if body.total_detector_shift()? != 0 || body.items().is_empty() {
        return Ok(None);
    }
    let mut folded_errors = Vec::with_capacity(body.items().len());
    if collect_folded_selected_sat_repeat_errors(
        body,
        repeat_count,
        detector_offset,
        mode,
        depth,
        &mut folded_errors,
    )? {
        Ok(Some(folded_errors))
    } else {
        Ok(None)
    }
}

fn collect_folded_selected_sat_repeat_errors(
    body: &DetectorErrorModel,
    repeat_multiplier: u64,
    detector_offset: u64,
    mode: SatProblemMode,
    depth: usize,
    folded_errors: &mut Vec<FlattenedError>,
) -> CircuitResult<bool> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "DEM SAT problem generation repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    if body.total_detector_shift()? != 0 {
        return Ok(false);
    }
    for item in body.items() {
        match item {
            DemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    let probability = instruction.args().first().copied().ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "SAT error instruction is missing probability",
                        )
                    })?;
                    match mode {
                        SatProblemMode::Unweighted => {
                            folded_errors.push(flatten_error(instruction, detector_offset)?);
                        }
                        SatProblemMode::Weighted { .. } => {
                            if probability == 0.0 {
                                continue;
                            }
                            let mut folded_error = flatten_error(instruction, detector_offset)?;
                            folded_error.probability =
                                weighted_repeat_map_probability(probability, repeat_multiplier);
                            if folded_error.probability != 0.0 {
                                folded_errors.push(folded_error);
                            }
                        }
                    }
                }
                DemInstructionKind::ShiftDetectors => {
                    if instruction.detector_shift()? != 0 {
                        return Ok(false);
                    }
                }
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
            },
            DemItem::RepeatBlock(repeat) => {
                if repeat.body().total_detector_shift()? != 0 {
                    return Ok(false);
                }
                let nested_multiplier = repeat_multiplier
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "SAT folded repeat multiplier overflowed",
                        )
                    })?;
                if !collect_folded_selected_sat_repeat_errors(
                    repeat.body(),
                    nested_multiplier,
                    detector_offset,
                    mode,
                    depth + 1,
                    folded_errors,
                )? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn flatten_error(
    instruction: &DemInstruction,
    detector_offset: u64,
) -> CircuitResult<FlattenedError> {
    let probability = instruction.args().first().copied().ok_or_else(|| {
        CircuitError::invalid_detector_error_model("SAT error instruction is missing probability")
    })?;
    let targets = shifted_targets(instruction.targets(), detector_offset)?;
    Ok(FlattenedError {
        probability,
        targets,
    })
}

fn shifted_targets(targets: &[DemTarget], detector_offset: u64) -> CircuitResult<Vec<DemTarget>> {
    targets
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector_offset.checked_add(detector.get()).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("SAT detector target overflowed")
                })?,
            ),
            DemTarget::LogicalObservable(observable) => {
                DemTarget::logical_observable(observable.get())
            }
            DemTarget::Separator => Ok(DemTarget::separator()),
            DemTarget::Numeric(value) => Ok(DemTarget::numeric(value)),
        })
        .collect()
}

fn flattened_error_target_counts(errors: &[FlattenedError]) -> CircuitResult<(u64, u64)> {
    let mut detector_count = 0_u64;
    let mut observable_count = 0_u64;
    for error in errors {
        for target in &error.targets {
            match *target {
                DemTarget::RelativeDetector(detector) => {
                    detector_count =
                        detector_count.max(detector.get().checked_add(1).ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "SAT detector count overflowed",
                            )
                        })?);
                }
                DemTarget::LogicalObservable(observable) => {
                    observable_count =
                        observable_count.max(observable.get().checked_add(1).ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(
                                "SAT observable count overflowed",
                            )
                        })?);
                }
                DemTarget::Separator | DemTarget::Numeric(_) => {}
            }
        }
    }
    Ok((detector_count, observable_count))
}

fn dem_detector_index(detector: super::DemDetectorId) -> CircuitResult<usize> {
    usize::try_from(detector.get()).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "SAT detector target D{} does not fit usize",
            detector.get()
        ))
    })
}

fn dem_observable_index(observable: DemObservableId) -> CircuitResult<usize> {
    usize::try_from(observable.get()).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "SAT observable target L{} does not fit usize",
            observable.get()
        ))
    })
}

fn rounded_nonnegative_usize(value: f64) -> CircuitResult<usize> {
    if !value.is_finite() || value < 0.0 {
        return Err(CircuitError::invalid_detector_error_model(
            "SAT quantized weight is not a finite nonnegative value",
        ));
    }
    format!("{:.0}", value.round())
        .parse::<usize>()
        .map_err(|_| CircuitError::invalid_detector_error_model("SAT quantized weight overflowed"))
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic_in_result_fn,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use super::{likeliest_error_sat_problem, shortest_error_sat_problem};
    use crate::{CircuitError, CircuitResult, DetectorErrorModel};

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
    fn sat_problem_likeliest_rejects_shifted_zero_probability_repeat_node_explosion()
    -> CircuitResult<()> {
        let model = dem("\
repeat 1000001 {
    error(0) D0
    shift_detectors 1
}
error(0.1) D0
error(0.1) D0 L0
")?;
        let error = match likeliest_error_sat_problem(&model, 10) {
            Ok(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "weighted SAT unexpectedly accepted huge shifted dense detector allocation",
                ));
            }
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(
                "SAT problem generation currently supports at most 1000000 effective detector nodes"
            ),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn sat_problem_likeliest_rejects_huge_dense_observable_vector() -> CircuitResult<()> {
        let error = match likeliest_error_sat_problem(&dem("error(0.1) L1000001\n")?, 10) {
            Ok(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "weighted SAT unexpectedly accepted huge dense observable allocation",
                ));
            }
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("SAT problem generation currently supports at most 1000000 effective observable nodes"),
            "{error}"
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
    fn sat_problem_likeliest_header_counts_emitted_clauses() -> CircuitResult<()> {
        let wcnf = likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.49) D0\n")?, 1)?;
        let clause_count = wcnf_clause_count(&wcnf)?;
        assert_eq!(wcnf.lines().skip(1).count(), clause_count, "{wcnf}");
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
