use super::{PauliBasis, PauliSign, PauliString};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauliStringIterator {
    num_qubits: usize,
    min_weight: usize,
    max_weight: usize,
    allowed: AllowedPaulis,
    current_weight: usize,
    positions: Vec<usize>,
    active_bases: Vec<PauliBasis>,
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
            state: PauliStringIteratorState::Done,
        };
        result.restart();
        result
    }

    pub fn restart(&mut self) {
        self.current_weight = self.min_weight;
        self.positions.clear();
        self.active_bases.clear();
        self.state = if self.max_weight < self.min_weight {
            PauliStringIteratorState::Done
        } else {
            PauliStringIteratorState::NeedFirst
        };
    }

    fn prepare_current_weight(&mut self) -> bool {
        self.positions.clear();
        self.active_bases.clear();
        if self.current_weight > 0 {
            let Some(first_basis) = self.allowed.first() else {
                return false;
            };
            self.positions.extend(0..self.current_weight);
            self.active_bases
                .extend(std::iter::repeat_n(first_basis, self.current_weight));
        }
        self.state = PauliStringIteratorState::Active;
        true
    }

    fn current_result(&self) -> PauliString {
        let mut active_terms = self
            .positions
            .iter()
            .copied()
            .zip(self.active_bases.iter().copied())
            .peekable();
        let bases = (0..self.num_qubits).map(|index| {
            if active_terms
                .peek()
                .is_some_and(|(position, _basis)| *position == index)
            {
                active_terms
                    .next()
                    .map_or(PauliBasis::I, |(_position, basis)| basis)
            } else {
                PauliBasis::I
            }
        });
        PauliString::from_bases(PauliSign::Plus, bases)
    }

    fn advance_basis_digits(&mut self) -> bool {
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
                return true;
            }
        }
        false
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
}

impl Iterator for PauliStringIterator {
    type Item = PauliString;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.state {
                PauliStringIteratorState::Done => return None,
                PauliStringIteratorState::NeedFirst => {
                    if self.prepare_current_weight() {
                        return Some(self.current_result());
                    }
                    self.advance_to_next_weight();
                }
                PauliStringIteratorState::Active => {
                    if self.advance_basis_digits() || self.advance_positions_and_reset_bases() {
                        return Some(self.current_result());
                    }
                    if !self.advance_to_next_weight() {
                        return None;
                    }
                }
            }
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
