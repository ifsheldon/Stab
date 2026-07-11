use crate::{CircuitError, CircuitResult};

pub(super) const MAX_SAT_ERROR_MECHANISMS: usize = 250_000;
pub(super) const MAX_SAT_TARGET_OCCURRENCES: usize = 500_000;
const MAX_SAT_VARIABLES: usize = 500_000;
const MAX_SAT_CLAUSES: usize = 500_000;
const MAX_SAT_CLAUSE_LITERALS: usize = 1_500_000;
const MAX_SAT_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

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
    pub(super) output_bytes: usize,
}

impl SatShape {
    pub(super) fn with_output_bound(mut self, mode: SatProblemMode) -> CircuitResult<Self> {
        let top = top_weight_for_clause_count(mode, self.clauses)?;
        self.output_bytes =
            estimated_output_bytes(self.variables, self.clauses, self.clause_literals, top)?;
        self.validate()
    }

    pub(super) fn validate(self) -> CircuitResult<Self> {
        validate_limit(
            "error mechanisms",
            self.error_mechanisms,
            MAX_SAT_ERROR_MECHANISMS,
        )?;
        validate_limit(
            "target occurrences",
            self.target_occurrences,
            MAX_SAT_TARGET_OCCURRENCES,
        )?;
        validate_limit("variables", self.variables, MAX_SAT_VARIABLES)?;
        validate_limit("clauses", self.clauses, MAX_SAT_CLAUSES)?;
        validate_limit(
            "clause literals",
            self.clause_literals,
            MAX_SAT_CLAUSE_LITERALS,
        )?;
        validate_limit(
            "WDIMACS output bytes",
            self.output_bytes,
            MAX_SAT_OUTPUT_BYTES,
        )?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct MaxSatInstance {
    num_variables: usize,
    max_weight: f64,
    clauses: Vec<Clause>,
    clause_literals: usize,
}

impl MaxSatInstance {
    pub(super) fn with_shape(shape: SatShape) -> CircuitResult<Self> {
        let shape = shape.validate()?;
        let mut clauses = Vec::new();
        clauses.try_reserve_exact(shape.clauses).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "SAT problem generation cannot reserve {} clauses",
                shape.clauses
            ))
        })?;
        Ok(Self {
            clauses,
            ..Self::default()
        })
    }

    pub(super) fn new_bool(&mut self) -> CircuitResult<BoolRef> {
        let variable = self.num_variables;
        let next = self.num_variables.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT variable count overflowed")
        })?;
        validate_limit("variables", next, MAX_SAT_VARIABLES)?;
        self.num_variables = next;
        Ok(BoolRef::variable(variable))
    }

    pub(super) fn add_clause(&mut self, clause: Clause) -> CircuitResult<()> {
        if let ClauseWeight::Soft(weight) = clause.weight {
            if !weight.is_finite() || weight <= 0.0 {
                return Err(CircuitError::invalid_detector_error_model(
                    "SAT soft clause weight must be finite and positive",
                ));
            }
            self.max_weight = self.max_weight.max(weight);
        }
        let clause_count = self.clauses.len().checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT clause count overflowed")
        })?;
        validate_limit("clauses", clause_count, MAX_SAT_CLAUSES)?;
        let literal_count = self
            .clause_literals
            .checked_add(clause.vars.len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("SAT clause literal count overflowed")
            })?;
        validate_limit("clause literals", literal_count, MAX_SAT_CLAUSE_LITERALS)?;
        self.clauses.push(clause);
        self.clause_literals = literal_count;
        Ok(())
    }

    pub(super) fn xor(&mut self, left: BoolRef, right: BoolRef) -> CircuitResult<BoolRef> {
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

    pub(super) fn validate_shape(&self, shape: SatShape) -> CircuitResult<()> {
        if self.num_variables != shape.variables
            || self.clauses.len() != shape.clauses
            || self.clause_literals != shape.clause_literals
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
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

    pub(super) fn to_wdimacs(&self, mode: SatProblemMode) -> CircuitResult<String> {
        let clause_count = self.clauses.len();
        let top = self.top_weight(mode, clause_count)?;
        let output_bound =
            estimated_output_bytes(self.num_variables, clause_count, self.clause_literals, top)?;
        validate_limit("WDIMACS output bytes", output_bound, MAX_SAT_OUTPUT_BYTES)?;
        let mut out = String::new();
        out.try_reserve(output_bound).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
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

    fn top_weight(&self, mode: SatProblemMode, clause_count: usize) -> CircuitResult<usize> {
        top_weight_for_clause_count(mode, clause_count)
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

fn top_weight_for_clause_count(mode: SatProblemMode, clause_count: usize) -> CircuitResult<usize> {
    match mode {
        SatProblemMode::Unweighted => clause_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("unweighted SAT top weight overflowed")
        }),
        SatProblemMode::Weighted { quantization } => {
            let quantization = usize::try_from(quantization).map_err(|_| {
                CircuitError::invalid_detector_error_model(
                    "weighted SAT quantization does not fit usize",
                )
            })?;
            quantization
                .checked_mul(clause_count)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("weighted SAT top weight overflowed")
                })
        }
    }
}

fn estimated_output_bytes(
    variables: usize,
    clauses: usize,
    literals: usize,
    top: usize,
) -> CircuitResult<usize> {
    let header = 16_usize
        .checked_add(decimal_digits(variables))
        .and_then(|value| value.checked_add(decimal_digits(clauses)))
        .and_then(|value| value.checked_add(decimal_digits(top)))
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT output byte estimate overflowed")
        })?;
    let clause_overhead = clauses
        .checked_mul(decimal_digits(top).saturating_add(3))
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT output byte estimate overflowed")
        })?;
    let literal_width = decimal_digits(variables).saturating_add(2);
    header
        .checked_add(clause_overhead)
        .and_then(|value| value.checked_add(literals.checked_mul(literal_width)?))
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model("SAT output byte estimate overflowed")
        })
}

fn decimal_digits(value: usize) -> usize {
    value
        .checked_ilog10()
        .map_or(1, |digits| digits as usize + 1)
}

fn rounded_nonnegative_usize(value: f64) -> CircuitResult<usize> {
    if !value.is_finite() || value < 0.0 {
        return Err(CircuitError::invalid_detector_error_model(
            "SAT quantized weight is not a finite nonnegative value",
        ));
    }
    let rounded = value.round();
    if rounded > usize::MAX as f64 {
        return Err(CircuitError::invalid_detector_error_model(
            "SAT quantized weight exceeds usize",
        ));
    }
    format!("{rounded:.0}")
        .parse::<usize>()
        .map_err(|_| CircuitError::invalid_detector_error_model("SAT quantized weight overflowed"))
}

fn validate_limit(label: &str, actual: usize, limit: usize) -> CircuitResult<()> {
    if actual > limit {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "SAT problem generation currently supports at most {limit} {label}, got at least {actual}"
        )));
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
        let baseline = SatShape {
            error_mechanisms: 1,
            target_occurrences: 1,
            variables: 1,
            clauses: 1,
            clause_literals: 1,
            output_bytes: 1,
        };
        for (shape, expected) in [
            (
                SatShape {
                    error_mechanisms: MAX_SAT_ERROR_MECHANISMS + 1,
                    ..baseline
                },
                "error mechanisms",
            ),
            (
                SatShape {
                    target_occurrences: MAX_SAT_TARGET_OCCURRENCES + 1,
                    ..baseline
                },
                "target occurrences",
            ),
            (
                SatShape {
                    variables: MAX_SAT_VARIABLES + 1,
                    ..baseline
                },
                "variables",
            ),
            (
                SatShape {
                    clauses: MAX_SAT_CLAUSES + 1,
                    ..baseline
                },
                "clauses",
            ),
            (
                SatShape {
                    clause_literals: MAX_SAT_CLAUSE_LITERALS + 1,
                    ..baseline
                },
                "clause literals",
            ),
            (
                SatShape {
                    output_bytes: MAX_SAT_OUTPUT_BYTES + 1,
                    ..baseline
                },
                "WDIMACS output bytes",
            ),
        ] {
            assert!(
                shape
                    .validate()
                    .expect_err("shape above resource limit")
                    .to_string()
                    .contains(expected)
            );
        }
    }
}
