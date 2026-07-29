use super::SatMaterializationLimits;
use crate::resources::SatMaterializationResource;
use crate::{AnalysisError, AnalysisResult, ResourceLimitError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SatProblemMode {
    Unweighted,
    Weighted { quantization: u32 },
}

impl SatProblemMode {
    pub(super) fn includes_zero_probability_errors(self) -> bool {
        matches!(self, Self::Unweighted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoolAtom {
    Constant(bool),
    Variable(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BoolRef {
    atom: BoolAtom,
    negated: bool,
}

impl BoolRef {
    pub(super) fn false_ref() -> Self {
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

    pub(super) fn not(self) -> Self {
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

    pub(super) fn variable_index(self) -> Option<usize> {
        match self.atom {
            BoolAtom::Variable(index) => Some(index),
            BoolAtom::Constant(_) => None,
        }
    }

    fn to_wdimacs_literal(self) -> AnalysisResult<Option<String>> {
        let Some(index) = self.variable_index() else {
            return Ok(None);
        };
        let one_based = index.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("SAT variable index overflowed")
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
pub(super) struct Clause {
    vars: Vec<BoolRef>,
    weight: ClauseWeight,
}

impl Clause {
    pub(super) fn hard(vars: Vec<BoolRef>) -> Self {
        Self {
            vars,
            weight: ClauseWeight::Hard,
        }
    }

    pub(super) fn soft(var: BoolRef, weight: f64) -> Self {
        Self {
            vars: vec![var],
            weight: ClauseWeight::Soft(weight),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SatShape {
    pub(super) error_mechanisms: usize,
    pub(super) target_occurrences: usize,
    pub(super) variables: usize,
    pub(super) clauses: usize,
    pub(super) clause_literals: usize,
}

impl SatShape {
    pub(super) fn validate(self, limits: SatMaterializationLimits) -> AnalysisResult<Self> {
        validate_limit(
            SatMaterializationResource::ErrorMechanisms,
            self.error_mechanisms,
            limits.max_error_mechanisms(),
        )?;
        validate_limit(
            SatMaterializationResource::TargetOccurrences,
            self.target_occurrences,
            limits.max_target_occurrences(),
        )?;
        validate_limit(
            SatMaterializationResource::Variables,
            self.variables,
            limits.max_variables(),
        )?;
        validate_limit(
            SatMaterializationResource::Clauses,
            self.clauses,
            limits.max_clauses(),
        )?;
        validate_limit(
            SatMaterializationResource::ClauseLiterals,
            self.clause_literals,
            limits.max_clause_literals(),
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct MaxSatInstance {
    limits: SatMaterializationLimits,
    num_variables: usize,
    max_weight: f64,
    clauses: Vec<Clause>,
    clause_literals: usize,
}

impl MaxSatInstance {
    pub(super) fn with_shape(
        shape: SatShape,
        limits: SatMaterializationLimits,
    ) -> AnalysisResult<Self> {
        let shape = shape.validate(limits)?;
        let mut clauses = Vec::new();
        clauses.try_reserve_exact(shape.clauses).map_err(|_| {
            AnalysisError::invalid_detector_error_model(format!(
                "SAT problem generation cannot reserve {} clauses",
                shape.clauses
            ))
        })?;
        Ok(Self {
            limits,
            clauses,
            ..Self::default()
        })
    }

    pub(super) fn new_bool(&mut self) -> AnalysisResult<BoolRef> {
        let variable = self.num_variables;
        let next = self.num_variables.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("SAT variable count overflowed")
        })?;
        validate_limit(
            SatMaterializationResource::Variables,
            next,
            self.limits.max_variables(),
        )?;
        self.num_variables = next;
        Ok(BoolRef::variable(variable))
    }

    pub(super) fn add_clause(&mut self, clause: Clause) -> AnalysisResult<()> {
        if let ClauseWeight::Soft(weight) = clause.weight {
            if !weight.is_finite() || weight <= 0.0 {
                return Err(AnalysisError::invalid_detector_error_model(
                    "SAT soft clause weight must be finite and positive",
                ));
            }
            self.max_weight = self.max_weight.max(weight);
        }
        let clause_count = self.clauses.len().checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("SAT clause count overflowed")
        })?;
        validate_limit(
            SatMaterializationResource::Clauses,
            clause_count,
            self.limits.max_clauses(),
        )?;
        let literal_count = self
            .clause_literals
            .checked_add(clause.vars.len())
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model("SAT clause literal count overflowed")
            })?;
        validate_limit(
            SatMaterializationResource::ClauseLiterals,
            literal_count,
            self.limits.max_clause_literals(),
        )?;
        self.clauses.push(clause);
        self.clause_literals = literal_count;
        Ok(())
    }

    pub(super) fn xor(&mut self, left: BoolRef, right: BoolRef) -> AnalysisResult<BoolRef> {
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

    pub(super) fn validate_shape(&self, shape: SatShape) -> AnalysisResult<()> {
        if self.num_variables != shape.variables
            || self.clauses.len() != shape.clauses
            || self.clause_literals != shape.clause_literals
        {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "SAT preflight shape changed during encoding: expected {} variables, {} clauses, and {} literals; got {}, {}, and {}",
                shape.variables,
                shape.clauses,
                shape.clause_literals,
                self.num_variables,
                self.clauses.len(),
                self.clause_literals
            )));
        }
        Ok(())
    }

    pub(super) fn to_wdimacs(&self, mode: SatProblemMode) -> AnalysisResult<String> {
        let clause_count = self.clauses.len();
        let top = self.top_weight(mode, clause_count)?;
        let output_bound = self.exact_output_bytes(mode, top)?;
        validate_limit(
            SatMaterializationResource::OutputBytes,
            output_bound,
            self.limits.max_output_bytes(),
        )?;
        let mut out = String::new();
        out.try_reserve(output_bound).map_err(|_| {
            AnalysisError::invalid_detector_error_model(format!(
                "SAT problem generation cannot reserve {output_bound} WDIMACS output bytes"
            ))
        })?;
        out.push_str("p wcnf ");
        out.push_str(&self.num_variables.to_string());
        out.push(' ');
        out.push_str(&clause_count.to_string());
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

    fn exact_output_bytes(&self, mode: SatProblemMode, top: usize) -> AnalysisResult<usize> {
        let mut bytes = "p wcnf ".len();
        bytes = checked_output_add(bytes, decimal_digits(self.num_variables))?;
        bytes = checked_output_add(bytes, 1)?;
        bytes = checked_output_add(bytes, decimal_digits(self.clauses.len()))?;
        bytes = checked_output_add(bytes, 1)?;
        bytes = checked_output_add(bytes, decimal_digits(top))?;
        bytes = checked_output_add(bytes, 1)?;

        for clause in &self.clauses {
            let weight = self.quantized_weight(mode, top, &clause.weight)?;
            if weight == 0 {
                continue;
            }
            bytes = checked_output_add(bytes, decimal_digits(weight))?;
            for var in &clause.vars {
                let Some(index) = var.variable_index() else {
                    continue;
                };
                let one_based = index.checked_add(1).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "SAT variable index overflowed while sizing output",
                    )
                })?;
                bytes = checked_output_add(bytes, 1)?;
                bytes = checked_output_add(bytes, usize::from(var.negated))?;
                bytes = checked_output_add(bytes, decimal_digits(one_based))?;
            }
            bytes = checked_output_add(bytes, " 0\n".len())?;
        }
        Ok(bytes)
    }

    fn top_weight(&self, mode: SatProblemMode, clause_count: usize) -> AnalysisResult<usize> {
        top_weight_for_clause_count(mode, clause_count)
    }

    fn quantized_weight(
        &self,
        mode: SatProblemMode,
        top: usize,
        weight: &ClauseWeight,
    ) -> AnalysisResult<usize> {
        match weight {
            ClauseWeight::Hard => Ok(top),
            ClauseWeight::Soft(_) if matches!(mode, SatProblemMode::Unweighted) => Ok(1),
            ClauseWeight::Soft(weight) => {
                let SatProblemMode::Weighted { quantization } = mode else {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "unweighted SAT problem received weighted clause",
                    ));
                };
                if self.max_weight <= 0.0 {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "weighted SAT problem has no positive soft-clause weight",
                    ));
                }
                rounded_nonnegative_usize(*weight / self.max_weight * f64::from(quantization))
            }
        }
    }
}

fn top_weight_for_clause_count(mode: SatProblemMode, clause_count: usize) -> AnalysisResult<usize> {
    match mode {
        SatProblemMode::Unweighted => clause_count.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("unweighted SAT top weight overflowed")
        }),
        SatProblemMode::Weighted { quantization } => {
            let quantization = usize::try_from(quantization).map_err(|_| {
                AnalysisError::invalid_detector_error_model(
                    "weighted SAT quantization does not fit usize",
                )
            })?;
            quantization
                .checked_mul(clause_count)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "weighted SAT top weight overflowed",
                    )
                })
        }
    }
}

fn checked_output_add(left: usize, right: usize) -> AnalysisResult<usize> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model("SAT exact output byte count overflowed")
    })
}

fn decimal_digits(value: usize) -> usize {
    value
        .checked_ilog10()
        .map_or(1, |digits| digits as usize + 1)
}

fn rounded_nonnegative_usize(value: f64) -> AnalysisResult<usize> {
    if !value.is_finite() || value < 0.0 {
        return Err(AnalysisError::invalid_detector_error_model(
            "SAT quantized weight is not a finite nonnegative value",
        ));
    }
    let rounded = value.round();
    if rounded > usize::MAX as f64 {
        return Err(AnalysisError::invalid_detector_error_model(
            "SAT quantized weight exceeds usize",
        ));
    }
    format!("{rounded:.0}")
        .parse::<usize>()
        .map_err(|_| AnalysisError::invalid_detector_error_model("SAT quantized weight overflowed"))
}

pub(super) fn validate_limit(
    resource: SatMaterializationResource,
    actual: usize,
    limit: usize,
) -> AnalysisResult<()> {
    if actual > limit {
        let actual = u64::try_from(actual).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "SAT resource amount does not fit resource diagnostics",
            )
        })?;
        let limit = u64::try_from(limit).map_err(|_| {
            AnalysisError::invalid_detector_error_model(
                "SAT resource limit does not fit resource diagnostics",
            )
        })?;
        return Err(ResourceLimitError::sat_materialization(resource, actual, limit).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "unit tests use direct assertions for compact resource diagnostics"
    )]

    use super::*;

    #[test]
    fn sat_shape_rejects_each_resource_above_its_limit() {
        let limits = SatMaterializationLimits::default();
        let baseline = SatShape {
            error_mechanisms: 1,
            target_occurrences: 1,
            variables: 1,
            clauses: 1,
            clause_literals: 1,
        };
        for (shape, expected) in [
            (
                SatShape {
                    error_mechanisms: limits.max_error_mechanisms() + 1,
                    ..baseline
                },
                "error mechanisms",
            ),
            (
                SatShape {
                    target_occurrences: limits.max_target_occurrences() + 1,
                    ..baseline
                },
                "target occurrences",
            ),
            (
                SatShape {
                    variables: limits.max_variables() + 1,
                    ..baseline
                },
                "variables",
            ),
            (
                SatShape {
                    clauses: limits.max_clauses() + 1,
                    ..baseline
                },
                "clauses",
            ),
            (
                SatShape {
                    clause_literals: limits.max_clause_literals() + 1,
                    ..baseline
                },
                "clause literals",
            ),
        ] {
            assert!(
                shape
                    .validate(limits)
                    .expect_err("shape above resource limit")
                    .to_string()
                    .contains(expected)
            );
        }
    }
}
