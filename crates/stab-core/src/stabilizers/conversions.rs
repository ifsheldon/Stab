use super::{PauliBasis, PauliSign, PauliString, StabilizerError, StabilizerResult, Tableau};

pub fn stabilizers_to_tableau(
    stabilizers: &[PauliString],
    allow_redundant: bool,
    allow_underconstrained: bool,
    inverse: bool,
) -> StabilizerResult<Tableau> {
    let num_qubits = stabilizers.iter().map(PauliString::len).max().unwrap_or(0);
    let mut selected = StabilizerSelection::new(num_qubits);
    for stabilizer in stabilizers {
        selected.add_input(stabilizer, allow_redundant)?;
    }
    if selected.len() > num_qubits {
        return Err(StabilizerError::OverconstrainedStabilizers {
            independent: selected.len(),
            num_qubits,
        });
    }
    if selected.len() < num_qubits && !allow_underconstrained {
        return Err(StabilizerError::UnderconstrainedStabilizers {
            independent: selected.len(),
            num_qubits,
        });
    }
    selected.fill_underconstrained_rows()?;

    let zs = selected.outputs;
    let xs = destabilizers_for(&zs)?;
    let tableau = Tableau::from_output_columns_unchecked(xs, zs);
    if !tableau.satisfies_invariants()? {
        return Err(StabilizerError::InvalidStabilizerTableauSynthesis);
    }
    if inverse {
        tableau.inverse()
    } else {
        Ok(tableau)
    }
}

#[derive(Clone, Debug)]
struct StabilizerSelection {
    num_qubits: usize,
    outputs: Vec<PauliString>,
    span: PauliSpan,
}

impl StabilizerSelection {
    fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            outputs: Vec::with_capacity(num_qubits),
            span: PauliSpan::new(num_qubits),
        }
    }

    fn len(&self) -> usize {
        self.outputs.len()
    }

    fn add_input(
        &mut self,
        stabilizer: &PauliString,
        allow_redundant: bool,
    ) -> StabilizerResult<()> {
        let normalized = normalize_pauli(stabilizer, self.num_qubits);
        self.check_commutes_with_selected(&normalized)?;
        match self.span.classify(&normalized)? {
            SpanMembership::Independent(reduced) => {
                self.span.insert_reduced(reduced)?;
                self.outputs.push(normalized);
                Ok(())
            }
            SpanMembership::RedundantExact if allow_redundant => Ok(()),
            SpanMembership::RedundantExact => Err(StabilizerError::RedundantStabilizer {
                stabilizer: normalized.to_string(),
            }),
            SpanMembership::RedundantWrongSign => Err(StabilizerError::InconsistentStabilizer {
                stabilizer: normalized.to_string(),
            }),
        }
    }

    fn fill_underconstrained_rows(&mut self) -> StabilizerResult<()> {
        while self.outputs.len() < self.num_qubits {
            let basis = commuting_nullspace_basis(&self.outputs, self.num_qubits)?;
            let mut found = false;
            for vector in basis {
                let candidate = vector_to_pauli(&vector, self.num_qubits);
                match self.span.classify(&candidate)? {
                    SpanMembership::Independent(reduced) => {
                        self.span.insert_reduced(reduced)?;
                        self.outputs.push(candidate);
                        found = true;
                        break;
                    }
                    SpanMembership::RedundantExact | SpanMembership::RedundantWrongSign => {}
                }
            }
            if !found {
                return Err(StabilizerError::InvalidStabilizerTableauSynthesis);
            }
        }
        Ok(())
    }

    fn check_commutes_with_selected(&self, stabilizer: &PauliString) -> StabilizerResult<()> {
        for selected in &self.outputs {
            if !stabilizer.commutes(selected)? {
                return Err(StabilizerError::AntiCommutingStabilizer {
                    stabilizer: stabilizer.to_string(),
                    conflict: selected.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct PauliSpan {
    num_qubits: usize,
    rows: Vec<SpanRow>,
}

impl PauliSpan {
    fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            rows: Vec::with_capacity(num_qubits),
        }
    }

    fn classify(&self, candidate: &PauliString) -> StabilizerResult<SpanMembership> {
        let reduced = self.reduce(candidate)?;
        if reduced.has_no_pauli_terms() {
            if reduced.sign() == PauliSign::Plus {
                Ok(SpanMembership::RedundantExact)
            } else {
                Ok(SpanMembership::RedundantWrongSign)
            }
        } else {
            Ok(SpanMembership::Independent(reduced))
        }
    }

    fn insert_reduced(&mut self, reduced: PauliString) -> StabilizerResult<()> {
        let pivot = first_vector_bit(&reduced, self.num_qubits)
            .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
        self.rows.push(SpanRow {
            pivot,
            value: reduced,
        });
        self.rows.sort_by_key(|row| row.pivot);
        Ok(())
    }

    fn reduce(&self, candidate: &PauliString) -> StabilizerResult<PauliString> {
        let mut reduced = normalize_pauli(candidate, self.num_qubits);
        for row in &self.rows {
            if has_vector_bit(&reduced, self.num_qubits, row.pivot) {
                reduced = reduced.multiply_real(&row.value)?;
            }
        }
        Ok(reduced)
    }
}

#[derive(Clone, Debug)]
struct SpanRow {
    pivot: usize,
    value: PauliString,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SpanMembership {
    Independent(PauliString),
    RedundantExact,
    RedundantWrongSign,
}

fn destabilizers_for(zs: &[PauliString]) -> StabilizerResult<Vec<PauliString>> {
    let num_qubits = zs.len();
    let mut xs = Vec::with_capacity(num_qubits);
    for index in 0..num_qubits {
        let mut equations = Vec::with_capacity(zs.len() + xs.len());
        for (candidate_index, z) in zs.iter().enumerate() {
            equations.push(symplectic_equation(
                z,
                candidate_index == index,
                num_qubits,
            )?);
        }
        for x in &xs {
            equations.push(symplectic_equation(x, false, num_qubits)?);
        }
        let solution = solve_affine(num_qubits * 2, equations)?
            .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
        xs.push(vector_to_pauli(&solution, num_qubits));
    }
    Ok(xs)
}

fn commuting_nullspace_basis(
    stabilizers: &[PauliString],
    num_qubits: usize,
) -> StabilizerResult<Vec<BinaryVector>> {
    let equations = stabilizers
        .iter()
        .map(|stabilizer| symplectic_equation(stabilizer, false, num_qubits))
        .collect::<StabilizerResult<Vec<_>>>()?;
    nullspace_basis(num_qubits * 2, equations)
}

fn normalize_pauli(pauli: &PauliString, num_qubits: usize) -> PauliString {
    PauliString::from_bases(
        pauli.sign(),
        (0..num_qubits).map(|index| pauli.get(index).unwrap_or(PauliBasis::I)),
    )
}

fn first_vector_bit(pauli: &PauliString, num_qubits: usize) -> Option<usize> {
    (0..num_qubits * 2).find(|&bit| has_vector_bit(pauli, num_qubits, bit))
}

fn has_vector_bit(pauli: &PauliString, num_qubits: usize, bit: usize) -> bool {
    if bit < num_qubits {
        pauli.get(bit).is_some_and(|basis| basis.x_bit())
    } else {
        pauli
            .get(bit - num_qubits)
            .is_some_and(|basis| basis.z_bit())
    }
}

fn symplectic_equation(
    pauli: &PauliString,
    rhs: bool,
    num_qubits: usize,
) -> StabilizerResult<LinearEquation> {
    let mut coefficients = BinaryVector::new(num_qubits * 2);
    for index in 0..num_qubits {
        let basis = pauli.get(index).unwrap_or(PauliBasis::I);
        coefficients.set(index, basis.z_bit())?;
        coefficients.set(num_qubits + index, basis.x_bit())?;
    }
    Ok(LinearEquation { coefficients, rhs })
}

fn solve_affine(
    num_vars: usize,
    equations: Vec<LinearEquation>,
) -> StabilizerResult<Option<BinaryVector>> {
    let reduced = match ReducedSystem::from_equations(num_vars, equations)? {
        Some(reduced) => reduced,
        None => return Ok(None),
    };
    let mut solution = BinaryVector::new(num_vars);
    for pivot in &reduced.pivots {
        let row = reduced
            .rows
            .get(pivot.row)
            .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
        solution.set(pivot.column, row.rhs)?;
    }
    Ok(Some(solution))
}

fn nullspace_basis(
    num_vars: usize,
    equations: Vec<LinearEquation>,
) -> StabilizerResult<Vec<BinaryVector>> {
    let reduced = ReducedSystem::from_equations(num_vars, equations)?
        .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
    let mut is_pivot = vec![false; num_vars];
    for pivot in &reduced.pivots {
        let slot = is_pivot
            .get_mut(pivot.column)
            .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
        *slot = true;
    }

    let mut basis = Vec::new();
    for free_column in 0..num_vars {
        if is_pivot.get(free_column).copied().unwrap_or(false) {
            continue;
        }
        let mut vector = BinaryVector::new(num_vars);
        vector.set(free_column, true)?;
        for pivot in &reduced.pivots {
            let row = reduced
                .rows
                .get(pivot.row)
                .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
            if row.coefficients.get(free_column) {
                vector.set(pivot.column, true)?;
            }
        }
        basis.push(vector);
    }
    Ok(basis)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReducedSystem {
    rows: Vec<LinearEquation>,
    pivots: Vec<Pivot>,
}

impl ReducedSystem {
    fn from_equations(
        num_vars: usize,
        equations: Vec<LinearEquation>,
    ) -> StabilizerResult<Option<Self>> {
        let mut rows = equations;
        let mut pivots = Vec::new();
        let mut next_pivot_row = 0;
        for column in 0..num_vars {
            let Some(pivot_row) = rows
                .iter()
                .enumerate()
                .skip(next_pivot_row)
                .find(|(_, row)| row.coefficients.get(column))
                .map(|(row, _)| row)
            else {
                continue;
            };
            rows.swap(next_pivot_row, pivot_row);
            let pivot = rows
                .get(next_pivot_row)
                .cloned()
                .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
            for (row_index, row) in rows.iter_mut().enumerate() {
                if row_index != next_pivot_row && row.coefficients.get(column) {
                    row.xor_assign(&pivot);
                }
            }
            pivots.push(Pivot {
                row: next_pivot_row,
                column,
            });
            next_pivot_row += 1;
        }
        for row in &rows {
            if row.coefficients.is_zero() && row.rhs {
                return Ok(None);
            }
        }
        Ok(Some(Self { rows, pivots }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Pivot {
    row: usize,
    column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinearEquation {
    coefficients: BinaryVector,
    rhs: bool,
}

impl LinearEquation {
    fn xor_assign(&mut self, rhs: &Self) {
        self.coefficients.xor_assign(&rhs.coefficients);
        self.rhs ^= rhs.rhs;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BinaryVector {
    len: usize,
    words: Vec<u64>,
}

impl BinaryVector {
    fn new(len: usize) -> Self {
        Self {
            len,
            words: vec![0; len.div_ceil(64)],
        }
    }

    fn get(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }
        self.words
            .get(index / 64)
            .is_some_and(|word| (word & (1_u64 << (index % 64))) != 0)
    }

    fn set(&mut self, index: usize, value: bool) -> StabilizerResult<()> {
        if index >= self.len {
            return Err(StabilizerError::InvalidStabilizerTableauSynthesis);
        }
        let word = self
            .words
            .get_mut(index / 64)
            .ok_or(StabilizerError::InvalidStabilizerTableauSynthesis)?;
        let mask = 1_u64 << (index % 64);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        Ok(())
    }

    fn xor_assign(&mut self, rhs: &Self) {
        for (left, right) in self.words.iter_mut().zip(&rhs.words) {
            *left ^= *right;
        }
    }

    fn is_zero(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }
}

fn vector_to_pauli(vector: &BinaryVector, num_qubits: usize) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..num_qubits)
            .map(|index| PauliBasis::from_xz(vector.get(index), vector.get(num_qubits + index))),
    )
}
