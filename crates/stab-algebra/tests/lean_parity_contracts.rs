#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "lean parity contracts use fixed domain fixtures and exact semantic assertions"
)]

use std::str::FromStr;

use num_complex::Complex32;
use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use stab_algebra::{
    CommutingPauliStringIterator, FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString,
    SingleQubitClifford, StabilizerError, Tableau, TableauIterator, stabilizers_to_tableau,
    unitary_to_tableau,
};

#[test]
fn pauli_values_preserve_text_indexing_and_scalar_algebra() {
    for (source, canonical) in [("", "+"), ("-", "-"), ("IXYZ", "+_XYZ"), ("-_ZYX", "-_ZYX")] {
        let parsed = PauliString::from_str(source).expect("valid Pauli fixture");
        assert_eq!(parsed.to_string(), canonical, "source {source:?}");
        assert_eq!(
            PauliString::from_str(canonical).expect("canonical Pauli"),
            parsed
        );
    }

    let mut indexed = PauliString::from_str("-XYZ_").expect("indexed Pauli");
    assert_eq!(
        (0..indexed.len())
            .map(|index| indexed.get(index).expect("in-range Pauli basis"))
            .collect::<Vec<_>>(),
        [PauliBasis::X, PauliBasis::Y, PauliBasis::Z, PauliBasis::I,]
    );
    indexed.set(3, PauliBasis::X).expect("set in-range basis");
    assert_eq!(indexed.to_string(), "-XYZX");
    assert!(indexed.set(4, PauliBasis::Z).is_err());
    assert_eq!(indexed.get(4), None);

    for (basis, x, z) in [
        (PauliBasis::I, false, false),
        (PauliBasis::X, true, false),
        (PauliBasis::Y, true, true),
        (PauliBasis::Z, false, true),
    ] {
        assert_eq!((basis.x_bit(), basis.z_bit()), (x, z));
        assert_eq!(PauliBasis::from_xz(x, z), basis);
    }

    let mut rng = SmallRng::seed_from_u64(0x5041_554c_495f_5031);
    for width in [0, 1, 2, 7, 63, 64, 65, 257] {
        for _ in 0..8 {
            let left = random_pauli(width, &mut rng);
            let right = random_pauli(width, &mut rng);
            let (expected_bases, expected_phase, expected_commutes, expected_byproduct) =
                scalar_pauli_product(&left, &right);
            let actual = left.multiply(&right).expect("equal-width Pauli product");

            assert_eq!(actual.phase(), expected_phase, "width {width}");
            assert_eq!(
                (0..width)
                    .map(|index| actual.get(index).expect("product basis"))
                    .collect::<Vec<_>>(),
                expected_bases,
                "width {width}"
            );
            assert_eq!(
                left.commutes(&right).expect("equal-width commutation"),
                expected_commutes,
                "width {width}"
            );
            assert_eq!(
                left.log_i_scalar_byproduct(&right)
                    .expect("equal-width scalar byproduct"),
                expected_byproduct,
                "width {width}"
            );
        }
    }

    assert_eq!(
        PauliString::from_str("X")
            .expect("X")
            .multiply(&PauliString::from_str("_Z").expect("_Z"))
            .expect("shorter operands extend with identity")
            .to_string(),
        "+XZ"
    );

    assert_eq!(indexed.sparse_string(), "-X0*Y1*Z2*X3");
    assert!(PauliString::from_str("X?").is_err());

    let bases = [PauliBasis::X, PauliBasis::I, PauliBasis::Z];
    for (phase, text) in [
        (PauliPhase::Plus, "+X_Z"),
        (PauliPhase::PlusI, "+iX_Z"),
        (PauliPhase::Minus, "-X_Z"),
        (PauliPhase::MinusI, "-iX_Z"),
    ] {
        let value =
            FlexPauliString::from_phase_and_bases(phase, bases).expect("construct flexible Pauli");
        assert_eq!(value.phase(), phase);
        assert_eq!(value.to_string(), text);
        assert_eq!(FlexPauliString::from_str(text), Ok(value));
    }
    assert_eq!(
        FlexPauliString::from_str("X8*Y2")
            .expect("sparse flexible Pauli")
            .to_string(),
        "+__Y_____X"
    );
    assert_eq!(
        FlexPauliString::from_str("X5*Y5")
            .expect("sparse phase accumulation")
            .to_string(),
        "+i_____Z"
    );
    assert_eq!(
        FlexPauliString::from_str("X")
            .expect("X")
            .multiply(&FlexPauliString::from_str("Y").expect("Y"))
            .expect("X times Y")
            .to_string(),
        "+iZ"
    );
    assert_eq!(
        FlexPauliString::from_str("-Z")
            .expect("real Pauli")
            .try_into_real()
            .expect("real conversion")
            .to_string(),
        "-Z"
    );
    assert_eq!(
        FlexPauliString::from_str("-iZ")
            .expect("imaginary Pauli")
            .try_into_real(),
        Err(StabilizerError::ImaginaryProduct {
            phase: PauliPhase::MinusI,
        })
    );
    for rejected in ["X*", "+-X", "--X", "-+X", "i-X", "-i+X"] {
        assert!(
            FlexPauliString::from_str(rejected).is_err(),
            "accepted malformed flexible Pauli {rejected:?}"
        );
    }
}

#[test]
fn tableaux_preserve_generators_composition_inversion_and_actions() {
    let cnot = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT tableau");
    for (input, expected) in [
        ("+X_", "+XX"),
        ("+Z_", "+Z_"),
        ("+_X", "+_X"),
        ("+_Z", "+ZZ"),
        ("+Y_", "+YX"),
        ("+_Y", "+ZY"),
        ("+YY", "-XZ"),
    ] {
        assert_eq!(
            cnot.apply(&PauliString::from_str(input).expect("input Pauli"))
                .expect("CNOT action")
                .to_string(),
            expected,
            "input {input}"
        );
    }
    assert_eq!(cnot.x_output(0).expect("X0 output").to_string(), "+XX");
    assert_eq!(cnot.y_output(0).expect("Y0 output").to_string(), "+YX");
    assert_eq!(cnot.z_output(1).expect("Z1 output").to_string(), "+ZZ");
    assert!(cnot.x_output(2).is_err());
    let x_rows = [[1, 1], [0, 1]];
    let y_rows = [[2, 1], [3, 2]];
    let z_rows = [[3, 0], [3, 3]];
    for input in 0..2 {
        for output in 0..2 {
            assert_eq!(
                cnot.x_output_pauli_xyz(input, output),
                Ok(x_rows[input][output])
            );
            assert_eq!(
                cnot.y_output_pauli_xyz(input, output),
                Ok(y_rows[input][output])
            );
            assert_eq!(
                cnot.z_output_pauli_xyz(input, output),
                Ok(z_rows[input][output])
            );
        }
    }
    assert!(cnot.y_output(2).is_err());
    assert!(cnot.z_output_pauli_xyz(0, 2).is_err());

    let malformed = stab_algebra::advanced::tableau_from_output_columns_unchecked(
        vec![PauliString::from_str("+X").expect("malformed X output")],
        vec![PauliString::from_str("+X").expect("malformed Z output")],
    );
    assert!(
        !malformed
            .satisfies_invariants()
            .expect("check malformed advanced Tableau")
    );
    assert!(Tableau::gate1("+X", "+X").is_err());
    assert_eq!(
        Tableau::gate1("XX", "Z"),
        Err(StabilizerError::LengthMismatch { left: 2, right: 1 })
    );
    assert_eq!(
        cnot.apply(&PauliString::from_str("X").expect("short Pauli")),
        Err(StabilizerError::LengthMismatch { left: 1, right: 2 })
    );
    assert_eq!(
        Tableau::gate1("+Z", "+X")
            .expect("Hadamard tableau")
            .to_pauli_string(),
        Err(StabilizerError::NotPauliProduct)
    );

    let phase = Tableau::gate1("+Y", "+Z").expect("phase tableau");
    for (input, expected) in [("+X", "+Y"), ("+Y", "-X"), ("+Z", "+Z")] {
        assert_eq!(
            phase
                .apply(&PauliString::from_str(input).expect("phase input"))
                .expect("phase action")
                .to_string(),
            expected
        );
    }
    assert_eq!(
        phase.inverse().expect("phase inverse"),
        Tableau::gate1("-Y", "+Z").expect("inverse phase tableau")
    );

    let mut rng = SmallRng::seed_from_u64(0x5441_424c_4541_5558);
    for width in [1, 2, 7, 16] {
        let identity = Tableau::identity(width).expect("identity tableau");
        for _ in 0..4 {
            let first = Tableau::random(width, &mut rng).expect("first random tableau");
            let second = Tableau::random(width, &mut rng).expect("second random tableau");
            let input = PauliString::random(width, &mut rng).expect("random Pauli");
            let composed = first.then(&second).expect("tableau composition");
            let inverse = composed.inverse().expect("tableau inverse");

            assert!(composed.satisfies_invariants().expect("tableau invariants"));
            assert_eq!(
                composed.apply(&input).expect("composed action"),
                second
                    .apply(&first.apply(&input).expect("first action"))
                    .expect("second action")
            );
            assert_eq!(
                inverse
                    .apply(&composed.apply(&input).expect("forward action"))
                    .expect("inverse action"),
                input
            );
            assert_eq!(composed.then(&inverse).expect("right inverse"), identity);
            assert_eq!(inverse.then(&composed).expect("left inverse"), identity);
        }
    }

    let boundary_x =
        PauliString::from_str(&format!("{}X", "_".repeat(64))).expect("word-boundary X");
    let boundary_z =
        PauliString::from_str(&format!("{}Z", "_".repeat(64))).expect("word-boundary Z");
    let word_boundary = Tableau::from_pauli_string(&boundary_x).expect("65-qubit Pauli tableau");
    let boundary_inverse = word_boundary.inverse().expect("word-boundary inverse");
    assert_eq!(
        word_boundary
            .apply(&boundary_z)
            .expect("word-boundary action"),
        boundary_z.with_sign(PauliSign::Minus)
    );
    assert_eq!(
        word_boundary
            .then(&boundary_inverse)
            .expect("word-boundary inverse composition"),
        Tableau::identity(65).expect("65-qubit identity")
    );

    let wide = Tableau::identity(500).expect("wide identity tableau");
    assert_eq!(
        wide.x_output(0)
            .expect("first wide X")
            .active_terms()
            .collect::<Vec<_>>(),
        vec![(0, PauliBasis::X)]
    );
    assert_eq!(
        wide.z_output(499)
            .expect("last wide Z")
            .active_terms()
            .collect::<Vec<_>>(),
        vec![(499, PauliBasis::Z)]
    );

    let mut commuting = CommutingPauliStringIterator::new(2).expect("commuting iterator");
    commuting
        .restart_iter(
            std::slice::from_ref(&PauliString::from_str("+Z_").expect("Z_")),
            std::slice::from_ref(&PauliString::from_str("+XX").expect("XX")),
        )
        .expect("commuting constraints");
    let first_pass = commuting
        .by_ref()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    commuting.restart_iter_same_constraints();
    assert_eq!(
        commuting
            .by_ref()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        first_pass
    );
    let before_invalid_restart = commuting.clone();
    assert_eq!(
        commuting.restart_iter(
            std::slice::from_ref(&PauliString::from_str("+Z").expect("wrong-width Z")),
            &[],
        ),
        Err(StabilizerError::LengthMismatch { left: 1, right: 2 })
    );
    assert_eq!(commuting, before_invalid_restart);

    commuting
        .restart_iter(
            std::slice::from_ref(&PauliString::from_str("+Z_").expect("Z_")),
            std::slice::from_ref(&PauliString::from_str("+Z_").expect("Z_")),
        )
        .expect("contradictory constraints are a valid empty query");
    assert_eq!(commuting.next(), None);

    for signed in [false, true] {
        let mut empty = TableauIterator::new(0, signed).expect("zero-qubit tableau iterator");
        assert_eq!(
            empty.next(),
            Some(Tableau::identity(0).expect("empty tableau"))
        );
        assert_eq!(empty.next(), None);
        empty.restart().expect("restart empty iterator");
        assert_eq!(empty.count(), 1);
    }

    let mut tableaus = TableauIterator::new(1, true).expect("tableau iterator");
    let first = tableaus.next().expect("first tableau");
    let mut tableaus_clone = tableaus.clone();
    assert_eq!(tableaus.next(), tableaus_clone.next());
    assert!(first.satisfies_invariants().expect("iterator invariants"));
    tableaus.restart().expect("restart tableau iterator");
    assert_eq!(tableaus.next(), Some(first));
}

#[test]
fn algebra_owned_representation_conversions_preserve_values() {
    for clifford in SingleQubitClifford::all() {
        let tableau = clifford.tableau();
        for basis in [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z] {
            let input = PauliString::from_bases(PauliSign::Plus, [basis]).expect("one Pauli");
            let output = tableau.apply(&input).expect("Clifford tableau action");
            assert_eq!(
                output.get(0).expect("one output basis"),
                clifford.apply_basis(basis).expect("Clifford basis action"),
                "{} acting on {basis:?}",
                clifford.canonical_name()
            );
        }
    }

    for source in ["+", "+X", "+Y", "+_XZX__YZZX"] {
        let pauli = PauliString::from_str(source).expect("Pauli-product fixture");
        assert_eq!(
            Tableau::from_pauli_string(&pauli)
                .expect("Pauli to tableau")
                .to_pauli_string()
                .expect("tableau to Pauli"),
            pauli
        );
    }
    let plus_y = PauliString::from_str("+Y").expect("positive Y");
    let minus_y = PauliString::from_str("-Y").expect("negative Y");
    assert_eq!(
        Tableau::from_pauli_string(&minus_y).expect("negative Pauli tableau"),
        Tableau::from_pauli_string(&plus_y).expect("positive Pauli tableau")
    );
    assert_eq!(
        Tableau::from_pauli_string(&minus_y)
            .expect("negative Pauli tableau")
            .to_pauli_string()
            .expect("canonical Pauli product"),
        plus_y,
        "global phase is not represented by a conjugation tableau"
    );

    let hadamard = matrix(&[
        &[(FRAC_1_SQRT_2, 0.0), (FRAC_1_SQRT_2, 0.0)],
        &[(FRAC_1_SQRT_2, 0.0), (-FRAC_1_SQRT_2, 0.0)],
    ]);
    let cnot = matrix(&[
        &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        &[(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
    ]);
    assert_eq!(
        unitary_to_tableau(&hadamard, true).expect("Hadamard unitary"),
        Tableau::gate1("+Z", "+X").expect("Hadamard tableau")
    );
    assert_eq!(
        unitary_to_tableau(&cnot, true).expect("little-endian reversed CNOT unitary"),
        Tableau::gate2("+X_", "+ZZ", "+XX", "+_Z").expect("reversed CNOT tableau")
    );
    assert_eq!(
        unitary_to_tableau(&cnot, false).expect("big-endian CNOT unitary"),
        Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT tableau")
    );
    assert_eq!(
        unitary_to_tableau(&[], true),
        Err(StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height: 0 })
    );
    assert_eq!(
        unitary_to_tableau(&[vec![], vec![]], true),
        Err(StabilizerError::UnitaryMatrixRowWidthMismatch {
            row: 0,
            width: 0,
            height: 2,
        })
    );
    assert_eq!(
        unitary_to_tableau(
            &[
                vec![Complex32::new(1.0, 0.0), Complex32::new(0.0, 0.0)],
                vec![Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0)],
            ],
            true,
        ),
        Err(StabilizerError::MatrixNotUnitary)
    );
    assert_eq!(
        unitary_to_tableau(
            &[
                vec![Complex32::new(1.0, 0.0), Complex32::new(0.0, 0.0)],
                vec![
                    Complex32::new(0.0, 0.0),
                    Complex32::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2),
                ],
            ],
            true,
        ),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );

    let mut rng = SmallRng::seed_from_u64(0x434f_4e56_4552_5431);
    for width in [1, 2, 5, 16] {
        for _ in 0..4 {
            let source = Tableau::random(width, &mut rng).expect("source tableau");
            let stabilizers = (0..width)
                .map(|index| source.z_output(index).expect("Z generator").clone())
                .collect::<Vec<_>>();
            let recovered = stabilizers_to_tableau(&stabilizers, false, false, false)
                .expect("stabilizers to tableau");
            for index in 0..width {
                assert_eq!(
                    recovered.z_output(index).expect("recovered generator"),
                    source.z_output(index).expect("source generator")
                );
            }
            assert!(recovered.satisfies_invariants().expect("valid recovery"));
        }
    }
}

const FRAC_1_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn random_pauli(width: usize, rng: &mut SmallRng) -> PauliString {
    let sign = if rng.random::<bool>() {
        PauliSign::Plus
    } else {
        PauliSign::Minus
    };
    let bases = (0..width).map(|_| match rng.random_range(0..4) {
        0 => PauliBasis::I,
        1 => PauliBasis::X,
        2 => PauliBasis::Y,
        _ => PauliBasis::Z,
    });
    PauliString::from_bases(sign, bases).expect("admitted random Pauli")
}

fn scalar_pauli_product(
    left: &PauliString,
    right: &PauliString,
) -> (Vec<PauliBasis>, PauliPhase, bool, u8) {
    let mut phase_exponent =
        u8::from(left.sign().is_negative()) * 2 + u8::from(right.sign().is_negative()) * 2;
    let mut byproduct_exponent = 0_u8;
    let mut anticommutes = false;
    let bases = (0..left.len())
        .map(|index| {
            let left_basis = left.get(index).expect("left scalar basis");
            let right_basis = right.get(index).expect("right scalar basis");
            let (basis, exponent) = scalar_basis_product(left_basis, right_basis);
            phase_exponent = phase_exponent.wrapping_add(exponent);
            byproduct_exponent = byproduct_exponent.wrapping_add(exponent);
            anticommutes ^= left_basis != PauliBasis::I
                && right_basis != PauliBasis::I
                && left_basis != right_basis;
            basis
        })
        .collect();
    let phase = match phase_exponent & 3 {
        0 => PauliPhase::Plus,
        1 => PauliPhase::PlusI,
        2 => PauliPhase::Minus,
        _ => PauliPhase::MinusI,
    };
    (bases, phase, !anticommutes, byproduct_exponent & 3)
}

fn scalar_basis_product(left: PauliBasis, right: PauliBasis) -> (PauliBasis, u8) {
    use PauliBasis::{I, X, Y, Z};
    match (left, right) {
        (I, basis) | (basis, I) => (basis, 0),
        (X, X) | (Y, Y) | (Z, Z) => (I, 0),
        (X, Y) => (Z, 1),
        (Y, Z) => (X, 1),
        (Z, X) => (Y, 1),
        (Y, X) => (Z, 3),
        (Z, Y) => (X, 3),
        (X, Z) => (Y, 3),
    }
}

fn matrix(rows: &[&[(f32, f32)]]) -> Vec<Vec<Complex32>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|&(real, imaginary)| Complex32::new(real, imaginary))
                .collect()
        })
        .collect()
}
