//! Two-qubit Clifford operations given as fixed-size generator images.

use crate::tableau::PauliRows;
use crate::{PauliBasis, PauliSign, PauliString, Tableau};

/// A signed two-qubit Pauli product, the image of one generator under a [`TwoQubitClifford`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoQubitPauli {
    pub sign: PauliSign,
    /// The bases on qubit 0 and qubit 1.
    pub bases: [PauliBasis; 2],
}

impl TwoQubitPauli {
    pub const fn new(sign: PauliSign, bases: [PauliBasis; 2]) -> Self {
        Self { sign, bases }
    }

    /// Returns this product as a two-qubit Pauli string.
    pub fn to_pauli_string(self) -> PauliString {
        PauliString::from_bases_unbudgeted(self.sign, self.bases)
    }

    const fn x_bits(self) -> u64 {
        let [first, second] = self.bases;
        first.x_bit() as u64 | (second.x_bit() as u64) << 1
    }

    const fn z_bits(self) -> u64 {
        let [first, second] = self.bases;
        first.z_bit() as u64 | (second.z_bit() as u64) << 1
    }

    /// Whether the two products anticommute, which is the parity of their symplectic product.
    const fn anticommutes(self, other: Self) -> bool {
        ((self.x_bits() & other.z_bits()) ^ (self.z_bits() & other.x_bits())).count_ones() % 2 == 1
    }

    const fn row(self) -> (bool, u64, u64) {
        (self.sign.is_negative(), self.x_bits(), self.z_bits())
    }
}

/// A two-qubit Clifford operation stored as the signed images of `X_0`, `X_1`, `Z_0`, and `Z_1`.
///
/// Every value satisfies the canonical commutation relations, so it always converts to a valid
/// [`Tableau`] without a fallible step. Because [`Self::try_from_images`] is a `const fn`, a
/// fixed table of gates can be checked when it is compiled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoQubitClifford {
    x_images: [TwoQubitPauli; 2],
    z_images: [TwoQubitPauli; 2],
}

impl TwoQubitClifford {
    /// Builds a Clifford from the images of `X_0`, `X_1` and of `Z_0`, `Z_1`.
    ///
    /// Returns `None` unless each `X_q` image anticommutes with the `Z_q` image and commutes with
    /// every other image, which is exactly when the images define a Clifford operation.
    pub const fn try_from_images(
        x_images: [TwoQubitPauli; 2],
        z_images: [TwoQubitPauli; 2],
    ) -> Option<Self> {
        let [x0, x1] = x_images;
        let [z0, z1] = z_images;
        let valid = x0.anticommutes(z0)
            && x1.anticommutes(z1)
            && !x0.anticommutes(z1)
            && !x1.anticommutes(z0)
            && !x0.anticommutes(x1)
            && !z0.anticommutes(z1);
        if valid {
            Some(Self { x_images, z_images })
        } else {
            None
        }
    }

    /// Returns the signed images of `X_0` and `X_1`.
    pub const fn x_images(self) -> [TwoQubitPauli; 2] {
        self.x_images
    }

    /// Returns the signed images of `Z_0` and `Z_1`.
    pub const fn z_images(self) -> [TwoQubitPauli; 2] {
        self.z_images
    }

    /// Returns the two-qubit tableau represented by this Clifford.
    pub fn tableau(self) -> Tableau {
        let [x0, x1] = self.x_images;
        let [z0, z1] = self.z_images;
        Tableau::from_rows(PauliRows::two_qubit([
            x0.row(),
            x1.row(),
            z0.row(),
            z1.row(),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::{TwoQubitClifford, TwoQubitPauli};
    use crate::{PauliBasis, PauliSign, Tableau};

    const BASES: [PauliBasis; 4] = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];

    fn text(pauli: TwoQubitPauli) -> String {
        let sign = if pauli.sign.is_negative() { '-' } else { '+' };
        let basis = |basis| match basis {
            PauliBasis::I => '_',
            PauliBasis::X => 'X',
            PauliBasis::Y => 'Y',
            PauliBasis::Z => 'Z',
        };
        [sign, basis(pauli.bases[0]), basis(pauli.bases[1])]
            .into_iter()
            .collect()
    }

    /// Every assignment of unsigned images is accepted exactly when the validating string
    /// constructor accepts it, and accepted images give the same tableau.
    #[test]
    fn images_are_accepted_exactly_when_they_form_a_clifford() {
        let products: Vec<TwoQubitPauli> = BASES
            .iter()
            .flat_map(|&first| BASES.iter().map(move |&second| [first, second]))
            .map(|bases| TwoQubitPauli::new(PauliSign::Plus, bases))
            .collect();
        let mut accepted = 0;
        for &x0 in &products {
            for &z0 in &products {
                for &x1 in &products {
                    for &z1 in &products {
                        // Vary signs with the bases so both sign planes are exercised.
                        let x1 = TwoQubitPauli::new(
                            if x0.bases[0] == PauliBasis::Y {
                                PauliSign::Minus
                            } else {
                                PauliSign::Plus
                            },
                            x1.bases,
                        );
                        let z0 = TwoQubitPauli::new(
                            if z1.bases[1] == PauliBasis::X {
                                PauliSign::Minus
                            } else {
                                PauliSign::Plus
                            },
                            z0.bases,
                        );
                        let typed = TwoQubitClifford::try_from_images([x0, x1], [z0, z1]);
                        let parsed =
                            Tableau::gate2(&text(x0), &text(z0), &text(x1), &text(z1)).ok();
                        assert_eq!(typed.map(TwoQubitClifford::tableau), parsed);
                        accepted += usize::from(typed.is_some());
                    }
                }
            }
        }
        // The two-qubit symplectic group has 720 elements.
        assert_eq!(accepted, 720);
    }
}
