use rand::{Rng, RngExt as _};

use crate::{
    CircuitError, CircuitResult, PauliBasis, PauliSign, PauliString, StabilizerError, Tableau,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalTableauTransform {
    target_count: usize,
    outputs: Vec<LocalTableauOutput>,
}

impl LocalTableauTransform {
    pub(super) fn from_tableau(tableau: &Tableau) -> CircuitResult<Self> {
        let target_count = tableau.len();
        let output_count = local_basis_count(target_count)?;
        let mut outputs = Vec::with_capacity(output_count);
        for input_index in 0..output_count {
            let input_bases = bases_from_index(input_index, target_count);
            let input = PauliString::from_bases(PauliSign::Plus, input_bases);
            let output = tableau.apply(&input).map_err(map_stabilizer_error)?;
            let mut output_bases = Vec::with_capacity(target_count);
            for target in 0..target_count {
                let Some(basis) = output.get(target) else {
                    return Err(CircuitError::invalid_sampler_compilation(
                        "tableau output length changed while compiling sampler",
                    ));
                };
                output_bases.push(basis);
            }
            outputs.push(LocalTableauOutput {
                negative: output.sign().is_negative(),
                bases: output_bases,
            });
        }
        Ok(Self {
            target_count,
            outputs,
        })
    }

    pub(super) fn target_count(&self) -> usize {
        self.target_count
    }

    fn output(&self, index: usize) -> Option<&LocalTableauOutput> {
        self.outputs.get(index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalTableauOutput {
    negative: bool,
    bases: Vec<PauliBasis>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StabilizerFrame {
    qubit_count: usize,
    generators: Vec<StabilizerGenerator>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MeasurementRandomness {
    Random,
    DeterministicFalse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MeasurementOutcome {
    Deterministic(bool),
    Random { pivot_index: Option<usize> },
}

impl StabilizerFrame {
    pub(super) fn new(qubit_count: usize) -> Self {
        let generators = (0..qubit_count)
            .map(|qubit| StabilizerGenerator::single(qubit_count, qubit, PauliBasis::Z, false))
            .collect();
        Self {
            qubit_count,
            generators,
        }
    }

    pub(super) fn new_unknown(qubit_count: usize) -> Self {
        Self {
            qubit_count,
            generators: Vec::new(),
        }
    }

    pub(super) fn reset_to_z_basis(&mut self) {
        if self.generators.len() != self.qubit_count {
            *self = Self::new(self.qubit_count);
            return;
        }
        for (qubit, generator) in self.generators.iter_mut().enumerate() {
            generator.reset_to_single(self.qubit_count, qubit, PauliBasis::Z, false);
        }
    }

    pub(super) fn apply_tableau(&mut self, targets: &[usize], transform: &LocalTableauTransform) {
        if targets.len() != transform.target_count() {
            return;
        }
        match targets {
            [target] => {
                for generator in &mut self.generators {
                    generator.apply_single_qubit_tableau(*target, transform);
                }
                return;
            }
            [left, right] => {
                for generator in &mut self.generators {
                    generator.apply_two_qubit_tableau(*left, *right, transform);
                }
                return;
            }
            _ => {}
        }
        for generator in &mut self.generators {
            generator.apply_tableau(targets, transform);
        }
    }

    pub(super) fn apply_hadamard(&mut self, qubit: usize) {
        for generator in &mut self.generators {
            generator.apply_hadamard(qubit);
        }
    }

    pub(super) fn apply_controlled_x(&mut self, control: usize, target: usize) {
        for generator in &mut self.generators {
            generator.apply_controlled_x(control, target);
        }
    }

    pub(super) fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        for generator in &mut self.generators {
            generator.apply_pauli(qubit, basis);
        }
    }

    pub(super) fn reset(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        rng: &mut impl Rng,
        randomness: MeasurementRandomness,
    ) {
        let measured = self.measure(qubit, basis, false, rng, randomness);
        if measured {
            self.apply_pauli(qubit, reset_correction(basis));
        }
    }

    pub(super) fn measure(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        inverted: bool,
        rng: &mut impl Rng,
        randomness: MeasurementRandomness,
    ) -> bool {
        let observable = StabilizerGenerator::single(self.len(), qubit, basis, false);
        self.measure_observable(&observable, rng, randomness) ^ inverted
    }

    pub(super) fn measure_is_deterministic(&self, qubit: usize, basis: PauliBasis) -> bool {
        let observable = StabilizerGenerator::single(self.len(), qubit, basis, false);
        self.deterministic_measurement_bit(&observable).is_some()
    }

    pub(super) fn measure_pauli_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        inverted: bool,
        rng: &mut impl Rng,
        randomness: MeasurementRandomness,
    ) -> bool {
        let observable = self.pauli_product_observable(terms);
        self.measure_observable(&observable, rng, randomness) ^ inverted
    }

    pub(super) fn pauli_product_measurement_is_deterministic(
        &self,
        terms: &[(usize, PauliBasis)],
    ) -> bool {
        let observable = self.pauli_product_observable(terms);
        self.deterministic_measurement_bit(&observable).is_some()
    }

    fn measure_observable(
        &mut self,
        observable: &StabilizerGenerator,
        rng: &mut impl Rng,
        randomness: MeasurementRandomness,
    ) -> bool {
        let pivot_index = match self.measurement_outcome(observable) {
            MeasurementOutcome::Deterministic(bit) => return bit,
            MeasurementOutcome::Random { pivot_index } => pivot_index,
        };
        let sampled = random_measurement_bit(rng, randomness);
        let mut collapsed = observable.clone();
        collapsed.negative ^= sampled;
        if let Some(pivot_index) = pivot_index {
            let Some(pivot) = self.generators.get(pivot_index).cloned() else {
                return sampled;
            };
            for (index, generator) in self.generators.iter_mut().enumerate() {
                if index != pivot_index && !generator.commutes_with(observable) {
                    generator.multiply_assign(&pivot);
                }
            }
            if let Some(generator) = self.generators.get_mut(pivot_index) {
                *generator = collapsed;
            }
        } else {
            self.generators.push(collapsed);
        }
        sampled
    }

    fn pauli_product_observable(&self, terms: &[(usize, PauliBasis)]) -> StabilizerGenerator {
        let mut observable = StabilizerGenerator::identity(self.len());
        for (qubit, basis) in terms {
            observable.multiply_assign(&StabilizerGenerator::single(
                self.len(),
                *qubit,
                *basis,
                false,
            ));
        }
        observable
    }

    fn deterministic_measurement_bit(&self, observable: &StabilizerGenerator) -> Option<bool> {
        let MeasurementOutcome::Deterministic(bit) = self.measurement_outcome(observable) else {
            return None;
        };
        Some(bit)
    }

    fn measurement_outcome(&self, observable: &StabilizerGenerator) -> MeasurementOutcome {
        if let Some(generator) = self
            .generators
            .iter()
            .find(|generator| generator.same_bases_as(observable))
        {
            return MeasurementOutcome::Deterministic(generator.negative ^ observable.negative);
        }

        if let Some(pivot_index) = self
            .generators
            .iter()
            .position(|generator| !generator.commutes_with(observable))
        {
            return MeasurementOutcome::Random {
                pivot_index: Some(pivot_index),
            };
        }

        let Some(solution) = self.solve_span(observable) else {
            return MeasurementOutcome::Random { pivot_index: None };
        };
        let mut product = StabilizerGenerator::identity(self.len());
        for (include, generator) in solution.into_iter().zip(&self.generators) {
            if include {
                product.multiply_assign(generator);
            }
        }
        MeasurementOutcome::Deterministic(product.negative ^ observable.negative)
    }

    fn solve_span(&self, observable: &StabilizerGenerator) -> Option<Vec<bool>> {
        let width = self.len().checked_mul(2)?;
        let generator_count = self.generators.len();
        let mut basis = vec![None; width];
        for (generator_index, generator) in self.generators.iter().enumerate() {
            let mut row = SpanRow::from_generator(generator, generator_count, generator_index);
            reduce_span_row(&mut row, &basis);
            if let Some(pivot) = row.first_one()
                && let Some(slot) = basis.get_mut(pivot)
            {
                *slot = Some(row);
            }
        }

        let mut target = SpanRow {
            bits: observable.symplectic_bits(),
            coefficients: vec![false; generator_count],
        };
        for column in 0..width {
            if !target.bit(column) {
                continue;
            }
            let pivot = basis.get(column).and_then(Option::as_ref)?;
            target.xor_assign(pivot);
        }
        if target.bits.iter().any(|bit| *bit) {
            None
        } else {
            Some(target.coefficients)
        }
    }

    fn len(&self) -> usize {
        self.qubit_count
    }
}

fn random_measurement_bit(rng: &mut impl Rng, randomness: MeasurementRandomness) -> bool {
    match randomness {
        MeasurementRandomness::Random => rng.random_bool(0.5),
        MeasurementRandomness::DeterministicFalse => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StabilizerGenerator {
    negative: bool,
    bases: Vec<PauliBasis>,
}

impl StabilizerGenerator {
    fn identity(qubit_count: usize) -> Self {
        Self {
            negative: false,
            bases: vec![PauliBasis::I; qubit_count],
        }
    }

    fn single(qubit_count: usize, qubit: usize, basis: PauliBasis, negative: bool) -> Self {
        let mut generator = Self::identity(qubit_count);
        generator.set_basis(qubit, basis);
        generator.negative = negative;
        generator
    }

    fn reset_to_single(
        &mut self,
        qubit_count: usize,
        qubit: usize,
        basis: PauliBasis,
        negative: bool,
    ) {
        self.negative = negative;
        self.bases.resize(qubit_count, PauliBasis::I);
        self.bases.fill(PauliBasis::I);
        self.set_basis(qubit, basis);
    }

    fn basis(&self, qubit: usize) -> PauliBasis {
        self.bases.get(qubit).copied().unwrap_or(PauliBasis::I)
    }

    fn set_basis(&mut self, qubit: usize, basis: PauliBasis) {
        if let Some(slot) = self.bases.get_mut(qubit) {
            *slot = basis;
        }
    }

    fn apply_tableau(&mut self, targets: &[usize], transform: &LocalTableauTransform) {
        let input_index = self.local_input_index(targets);
        let Some(output) = transform.output(input_index) else {
            return;
        };
        self.negative ^= output.negative;
        for (target, basis) in targets.iter().copied().zip(output.bases.iter().copied()) {
            self.set_basis(target, basis);
        }
    }

    fn apply_single_qubit_tableau(&mut self, target: usize, transform: &LocalTableauTransform) {
        let input_index = basis_digit(self.basis(target));
        let Some(output) = transform.output(input_index) else {
            return;
        };
        self.negative ^= output.negative;
        if let Some(basis) = output.bases.first().copied() {
            self.set_basis(target, basis);
        }
    }

    fn apply_two_qubit_tableau(
        &mut self,
        left: usize,
        right: usize,
        transform: &LocalTableauTransform,
    ) {
        let input_index = basis_digit(self.basis(left))
            .saturating_add(basis_digit(self.basis(right)).saturating_mul(4));
        let Some(output) = transform.output(input_index) else {
            return;
        };
        self.negative ^= output.negative;
        if let Some(basis) = output.bases.first().copied() {
            self.set_basis(left, basis);
        }
        if let Some(basis) = output.bases.get(1).copied() {
            self.set_basis(right, basis);
        }
    }

    fn apply_hadamard(&mut self, qubit: usize) {
        let basis = self.basis(qubit);
        if basis == PauliBasis::Y {
            self.negative = !self.negative;
        }
        self.set_basis(qubit, PauliBasis::from_xz(basis.z_bit(), basis.x_bit()));
    }

    fn apply_controlled_x(&mut self, control: usize, target: usize) {
        let control_basis = self.basis(control);
        let target_basis = self.basis(target);
        let control_x = control_basis.x_bit();
        let control_z = control_basis.z_bit();
        let target_x = target_basis.x_bit();
        let target_z = target_basis.z_bit();
        if control_x && target_z && !(target_x ^ control_z) {
            self.negative = !self.negative;
        }
        self.set_basis(
            control,
            PauliBasis::from_xz(control_x, control_z ^ target_z),
        );
        self.set_basis(target, PauliBasis::from_xz(target_x ^ control_x, target_z));
    }

    fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        if anticommutes(self.basis(qubit), basis) {
            self.negative = !self.negative;
        }
    }

    fn commutes_with(&self, rhs: &Self) -> bool {
        self.bases
            .iter()
            .copied()
            .zip(rhs.bases.iter().copied())
            .filter(|(left, right)| anticommutes(*left, *right))
            .count()
            .is_multiple_of(2)
    }

    fn same_bases_as(&self, rhs: &Self) -> bool {
        self.bases == rhs.bases
    }

    fn multiply_assign(&mut self, rhs: &Self) {
        let mut log_i = sign_log_i(self.negative).wrapping_add(sign_log_i(rhs.negative));
        let len = self.bases.len().max(rhs.bases.len());
        if self.bases.len() < len {
            self.bases.resize(len, PauliBasis::I);
        }
        for index in 0..len {
            let left = self.basis(index);
            let right = rhs.basis(index);
            log_i = log_i.wrapping_add(left.log_i_scalar_byproduct(right));
            self.set_basis(
                index,
                PauliBasis::from_xz(left.x_bit() ^ right.x_bit(), left.z_bit() ^ right.z_bit()),
            );
        }
        self.negative = (log_i & 2) != 0;
    }

    fn symplectic_bits(&self) -> Vec<bool> {
        self.bases
            .iter()
            .map(|basis| basis.x_bit())
            .chain(self.bases.iter().map(|basis| basis.z_bit()))
            .collect()
    }

    fn local_input_index(&self, targets: &[usize]) -> usize {
        let mut index = 0usize;
        let mut scale = 1usize;
        for target in targets {
            index = index.saturating_add(basis_digit(self.basis(*target)).saturating_mul(scale));
            scale = scale.saturating_mul(4);
        }
        index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SpanRow {
    bits: Vec<bool>,
    coefficients: Vec<bool>,
}

impl SpanRow {
    fn from_generator(
        generator: &StabilizerGenerator,
        generator_count: usize,
        index: usize,
    ) -> Self {
        let mut coefficients = vec![false; generator_count];
        if let Some(coefficient) = coefficients.get_mut(index) {
            *coefficient = true;
        }
        Self {
            bits: generator.symplectic_bits(),
            coefficients,
        }
    }

    fn bit(&self, index: usize) -> bool {
        self.bits.get(index).copied().unwrap_or(false)
    }

    fn first_one(&self) -> Option<usize> {
        self.bits.iter().position(|bit| *bit)
    }

    fn xor_assign(&mut self, rhs: &Self) {
        for (bit, rhs_bit) in self.bits.iter_mut().zip(&rhs.bits) {
            *bit ^= *rhs_bit;
        }
        for (coefficient, rhs_coefficient) in self.coefficients.iter_mut().zip(&rhs.coefficients) {
            *coefficient ^= *rhs_coefficient;
        }
    }
}

fn reduce_span_row(row: &mut SpanRow, basis: &[Option<SpanRow>]) {
    for column in 0..row.bits.len() {
        if !row.bit(column) {
            continue;
        }
        let Some(pivot) = basis.get(column).and_then(Option::as_ref) else {
            return;
        };
        row.xor_assign(pivot);
    }
}

fn local_basis_count(target_count: usize) -> CircuitResult<usize> {
    let mut count = 1usize;
    for _ in 0..target_count {
        count = count.checked_mul(4).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "local tableau transform has too many target basis states",
            )
        })?;
    }
    Ok(count)
}

fn bases_from_index(mut index: usize, target_count: usize) -> Vec<PauliBasis> {
    let mut bases = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        bases.push(match index % 4 {
            0 => PauliBasis::I,
            1 => PauliBasis::X,
            2 => PauliBasis::Y,
            _ => PauliBasis::Z,
        });
        index /= 4;
    }
    bases
}

fn basis_digit(basis: PauliBasis) -> usize {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Y => 2,
        PauliBasis::Z => 3,
    }
}

fn sign_log_i(negative: bool) -> u8 {
    if negative { 2 } else { 0 }
}

fn anticommutes(left: PauliBasis, right: PauliBasis) -> bool {
    (left.x_bit() && right.z_bit()) ^ (left.z_bit() && right.x_bit())
}

pub(super) fn reset_correction(basis: PauliBasis) -> PauliBasis {
    match basis {
        PauliBasis::I | PauliBasis::Z => PauliBasis::X,
        PauliBasis::X | PauliBasis::Y => PauliBasis::Z,
    }
}

fn map_stabilizer_error(error: StabilizerError) -> CircuitError {
    CircuitError::invalid_sampler_compilation(error.to_string())
}
