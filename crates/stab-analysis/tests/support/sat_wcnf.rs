#![allow(
    dead_code,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "the independent SAT oracle indexes variables it validated while parsing; test crates use a subset"
)]

#[derive(Debug)]
struct Clause {
    weight: usize,
    literals: Vec<i64>,
}

#[derive(Debug)]
struct Wcnf {
    variables: usize,
    top: usize,
    clauses: Vec<Clause>,
}

/// Exhaustive optimum over every assignment; only for instances with a handful of variables.
pub(crate) fn optimum(source: &str) -> Option<usize> {
    Wcnf::parse(source).optimum()
}

/// Exact optimum by branch and bound with unit propagation over hard clauses.
///
/// Independent of the encoder: it reads only the WCNF text. Search cost grows with the optimum,
/// not the variable count, so it handles small-distance QEC instances that are far beyond
/// [`optimum`].
pub(crate) fn optimum_by_branch_and_bound(source: &str) -> Option<usize> {
    let wcnf = Wcnf::parse(source);
    let mut occurrences = vec![Vec::new(); wcnf.variables];
    for (index, clause) in wcnf.clauses.iter().enumerate() {
        for &literal in &clause.literals {
            occurrences[variable_of(literal)].push(index);
        }
    }
    let mut search = BranchAndBound {
        wcnf: &wcnf,
        occurrences,
        best: None,
    };
    let assignment = vec![None; wcnf.variables];
    let all_variables = (0..wcnf.variables).collect();
    search.branch(assignment, all_variables);
    search.best
}

fn variable_of(literal: i64) -> usize {
    usize::try_from(literal.unsigned_abs())
        .expect("WCNF literal fits the target pointer width")
        .checked_sub(1)
        .expect("nonzero WCNF literal")
}

struct BranchAndBound<'a> {
    wcnf: &'a Wcnf,
    occurrences: Vec<Vec<usize>>,
    best: Option<usize>,
}

impl BranchAndBound<'_> {
    fn branch(&mut self, mut assignment: Vec<Option<bool>>, touched: Vec<usize>) {
        if !self.propagate(&mut assignment, touched) {
            return;
        }
        let cost = self
            .wcnf
            .clauses
            .iter()
            .filter(|clause| clause.weight < self.wcnf.top)
            .filter(|clause| {
                clause.literals.iter().all(|&literal| {
                    assignment[variable_of(literal)].is_some_and(|value| value != (literal > 0))
                })
            })
            .map(|clause| clause.weight)
            .sum::<usize>();
        if self.best.is_some_and(|best| cost >= best) {
            return;
        }
        let Some(variable) = assignment.iter().position(Option::is_none) else {
            self.best = Some(cost);
            return;
        };
        for value in [false, true] {
            let mut next = assignment.clone();
            next[variable] = Some(value);
            self.branch(next, vec![variable]);
        }
    }

    /// Unit propagation over hard clauses touching newly assigned variables; false on conflict.
    fn propagate(&self, assignment: &mut [Option<bool>], mut pending: Vec<usize>) -> bool {
        while let Some(variable) = pending.pop() {
            for &index in &self.occurrences[variable] {
                let clause = &self.wcnf.clauses[index];
                if clause.weight < self.wcnf.top {
                    continue;
                }
                let mut unassigned = None;
                let mut unassigned_count = 0;
                let mut satisfied = false;
                for &literal in &clause.literals {
                    match assignment[variable_of(literal)] {
                        Some(value) if value == (literal > 0) => {
                            satisfied = true;
                            break;
                        }
                        Some(_) => {}
                        None => {
                            unassigned = Some(literal);
                            unassigned_count += 1;
                        }
                    }
                }
                if satisfied || unassigned_count > 1 {
                    continue;
                }
                let Some(literal) = unassigned else {
                    return false;
                };
                let forced = variable_of(literal);
                assignment[forced] = Some(literal > 0);
                pending.push(forced);
            }
        }
        true
    }
}

impl Wcnf {
    fn parse(source: &str) -> Self {
        let mut lines = source.lines();
        let header = lines.next().expect("WCNF header");
        let mut fields = header.split_whitespace();
        assert_eq!(fields.next(), Some("p"));
        assert_eq!(fields.next(), Some("wcnf"));
        let variables = fields
            .next()
            .expect("WCNF variable count")
            .parse()
            .expect("numeric WCNF variable count");
        let declared_clauses: usize = fields
            .next()
            .expect("WCNF clause count")
            .parse()
            .expect("numeric WCNF clause count");
        let top = fields
            .next()
            .expect("WCNF top weight")
            .parse()
            .expect("numeric WCNF top weight");
        assert_eq!(fields.next(), None, "unexpected WCNF header field");

        let clauses = lines
            .map(|line| {
                let mut fields = line.split_whitespace();
                let weight = fields
                    .next()
                    .expect("WCNF clause weight")
                    .parse()
                    .expect("numeric WCNF clause weight");
                assert!(weight <= top, "clause weight exceeds WCNF top");
                let mut literals = Vec::new();
                loop {
                    let literal: i64 = fields
                        .next()
                        .expect("WCNF clause terminator")
                        .parse()
                        .expect("numeric WCNF literal");
                    if literal == 0 {
                        break;
                    }
                    literals.push(literal);
                }
                assert_eq!(fields.next(), None, "field after WCNF clause terminator");
                Clause { weight, literals }
            })
            .collect::<Vec<_>>();
        assert_eq!(clauses.len(), declared_clauses);
        Self {
            variables,
            top,
            clauses,
        }
    }

    fn optimum(&self) -> Option<usize> {
        assert!(self.variables < usize::BITS as usize);
        let mut optimum = None;
        for assignment in 0..(1usize << self.variables) {
            let mut cost = 0usize;
            let mut feasible = true;
            for clause in &self.clauses {
                let satisfied = clause.literals.iter().any(|&literal| {
                    let one_based = usize::try_from(literal.unsigned_abs())
                        .expect("WCNF literal fits the target pointer width");
                    let variable = one_based.checked_sub(1).expect("nonzero WCNF literal");
                    assert!(variable < self.variables);
                    let value = assignment & (1usize << variable) != 0;
                    if literal < 0 { !value } else { value }
                });
                if !satisfied && clause.weight == self.top {
                    feasible = false;
                    break;
                }
                if !satisfied {
                    cost = cost.checked_add(clause.weight).expect("WCNF cost");
                }
            }
            if feasible {
                optimum = Some(optimum.map_or(cost, |current: usize| current.min(cost)));
            }
        }
        optimum
    }
}
