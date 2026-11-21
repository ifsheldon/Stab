use std::fmt::{Display, Formatter};

use rand::{Rng, RngExt as _};

use super::{
    PauliBasis, PauliSign, StabilizerError, StabilizerResource, StabilizerResult, Tableau,
};
use crate::MemoryBudget;
use crate::bit_runs::{extend_periodic, or_bits_at};
use crate::budget::admit;
use crate::budget::{clifford_bytes, try_reserve_one, try_resize_zeroed_bits, try_zeroed_bits};
use crate::kernels::{
    CliffordNonIdentityCounts, CliffordPlanes, CliffordPlanesMut, clifford_right_multiply_words,
};
use stab_bits::{BitError, BitVec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedPauli {
    sign: PauliSign,
    basis: PauliBasis,
}

impl SignedPauli {
    const fn new(sign: PauliSign, basis: PauliBasis) -> Self {
        Self { sign, basis }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleQubitClifford {
    I,
    X,
    Y,
    Z,
    H,
    SqrtYDag,
    Hnxz,
    SqrtY,
    S,
    Hxy,
    Hnxy,
    SDag,
    SqrtXDag,
    SqrtX,
    Hnyz,
    Hyz,
    Cxyz,
    Cxynz,
    Cnxyz,
    Cxnyz,
    Czyx,
    Cznyx,
    Cnzyx,
    Czynx,
}

const SINGLE_QUBIT_CLIFFORDS: [SingleQubitClifford; 24] = [
    SingleQubitClifford::I,
    SingleQubitClifford::X,
    SingleQubitClifford::Y,
    SingleQubitClifford::Z,
    SingleQubitClifford::H,
    SingleQubitClifford::SqrtYDag,
    SingleQubitClifford::Hnxz,
    SingleQubitClifford::SqrtY,
    SingleQubitClifford::S,
    SingleQubitClifford::Hxy,
    SingleQubitClifford::Hnxy,
    SingleQubitClifford::SDag,
    SingleQubitClifford::SqrtXDag,
    SingleQubitClifford::SqrtX,
    SingleQubitClifford::Hnyz,
    SingleQubitClifford::Hyz,
    SingleQubitClifford::Cxyz,
    SingleQubitClifford::Cxynz,
    SingleQubitClifford::Cnxyz,
    SingleQubitClifford::Cxnyz,
    SingleQubitClifford::Czyx,
    SingleQubitClifford::Cznyx,
    SingleQubitClifford::Cnzyx,
    SingleQubitClifford::Czynx,
];

impl SingleQubitClifford {
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        SINGLE_QUBIT_CLIFFORDS.into_iter()
    }

    /// Samples one of the 24 single-qubit Clifford gates uniformly using the caller-owned RNG.
    pub fn random<R>(rng: &mut R) -> Self
    where
        R: Rng + ?Sized,
    {
        let index = rng.random_range(0..SINGLE_QUBIT_CLIFFORDS.len());
        SINGLE_QUBIT_CLIFFORDS
            .get(index)
            .copied()
            .unwrap_or(Self::I)
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::I => "I",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::H => "H",
            Self::SqrtYDag => "SQRT_Y_DAG",
            Self::Hnxz => "H_NXZ",
            Self::SqrtY => "SQRT_Y",
            Self::S => "S",
            Self::Hxy => "H_XY",
            Self::Hnxy => "H_NXY",
            Self::SDag => "S_DAG",
            Self::SqrtXDag => "SQRT_X_DAG",
            Self::SqrtX => "SQRT_X",
            Self::Hnyz => "H_NYZ",
            Self::Hyz => "H_YZ",
            Self::Cxyz => "C_XYZ",
            Self::Cxynz => "C_XYNZ",
            Self::Cnxyz => "C_NXYZ",
            Self::Cxnyz => "C_XNYZ",
            Self::Czyx => "C_ZYX",
            Self::Cznyx => "C_ZNYX",
            Self::Cnzyx => "C_NZYX",
            Self::Czynx => "C_ZYNX",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::I => "_I",
            Self::X => "_X",
            Self::Y => "_Y",
            Self::Z => "_Z",
            Self::H => "HI",
            Self::SqrtYDag => "HX",
            Self::Hnxz => "HY",
            Self::SqrtY => "HZ",
            Self::S => "SI",
            Self::Hxy => "SX",
            Self::Hnxy => "SY",
            Self::SDag => "SZ",
            Self::SqrtXDag => "VI",
            Self::SqrtX => "VX",
            Self::Hnyz => "VY",
            Self::Hyz => "VZ",
            Self::Cxyz => "uI",
            Self::Cxynz => "uX",
            Self::Cnxyz => "uY",
            Self::Cxnyz => "uZ",
            Self::Czyx => "dI",
            Self::Cznyx => "dX",
            Self::Cnzyx => "dY",
            Self::Czynx => "dZ",
        }
    }

    /// Returns the Clifford that applies `rhs` first and then `self`.
    ///
    /// Its X and Z images are `self`'s images of `rhs`'s images. The 24 single-qubit Cliffords
    /// form a group, so this is a lookup in a table built and checked at compile time.
    pub fn multiply(self, rhs: Self) -> Self {
        *lookup(lookup(&PRODUCTS, self), rhs)
    }

    /// Returns the inverse Clifford, the one whose product with `self` in either order is `I`.
    pub fn inverse(self) -> Self {
        *lookup(&INVERSES, self)
    }

    /// Returns the one-qubit tableau represented by this Clifford.
    pub fn tableau(self) -> Tableau {
        let image = |pauli: SignedPauli| {
            (
                pauli.sign.is_negative(),
                pauli.basis.x_bit(),
                pauli.basis.z_bit(),
            )
        };
        Tableau::from_rows(crate::tableau::PauliRows::one_qubit(
            image(self.x_output()),
            image(self.z_output()),
        ))
    }

    /// Applies this Clifford to one Pauli basis, dropping the sign of the image.
    pub fn apply_basis(self, basis: PauliBasis) -> PauliBasis {
        let x = self.x_output().basis;
        let z = self.z_output().basis;
        match basis {
            PauliBasis::I => PauliBasis::I,
            PauliBasis::X => x,
            PauliBasis::Z => z,
            PauliBasis::Y => PauliBasis::from_xz(x.x_bit() ^ z.x_bit(), x.z_bit() ^ z.z_bit()),
        }
    }

    const fn from_table_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::I),
            1 => Some(Self::X),
            2 => Some(Self::Z),
            3 => Some(Self::Y),
            8 => Some(Self::S),
            9 => Some(Self::Hxy),
            10 => Some(Self::SDag),
            11 => Some(Self::Hnxy),
            16 => Some(Self::SqrtXDag),
            17 => Some(Self::SqrtX),
            18 => Some(Self::Hyz),
            19 => Some(Self::Hnyz),
            28 => Some(Self::Czyx),
            29 => Some(Self::Cznyx),
            30 => Some(Self::Czynx),
            31 => Some(Self::Cnzyx),
            56 => Some(Self::Cxyz),
            57 => Some(Self::Cxynz),
            58 => Some(Self::Cxnyz),
            59 => Some(Self::Cnxyz),
            60 => Some(Self::H),
            61 => Some(Self::SqrtYDag),
            62 => Some(Self::SqrtY),
            63 => Some(Self::Hnxz),
            _ => None,
        }
    }

    const fn table_index(self) -> u8 {
        match self {
            Self::I => 0,
            Self::X => 1,
            Self::Z => 2,
            Self::Y => 3,
            Self::S => 8,
            Self::Hxy => 9,
            Self::SDag => 10,
            Self::Hnxy => 11,
            Self::SqrtXDag => 16,
            Self::SqrtX => 17,
            Self::Hyz => 18,
            Self::Hnyz => 19,
            Self::Czyx => 28,
            Self::Cznyx => 29,
            Self::Czynx => 30,
            Self::Cnzyx => 31,
            Self::Cxyz => 56,
            Self::Cxynz => 57,
            Self::Cxnyz => 58,
            Self::Cnxyz => 59,
            Self::H => 60,
            Self::SqrtYDag => 61,
            Self::SqrtY => 62,
            Self::Hnxz => 63,
        }
    }

    fn x_output(self) -> SignedPauli {
        let index = self.table_index();
        SignedPauli::new(
            sign_from_bit(index & 0b10 != 0),
            PauliBasis::from_xz(index & 0b100 == 0, index & 0b1000 != 0),
        )
    }

    fn z_output(self) -> SignedPauli {
        let index = self.table_index();
        SignedPauli::new(
            sign_from_bit(index & 0b1 != 0),
            PauliBasis::from_xz(index & 0b1_0000 != 0, index & 0b10_0000 == 0),
        )
    }
}

/// A signed single-qubit Pauli packed as `x | z << 1 | negative << 2`, for the compile-time tables.
type PackedImage = u8;

const IMAGE_NEGATIVE: PackedImage = 0b100;

/// `PRODUCTS[a][b]` is `a.multiply(b)`, indexed by declaration order.
const PRODUCTS: [[SingleQubitClifford; 24]; 24] = product_table();

/// `INVERSES[a]` is `a.inverse()`, indexed by declaration order.
const INVERSES: [SingleQubitClifford; 24] = inverse_table();

/// Reads the entry of `gate` in a table indexed by declaration order.
#[expect(
    clippy::indexing_slicing,
    reason = "a SingleQubitClifford discriminant is its position among the 24 variants, which \
              check_declaration_order confirms at compile time, so it is always below 24"
)]
fn lookup<T>(table: &[T; 24], gate: SingleQubitClifford) -> &T {
    &table[gate as usize]
}

/// The packed X image of the gate with table index `code`.
const fn packed_x_image(code: u8) -> PackedImage {
    let x = (code >> 2 & 1) ^ 1;
    let z = code >> 3 & 1;
    let negative = code >> 1 & 1;
    x | z << 1 | negative << 2
}

/// The packed Z image of the gate with table index `code`.
const fn packed_z_image(code: u8) -> PackedImage {
    let x = code >> 4 & 1;
    let z = (code >> 5 & 1) ^ 1;
    let negative = code & 1;
    x | z << 1 | negative << 2
}

/// The base-`i` exponent of `P_left P_right / P_(left ^ right)` for single-qubit Paulis coded with
/// X at bit 0 and Z at bit 1.
const fn basis_product_log_i(left: u8, right: u8) -> u8 {
    let (x1, z1) = (left & 1, left >> 1 & 1);
    let (x2, z2) = (right & 1, right >> 1 & 1);
    // Aaronson and Gottesman's g, plus four to stay non-negative.
    let shifted = match (x1, z1) {
        (0, 0) => 4,
        (1, 1) => 4 + z2 - x2,
        (1, 0) => 4 + 2 * z2 * x2 - z2,
        _ => 4 + x2 - 2 * x2 * z2,
    };
    shifted & 3
}

/// The packed image of the packed Pauli `input` under the gate with table index `code`.
const fn packed_apply(code: u8, input: PackedImage) -> PackedImage {
    let sign = input & IMAGE_NEGATIVE;
    let output = match input & 0b11 {
        0 => 0,
        0b01 => packed_x_image(code),
        0b10 => packed_z_image(code),
        _ => {
            // Y = i X Z, so its image is i X' Z', which is Hermitian because X' and Z'
            // anticommute.
            let x = packed_x_image(code);
            let z = packed_z_image(code);
            let log_i = 1 + basis_product_log_i(x & 0b11, z & 0b11);
            assert!(log_i.is_multiple_of(2), "the image of Y is Hermitian");
            let negative = (log_i / 2 % 2) ^ (x >> 2) ^ (z >> 2);
            ((x ^ z) & 0b11) | negative << 2
        }
    };
    output ^ sign
}

/// The table index of the gate whose X and Z images are `x` and `z`.
const fn code_from_images(x: PackedImage, z: PackedImage) -> u8 {
    (z >> 2)
        | (x >> 2) << 1
        | ((x & 1) ^ 1) << 2
        | (x >> 1 & 1) << 3
        | (z & 1) << 4
        | ((z >> 1 & 1) ^ 1) << 5
}

/// Confirms that each variant's discriminant is its position in [`SINGLE_QUBIT_CLIFFORDS`].
#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build"
)]
const fn check_declaration_order() {
    let mut index = 0;
    while index < SINGLE_QUBIT_CLIFFORDS.len() {
        assert!(
            SINGLE_QUBIT_CLIFFORDS[index] as usize == index,
            "SINGLE_QUBIT_CLIFFORDS lists the variants in declaration order"
        );
        index += 1;
    }
}

#[expect(
    clippy::indexing_slicing,
    clippy::panic,
    reason = "evaluated only at compile time, where an out-of-range index or a product outside \
              the group fails the build"
)]
const fn product_table() -> [[SingleQubitClifford; 24]; 24] {
    check_declaration_order();
    let mut table = [[SingleQubitClifford::I; 24]; 24];
    let mut left = 0;
    while left < 24 {
        let mut right = 0;
        while right < 24 {
            let outer = SINGLE_QUBIT_CLIFFORDS[left].table_index();
            let inner = SINGLE_QUBIT_CLIFFORDS[right].table_index();
            let x = packed_apply(outer, packed_x_image(inner));
            let z = packed_apply(outer, packed_z_image(inner));
            table[left][right] = match SingleQubitClifford::from_table_index(code_from_images(x, z))
            {
                Some(product) => product,
                None => panic!("the single-qubit Cliffords are closed under products"),
            };
            right += 1;
        }
        left += 1;
    }
    table
}

#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build"
)]
const fn inverse_table() -> [SingleQubitClifford; 24] {
    let mut table = [SingleQubitClifford::I; 24];
    let mut gate = 0;
    while gate < 24 {
        let mut candidate = 0;
        let mut found = false;
        while candidate < 24 {
            let identity = SingleQubitClifford::I as usize;
            if PRODUCTS[gate][candidate] as usize == identity {
                assert!(
                    PRODUCTS[candidate][gate] as usize == identity,
                    "a left inverse is a right inverse"
                );
                assert!(!found, "each Clifford has one inverse");
                table[gate] = SINGLE_QUBIT_CLIFFORDS[candidate];
                found = true;
            }
            candidate += 1;
        }
        assert!(found, "every single-qubit Clifford has an inverse");
        gate += 1;
    }
    table
}

impl std::ops::Mul for SingleQubitClifford {
    type Output = Self;

    /// See [`SingleQubitClifford::multiply`].
    fn mul(self, rhs: Self) -> Self {
        self.multiply(rhs)
    }
}

impl Display for SingleQubitClifford {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.token())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliffordString {
    z_signs: BitVec,
    x_signs: BitVec,
    inv_x2x: BitVec,
    x2z: BitVec,
    z2x: BitVec,
    inv_z2z: BitVec,
    non_identity_count: usize,
}

impl CliffordString {
    /// Creates an identity string admitted against [`MemoryBudget::DEFAULT`].
    ///
    /// See [`Self::identity_with_budget`].
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        Self::identity_with_budget(num_qubits, MemoryBudget::DEFAULT)
    }

    /// Creates an identity string after admitting its six packed planes against `budget`.
    pub fn identity_with_budget(num_qubits: usize, budget: MemoryBudget) -> StabilizerResult<Self> {
        admit_clifford_planes(budget, num_qubits, 0)?;
        Ok(Self::from_planes(zeroed_planes(num_qubits)?, 0))
    }

    /// Wraps six planes of one length, in table-index bit order.
    fn from_planes(planes: [BitVec; 6], non_identity_count: usize) -> Self {
        let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes;
        Self {
            z_signs,
            x_signs,
            inv_x2x,
            x2z,
            z2x,
            inv_z2z,
            non_identity_count,
        }
    }

    /// The six planes in table-index bit order: bit `b` of a gate's table index is in plane `b`.
    fn planes(&self) -> [&BitVec; 6] {
        [
            &self.z_signs,
            &self.x_signs,
            &self.inv_x2x,
            &self.x2z,
            &self.z2x,
            &self.inv_z2z,
        ]
    }

    #[cfg(test)]
    pub(crate) fn identity_unbudgeted(num_qubits: usize) -> Self {
        Self {
            z_signs: BitVec::zeros(num_qubits),
            x_signs: BitVec::zeros(num_qubits),
            inv_x2x: BitVec::zeros(num_qubits),
            x2z: BitVec::zeros(num_qubits),
            z2x: BitVec::zeros(num_qubits),
            inv_z2z: BitVec::zeros(num_qubits),
            non_identity_count: 0,
        }
    }

    /// Collects Clifford gates admitted against [`MemoryBudget::DEFAULT`].
    ///
    /// See [`Self::from_gates_with_budget`].
    pub fn from_gates(
        gates: impl IntoIterator<Item = SingleQubitClifford>,
    ) -> StabilizerResult<Self> {
        Self::from_gates_with_budget(gates, MemoryBudget::DEFAULT)
    }

    /// Collects Clifford gates, refusing the first item whose collected gates and packed planes
    /// would exceed `budget`.
    pub fn from_gates_with_budget(
        gates: impl IntoIterator<Item = SingleQubitClifford>,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let mut collected = Vec::new();
        for gate in gates {
            let count = collected.len().saturating_add(1);
            admit_clifford_planes(
                budget,
                count,
                count as u128 * size_of::<SingleQubitClifford>() as u128,
            )?;
            try_reserve_one(StabilizerResource::CliffordString, &mut collected)?;
            collected.push(gate);
        }
        Self::from_gate_slice(&collected)
    }

    /// Packs `gates` straight into fallibly reserved planes.
    fn from_gate_slice(gates: &[SingleQubitClifford]) -> StabilizerResult<Self> {
        let mut planes = zeroed_planes(gates.len())?;
        let mut words = planes.each_mut().map(BitVec::words_mut);
        let mut non_identity_count = 0_usize;
        for (word_index, chunk) in gates.chunks(u64::BITS as usize).enumerate() {
            let mut packed = [0_u64; 6];
            for (bit_index, gate) in chunk.iter().enumerate() {
                let code = gate.table_index();
                non_identity_count += usize::from(code != 0);
                for (plane_bit, word) in packed.iter_mut().enumerate() {
                    *word |= u64::from((code >> plane_bit) & 1) << bit_index;
                }
            }
            for (plane, word) in words.iter_mut().zip(packed) {
                *plane.get_mut(word_index).ok_or(StabilizerError::Bit(
                    BitError::BitIndexOutOfRange {
                        index: word_index * u64::BITS as usize,
                        len: gates.len(),
                    },
                ))? = word;
            }
        }
        drop(words);
        Ok(Self::from_planes(planes, non_identity_count))
    }

    /// Creates a random string of independent single-qubit Clifford gates.
    ///
    /// Passing a seeded `rand` RNG gives deterministic Stab output. The generated stream is not
    /// intended to match Stim's C++ RNG stream. The string is admitted against
    /// [`MemoryBudget::DEFAULT`]; see [`Self::random_with_budget`].
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        Self::random_with_budget(num_qubits, MemoryBudget::DEFAULT, rng)
    }

    /// Creates a random string after admitting its six packed planes against `budget`, before
    /// consuming randomness.
    pub fn random_with_budget<R>(
        num_qubits: usize,
        budget: MemoryBudget,
        rng: &mut R,
    ) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        let mut result = Self::identity_with_budget(num_qubits, budget)?;
        result.randomize(rng);
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.z_signs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.z_signs.is_empty()
    }

    pub fn gate_at(&self, index: usize) -> Option<SingleQubitClifford> {
        if index >= self.len() {
            return None;
        }
        let table_index = u8::from(self.z_signs.get(index)?)
            | (u8::from(self.x_signs.get(index)?) << 1)
            | (u8::from(self.inv_x2x.get(index)?) << 2)
            | (u8::from(self.x2z.get(index)?) << 3)
            | (u8::from(self.z2x.get(index)?) << 4)
            | (u8::from(self.inv_z2z.get(index)?) << 5);
        SingleQubitClifford::from_table_index(table_index)
    }

    pub fn set_gate_at(&mut self, index: usize, gate: SingleQubitClifford) -> StabilizerResult<()> {
        let len = self.len();
        let old_gate = self
            .gate_at(index)
            .ok_or(StabilizerError::CliffordIndexOutOfRange { index, len })?;
        let non_identity_count = match (
            old_gate == SingleQubitClifford::I,
            gate == SingleQubitClifford::I,
        ) {
            (true, false) => self.non_identity_count + 1,
            (false, true) => self
                .non_identity_count
                .checked_sub(1)
                .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?,
            _ => self.non_identity_count,
        };
        self.write_gate_bits(index, gate)?;
        self.non_identity_count = non_identity_count;
        Ok(())
    }

    /// Replaces every position with a random single-qubit Clifford gate.
    pub fn randomize<R>(&mut self, rng: &mut R)
    where
        R: Rng + ?Sized,
    {
        let mut non_identity_count = 0_usize;
        let mut remaining = self.len();
        let mut z_signs = self.z_signs.words_mut();
        let mut x_signs = self.x_signs.words_mut();
        let mut inv_x2x = self.inv_x2x.words_mut();
        let mut x2z = self.x2z.words_mut();
        let mut z2x = self.z2x.words_mut();
        let mut inv_z2z = self.inv_z2z.words_mut();
        let words = z_signs
            .iter_mut()
            .zip(x_signs.iter_mut())
            .zip(inv_x2x.iter_mut())
            .zip(x2z.iter_mut())
            .zip(z2x.iter_mut())
            .zip(inv_z2z.iter_mut());
        for (((((z_signs, x_signs), inv_x2x), x2z), z2x), inv_z2z) in words {
            let chunk_len = remaining.min(u64::BITS as usize);
            let mut packed = [0_u64; 6];
            for bit_index in 0..chunk_len {
                let code = SingleQubitClifford::random(rng).table_index();
                non_identity_count += usize::from(code != 0);
                let mask = 1_u64 << bit_index;
                packed[0] |= mask * u64::from(code & (1 << 0) != 0);
                packed[1] |= mask * u64::from(code & (1 << 1) != 0);
                packed[2] |= mask * u64::from(code & (1 << 2) != 0);
                packed[3] |= mask * u64::from(code & (1 << 3) != 0);
                packed[4] |= mask * u64::from(code & (1 << 4) != 0);
                packed[5] |= mask * u64::from(code & (1 << 5) != 0);
            }
            [*z_signs, *x_signs, *inv_x2x, *x2z, *z2x, *inv_z2z] = packed;
            remaining -= chunk_len;
        }
        debug_assert_eq!(remaining, 0);
        self.non_identity_count = non_identity_count;
    }

    /// Concatenates two strings, admitting the result against [`MemoryBudget::DEFAULT`].
    pub fn concat(&self, rhs: &Self) -> StabilizerResult<Self> {
        self.concat_with_budget(rhs, MemoryBudget::DEFAULT)
    }

    /// Concatenates two strings after admitting the result's planes and table indices against
    /// `budget`.
    pub fn concat_with_budget(&self, rhs: &Self, budget: MemoryBudget) -> StabilizerResult<Self> {
        let new_len = self
            .len()
            .checked_add(rhs.len())
            .ok_or(StabilizerError::LengthMismatch {
                left: self.len(),
                right: rhs.len(),
            })?;
        admit_clifford_planes(budget, new_len, 0)?;
        let non_identity_count = self
            .non_identity_count
            .checked_add(rhs.non_identity_count)
            .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?;
        let mut planes = zeroed_planes(new_len)?;
        for (target, (left, right)) in planes
            .iter_mut()
            .zip(self.planes().into_iter().zip(rhs.planes()))
        {
            let mut target = target.words_mut();
            or_bits_at(&mut target, 0, left.words(), self.len())?;
            or_bits_at(&mut target, self.len(), right.words(), rhs.len())?;
        }
        Ok(Self::from_planes(planes, non_identity_count))
    }

    /// Repeats this string, admitting the result against [`MemoryBudget::DEFAULT`].
    ///
    /// See [`Self::repeat_with_budget`].
    pub fn repeat(&self, repetitions: usize) -> StabilizerResult<Self> {
        self.repeat_with_budget(repetitions, MemoryBudget::DEFAULT)
    }

    /// Repeats this string after checking multiplication overflow and admitting the resulting
    /// planes and table indices against `budget`.
    pub fn repeat_with_budget(
        &self,
        repetitions: usize,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        if repetitions == 0 || self.is_empty() {
            return Ok(Self::from_planes(zeroed_planes(0)?, 0));
        }
        let new_len =
            self.len()
                .checked_mul(repetitions)
                .ok_or(StabilizerError::ResourceSizeOverflow {
                    resource: StabilizerResource::CliffordString,
                    item_count: self.len(),
                    repetitions,
                })?;
        admit_clifford_planes(budget, new_len, 0)?;
        let non_identity_count = self
            .non_identity_count
            .checked_mul(repetitions)
            .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?;
        let mut planes = zeroed_planes(new_len)?;
        for (target, source) in planes.iter_mut().zip(self.planes()) {
            let mut target = target.words_mut();
            or_bits_at(&mut target, 0, source.words(), self.len())?;
            extend_periodic(&mut target, self.len(), new_len)?;
        }
        Ok(Self::from_planes(planes, non_identity_count))
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<Self> {
        let mut result = self.clone();
        result.right_multiply_in_place(rhs)?;
        Ok(result)
    }

    /// Right-multiplies `rhs` into this Clifford string in place.
    ///
    /// If `rhs` is longer than this string, this string is extended with identity rotations before
    /// multiplication.
    #[inline]
    pub fn right_multiply_in_place(&mut self, rhs: &Self) -> StabilizerResult<()> {
        self.ensure_len(rhs.len())?;
        if rhs.non_identity_count == 0 {
            return Ok(());
        }
        let word_count = rhs.z_signs.word_count();
        let mut z_signs = self.z_signs.words_mut();
        let mut x_signs = self.x_signs.words_mut();
        let mut inv_x2x = self.inv_x2x.words_mut();
        let mut x2z = self.x2z.words_mut();
        let mut z2x = self.z2x.words_mut();
        let mut inv_z2z = self.inv_z2z.words_mut();
        let left = CliffordPlanesMut {
            z_signs: prefix_mut(&mut z_signs, word_count)?,
            x_signs: prefix_mut(&mut x_signs, word_count)?,
            inv_x2x: prefix_mut(&mut inv_x2x, word_count)?,
            x2z: prefix_mut(&mut x2z, word_count)?,
            z2x: prefix_mut(&mut z2x, word_count)?,
            inv_z2z: prefix_mut(&mut inv_z2z, word_count)?,
        };
        let right = CliffordPlanes {
            z_signs: rhs.z_signs.words(),
            x_signs: rhs.x_signs.words(),
            inv_x2x: rhs.inv_x2x.words(),
            x2z: rhs.x2z.words(),
            z2x: rhs.z2x.words(),
            inv_z2z: rhs.inv_z2z.words(),
        };
        let CliffordNonIdentityCounts { before, after } =
            clifford_right_multiply_words(left, right)?;
        self.non_identity_count = self
            .non_identity_count
            .checked_sub(before)
            .and_then(|tail| tail.checked_add(after))
            .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?;
        Ok(())
    }

    /// Pads every plane with identities up to `len` gates, reserving the new words fallibly; on
    /// failure every plane keeps its old length.
    fn ensure_len(&mut self, len: usize) -> StabilizerResult<()> {
        let old_len = self.len();
        if len <= old_len {
            return Ok(());
        }
        let mut planes = [
            &mut self.z_signs,
            &mut self.x_signs,
            &mut self.inv_x2x,
            &mut self.x2z,
            &mut self.z2x,
            &mut self.inv_z2z,
        ];
        let grown = planes.iter_mut().try_for_each(|plane| {
            try_resize_zeroed_bits(StabilizerResource::CliffordString, plane, len)
        });
        if grown.is_err() {
            // Shrinking keeps the reserved words, so it cannot fail.
            for plane in &mut planes {
                plane.resize_zeros(old_len);
            }
        }
        grown
    }

    fn write_gate_bits(&mut self, index: usize, gate: SingleQubitClifford) -> StabilizerResult<()> {
        let table_index = gate.table_index();
        self.z_signs.set(index, table_index & (1 << 0) != 0)?;
        self.x_signs.set(index, table_index & (1 << 1) != 0)?;
        self.inv_x2x.set(index, table_index & (1 << 2) != 0)?;
        self.x2z.set(index, table_index & (1 << 3) != 0)?;
        self.z2x.set(index, table_index & (1 << 4) != 0)?;
        self.inv_z2z.set(index, table_index & (1 << 5) != 0)?;
        Ok(())
    }
}

/// Admits the six planes of a `num_qubits`-gate string and `extra` bytes against `budget`.
fn admit_clifford_planes(
    budget: MemoryBudget,
    num_qubits: usize,
    extra: u128,
) -> StabilizerResult<()> {
    admit(
        budget,
        StabilizerResource::CliffordString,
        clifford_bytes(num_qubits) + extra,
    )
}

/// Reserves six zeroed planes of `num_qubits` bits fallibly; these are the only storage a string
/// holds, so they are exactly what [`admit_clifford_planes`] charged.
fn zeroed_planes(num_qubits: usize) -> StabilizerResult<[BitVec; 6]> {
    let plane = || try_zeroed_bits(StabilizerResource::CliffordString, num_qubits);
    Ok([plane()?, plane()?, plane()?, plane()?, plane()?, plane()?])
}

fn prefix_mut(words: &mut [u64], len: usize) -> StabilizerResult<&mut [u64]> {
    let word_len = words.len();
    words.get_mut(..len).ok_or_else(|| {
        StabilizerError::Bit(BitError::LengthMismatch {
            left: word_len * u64::BITS as usize,
            right: len * u64::BITS as usize,
        })
    })
}

impl Display for CliffordString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for index in 0..self.len() {
            if index > 0 {
                f.write_str(" ")?;
            }
            let gate = self.gate_at(index).ok_or(std::fmt::Error)?;
            write!(f, "{gate}")?;
        }
        Ok(())
    }
}

fn sign_from_bit(negative: bool) -> PauliSign {
    if negative {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "stabilizer algebra tests use compact literal Clifford fixtures"
    )]

    use super::*;
    use rand::SeedableRng as _;
    use rand::rngs::SmallRng;

    #[test]
    fn right_multiply_in_place_matches_per_gate_products() {
        let mut left = CliffordString::from_gates([
            SingleQubitClifford::H,
            SingleQubitClifford::S,
            SingleQubitClifford::I,
        ])
        .unwrap();
        let right = CliffordString::from_gates([
            SingleQubitClifford::S,
            SingleQubitClifford::H,
            SingleQubitClifford::SqrtX,
        ])
        .unwrap();
        let expected = CliffordString::from_gates([
            SingleQubitClifford::H.multiply(SingleQubitClifford::S),
            SingleQubitClifford::S.multiply(SingleQubitClifford::H),
            SingleQubitClifford::I.multiply(SingleQubitClifford::SqrtX),
        ])
        .unwrap();

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left, expected);
    }

    #[test]
    fn right_multiply_matches_every_single_qubit_product_across_word_blocks() {
        let width = 257;
        for left_gate in SingleQubitClifford::all() {
            for right_gate in SingleQubitClifford::all() {
                let expected_gate = left_gate * right_gate;
                let mut left =
                    CliffordString::from_gates(std::iter::repeat_n(left_gate, width)).unwrap();
                let right =
                    CliffordString::from_gates(std::iter::repeat_n(right_gate, width)).unwrap();

                left.right_multiply_in_place(&right).unwrap();

                assert_eq!(
                    left,
                    CliffordString::from_gates(std::iter::repeat_n(expected_gate, width)).unwrap(),
                    "left={left_gate} right={right_gate}"
                );
                assert_eq!(
                    left.non_identity_count,
                    usize::from(expected_gate != SingleQubitClifford::I) * width,
                    "left={left_gate} right={right_gate}"
                );
            }
        }
    }

    #[test]
    fn right_multiply_in_place_extends_shorter_left_side() {
        let mut left = CliffordString::from_gates([SingleQubitClifford::H]).unwrap();
        let right =
            CliffordString::from_gates([SingleQubitClifford::I, SingleQubitClifford::S]).unwrap();

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left.gate_at(0), Some(SingleQubitClifford::H));
        assert_eq!(left.gate_at(1), Some(SingleQubitClifford::S));
    }

    #[test]
    fn right_multiply_in_place_extends_for_longer_identity_rhs() {
        let mut left = CliffordString::from_gates([SingleQubitClifford::H]).unwrap();
        let right = CliffordString::identity_unbudgeted(3);

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left.len(), 3);
        assert_eq!(left.gate_at(0), Some(SingleQubitClifford::H));
        assert_eq!(left.gate_at(1), Some(SingleQubitClifford::I));
        assert_eq!(left.gate_at(2), Some(SingleQubitClifford::I));
    }

    #[test]
    fn right_multiply_in_place_preserves_left_tail_across_word_boundaries() {
        for (left_len, right_len, tail_index) in [(63, 17, 62), (65, 63, 64), (129, 65, 128)] {
            let mut left = CliffordString::identity_unbudgeted(left_len);
            left.set_gate_at(0, SingleQubitClifford::H).unwrap();
            left.set_gate_at(tail_index, SingleQubitClifford::S)
                .unwrap();
            let mut right = CliffordString::identity_unbudgeted(right_len);
            right.set_gate_at(0, SingleQubitClifford::SqrtX).unwrap();

            let expected_head = SingleQubitClifford::H.multiply(SingleQubitClifford::SqrtX);
            left.right_multiply_in_place(&right).unwrap();

            assert_eq!(left.gate_at(0), Some(expected_head), "left_len={left_len}");
            assert_eq!(
                left.gate_at(tail_index),
                Some(SingleQubitClifford::S),
                "left_len={left_len}"
            );
            assert_eq!(left.non_identity_count, 2, "left_len={left_len}");
        }
    }

    #[test]
    fn short_rhs_updates_only_prefix_non_identity_metadata() {
        let width = 65_537;
        let mut left =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, width)).unwrap();
        let right = CliffordString::from_gates([SingleQubitClifford::H]).unwrap();

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left.gate_at(0), Some(SingleQubitClifford::I));
        assert_eq!(left.gate_at(width - 1), Some(SingleQubitClifford::H));
        assert_eq!(left.non_identity_count, width - 1);
    }

    #[test]
    fn clearing_last_gate_restores_identity_metadata() {
        let mut right = CliffordString::identity_unbudgeted(257);
        right.set_gate_at(256, SingleQubitClifford::SqrtY).unwrap();
        assert_eq!(right.non_identity_count, 1);

        right.set_gate_at(256, SingleQubitClifford::I).unwrap();
        assert_eq!(right.non_identity_count, 0);

        let mut left = CliffordString::from_gates((0..257).map(|index| {
            if index % 2 == 0 {
                SingleQubitClifford::H
            } else {
                SingleQubitClifford::S
            }
        }))
        .unwrap();
        let expected = left.clone();
        left.right_multiply_in_place(&right).unwrap();
        assert_eq!(left, expected);
    }

    #[test]
    fn clearing_many_gates_updates_non_identity_metadata_in_constant_time() {
        let width = 16_384;
        let mut value =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, width)).unwrap();
        assert_eq!(value.non_identity_count, width);

        for index in 0..width {
            value.set_gate_at(index, SingleQubitClifford::I).unwrap();
            assert_eq!(value.non_identity_count, width - index - 1);
        }
        assert_eq!(value, CliffordString::identity_unbudgeted(width));
    }

    fn random_gates(len: usize, rng: &mut SmallRng) -> Vec<SingleQubitClifford> {
        (0..len)
            .map(|_| SingleQubitClifford::random(&mut *rng))
            .collect()
    }

    /// Checks `value` against the scalar reference `expected`, gate by gate, and that every plane
    /// keeps the bits past its length zero.
    fn assert_matches_gates(value: &CliffordString, expected: &[SingleQubitClifford], label: &str) {
        assert_eq!(value.len(), expected.len(), "{label}");
        for (index, gate) in expected.iter().enumerate() {
            assert_eq!(value.gate_at(index), Some(*gate), "{label} index={index}");
        }
        let non_identity = expected
            .iter()
            .filter(|gate| **gate != SingleQubitClifford::I)
            .count();
        assert_eq!(value.non_identity_count, non_identity, "{label}");
        let tail = value.len() % u64::BITS as usize;
        for plane in value.planes() {
            assert_eq!(plane.word_count(), value.len().div_ceil(64), "{label}");
            if tail != 0 {
                assert_eq!(plane.words().last().unwrap() >> tail, 0, "{label}");
            }
        }
    }

    #[test]
    fn concat_matches_scalar_reference_at_unaligned_lengths() {
        let mut rng = SmallRng::seed_from_u64(0xc0_ca7);
        let lengths = [0, 1, 37, 63, 64, 65, 127, 129, 200];
        for left_len in lengths {
            for right_len in lengths {
                let left_gates = random_gates(left_len, &mut rng);
                let right_gates = random_gates(right_len, &mut rng);
                let left = CliffordString::from_gates(left_gates.iter().copied()).unwrap();
                let right = CliffordString::from_gates(right_gates.iter().copied()).unwrap();

                let joined = left.concat(&right).unwrap();

                let expected = [left_gates, right_gates].concat();
                assert_matches_gates(&joined, &expected, &format!("{left_len}+{right_len}"));
            }
        }
    }

    #[test]
    fn repeat_matches_scalar_reference_for_word_boundary_patterns() {
        let mut rng = SmallRng::seed_from_u64(0x007e_9ea7);
        for pattern_len in [1, 3, 63, 64, 65, 130] {
            let gates = random_gates(pattern_len, &mut rng);
            let pattern = CliffordString::from_gates(gates.iter().copied()).unwrap();
            for repetitions in [1, 2, 3, 64, 65, 131] {
                let repeated = pattern.repeat(repetitions).unwrap();

                let expected = gates
                    .iter()
                    .copied()
                    .cycle()
                    .take(pattern_len * repetitions)
                    .collect::<Vec<_>>();
                assert_matches_gates(
                    &repeated,
                    &expected,
                    &format!("pattern={pattern_len} repetitions={repetitions}"),
                );
            }
        }
    }

    #[test]
    fn concat_and_repeat_allocate_only_the_admitted_planes() {
        let mut rng = SmallRng::seed_from_u64(0x000a_110c);
        let left = CliffordString::from_gates(random_gates(1_000, &mut rng)).unwrap();
        let right = CliffordString::from_gates(random_gates(77, &mut rng)).unwrap();
        let held = |len: usize| u64::try_from(clifford_bytes(len)).unwrap();

        let mut joined = None;
        let allocations = allocation_counter::measure(|| joined = Some(left.concat(&right)));
        assert_eq!(allocations.count_total, 6, "{allocations:?}");
        assert_eq!(allocations.bytes_total, held(1_077), "{allocations:?}");
        assert_eq!(joined.unwrap().unwrap().len(), 1_077);

        let mut repeated = None;
        let allocations = allocation_counter::measure(|| repeated = Some(right.repeat(1_000)));
        assert_eq!(allocations.count_total, 6, "{allocations:?}");
        assert_eq!(allocations.bytes_total, held(77_000), "{allocations:?}");
        assert_eq!(repeated.unwrap().unwrap().len(), 77_000);
    }

    #[test]
    fn randomize_reuses_packed_plane_storage_without_allocating() {
        let width = 65_537;
        let mut value = CliffordString::identity_unbudgeted(width);
        let mut rng = SmallRng::seed_from_u64(0xc11f_f07d);

        let allocations = allocation_counter::measure(|| value.randomize(&mut rng));

        assert_eq!(allocations.count_total, 0);
        assert_eq!(allocations.bytes_total, 0);
        let observed_count = (0..width)
            .filter(|&index| value.gate_at(index) != Some(SingleQubitClifford::I))
            .count();
        assert_eq!(value.non_identity_count, observed_count);
    }

    #[test]
    fn right_multiply_reuses_packed_storage_without_allocating() {
        let width = 65_537;
        let mut left =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, width)).unwrap();
        let right =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, width)).unwrap();

        let allocations =
            allocation_counter::measure(|| left.right_multiply_in_place(&right).unwrap());

        assert_eq!(allocations.count_total, 0);
        assert_eq!(allocations.bytes_total, 0);
        assert_eq!(left.non_identity_count, width);
    }
}
