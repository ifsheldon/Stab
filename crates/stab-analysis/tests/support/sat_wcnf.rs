#![allow(
    clippy::expect_used,
    reason = "the independent SAT oracle uses compact parser assertions"
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

pub(crate) fn optimum(source: &str) -> Option<usize> {
    Wcnf::parse(source).optimum()
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
