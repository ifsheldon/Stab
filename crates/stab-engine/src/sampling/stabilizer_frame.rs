use std::fmt;

use rand::{Rng, RngExt as _};
use smallvec::{SmallVec, smallvec};
use stab_algebra::{PauliBasis, PauliSign, StabilizerError, Tableau};

use super::SamplingCompileError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalTableauTransform {
    target_count: usize,
    outputs: Vec<LocalTableauOutput>,
}

impl LocalTableauTransform {
    pub(super) fn from_tableau(tableau: &Tableau) -> Result<Self, SamplingCompileError> {
        let target_count = tableau.len();
        let output_count = local_basis_count(target_count)?;
        let mut outputs = Vec::with_capacity(output_count);
        for input_index in 0..output_count {
            let input_bases = bases_from_index(input_index, target_count);
            let input =
                stab_algebra::advanced::pauli_from_bases_unchecked(PauliSign::Plus, input_bases);
            let output = tableau.apply(&input).map_err(map_stabilizer_error)?;
            let mut output_bases = Vec::with_capacity(target_count);
            for target in 0..target_count {
                let Some(basis) = output.get(target) else {
                    return Err(SamplingCompileError::invalid_circuit(
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

#[derive(Clone, Debug)]
pub(super) struct StabilizerFrame {
    qubit_count: usize,
    destabilizers: Vec<StabilizerGenerator>,
    generators: Vec<StabilizerGenerator>,
    observable_scratch: StabilizerGenerator,
    pivot_scratch: StabilizerGenerator,
    collapsed_scratch: StabilizerGenerator,
    span_scratch: SpanScratch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FrameStorageError {
    component: &'static str,
    items: usize,
}

impl FrameStorageError {
    pub(super) const fn new(component: &'static str, items: usize) -> Self {
        Self { component, items }
    }
}

impl fmt::Display for FrameStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not allocate {0} with {1} items",
            self.component, self.items
        )
    }
}

impl PartialEq for StabilizerFrame {
    fn eq(&self, other: &Self) -> bool {
        self.qubit_count == other.qubit_count && self.generators == other.generators
    }
}

impl Eq for StabilizerFrame {}

#[derive(Debug)]
pub(super) struct StabilizerStateSnapshot {
    generators: Vec<StabilizerGenerator>,
}

impl StabilizerStateSnapshot {
    pub(super) fn try_new(qubit_count: usize) -> Result<Self, FrameStorageError> {
        let mut generators = Vec::new();
        generators
            .try_reserve_exact(qubit_count)
            .map_err(|_| FrameStorageError::new("stabilizer snapshot generators", qubit_count))?;
        for _ in 0..qubit_count {
            generators.push(StabilizerGenerator::try_identity(qubit_count)?);
        }
        Ok(Self { generators })
    }

    pub(super) fn capture(&mut self, frame: &StabilizerFrame) -> bool {
        if self.generators.len() != frame.generators.len() {
            return false;
        }
        for (destination, source) in self.generators.iter_mut().zip(&frame.generators) {
            destination.copy_from_generator(source);
        }
        true
    }

    pub(super) fn matches(&self, frame: &StabilizerFrame) -> bool {
        self.generators == frame.generators
    }

    pub(super) fn storage_bytes(qubit_count: usize) -> u128 {
        (qubit_count as u128)
            .saturating_mul(size_of::<StabilizerGenerator>() as u128)
            .saturating_add(
                (qubit_count as u128)
                    .saturating_mul(qubit_count as u128)
                    .saturating_mul(size_of::<PauliBasis>() as u128),
            )
    }
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
        let destabilizers = (0..qubit_count)
            .map(|qubit| StabilizerGenerator::single(qubit_count, qubit, PauliBasis::X, false))
            .collect();
        Self {
            qubit_count,
            destabilizers,
            generators,
            observable_scratch: StabilizerGenerator::identity(qubit_count),
            pivot_scratch: StabilizerGenerator::identity(qubit_count),
            collapsed_scratch: StabilizerGenerator::identity(qubit_count),
            span_scratch: SpanScratch::default(),
        }
    }

    pub(super) fn try_new(qubit_count: usize) -> Result<Self, FrameStorageError> {
        let mut generators = Vec::new();
        generators
            .try_reserve_exact(qubit_count)
            .map_err(|_| FrameStorageError::new("stabilizer generators", qubit_count))?;
        for qubit in 0..qubit_count {
            generators.push(StabilizerGenerator::try_single(
                qubit_count,
                qubit,
                PauliBasis::Z,
                false,
            )?);
        }
        let mut destabilizers = Vec::new();
        destabilizers
            .try_reserve_exact(qubit_count)
            .map_err(|_| FrameStorageError::new("destabilizer generators", qubit_count))?;
        for qubit in 0..qubit_count {
            destabilizers.push(StabilizerGenerator::try_single(
                qubit_count,
                qubit,
                PauliBasis::X,
                false,
            )?);
        }
        Ok(Self {
            qubit_count,
            destabilizers,
            generators,
            observable_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            pivot_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            collapsed_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            span_scratch: SpanScratch::try_new(qubit_count)?,
        })
    }

    pub(super) fn try_new_unknown(qubit_count: usize) -> Result<Self, FrameStorageError> {
        Ok(Self {
            qubit_count,
            destabilizers: Vec::new(),
            generators: Vec::new(),
            observable_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            pivot_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            collapsed_scratch: StabilizerGenerator::try_identity(qubit_count)?,
            span_scratch: SpanScratch::try_new(qubit_count)?,
        })
    }

    pub(super) fn reset_to_z_basis(&mut self) {
        if self.generators.len() < self.qubit_count {
            *self = Self::new(self.qubit_count);
            return;
        }
        while self.generators.len() > self.qubit_count {
            let Some(mut spare) = self.generators.pop() else {
                break;
            };
            spare.reset_to_identity(self.qubit_count);
            self.collapsed_scratch = spare;
        }
        for (qubit, generator) in self.generators.iter_mut().enumerate() {
            generator.reset_to_single(self.qubit_count, qubit, PauliBasis::Z, false);
        }
        if self.destabilizers.len() != self.qubit_count {
            self.destabilizers = (0..self.qubit_count)
                .map(|qubit| {
                    StabilizerGenerator::single(self.qubit_count, qubit, PauliBasis::X, false)
                })
                .collect();
        } else {
            for (qubit, generator) in self.destabilizers.iter_mut().enumerate() {
                generator.reset_to_single(self.qubit_count, qubit, PauliBasis::X, false);
            }
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
                for generator in &mut self.destabilizers {
                    generator.apply_single_qubit_tableau(*target, transform);
                }
                return;
            }
            [left, right] => {
                for generator in &mut self.generators {
                    generator.apply_two_qubit_tableau(*left, *right, transform);
                }
                for generator in &mut self.destabilizers {
                    generator.apply_two_qubit_tableau(*left, *right, transform);
                }
                return;
            }
            _ => {}
        }
        for generator in &mut self.generators {
            generator.apply_tableau(targets, transform);
        }
        for generator in &mut self.destabilizers {
            generator.apply_tableau(targets, transform);
        }
    }

    pub(super) fn apply_hadamard(&mut self, qubit: usize) {
        for generator in &mut self.generators {
            generator.apply_hadamard(qubit);
        }
        for generator in &mut self.destabilizers {
            generator.apply_hadamard(qubit);
        }
    }

    pub(super) fn apply_controlled_x(&mut self, control: usize, target: usize) {
        for generator in &mut self.generators {
            generator.apply_controlled_x(control, target);
        }
        for generator in &mut self.destabilizers {
            generator.apply_controlled_x(control, target);
        }
    }

    pub(super) fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        for generator in &mut self.generators {
            generator.apply_pauli(qubit, basis);
        }
        for generator in &mut self.destabilizers {
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
        let mut observable = std::mem::take(&mut self.observable_scratch);
        observable.reset_to_single(self.len(), qubit, basis, false);
        let result = self.measure_observable(&observable, rng, randomness) ^ inverted;
        observable.reset_to_identity(self.len());
        self.observable_scratch = observable;
        result
    }

    pub(super) fn measure_is_deterministic(&mut self, qubit: usize, basis: PauliBasis) -> bool {
        let mut observable = std::mem::take(&mut self.observable_scratch);
        observable.reset_to_single(self.len(), qubit, basis, false);
        let deterministic = self.deterministic_measurement_bit(&observable).is_some();
        observable.reset_to_identity(self.len());
        self.observable_scratch = observable;
        deterministic
    }

    pub(super) fn measure_pauli_product(
        &mut self,
        terms: &[(usize, PauliBasis)],
        inverted: bool,
        rng: &mut impl Rng,
        randomness: MeasurementRandomness,
    ) -> bool {
        let mut observable = std::mem::take(&mut self.observable_scratch);
        observable.reset_to_identity(self.len());
        for (qubit, basis) in terms {
            observable.multiply_single_assign(*qubit, *basis);
        }
        let result = self.measure_observable(&observable, rng, randomness) ^ inverted;
        observable.reset_to_identity(self.len());
        self.observable_scratch = observable;
        result
    }

    pub(super) fn pauli_product_measurement_is_deterministic(
        &mut self,
        terms: &[(usize, PauliBasis)],
    ) -> bool {
        let mut observable = std::mem::take(&mut self.observable_scratch);
        observable.reset_to_identity(self.len());
        for (qubit, basis) in terms {
            observable.multiply_single_assign(*qubit, *basis);
        }
        let deterministic = self.deterministic_measurement_bit(&observable).is_some();
        observable.reset_to_identity(self.len());
        self.observable_scratch = observable;
        deterministic
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
        if let Some(pivot_index) = pivot_index {
            let mut pivot = std::mem::take(&mut self.pivot_scratch);
            let Some(source) = self.generators.get(pivot_index) else {
                self.pivot_scratch = pivot;
                return sampled;
            };
            pivot.copy_from_generator(source);
            for (index, generator) in self.generators.iter_mut().enumerate() {
                if index != pivot_index && !generator.commutes_with(observable) {
                    generator.multiply_assign(&pivot);
                }
            }
            for (index, destabilizer) in self.destabilizers.iter_mut().enumerate() {
                if index != pivot_index && !destabilizer.commutes_with(observable) {
                    destabilizer.multiply_assign(&pivot);
                }
            }
            if let Some(destabilizer) = self.destabilizers.get_mut(pivot_index) {
                destabilizer.copy_from_generator(&pivot);
            }
            if let Some(generator) = self.generators.get_mut(pivot_index) {
                generator.copy_from_generator(observable);
                generator.negative ^= sampled;
            }
            pivot.reset_to_identity(self.qubit_count);
            self.pivot_scratch = pivot;
        } else {
            self.destabilizers.clear();
            let mut collapsed = std::mem::take(&mut self.collapsed_scratch);
            collapsed.copy_from_generator(observable);
            collapsed.negative ^= sampled;
            self.generators.push(collapsed);
        }
        sampled
    }

    fn deterministic_measurement_bit(&mut self, observable: &StabilizerGenerator) -> Option<bool> {
        let MeasurementOutcome::Deterministic(bit) = self.measurement_outcome(observable) else {
            return None;
        };
        Some(bit)
    }

    fn measurement_outcome(&mut self, observable: &StabilizerGenerator) -> MeasurementOutcome {
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

        let dual_basis_result = if self.generators.len() == self.qubit_count
            && self.destabilizers.len() == self.qubit_count
        {
            let mut product = std::mem::take(&mut self.collapsed_scratch);
            product.reset_to_identity(self.qubit_count);
            for (destabilizer, generator) in self.destabilizers.iter().zip(&self.generators) {
                if !destabilizer.commutes_with(observable) {
                    product.multiply_assign(generator);
                }
            }
            let matches = product.same_bases_as(observable);
            let negative = product.negative;
            product.reset_to_identity(self.qubit_count);
            self.collapsed_scratch = product;
            matches.then_some(negative)
        } else {
            None
        };
        let product_negative = match dual_basis_result {
            Some(product_negative) => product_negative,
            None => {
                let Some(product_negative) = self.span_scratch.solve_negative(
                    &self.generators,
                    observable,
                    self.qubit_count,
                ) else {
                    return MeasurementOutcome::Random { pivot_index: None };
                };
                product_negative
            }
        };
        MeasurementOutcome::Deterministic(product_negative ^ observable.negative)
    }

    pub(super) fn len(&self) -> usize {
        self.qubit_count
    }
}

fn random_measurement_bit(rng: &mut impl Rng, randomness: MeasurementRandomness) -> bool {
    match randomness {
        MeasurementRandomness::Random => rng.random_bool(0.5),
        MeasurementRandomness::DeterministicFalse => false,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct StabilizerGenerator {
    negative: bool,
    qubit_count: usize,
    xs: SmallVec<[u64; 4]>,
    zs: SmallVec<[u64; 4]>,
}

impl StabilizerGenerator {
    fn identity(qubit_count: usize) -> Self {
        Self {
            negative: false,
            qubit_count,
            xs: smallvec![0; word_count(qubit_count)],
            zs: smallvec![0; word_count(qubit_count)],
        }
    }

    fn try_identity(qubit_count: usize) -> Result<Self, FrameStorageError> {
        let words = word_count(qubit_count);
        let mut xs = SmallVec::<[u64; 4]>::new();
        xs.try_reserve_exact(words)
            .map_err(|_| FrameStorageError::new("stabilizer generator X words", words))?;
        xs.resize(words, 0);
        let mut zs = SmallVec::<[u64; 4]>::new();
        zs.try_reserve_exact(words)
            .map_err(|_| FrameStorageError::new("stabilizer generator Z words", words))?;
        zs.resize(words, 0);
        Ok(Self {
            negative: false,
            qubit_count,
            xs,
            zs,
        })
    }

    fn single(qubit_count: usize, qubit: usize, basis: PauliBasis, negative: bool) -> Self {
        let mut generator = Self::identity(qubit_count);
        generator.set_basis(qubit, basis);
        generator.negative = negative;
        generator
    }

    pub(super) fn try_single(
        qubit_count: usize,
        qubit: usize,
        basis: PauliBasis,
        negative: bool,
    ) -> Result<Self, FrameStorageError> {
        let mut generator = Self::try_identity(qubit_count)?;
        generator.set_basis(qubit, basis);
        generator.negative = negative;
        Ok(generator)
    }

    fn reset_to_single(
        &mut self,
        qubit_count: usize,
        qubit: usize,
        basis: PauliBasis,
        negative: bool,
    ) {
        self.negative = negative;
        self.qubit_count = qubit_count;
        self.xs.resize(word_count(qubit_count), 0);
        self.zs.resize(word_count(qubit_count), 0);
        self.xs.fill(0);
        self.zs.fill(0);
        self.set_basis(qubit, basis);
    }

    fn reset_to_identity(&mut self, qubit_count: usize) {
        self.negative = false;
        self.qubit_count = qubit_count;
        self.xs.resize(word_count(qubit_count), 0);
        self.zs.resize(word_count(qubit_count), 0);
        self.xs.fill(0);
        self.zs.fill(0);
    }

    fn copy_from_generator(&mut self, source: &Self) {
        self.negative = source.negative;
        self.qubit_count = source.qubit_count;
        self.xs.resize(source.xs.len(), 0);
        self.zs.resize(source.zs.len(), 0);
        self.xs.copy_from_slice(&source.xs);
        self.zs.copy_from_slice(&source.zs);
    }

    fn multiply_single_assign(&mut self, qubit: usize, basis: PauliBasis) {
        let left = self.basis(qubit);
        let log_i = sign_log_i(self.negative).wrapping_add(left.log_i_scalar_byproduct(basis));
        self.set_basis(
            qubit,
            PauliBasis::from_xz(left.x_bit() ^ basis.x_bit(), left.z_bit() ^ basis.z_bit()),
        );
        self.negative = (log_i & 2) != 0;
    }

    #[inline(always)]
    pub(super) fn basis(&self, qubit: usize) -> PauliBasis {
        if qubit >= self.qubit_count {
            return PauliBasis::I;
        }
        let word = qubit / u64::BITS as usize;
        let mask = 1_u64 << (qubit % u64::BITS as usize);
        PauliBasis::from_xz(
            self.xs.get(word).is_some_and(|bits| bits & mask != 0),
            self.zs.get(word).is_some_and(|bits| bits & mask != 0),
        )
    }

    #[inline(always)]
    fn set_basis(&mut self, qubit: usize, basis: PauliBasis) {
        if qubit >= self.qubit_count {
            return;
        }
        let word = qubit / u64::BITS as usize;
        let mask = 1_u64 << (qubit % u64::BITS as usize);
        set_word_bit(&mut self.xs, word, mask, basis.x_bit());
        set_word_bit(&mut self.zs, word, mask, basis.z_bit());
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

    #[inline(always)]
    pub(super) fn apply_hadamard(&mut self, qubit: usize) {
        if qubit >= self.qubit_count {
            return;
        }
        let word = qubit / u64::BITS as usize;
        let mask = 1_u64 << (qubit % u64::BITS as usize);
        let Some(x_word) = self.xs.get_mut(word) else {
            return;
        };
        let Some(z_word) = self.zs.get_mut(word) else {
            return;
        };
        let x = *x_word & mask != 0;
        let z = *z_word & mask != 0;
        if x && z {
            self.negative = !self.negative;
        }
        if x != z {
            *x_word ^= mask;
            *z_word ^= mask;
        }
    }

    #[inline(always)]
    pub(super) fn apply_controlled_x(&mut self, control: usize, target: usize) {
        if control >= self.qubit_count || target >= self.qubit_count {
            return;
        }
        let control_word = control / u64::BITS as usize;
        let target_word = target / u64::BITS as usize;
        let control_mask = 1_u64 << (control % u64::BITS as usize);
        let target_mask = 1_u64 << (target % u64::BITS as usize);
        let control_x = self
            .xs
            .get(control_word)
            .is_some_and(|word| word & control_mask != 0);
        let control_z = self
            .zs
            .get(control_word)
            .is_some_and(|word| word & control_mask != 0);
        let target_x = self
            .xs
            .get(target_word)
            .is_some_and(|word| word & target_mask != 0);
        let target_z = self
            .zs
            .get(target_word)
            .is_some_and(|word| word & target_mask != 0);
        if control_x && target_z && !(target_x ^ control_z) {
            self.negative = !self.negative;
        }
        if target_z && let Some(word) = self.zs.get_mut(control_word) {
            *word ^= control_mask;
        }
        if control_x && let Some(word) = self.xs.get_mut(target_word) {
            *word ^= target_mask;
        }
    }

    pub(super) fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        if anticommutes(self.basis(qubit), basis) {
            self.negative = !self.negative;
        }
    }

    fn commutes_with(&self, rhs: &Self) -> bool {
        self.xs
            .iter()
            .zip(&self.zs)
            .zip(rhs.xs.iter().zip(&rhs.zs))
            .fold(0_u32, |parity, ((left_x, left_z), (right_x, right_z))| {
                parity ^ ((left_x & right_z) ^ (left_z & right_x)).count_ones()
            })
            .is_multiple_of(2)
    }

    fn same_bases_as(&self, rhs: &Self) -> bool {
        self.qubit_count == rhs.qubit_count && self.xs == rhs.xs && self.zs == rhs.zs
    }

    pub(super) fn multiply_assign(&mut self, rhs: &Self) {
        self.qubit_count = self.qubit_count.max(rhs.qubit_count);
        self.xs.resize(rhs.xs.len().max(self.xs.len()), 0);
        self.zs.resize(rhs.zs.len().max(self.zs.len()), 0);
        let mut count_bit_1 = 0_u64;
        let mut count_bit_2 = 0_u64;
        for (((left_x, left_z), right_x), right_z) in self
            .xs
            .iter_mut()
            .zip(&mut self.zs)
            .zip(&rhs.xs)
            .zip(&rhs.zs)
        {
            let old_left_x = *left_x;
            let old_left_z = *left_z;
            *left_x ^= *right_x;
            *left_z ^= *right_z;
            let old_x_new_z = old_left_x & *right_z;
            let anti_commutes = (*right_x & old_left_z) ^ old_x_new_z;
            count_bit_2 ^= (count_bit_1 ^ *left_x ^ *left_z ^ old_x_new_z) & anti_commutes;
            count_bit_1 ^= anti_commutes;
        }
        let mut log_i = sign_log_i(self.negative).wrapping_add(sign_log_i(rhs.negative));
        log_i ^= popcount_mod_4(count_bit_1);
        log_i ^= popcount_mod_4(count_bit_2) << 1;
        self.negative = (log_i & 2) != 0;
    }

    pub(super) fn is_negative(&self) -> bool {
        self.negative
    }

    pub(super) fn set_negative(&mut self, negative: bool) {
        self.negative = negative;
    }

    pub(super) fn flip_sign(&mut self) {
        self.negative = !self.negative;
    }

    pub(super) fn has_x_terms(&self) -> bool {
        self.xs.iter().any(|word| *word != 0)
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

const fn word_count(bit_count: usize) -> usize {
    bit_count.div_ceil(u64::BITS as usize)
}

fn set_word_bit(words: &mut [u64], word: usize, mask: u64, value: bool) {
    let Some(bits) = words.get_mut(word) else {
        return;
    };
    if value {
        *bits |= mask;
    } else {
        *bits &= !mask;
    }
}

fn popcount_mod_4(word: u64) -> u8 {
    (word.count_ones() & 3) as u8
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SpanRow {
    bits: Vec<bool>,
    coefficients: Vec<bool>,
}

impl SpanRow {
    fn try_with_capacity(
        bit_count: usize,
        coefficient_count: usize,
    ) -> Result<Self, FrameStorageError> {
        let mut bits = Vec::new();
        bits.try_reserve_exact(bit_count)
            .map_err(|_| FrameStorageError::new("span-row bits", bit_count))?;
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| FrameStorageError::new("span-row coefficients", coefficient_count))?;
        Ok(Self { bits, coefficients })
    }

    fn reset_from_generator(
        &mut self,
        generator: &StabilizerGenerator,
        qubit_count: usize,
        generator_count: usize,
        generator_index: Option<usize>,
    ) {
        self.bits.resize(qubit_count.saturating_mul(2), false);
        self.bits.fill(false);
        for qubit in 0..generator.qubit_count {
            let basis = generator.basis(qubit);
            if let Some(bit) = self.bits.get_mut(qubit) {
                *bit = basis.x_bit();
            }
            if let Some(bit) = self.bits.get_mut(qubit.saturating_add(qubit_count)) {
                *bit = basis.z_bit();
            }
        }
        self.coefficients.resize(generator_count, false);
        self.coefficients.fill(false);
        if let Some(coefficient) =
            generator_index.and_then(|index| self.coefficients.get_mut(index))
        {
            *coefficient = true;
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SpanScratch {
    rows: Vec<SpanRow>,
    basis: Vec<Option<usize>>,
    target: SpanRow,
    product: StabilizerGenerator,
}

impl SpanScratch {
    fn try_new(qubit_count: usize) -> Result<Self, FrameStorageError> {
        let bit_count = qubit_count
            .checked_mul(2)
            .ok_or_else(|| FrameStorageError::new("span-row bits", usize::MAX))?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(qubit_count)
            .map_err(|_| FrameStorageError::new("span rows", qubit_count))?;
        for _ in 0..qubit_count {
            rows.push(SpanRow::try_with_capacity(bit_count, qubit_count)?);
        }
        let mut basis = Vec::new();
        basis
            .try_reserve_exact(bit_count)
            .map_err(|_| FrameStorageError::new("span basis", bit_count))?;
        Ok(Self {
            rows,
            basis,
            target: SpanRow::try_with_capacity(bit_count, qubit_count)?,
            product: StabilizerGenerator::try_identity(qubit_count)?,
        })
    }

    fn solve_negative(
        &mut self,
        generators: &[StabilizerGenerator],
        observable: &StabilizerGenerator,
        qubit_count: usize,
    ) -> Option<bool> {
        let width = qubit_count.checked_mul(2)?;
        let generator_count = generators.len();
        self.basis.resize(width, None);
        self.basis.fill(None);
        self.rows.resize_with(generator_count, SpanRow::default);

        for (generator_index, generator) in generators.iter().enumerate() {
            let (prior_rows, current_and_later) = self.rows.split_at_mut(generator_index);
            let row = current_and_later.first_mut()?;
            row.reset_from_generator(
                generator,
                qubit_count,
                generator_count,
                Some(generator_index),
            );
            reduce_span_row(row, &self.basis, prior_rows);
            if let Some(pivot) = row.first_one()
                && let Some(slot) = self.basis.get_mut(pivot)
            {
                *slot = Some(generator_index);
            }
        }

        self.target
            .reset_from_generator(observable, qubit_count, generator_count, None);
        for column in 0..width {
            if !self.target.bit(column) {
                continue;
            }
            let pivot_index = self.basis.get(column).copied().flatten()?;
            let pivot = self.rows.get(pivot_index)?;
            self.target.xor_assign(pivot);
        }
        if self.target.bits.iter().any(|bit| *bit) {
            return None;
        }

        self.product.reset_to_identity(qubit_count);
        for (include, generator) in self.target.coefficients.iter().zip(generators) {
            if *include {
                self.product.multiply_assign(generator);
            }
        }
        Some(self.product.negative)
    }
}

fn reduce_span_row(row: &mut SpanRow, basis: &[Option<usize>], prior_rows: &[SpanRow]) {
    for column in 0..row.bits.len() {
        if !row.bit(column) {
            continue;
        }
        let Some(pivot) = basis
            .get(column)
            .copied()
            .flatten()
            .and_then(|index| prior_rows.get(index))
        else {
            return;
        };
        row.xor_assign(pivot);
    }
}

fn local_basis_count(target_count: usize) -> Result<usize, SamplingCompileError> {
    let mut count = 1usize;
    for _ in 0..target_count {
        count = count.checked_mul(4).ok_or_else(|| {
            SamplingCompileError::invalid_circuit(
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

fn map_stabilizer_error(error: StabilizerError) -> SamplingCompileError {
    SamplingCompileError::invalid_circuit(error.to_string())
}
