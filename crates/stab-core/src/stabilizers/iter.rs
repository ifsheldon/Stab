use super::{PauliBasis, PauliSign, PauliString, StabilizerError, StabilizerResult, Tableau};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommutingPauliStringIterator {
    num_qubits: usize,
    commutators: Vec<PauliString>,
    anticommutators: Vec<PauliString>,
    next_x_high: u64,
    next_z_high: u64,
    next_low_variant: u64,
}

impl CommutingPauliStringIterator {
    pub fn new(num_qubits: usize) -> StabilizerResult<Self> {
        if num_qubits == 0 || num_qubits >= 64 {
            return Err(StabilizerError::InvalidCommutingPauliIteratorQubitCount { num_qubits });
        }
        Ok(Self {
            num_qubits,
            commutators: Vec::new(),
            anticommutators: Vec::new(),
            next_x_high: 0,
            next_z_high: 0,
            next_low_variant: 0,
        })
    }

    pub fn restart_iter(
        &mut self,
        commutators: &[PauliString],
        anticommutators: &[PauliString],
    ) -> StabilizerResult<()> {
        self.ensure_constraints_match(commutators)?;
        self.ensure_constraints_match(anticommutators)?;
        self.commutators.clear();
        self.commutators.extend_from_slice(commutators);
        self.anticommutators.clear();
        self.anticommutators.extend_from_slice(anticommutators);
        self.restart_iter_same_constraints();
        Ok(())
    }

    pub fn restart_iter_same_constraints(&mut self) {
        self.next_x_high = 0;
        self.next_z_high = 0;
        self.next_low_variant = 0;
    }

    fn ensure_constraints_match(&self, constraints: &[PauliString]) -> StabilizerResult<()> {
        for constraint in constraints {
            if constraint.len() != self.num_qubits {
                return Err(StabilizerError::LengthMismatch {
                    left: constraint.len(),
                    right: self.num_qubits,
                });
            }
        }
        Ok(())
    }

    fn bit_limit(&self) -> u64 {
        1_u64 << self.num_qubits
    }

    fn candidate_from_bits(&self, x_bits: u64, z_bits: u64) -> PauliString {
        let bases = (0..self.num_qubits).map(|index| {
            let mask = 1_u64 << index;
            PauliBasis::from_xz((x_bits & mask) != 0, (z_bits & mask) != 0)
        });
        PauliString::from_bases(PauliSign::Plus, bases)
    }

    fn candidate_matches_constraints(&self, candidate: &PauliString) -> bool {
        self.commutators
            .iter()
            .all(|commutator| matches!(candidate.commutes(commutator), Ok(true)))
            && self
                .anticommutators
                .iter()
                .all(|anticommutator| matches!(candidate.commutes(anticommutator), Ok(false)))
    }

    fn advance_high_bits(&mut self, bit_limit: u64) {
        self.next_low_variant = 0;
        self.next_x_high += 8;
        if self.next_x_high >= bit_limit {
            self.next_x_high = 0;
            self.next_z_high += 8;
        }
    }
}

impl Iterator for CommutingPauliStringIterator {
    type Item = PauliString;

    fn next(&mut self) -> Option<Self::Item> {
        let bit_limit = self.bit_limit();
        while self.next_z_high < bit_limit {
            while self.next_low_variant < 64 {
                let low_variant = self.next_low_variant;
                self.next_low_variant += 1;
                let x_bits = self.next_x_high | (low_variant & 7);
                let z_bits = self.next_z_high | ((low_variant >> 3) & 7);
                if x_bits >= bit_limit || z_bits >= bit_limit || (x_bits == 0 && z_bits == 0) {
                    continue;
                }
                let candidate = self.candidate_from_bits(x_bits, z_bits);
                if self.candidate_matches_constraints(&candidate) {
                    return Some(candidate);
                }
            }
            self.advance_high_bits(bit_limit);
        }
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableauIterator {
    num_qubits: usize,
    also_iter_signs: bool,
    columns: Vec<PauliString>,
    pauli_iterators: Vec<CommutingPauliStringIterator>,
    cur_level: Option<usize>,
    pending_unsigned: Option<Tableau>,
    next_sign_mask: Option<u128>,
    yielded_empty: bool,
}

impl TableauIterator {
    pub fn new(num_qubits: usize, also_iter_signs: bool) -> StabilizerResult<Self> {
        if num_qubits >= 64 {
            return Err(StabilizerError::InvalidTableauIteratorQubitCount { num_qubits });
        }
        let total_columns = num_qubits * 2;
        let mut pauli_iterators = Vec::with_capacity(total_columns);
        for _ in 0..total_columns {
            pauli_iterators.push(CommutingPauliStringIterator::new(num_qubits)?);
        }
        let mut result = Self {
            num_qubits,
            also_iter_signs,
            columns: Vec::with_capacity(total_columns),
            pauli_iterators,
            cur_level: if total_columns == 0 { None } else { Some(0) },
            pending_unsigned: None,
            next_sign_mask: None,
            yielded_empty: false,
        };
        if total_columns > 0 {
            result.restart_level(0)?;
        }
        Ok(result)
    }

    pub fn restart(&mut self) -> StabilizerResult<()> {
        self.columns.clear();
        self.pending_unsigned = None;
        self.next_sign_mask = None;
        self.yielded_empty = false;
        self.cur_level = if self.total_columns() == 0 {
            None
        } else {
            Some(0)
        };
        if self.total_columns() > 0 {
            self.restart_level(0)?;
        }
        Ok(())
    }

    fn total_columns(&self) -> usize {
        self.num_qubits * 2
    }

    fn restart_level(&mut self, level: usize) -> StabilizerResult<()> {
        let mut commutators = self.columns.iter().take(level).cloned().collect::<Vec<_>>();
        let mut anticommutators = Vec::new();
        if level % 2 == 1
            && let Some(anticommutator) = commutators.pop()
        {
            anticommutators.push(anticommutator);
        }
        let len = self.pauli_iterators.len();
        let iterator = self
            .pauli_iterators
            .get_mut(level)
            .ok_or(StabilizerError::TableauIndexOutOfRange { index: level, len })?;
        iterator.restart_iter(&commutators, &anticommutators)
    }

    fn next_unsigned(&mut self) -> Option<Tableau> {
        if self.num_qubits == 0 {
            if self.yielded_empty {
                return None;
            }
            self.yielded_empty = true;
            return Some(Tableau::identity(0));
        }

        while let Some(level) = self.cur_level {
            if self.columns.len() > level {
                self.columns.truncate(level);
            }
            let candidate = self
                .pauli_iterators
                .get_mut(level)
                .and_then(CommutingPauliStringIterator::next);
            if let Some(candidate) = candidate {
                self.columns.push(candidate);
                let next_level = level + 1;
                if next_level == self.total_columns() {
                    return self.tableau_from_columns();
                }
                self.cur_level = Some(next_level);
                if self.restart_level(next_level).is_err() {
                    self.cur_level = None;
                    return None;
                }
            } else if level == 0 {
                self.cur_level = None;
                return None;
            } else {
                self.cur_level = Some(level - 1);
            }
        }
        None
    }

    fn next_signed_variant(&mut self) -> Option<Tableau> {
        let source = self.pending_unsigned.as_ref()?;
        let mask = self.next_sign_mask?;
        let signed = source.with_output_sign_mask(mask);
        if mask == 0 {
            self.pending_unsigned = None;
            self.next_sign_mask = None;
        } else {
            self.next_sign_mask = Some(mask - 1);
        }
        Some(signed)
    }

    fn max_sign_mask(&self) -> u128 {
        let sign_bits = self.total_columns();
        if sign_bits == 0 {
            0
        } else {
            (1_u128 << sign_bits) - 1
        }
    }

    fn tableau_from_columns(&self) -> Option<Tableau> {
        let mut xs = Vec::with_capacity(self.num_qubits);
        let mut zs = Vec::with_capacity(self.num_qubits);
        let mut columns = self.columns.iter();
        for _ in 0..self.num_qubits {
            xs.push(columns.next()?.clone());
            zs.push(columns.next()?.clone());
        }
        Some(Tableau::from_output_columns_unchecked(xs, zs))
    }
}

impl Iterator for TableauIterator {
    type Item = Tableau;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(signed) = self.next_signed_variant() {
                return Some(signed);
            }
            let unsigned = self.next_unsigned()?;
            if self.also_iter_signs {
                self.pending_unsigned = Some(unsigned);
                self.next_sign_mask = Some(self.max_sign_mask());
                continue;
            }
            return Some(unsigned);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauliStringIterator {
    num_qubits: usize,
    min_weight: usize,
    max_weight: usize,
    allowed: AllowedPaulis,
    current_weight: usize,
    positions: Vec<usize>,
    active_bases: Vec<PauliBasis>,
    result: PauliString,
    state: PauliStringIteratorState,
}

impl PauliStringIterator {
    pub fn new(
        num_qubits: usize,
        min_weight: usize,
        max_weight: usize,
        allow_x: bool,
        allow_y: bool,
        allow_z: bool,
    ) -> Self {
        let mut result = Self {
            num_qubits,
            min_weight,
            max_weight: max_weight.min(num_qubits),
            allowed: AllowedPaulis {
                allow_x,
                allow_y,
                allow_z,
            },
            current_weight: min_weight,
            positions: Vec::new(),
            active_bases: Vec::new(),
            result: PauliString::identity(num_qubits),
            state: PauliStringIteratorState::Done,
        };
        result.restart();
        result
    }

    /// Returns the current borrowed iterator result.
    ///
    /// The value is updated after each successful [`Self::iter_next`] call. This avoids allocating
    /// a fresh Pauli string for callers that only need to inspect the current result.
    pub fn result(&self) -> &PauliString {
        &self.result
    }

    /// Advances to the next Pauli string while reusing the borrowed [`Self::result`] storage.
    ///
    /// The standard [`Iterator`] implementation remains available for callers that need owned
    /// `PauliString` values.
    pub fn iter_next(&mut self) -> bool {
        loop {
            match self.state {
                PauliStringIteratorState::Done => return false,
                PauliStringIteratorState::NeedFirst => {
                    if self.prepare_current_weight() {
                        return true;
                    }
                    self.advance_to_next_weight();
                }
                PauliStringIteratorState::Active => {
                    if let Some(first_changed_basis) = self.advance_basis_digits() {
                        self.sync_active_result_from(first_changed_basis);
                        return true;
                    }
                    if self.advance_positions_and_reset_bases() {
                        self.rewrite_active_result();
                        return true;
                    }
                    if !self.advance_to_next_weight() {
                        return false;
                    }
                }
            }
        }
    }

    pub fn restart(&mut self) {
        self.current_weight = self.min_weight;
        self.positions.clear();
        self.active_bases.clear();
        self.result.clear_terms();
        self.state = if self.max_weight < self.min_weight {
            PauliStringIteratorState::Done
        } else {
            PauliStringIteratorState::NeedFirst
        };
    }

    fn prepare_current_weight(&mut self) -> bool {
        self.positions.clear();
        self.active_bases.clear();
        self.result.clear_terms();
        if self.current_weight > 0 {
            let Some(first_basis) = self.allowed.first() else {
                return false;
            };
            self.positions.extend(0..self.current_weight);
            self.active_bases
                .extend(std::iter::repeat_n(first_basis, self.current_weight));
            self.sync_active_result_from(0);
        }
        self.state = PauliStringIteratorState::Active;
        true
    }

    fn advance_basis_digits(&mut self) -> Option<usize> {
        for index in (0..self.active_bases.len()).rev() {
            let next_basis = self
                .active_bases
                .get(index)
                .copied()
                .and_then(|basis| self.allowed.next_after(basis));
            if let Some(next_basis) = next_basis {
                if let Some(active_basis) = self.active_bases.get_mut(index) {
                    *active_basis = next_basis;
                }
                self.reset_bases_after(index);
                return Some(index);
            }
        }
        None
    }

    fn advance_positions_and_reset_bases(&mut self) -> bool {
        let weight = self.positions.len();
        if weight == 0 {
            return false;
        }
        for index in (0..weight).rev() {
            let max_at_index = self.num_qubits - weight + index;
            let advanced_to = if let Some(position) = self.positions.get_mut(index)
                && *position < max_at_index
            {
                *position += 1;
                Some(*position)
            } else {
                None
            };
            if let Some(mut previous) = advanced_to {
                for position in self.positions.iter_mut().skip(index + 1) {
                    previous += 1;
                    *position = previous;
                }
                self.reset_all_bases();
                return true;
            }
        }
        false
    }

    fn advance_to_next_weight(&mut self) -> bool {
        self.positions.clear();
        self.active_bases.clear();
        if self.current_weight >= self.max_weight {
            self.state = PauliStringIteratorState::Done;
            false
        } else {
            self.current_weight += 1;
            self.state = PauliStringIteratorState::NeedFirst;
            true
        }
    }

    fn reset_bases_after(&mut self, index: usize) {
        if let Some(first_basis) = self.allowed.first() {
            for basis in self.active_bases.iter_mut().skip(index + 1) {
                *basis = first_basis;
            }
        }
    }

    fn reset_all_bases(&mut self) {
        if let Some(first_basis) = self.allowed.first() {
            for basis in &mut self.active_bases {
                *basis = first_basis;
            }
        }
    }

    fn rewrite_active_result(&mut self) {
        self.result.clear_terms();
        self.sync_active_result_from(0);
    }

    fn sync_active_result_from(&mut self, start: usize) {
        for (position, basis) in self
            .positions
            .iter()
            .copied()
            .zip(self.active_bases.iter().copied())
            .skip(start)
        {
            self.result.set_in_bounds(position, basis);
        }
    }
}

impl Iterator for PauliStringIterator {
    type Item = PauliString;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter_next() {
            Some(self.result.clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PauliStringIteratorState {
    NeedFirst,
    Active,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllowedPaulis {
    allow_x: bool,
    allow_y: bool,
    allow_z: bool,
}

impl AllowedPaulis {
    fn first(self) -> Option<PauliBasis> {
        if self.allow_x {
            Some(PauliBasis::X)
        } else if self.allow_y {
            Some(PauliBasis::Y)
        } else if self.allow_z {
            Some(PauliBasis::Z)
        } else {
            None
        }
    }

    fn next_after(self, basis: PauliBasis) -> Option<PauliBasis> {
        match basis {
            PauliBasis::I => self.first(),
            PauliBasis::X => {
                if self.allow_y {
                    Some(PauliBasis::Y)
                } else if self.allow_z {
                    Some(PauliBasis::Z)
                } else {
                    None
                }
            }
            PauliBasis::Y => {
                if self.allow_z {
                    Some(PauliBasis::Z)
                } else {
                    None
                }
            }
            PauliBasis::Z => None,
        }
    }
}
