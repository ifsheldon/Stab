use std::fmt::{Display, Formatter};

use rand::{Rng, RngExt as _};

use super::{
    FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString, SingleQubitClifford,
    StabilizerError, StabilizerResult,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tableau {
    xs: Vec<PauliString>,
    zs: Vec<PauliString>,
}

impl Tableau {
    /// Creates an identity Tableau within the [`crate::StabilizerResource::TableauQubits`] limit.
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        super::StabilizerResource::TableauQubits.ensure(num_qubits)?;
        Ok(Self::identity_unchecked(num_qubits))
    }

    pub(crate) fn identity_unchecked(num_qubits: usize) -> Self {
        debug_assert!(num_qubits <= super::StabilizerResource::TableauQubits.limit());
        let mut xs = Vec::with_capacity(num_qubits);
        let mut zs = Vec::with_capacity(num_qubits);
        for index in 0..num_qubits {
            xs.push(single_pauli(
                num_qubits,
                index,
                PauliBasis::X,
                PauliSign::Plus,
            ));
            zs.push(single_pauli(
                num_qubits,
                index,
                PauliBasis::Z,
                PauliSign::Plus,
            ));
        }
        Self { xs, zs }
    }

    /// Creates a Tableau from the images of its canonical X and Z generators.
    ///
    /// [`PauliString`] makes Hermiticity explicit by construction: each generator has a real
    /// positive or negative sign. This constructor additionally checks the generator counts and
    /// widths, then requires the complete canonical symplectic commutation relation. That relation
    /// proves the output map is invertible, so a second rank calculation would be redundant.
    pub fn from_conjugated_generators(
        xs: Vec<PauliString>,
        zs: Vec<PauliString>,
    ) -> StabilizerResult<Self> {
        let result = Self { xs, zs };
        result.validate_shape()?;
        if let Some(error) = result.first_symplectic_error()? {
            return Err(error);
        }
        Ok(result)
    }

    /// Creates a random valid Clifford tableau using the caller-owned RNG.
    ///
    /// Passing a seeded `rand` RNG gives deterministic Stab output. This hook samples from a
    /// random Clifford circuit shape and is not intended to be uniform over the Clifford group or
    /// to match Stim's C++ RNG stream. The current construction has the separate
    /// [`crate::StabilizerResource::RandomTableauQubits`] algorithmic limit.
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        super::StabilizerResource::RandomTableauQubits.ensure(num_qubits)?;
        let mut result = Self::identity_unchecked(num_qubits);
        for target in 0..num_qubits {
            let gate =
                single_qubit_gate_tableau(num_qubits, target, SingleQubitClifford::random(rng))?;
            result = result.then(&gate)?;
        }
        if num_qubits <= 1 {
            return Ok(result);
        }
        for _ in 0..num_qubits.saturating_mul(4) {
            let gate = if rng.random_bool(0.5) {
                let control = rng.random_range(0..num_qubits);
                let target = random_distinct_target(num_qubits, control, rng);
                cnot_gate_tableau(num_qubits, control, target)?
            } else {
                let target = rng.random_range(0..num_qubits);
                single_qubit_gate_tableau(num_qubits, target, SingleQubitClifford::random(rng))?
            };
            result = result.then(&gate)?;
        }
        Ok(result)
    }

    pub(crate) fn from_output_columns_unchecked(
        xs: Vec<PauliString>,
        zs: Vec<PauliString>,
    ) -> Self {
        Self { xs, zs }
    }

    pub(crate) fn with_output_sign_mask(&self, mask: u128) -> Self {
        let len = self.len();
        let xs = self
            .xs
            .iter()
            .enumerate()
            .map(|(index, output)| output.with_sign(sign_from_bit(((mask >> index) & 1) != 0)))
            .collect();
        let zs = self
            .zs
            .iter()
            .enumerate()
            .map(|(index, output)| {
                output.with_sign(sign_from_bit(((mask >> (len + index)) & 1) != 0))
            })
            .collect();
        Self { xs, zs }
    }

    pub fn gate1(x_output: &str, z_output: &str) -> StabilizerResult<Self> {
        let x = x_output.parse::<PauliString>()?;
        let z = z_output.parse::<PauliString>()?;
        ensure_pauli_len(&x, 1)?;
        ensure_pauli_len(&z, 1)?;
        Self::from_conjugated_generators(vec![x], vec![z])
    }

    pub fn gate2(
        x1_output: &str,
        z1_output: &str,
        x2_output: &str,
        z2_output: &str,
    ) -> StabilizerResult<Self> {
        let x1 = x1_output.parse::<PauliString>()?;
        let z1 = z1_output.parse::<PauliString>()?;
        let x2 = x2_output.parse::<PauliString>()?;
        let z2 = z2_output.parse::<PauliString>()?;
        for pauli in [&x1, &z1, &x2, &z2] {
            ensure_pauli_len(pauli, 2)?;
        }
        Self::from_conjugated_generators(vec![x1, x2], vec![z1, z2])
    }

    pub fn from_pauli_string(pauli: &PauliString) -> StabilizerResult<Self> {
        super::StabilizerResource::TableauQubits.ensure(pauli.len())?;
        let mut result = Self::identity_unchecked(pauli.len());
        for index in 0..pauli.len() {
            let basis = pauli
                .get(index)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index,
                    len: pauli.len(),
                })?;
            let x_sign = sign_from_bit(basis.z_bit());
            let z_sign = sign_from_bit(basis.x_bit());
            let x_basis = single_basis_row(pauli.len(), index, PauliBasis::X);
            let z_basis = single_basis_row(pauli.len(), index, PauliBasis::Z);
            let x_output = PauliString::from_bases_unchecked(x_sign, x_basis);
            let z_output = PauliString::from_bases_unchecked(z_sign, z_basis);
            result.set_outputs(index, x_output, z_output)?;
        }
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.xs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    pub fn x_output(&self, index: usize) -> StabilizerResult<&PauliString> {
        self.xs
            .get(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index,
                len: self.len(),
            })
    }

    pub fn z_output(&self, index: usize) -> StabilizerResult<&PauliString> {
        self.zs
            .get(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index,
                len: self.len(),
            })
    }

    pub fn y_output(&self, index: usize) -> StabilizerResult<PauliString> {
        self.x_output(index)?
            .multiply(self.z_output(index)?)?
            .multiply_phase(PauliPhase::PlusI)?
            .try_into_real()
    }

    pub fn apply(&self, input: &PauliString) -> StabilizerResult<PauliString> {
        if input.len() != self.len() {
            return Err(StabilizerError::LengthMismatch {
                left: input.len(),
                right: self.len(),
            });
        }
        let identity = vec![PauliBasis::I; self.len()];
        let mut result = FlexPauliString::from_phase_and_bases(input.phase(), identity)?;
        for index in 0..input.len() {
            let basis = input
                .get(index)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index,
                    len: input.len(),
                })?;
            let factor = match basis {
                PauliBasis::I => continue,
                PauliBasis::X => flex_from_pauli(self.x_output(index)?)?,
                PauliBasis::Y => flex_from_pauli(&self.y_output(index)?)?,
                PauliBasis::Z => flex_from_pauli(self.z_output(index)?)?,
            };
            result = result.multiply(&factor)?;
        }
        result.try_into_real()
    }

    pub fn then(&self, second: &Self) -> StabilizerResult<Self> {
        if self.len() != second.len() {
            return Err(StabilizerError::LengthMismatch {
                left: self.len(),
                right: second.len(),
            });
        }
        let mut xs = Vec::with_capacity(self.len());
        let mut zs = Vec::with_capacity(self.len());
        for index in 0..self.len() {
            xs.push(second.apply(self.x_output(index)?)?);
            zs.push(second.apply(self.z_output(index)?)?);
        }
        Ok(Self { xs, zs })
    }

    /// Appends a local Clifford action to the selected output qubits in place.
    ///
    /// This is semantically equivalent to composing with `gate.embedded(self.len(), targets)`,
    /// while one- and two-qubit gates update the existing symplectic rows without constructing a
    /// full-width temporary Tableau.
    pub fn append(&mut self, gate: &Self, targets: &[usize]) -> StabilizerResult<()> {
        self.validate_shape()?;
        gate.validate_shape()?;
        validate_tableau_targets(gate.len(), self.len(), targets)?;

        if gate.len() > 2 {
            let expanded = gate.embedded(self.len(), targets)?;
            *self = self.then(&expanded)?;
            return Ok(());
        }

        let images = local_pauli_images(gate)?;
        for output in self.xs.iter_mut().chain(&mut self.zs) {
            let code =
                targets
                    .iter()
                    .copied()
                    .enumerate()
                    .fold(0_usize, |code, (local_index, target)| {
                        let basis = output.get(target).unwrap_or(PauliBasis::I);
                        code | (usize::from(pauli_xyz(basis)) << (2 * local_index))
                    });
            let image = images
                .get(code)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index: code,
                    len: images.len(),
                })?;
            if image.phase.sign().is_negative() {
                output.flip_sign_in_place();
            }
            let bases =
                image
                    .bases
                    .get(..gate.len())
                    .ok_or(StabilizerError::TableauIndexOutOfRange {
                        index: gate.len(),
                        len: image.bases.len(),
                    })?;
            output.replace_bases_preserving_terms(targets, bases);
        }
        Ok(())
    }

    /// Raises this Tableau to a signed integer power by repeated squaring.
    pub fn pow(&self, exponent: i64) -> StabilizerResult<Self> {
        self.validate_shape()?;
        let mut power = exponent.unsigned_abs();
        let mut base = if exponent < 0 {
            self.inverse()?
        } else {
            self.clone()
        };
        let mut result = Self::identity_unchecked(self.len());
        while power != 0 {
            if power & 1 != 0 {
                result = result.then(&base)?;
            }
            power >>= 1;
            if power != 0 {
                base = base.then(&base)?;
            }
        }
        Ok(result)
    }

    /// Returns the tensor-product action of `self` and `other` on disjoint qubits.
    pub fn direct_sum(&self, other: &Self) -> StabilizerResult<Self> {
        self.validate_shape()?;
        other.validate_shape()?;
        let num_qubits = self.len().saturating_add(other.len());
        super::StabilizerResource::TableauQubits.ensure(num_qubits)?;
        let left_targets = (0..self.len()).collect::<Vec<_>>();
        let right_targets = (self.len()..num_qubits).collect::<Vec<_>>();
        let mut xs = Vec::with_capacity(num_qubits);
        let mut zs = Vec::with_capacity(num_qubits);
        for index in 0..self.len() {
            xs.push(expand_pauli(
                self.x_output(index)?,
                &left_targets,
                num_qubits,
            )?);
            zs.push(expand_pauli(
                self.z_output(index)?,
                &left_targets,
                num_qubits,
            )?);
        }
        for index in 0..other.len() {
            xs.push(expand_pauli(
                other.x_output(index)?,
                &right_targets,
                num_qubits,
            )?);
            zs.push(expand_pauli(
                other.z_output(index)?,
                &right_targets,
                num_qubits,
            )?);
        }
        Ok(Self::from_output_columns_unchecked(xs, zs))
    }

    /// Embeds this Tableau into `num_qubits`, acting on `targets` in the supplied order.
    pub fn embedded(&self, num_qubits: usize, targets: &[usize]) -> StabilizerResult<Self> {
        self.validate_shape()?;
        super::StabilizerResource::TableauQubits.ensure(num_qubits)?;
        validate_tableau_targets(self.len(), num_qubits, targets)?;
        let mut result = Self::identity_unchecked(num_qubits);
        for (local_index, global_index) in targets.iter().copied().enumerate() {
            result.set_outputs(
                global_index,
                expand_pauli(self.x_output(local_index)?, targets, num_qubits)?,
                expand_pauli(self.z_output(local_index)?, targets, num_qubits)?,
            )?;
        }
        Ok(result)
    }

    pub fn inverse(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(true)
    }

    pub fn inverse_skipping_signs(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(false)
    }

    /// Returns the unique row-reduced generators stabilizing this Tableau's output state.
    pub fn canonical_stabilizers(&self) -> StabilizerResult<Vec<PauliString>> {
        self.validate_shape()?;
        let stabilizers = self.zs.clone();
        crate::conversions::canonicalize_stabilizers(&stabilizers)
    }

    pub fn to_pauli_string(&self) -> StabilizerResult<PauliString> {
        if !self.is_pauli_product() {
            return Err(StabilizerError::NotPauliProduct);
        }
        let bases = (0..self.len()).map(|index| {
            let x = self
                .z_output(index)
                .map(|output| output.sign().is_negative())
                .unwrap_or(false);
            let z = self
                .x_output(index)
                .map(|output| output.sign().is_negative())
                .unwrap_or(false);
            PauliBasis::from_xz(x, z)
        });
        Ok(PauliString::from_bases_unchecked(PauliSign::Plus, bases))
    }

    pub fn x_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        self.output_pauli_xyz(self.x_output(input_index)?, output_index)
    }

    pub fn y_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        let y_output = self.y_output(input_index)?;
        self.output_pauli_xyz(&y_output, output_index)
    }

    pub fn z_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        self.output_pauli_xyz(self.z_output(input_index)?, output_index)
    }

    pub fn satisfies_invariants(&self) -> StabilizerResult<bool> {
        self.validate_shape()?;
        Ok(self.first_symplectic_error()?.is_none())
    }

    fn validate_shape(&self) -> StabilizerResult<()> {
        if self.xs.len() != self.zs.len() {
            return Err(StabilizerError::TableauGeneratorCountMismatch {
                x_generators: self.xs.len(),
                z_generators: self.zs.len(),
            });
        }
        let expected = self.len();
        super::StabilizerResource::TableauQubits.ensure(expected)?;
        for (basis, generators) in [(PauliBasis::X, &self.xs), (PauliBasis::Z, &self.zs)] {
            for (index, generator) in generators.iter().enumerate() {
                if generator.len() != expected {
                    return Err(StabilizerError::TableauGeneratorWidthMismatch {
                        basis,
                        index,
                        width: generator.len(),
                        expected,
                    });
                }
            }
        }
        Ok(())
    }

    fn first_symplectic_error(&self) -> StabilizerResult<Option<StabilizerError>> {
        for left in 0..self.len() {
            if self.x_output(left)?.commutes(self.z_output(left)?)? {
                return Ok(Some(StabilizerError::ConjugatedGeneratorPairCommutes {
                    index: left,
                }));
            }
            for right in left + 1..self.len() {
                for (left_basis, left_output, right_basis, right_output) in [
                    (
                        PauliBasis::X,
                        self.x_output(left)?,
                        PauliBasis::X,
                        self.x_output(right)?,
                    ),
                    (
                        PauliBasis::X,
                        self.x_output(left)?,
                        PauliBasis::Z,
                        self.z_output(right)?,
                    ),
                    (
                        PauliBasis::Z,
                        self.z_output(left)?,
                        PauliBasis::X,
                        self.x_output(right)?,
                    ),
                    (
                        PauliBasis::Z,
                        self.z_output(left)?,
                        PauliBasis::Z,
                        self.z_output(right)?,
                    ),
                ] {
                    if !left_output.commutes(right_output)? {
                        return Ok(Some(StabilizerError::ConjugatedGeneratorsAnticommute {
                            left_basis,
                            left_index: left,
                            right_basis,
                            right_index: right,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    fn inverse_with_signs(&self, include_signs: bool) -> StabilizerResult<Self> {
        let mut xs = Vec::with_capacity(self.len());
        let mut zs = Vec::with_capacity(self.len());
        for index in 0..self.len() {
            let target_x = single_pauli(self.len(), index, PauliBasis::X, PauliSign::Plus);
            let target_z = single_pauli(self.len(), index, PauliBasis::Z, PauliSign::Plus);
            xs.push(self.preimage(&target_x, include_signs)?);
            zs.push(self.preimage(&target_z, include_signs)?);
        }
        Ok(Self { xs, zs })
    }

    fn preimage(&self, target: &PauliString, include_sign: bool) -> StabilizerResult<PauliString> {
        ensure_pauli_len(target, self.len())?;
        let mut bases = Vec::with_capacity(self.len());
        for index in 0..self.len() {
            let has_x = !target.commutes(self.z_output(index)?)?;
            let has_z = !target.commutes(self.x_output(index)?)?;
            bases.push(PauliBasis::from_xz(has_x, has_z));
        }
        let unsigned = PauliString::from_bases_unchecked(PauliSign::Plus, bases.clone());
        if !include_sign || self.apply(&unsigned)? == *target {
            return Ok(unsigned);
        }
        let signed = PauliString::from_bases_unchecked(PauliSign::Minus, bases);
        if self.apply(&signed)? == *target {
            Ok(signed)
        } else {
            Err(StabilizerError::InvalidTableauInverse)
        }
    }

    fn set_outputs(
        &mut self,
        index: usize,
        x_output: PauliString,
        z_output: PauliString,
    ) -> StabilizerResult<()> {
        ensure_pauli_len(&x_output, self.len())?;
        ensure_pauli_len(&z_output, self.len())?;
        let len = self.len();
        let x_target = self
            .xs
            .get_mut(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange { index, len })?;
        *x_target = x_output;
        let z_target = self
            .zs
            .get_mut(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange { index, len })?;
        *z_target = z_output;
        Ok(())
    }

    fn is_pauli_product(&self) -> bool {
        (0..self.len()).all(|index| {
            self.x_output(index)
                .is_ok_and(|output| row_matches_single_pauli(output, index, PauliBasis::X))
                && self
                    .z_output(index)
                    .is_ok_and(|output| row_matches_single_pauli(output, index, PauliBasis::Z))
        })
    }

    fn output_pauli_xyz(&self, output: &PauliString, index: usize) -> StabilizerResult<u8> {
        let basis = output
            .get(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index,
                len: self.len(),
            })?;
        Ok(pauli_xyz(basis))
    }
}

impl Display for Tableau {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("+-")?;
        for _ in 0..self.len() {
            f.write_str("xz-")?;
        }
        f.write_str("\n|")?;
        for index in 0..self.len() {
            let x = self.xs.get(index).ok_or(std::fmt::Error)?;
            let z = self.zs.get(index).ok_or(std::fmt::Error)?;
            write!(f, " {}{}", x.sign(), z.sign())?;
        }
        for output_index in 0..self.len() {
            f.write_str("\n|")?;
            for input_index in 0..self.len() {
                let x = self.xs.get(input_index).ok_or(std::fmt::Error)?;
                let z = self.zs.get(input_index).ok_or(std::fmt::Error)?;
                let x_basis = x.get(output_index).unwrap_or(PauliBasis::I);
                let z_basis = z.get(output_index).unwrap_or(PauliBasis::I);
                write!(f, " {x_basis}{z_basis}")?;
            }
        }
        Ok(())
    }
}

fn ensure_pauli_len(pauli: &PauliString, expected: usize) -> StabilizerResult<()> {
    if pauli.len() == expected {
        Ok(())
    } else {
        Err(StabilizerError::LengthMismatch {
            left: pauli.len(),
            right: expected,
        })
    }
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis, sign: PauliSign) -> PauliString {
    let bases = single_basis_row(len, index, basis);
    PauliString::from_bases_unchecked(sign, bases)
}

fn single_basis_row(len: usize, index: usize, basis: PauliBasis) -> Vec<PauliBasis> {
    (0..len)
        .map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        })
        .collect()
}

fn single_qubit_gate_tableau(
    num_qubits: usize,
    target: usize,
    clifford: SingleQubitClifford,
) -> StabilizerResult<Tableau> {
    clifford.tableau().embedded(num_qubits, &[target])
}

fn cnot_gate_tableau(
    num_qubits: usize,
    control: usize,
    target: usize,
) -> StabilizerResult<Tableau> {
    let local = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ")?;
    local.embedded(num_qubits, &[control, target])
}

fn expand_pauli(
    local: &PauliString,
    targets: &[usize],
    num_qubits: usize,
) -> StabilizerResult<PauliString> {
    let mut bases = vec![PauliBasis::I; num_qubits];
    for (local_index, global_index) in targets.iter().copied().enumerate() {
        let basis = local
            .get(local_index)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: local_index,
                len: local.len(),
            })?;
        let target =
            bases
                .get_mut(global_index)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index: global_index,
                    len: num_qubits,
                })?;
        *target = basis;
    }
    Ok(PauliString::from_bases_unchecked(local.sign(), bases))
}

#[derive(Clone, Copy)]
struct LocalPauliImage {
    phase: PauliPhase,
    bases: [PauliBasis; 2],
}

impl LocalPauliImage {
    const IDENTITY: Self = Self {
        phase: PauliPhase::Plus,
        bases: [PauliBasis::I; 2],
    };
}

fn local_pauli_images(gate: &Tableau) -> StabilizerResult<[LocalPauliImage; 16]> {
    debug_assert!(gate.len() <= 2);
    let image_count = 1_usize << (2 * gate.len());
    let mut images = [LocalPauliImage::IDENTITY; 16];
    for (code, image) in images.iter_mut().enumerate().take(image_count) {
        for index in 0..gate.len() {
            let basis = basis_from_xyz((code >> (2 * index)) & 3);
            let factor = local_generator_image(gate, index, basis)?;
            multiply_local_images(image, factor);
        }
        if image.phase.is_imaginary() {
            return Err(StabilizerError::ImaginaryProduct { phase: image.phase });
        }
    }
    Ok(images)
}

fn local_generator_image(
    gate: &Tableau,
    index: usize,
    basis: PauliBasis,
) -> StabilizerResult<LocalPauliImage> {
    match basis {
        PauliBasis::I => Ok(LocalPauliImage::IDENTITY),
        PauliBasis::X => local_image_from_pauli(gate.x_output(index)?),
        PauliBasis::Z => local_image_from_pauli(gate.z_output(index)?),
        PauliBasis::Y => {
            let mut image = local_image_from_pauli(gate.x_output(index)?)?;
            multiply_local_images(&mut image, local_image_from_pauli(gate.z_output(index)?)?);
            image.phase = image.phase.multiply(PauliPhase::PlusI);
            Ok(image)
        }
    }
}

fn local_image_from_pauli(pauli: &PauliString) -> StabilizerResult<LocalPauliImage> {
    let mut image = LocalPauliImage {
        phase: pauli.phase(),
        ..LocalPauliImage::IDENTITY
    };
    for (index, basis) in image.bases.iter_mut().enumerate().take(pauli.len()) {
        *basis = pauli
            .get(index)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index,
                len: pauli.len(),
            })?;
    }
    Ok(image)
}

fn multiply_local_images(left: &mut LocalPauliImage, right: LocalPauliImage) {
    left.phase = left.phase.multiply(right.phase);
    for (left_basis, right_basis) in left.bases.iter_mut().zip(right.bases) {
        let (basis, phase) = left_basis.multiply(right_basis);
        *left_basis = basis;
        left.phase = left.phase.multiply(phase);
    }
}

fn basis_from_xyz(code: usize) -> PauliBasis {
    match code {
        0 => PauliBasis::I,
        1 => PauliBasis::X,
        2 => PauliBasis::Y,
        _ => PauliBasis::Z,
    }
}

fn validate_tableau_targets(
    tableau_qubits: usize,
    num_qubits: usize,
    targets: &[usize],
) -> StabilizerResult<()> {
    if targets.len() != tableau_qubits {
        return Err(StabilizerError::TableauTargetCountMismatch {
            tableau_qubits,
            target_count: targets.len(),
        });
    }
    for (index, target) in targets.iter().copied().enumerate() {
        if target >= num_qubits {
            return Err(StabilizerError::TableauTargetOutOfRange { target, num_qubits });
        }
        let prior_targets =
            targets
                .get(..index)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index,
                    len: targets.len(),
                })?;
        if prior_targets.contains(&target) {
            return Err(StabilizerError::DuplicateTableauTarget { target });
        }
    }
    Ok(())
}

fn random_distinct_target<R>(num_qubits: usize, first: usize, rng: &mut R) -> usize
where
    R: Rng + ?Sized,
{
    let mut target = rng.random_range(0..(num_qubits - 1));
    if target >= first {
        target += 1;
    }
    target
}

fn flex_from_pauli(pauli: &PauliString) -> StabilizerResult<FlexPauliString> {
    let bases = (0..pauli.len()).map(|index| pauli.get(index).unwrap_or(PauliBasis::I));
    FlexPauliString::from_phase_and_bases(pauli.phase(), bases)
}

fn row_matches_single_pauli(row: &PauliString, index: usize, basis: PauliBasis) -> bool {
    (0..row.len()).all(|candidate| {
        row.get(candidate).unwrap_or(PauliBasis::I)
            == if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
    })
}

fn pauli_xyz(basis: PauliBasis) -> u8 {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Y => 2,
        PauliBasis::Z => 3,
    }
}

fn sign_from_bit(negative: bool) -> PauliSign {
    if negative {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}
