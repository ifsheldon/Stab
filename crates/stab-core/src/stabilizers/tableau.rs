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
    pub fn identity(num_qubits: usize) -> Self {
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

    /// Creates a random valid Clifford tableau using the caller-owned RNG.
    ///
    /// Passing a seeded `rand` RNG gives deterministic Stab output. This hook samples from a
    /// random Clifford circuit shape and is not intended to be uniform over the Clifford group or
    /// to match Stim's C++ RNG stream.
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        let mut result = Self::identity(num_qubits);
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
        Ok(Self {
            xs: vec![x],
            zs: vec![z],
        })
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
        Ok(Self {
            xs: vec![x1, x2],
            zs: vec![z1, z2],
        })
    }

    pub fn from_pauli_string(pauli: &PauliString) -> StabilizerResult<Self> {
        let mut result = Self::identity(pauli.len());
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
            let x_output = PauliString::from_bases(x_sign, x_basis);
            let z_output = PauliString::from_bases(z_sign, z_basis);
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

    pub fn inverse(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(true)
    }

    pub fn inverse_skipping_signs(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(false)
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
        Ok(PauliString::from_bases(PauliSign::Plus, bases))
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
        for left in 0..self.len() {
            if self.x_output(left)?.commutes(self.z_output(left)?)? {
                return Ok(false);
            }
            for right in left + 1..self.len() {
                if !self.x_output(left)?.commutes(self.x_output(right)?)?
                    || !self.x_output(left)?.commutes(self.z_output(right)?)?
                    || !self.z_output(left)?.commutes(self.x_output(right)?)?
                    || !self.z_output(left)?.commutes(self.z_output(right)?)?
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
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
        let unsigned = PauliString::from_bases(PauliSign::Plus, bases.clone());
        if !include_sign || self.apply(&unsigned)? == *target {
            return Ok(unsigned);
        }
        let signed = PauliString::from_bases(PauliSign::Minus, bases);
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
    PauliString::from_bases(sign, bases)
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
    ensure_tableau_target(num_qubits, target)?;
    let local = clifford.tableau();
    let mut result = Tableau::identity(num_qubits);
    result.set_outputs(
        target,
        scatter_pauli(local.x_output(0)?, &[target], num_qubits)?,
        scatter_pauli(local.z_output(0)?, &[target], num_qubits)?,
    )?;
    Ok(result)
}

fn cnot_gate_tableau(
    num_qubits: usize,
    control: usize,
    target: usize,
) -> StabilizerResult<Tableau> {
    ensure_tableau_target(num_qubits, control)?;
    ensure_tableau_target(num_qubits, target)?;
    if control == target {
        return Err(StabilizerError::DuplicateTableauTarget { target });
    }
    let local = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ")?;
    let targets = [control, target];
    let mut result = Tableau::identity(num_qubits);
    for (local_index, global_index) in targets.iter().copied().enumerate() {
        result.set_outputs(
            global_index,
            scatter_pauli(local.x_output(local_index)?, &targets, num_qubits)?,
            scatter_pauli(local.z_output(local_index)?, &targets, num_qubits)?,
        )?;
    }
    Ok(result)
}

fn scatter_pauli(
    local: &PauliString,
    targets: &[usize],
    num_qubits: usize,
) -> StabilizerResult<PauliString> {
    if local.len() != targets.len() {
        return Err(StabilizerError::LengthMismatch {
            left: local.len(),
            right: targets.len(),
        });
    }
    let mut bases = vec![PauliBasis::I; num_qubits];
    let mut seen = Vec::with_capacity(targets.len());
    for (local_index, global_index) in targets.iter().copied().enumerate() {
        ensure_tableau_target(num_qubits, global_index)?;
        if seen.contains(&global_index) {
            return Err(StabilizerError::DuplicateTableauTarget {
                target: global_index,
            });
        }
        seen.push(global_index);
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
    Ok(PauliString::from_bases(local.sign(), bases))
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

fn ensure_tableau_target(num_qubits: usize, target: usize) -> StabilizerResult<()> {
    if target >= num_qubits {
        return Err(StabilizerError::TableauIndexOutOfRange {
            index: target,
            len: num_qubits,
        });
    }
    Ok(())
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
