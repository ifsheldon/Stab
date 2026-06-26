#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M6 parity tests use compact upstream examples and direct assertions keep failures readable"
)]

use std::str::FromStr;

use proptest::prelude::*;
use stab_core::{
    CliffordString, FlexPauliString, Gate, PauliBasis, PauliPhase, PauliString,
    SingleQubitClifford, Tableau,
};

#[test]
fn stabilizers_pauli_string_dense_text_round_trips_follow_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/pauli_string.test.cc.
    assert_eq!(
        PauliString::from_str("+IXYZ").expect("parse").to_string(),
        "+_XYZ"
    );
    assert_eq!(PauliString::from_str("X").expect("parse").to_string(), "+X");
    assert_eq!(
        PauliString::from_str("-XZ").expect("parse").to_string(),
        "-XZ"
    );
    assert_eq!(PauliString::identity(5).to_string(), "+_____");
    assert_eq!(PauliString::from_str("").expect("parse").to_string(), "+");
    assert_eq!(PauliString::from_str("-").expect("parse").to_string(), "-");
    assert_ne!(
        PauliString::from_str("").expect("parse"),
        PauliString::from_str("-").expect("parse")
    );
    assert!(PauliString::from_str("x").is_err());
}

#[test]
fn stabilizers_pauli_basis_xz_mapping_matches_stim() {
    assert_eq!(PauliBasis::from_xz(false, false), PauliBasis::I);
    assert_eq!(PauliBasis::from_xz(true, false), PauliBasis::X);
    assert_eq!(PauliBasis::from_xz(true, true), PauliBasis::Y);
    assert_eq!(PauliBasis::from_xz(false, true), PauliBasis::Z);

    assert_eq!(
        (PauliBasis::I.x_bit(), PauliBasis::I.z_bit()),
        (false, false)
    );
    assert_eq!(
        (PauliBasis::X.x_bit(), PauliBasis::X.z_bit()),
        (true, false)
    );
    assert_eq!((PauliBasis::Y.x_bit(), PauliBasis::Y.z_bit()), (true, true));
    assert_eq!(
        (PauliBasis::Z.x_bit(), PauliBasis::Z.z_bit()),
        (false, true)
    );
}

#[test]
fn stabilizers_pauli_scalar_byproduct_table_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/pauli_string.test.cc log_i_scalar_byproduct.
    let i = pauli("_");
    let x = pauli("X");
    let y = pauli("Y");
    let z = pauli("Z");

    assert_eq!(i.log_i_scalar_byproduct(&i).expect("byproduct"), 0);
    assert_eq!(i.log_i_scalar_byproduct(&x).expect("byproduct"), 0);
    assert_eq!(i.log_i_scalar_byproduct(&y).expect("byproduct"), 0);
    assert_eq!(i.log_i_scalar_byproduct(&z).expect("byproduct"), 0);
    assert_eq!(x.log_i_scalar_byproduct(&i).expect("byproduct"), 0);
    assert_eq!(x.log_i_scalar_byproduct(&x).expect("byproduct"), 0);
    assert_eq!(x.log_i_scalar_byproduct(&y).expect("byproduct"), 1);
    assert_eq!(x.log_i_scalar_byproduct(&z).expect("byproduct"), 3);
    assert_eq!(y.log_i_scalar_byproduct(&i).expect("byproduct"), 0);
    assert_eq!(y.log_i_scalar_byproduct(&x).expect("byproduct"), 3);
    assert_eq!(y.log_i_scalar_byproduct(&y).expect("byproduct"), 0);
    assert_eq!(y.log_i_scalar_byproduct(&z).expect("byproduct"), 1);
    assert_eq!(z.log_i_scalar_byproduct(&i).expect("byproduct"), 0);
    assert_eq!(z.log_i_scalar_byproduct(&x).expect("byproduct"), 1);
    assert_eq!(z.log_i_scalar_byproduct(&y).expect("byproduct"), 3);
    assert_eq!(z.log_i_scalar_byproduct(&z).expect("byproduct"), 0);

    assert_eq!(
        pauli("XX")
            .log_i_scalar_byproduct(&pauli("XY"))
            .expect("byproduct"),
        1
    );
    assert_eq!(
        pauli("XX")
            .log_i_scalar_byproduct(&pauli("ZY"))
            .expect("byproduct"),
        0
    );
    assert_eq!(
        pauli("XX")
            .log_i_scalar_byproduct(&pauli("YY"))
            .expect("byproduct"),
        2
    );
    assert_eq!(
        pauli("X_")
            .log_i_scalar_byproduct(&pauli("ZZ"))
            .expect("byproduct"),
        3
    );
    assert_eq!(
        pauli("X")
            .log_i_scalar_byproduct(&pauli("_Z"))
            .expect("byproduct"),
        0
    );
}

#[test]
fn stabilizers_pauli_multiplication_tracks_real_and_imaginary_phases() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/pauli_string.test.cc multiplication.
    assert_eq!(
        pauli("X").multiply(&pauli("Y")).expect("multiply").phase(),
        PauliPhase::PlusI
    );
    assert_eq!(
        pauli("X")
            .multiply(&pauli("Y"))
            .expect("multiply")
            .to_string(),
        "+iZ"
    );
    assert_eq!(
        pauli("Y")
            .multiply(&pauli("X"))
            .expect("multiply")
            .to_string(),
        "-iZ"
    );
    assert_eq!(
        pauli("X")
            .multiply(&pauli("Z"))
            .expect("multiply")
            .to_string(),
        "-iY"
    );
    assert_eq!(
        pauli("Z")
            .multiply(&pauli("X"))
            .expect("multiply")
            .to_string(),
        "+iY"
    );
    assert_eq!(
        pauli("XXI")
            .multiply(&pauli("YYY"))
            .expect("multiply")
            .to_string(),
        "-ZZY"
    );
    assert_eq!(
        pauli("-XXI")
            .multiply(&pauli("YYY"))
            .expect("multiply")
            .to_string(),
        "+ZZY"
    );
    assert!(pauli("X").multiply_real(&pauli("Y")).is_err());
    assert_eq!(
        pauli("XXI")
            .multiply_real(&pauli("YYY"))
            .expect("real product")
            .to_string(),
        "-ZZY"
    );
}

#[test]
fn stabilizers_pauli_commutation_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/pauli_string.test.cc and pauli_string_ref.test.cc.
    assert_commutes("I", "I", true);
    assert_commutes("I", "X", true);
    assert_commutes("X", "Y", false);
    assert_commutes("X", "Z", false);
    assert_commutes("Y", "Z", false);
    assert_commutes("Z", "Z", true);
    assert_commutes("XX", "ZZ", true);
    assert_commutes("-XX", "ZZ", true);
    assert_commutes("XZ", "ZZ", false);
    assert_commutes("X", "", true);
    assert_commutes("", "Z", true);
}

#[test]
fn stabilizers_pauli_ref_weight_intersection_and_active_terms_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/pauli_string_ref.test.cc.
    assert_eq!(pauli("+").weight(), 0);
    assert_eq!(pauli("+I").weight(), 0);
    assert_eq!(pauli("+X").weight(), 1);
    assert_eq!(pauli("+XZ").weight(), 2);
    assert!(pauli("+II").has_no_pauli_terms());
    assert!(!pauli("+IX").has_no_pauli_terms());

    assert!(!pauli("_").intersects(&pauli("_")).expect("intersects"));
    assert!(!pauli("_").intersects(&pauli("X")).expect("intersects"));
    assert!(pauli("X").intersects(&pauli("Y")).expect("intersects"));
    assert!(pauli("_Z").intersects(&pauli("ZZ")).expect("intersects"));
    assert!(!pauli("__").intersects(&pauli("XZ")).expect("intersects"));
    assert_eq!(
        pauli("X____________________Y")
            .active_terms()
            .collect::<Vec<_>>(),
        vec![(0, PauliBasis::X), (21, PauliBasis::Y)]
    );
}

#[test]
fn stabilizers_pauli_sparse_string_matches_stim() {
    assert_eq!(pauli("IIIII").sparse_string(), "+I");
    assert_eq!(pauli("-IIIII").sparse_string(), "-I");
    assert_eq!(pauli("IIIXI").sparse_string(), "+X3");
    assert_eq!(pauli("IYIXZ").sparse_string(), "+Y1*X3*Z4");
    assert_eq!(pauli("-IYIXZ").sparse_string(), "-Y1*X3*Z4");
}

#[test]
fn stabilizers_flex_pauli_dense_and_sparse_text_follow_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/flex_pauli_string.test.cc.
    let f = flex("-iIXYZ_xyz");
    assert_eq!(f.len(), 8);
    assert_eq!(f.phase(), PauliPhase::MinusI);
    assert_eq!(f.value().x_bits()[0], 0b01100110);
    assert_eq!(f.value().z_bits()[0], 0b11001100);

    assert_eq!(flex("iX").phase(), PauliPhase::PlusI);
    assert_eq!(flex("iX").to_string(), "+iX");
    assert_eq!(flex("Y").phase(), PauliPhase::Plus);
    assert_eq!(flex("+Z").phase(), PauliPhase::Plus);

    assert_eq!(flex("X8").len(), 9);
    assert_eq!(flex("X8").value().x_bits()[0], 0b100000000);
    assert_eq!(flex("X8*Y2").value().x_bits()[0], 0b100000100);
    assert_eq!(flex("X8*Y2").value().z_bits()[0], 0b000000100);
    assert_eq!(flex("X8*Y2*X8").to_string(), "+__Y______");
    assert_eq!(flex("X8*Y2*Y8").phase(), PauliPhase::PlusI);
    assert_eq!(flex("Y8*Y2*X8").phase(), PauliPhase::MinusI);
    assert_eq!(flex("X1"), flex("_X"));
    assert_eq!(flex("X20*I21"), flex("____________________X_"));
}

#[test]
fn stabilizers_flex_pauli_multiplication_matches_stim() {
    assert_eq!(
        flex("iXYZ").multiply(&flex("i__Z")).expect("multiply"),
        flex("-XY_")
    );
    assert_eq!(
        flex("-iX")
            .multiply(&flex("iY"))
            .expect("multiply")
            .to_string(),
        "+iZ"
    );
    assert_eq!(
        flex("X")
            .multiply(&flex("Y"))
            .expect("multiply")
            .to_string(),
        "+iZ"
    );
    assert_eq!(
        flex("X")
            .multiply(&flex("_Z"))
            .expect("multiply")
            .to_string(),
        "+XZ"
    );
}

#[test]
fn stabilizers_clifford_string_set_gate_at_vs_str_vs_gate_at_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/clifford_string.test.cc.
    let gates = upstream_clifford_gate_order();
    let mut cliffords = CliffordString::identity(gates.len());
    for (index, gate) in gates.iter().copied().enumerate() {
        cliffords
            .set_gate_at(index, gate)
            .expect("set Clifford gate");
    }

    assert_eq!(
        cliffords.to_string(),
        "_I _X _Y _Z HI HX HY HZ SI SX SY SZ VI VX VY VZ uI uX uY uZ dI dX dY dZ"
    );
    for (index, gate) in gates.into_iter().enumerate() {
        assert_eq!(cliffords.gate_at(index), Some(gate));
    }
    assert_eq!(cliffords.gate_at(24), None);
}

#[test]
fn stabilizers_single_qubit_clifford_gate_conversion_matches_stim() {
    for gate in SingleQubitClifford::all() {
        let parsed_gate = Gate::from_name(gate.canonical_name()).expect("single-qubit gate name");
        assert_eq!(
            SingleQubitClifford::from_gate(parsed_gate).expect("single-qubit Clifford"),
            gate
        );
    }
    assert!(SingleQubitClifford::from_gate(Gate::from_name("CX").expect("CX")).is_err());
}

#[test]
fn stabilizers_clifford_string_known_identities_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/clifford_string.test.cc known_identities.
    let h = CliffordString::from_gates([SingleQubitClifford::H]);
    let s = CliffordString::from_gates([SingleQubitClifford::S]);
    let s_dag = CliffordString::from_gates([SingleQubitClifford::SDag]);

    assert_eq!(h.multiply(&h).expect("H*H"), CliffordString::identity(1));
    assert_eq!(
        s.multiply(&s).expect("S*S"),
        CliffordString::from_gates([SingleQubitClifford::Z])
    );
    assert_eq!(
        h.multiply(&s_dag).expect("H*S_DAG"),
        CliffordString::from_gates([SingleQubitClifford::Cxyz])
    );
}

#[test]
fn stabilizers_clifford_string_concat_repeat_and_padding_are_stim_like() {
    let left = CliffordString::from_gates([SingleQubitClifford::H, SingleQubitClifford::S]);
    let right = CliffordString::from_gates([SingleQubitClifford::X]);

    assert_eq!(left.concat(&right).expect("concat").to_string(), "HI SI _X");
    assert_eq!(right.repeat(3).expect("repeat").to_string(), "_X _X _X");
    assert_eq!(left.multiply(&right).expect("padded multiply").len(), 2);
}

#[test]
fn stabilizers_single_qubit_clifford_multiplication_is_associative() {
    let gates = SingleQubitClifford::all().collect::<Vec<_>>();
    for left in gates.iter().copied() {
        for middle in gates.iter().copied() {
            for right in gates.iter().copied() {
                let lhs = left
                    .multiply(middle)
                    .expect("left middle")
                    .multiply(right)
                    .expect("(left middle) right");
                let rhs = left
                    .multiply(middle.multiply(right).expect("middle right"))
                    .expect("left (middle right)");
                assert_eq!(lhs, rhs);
            }
        }
    }
}

#[test]
fn stabilizers_tableau_identity_and_string_format_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/tableau.test.cc identity and str.
    let identity = Tableau::identity(4);
    assert_eq!(
        identity.to_string(),
        "+-xz-xz-xz-xz-\n\
         | ++ ++ ++ ++\n\
         | XZ __ __ __\n\
         | __ XZ __ __\n\
         | __ __ XZ __\n\
         | __ __ __ XZ"
    );

    assert_eq!(
        Tableau::gate1("+X", "-Z").expect("gate1").to_string(),
        "+-xz-\n\
         | +-\n\
         | XZ"
    );
}

#[test]
fn stabilizers_tableau_gate1_gate2_and_eval_y_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/tableau.test.cc gate1 and eval_y.
    let gate1 = Tableau::gate1("+X", "+Z").expect("gate1");
    assert_eq!(gate1.x_output(0).expect("x output").to_string(), "+X");
    assert_eq!(gate1.y_output(0).expect("y output").to_string(), "+Y");
    assert_eq!(gate1.z_output(0).expect("z output").to_string(), "+Z");

    let sqrt_z = Tableau::gate1("+Y", "+Z").expect("sqrt_z");
    assert_eq!(sqrt_z.y_output(0).expect("sqrt_z y").to_string(), "-X");

    let sqrt_x = Tableau::gate1("+X", "-Y").expect("sqrt_x");
    assert_eq!(sqrt_x.y_output(0).expect("sqrt_x y").to_string(), "+Z");

    let zcx = cnot_tableau();
    assert_eq!(zcx.z_output(1).expect("z1 output").to_string(), "+ZZ");
    assert_eq!(zcx.y_output(1).expect("y1 output").to_string(), "+ZY");
}

#[test]
fn stabilizers_tableau_eval_matches_stim_examples() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/tableau.test.cc eval.
    let cnot = cnot_tableau();
    assert_eq!(cnot.apply(&pauli("-XX")).expect("eval").to_string(), "-X_");
    assert_eq!(cnot.apply(&pauli("+XX")).expect("eval").to_string(), "+X_");
    assert_eq!(cnot.apply(&pauli("+ZZ")).expect("eval").to_string(), "+_Z");
    assert_eq!(cnot.apply(&pauli("+IY")).expect("eval").to_string(), "+ZY");
    assert_eq!(cnot.apply(&pauli("+YI")).expect("eval").to_string(), "+YX");
    assert_eq!(cnot.apply(&pauli("+YY")).expect("eval").to_string(), "-XZ");

    let sqrt_x = Tableau::gate1("+X", "-Y").expect("sqrt_x");
    assert_eq!(sqrt_x.apply(&pauli("+X")).expect("eval").to_string(), "+X");
    assert_eq!(sqrt_x.apply(&pauli("+Y")).expect("eval").to_string(), "+Z");
    assert_eq!(sqrt_x.apply(&pauli("+Z")).expect("eval").to_string(), "-Y");

    let sqrt_z = Tableau::gate1("+Y", "+Z").expect("sqrt_z");
    assert_eq!(sqrt_z.apply(&pauli("+X")).expect("eval").to_string(), "+Y");
    assert_eq!(sqrt_z.apply(&pauli("+Y")).expect("eval").to_string(), "-X");
    assert_eq!(sqrt_z.apply(&pauli("+Z")).expect("eval").to_string(), "+Z");
}

#[test]
fn stabilizers_tableau_then_and_pauli_product_round_trip_match_stim() {
    // Adapted from Stim v1.16.0 src/stim/stabilizers/tableau.test.cc then and from_pauli_string.
    let cnot = cnot_tableau();
    assert_eq!(cnot.then(&cnot).expect("cnot twice"), Tableau::identity(2));

    let pauli_string_empty = pauli("");
    let tableau_empty =
        Tableau::from_pauli_string(&pauli_string_empty).expect("empty pauli tableau");
    assert_eq!(
        tableau_empty
            .to_pauli_string()
            .expect("empty pauli round trip"),
        pauli_string_empty
    );

    let pauli_string = pauli("+_XZX__YZZX");
    let tableau = Tableau::from_pauli_string(&pauli_string).expect("pauli tableau");
    assert_eq!(
        tableau.to_pauli_string().expect("pauli round trip"),
        pauli_string
    );
}

proptest! {
    #[test]
    fn stabilizers_tableau_identity_preserves_dense_pauli_strings(body in bare_pauli_body_strategy(10)) {
        let pauli = pauli(&body);
        let identity = Tableau::identity(pauli.len());
        prop_assert_eq!(identity.apply(&pauli).expect("identity eval"), pauli);
    }
}

proptest! {
    #[test]
    fn stabilizers_pauli_product_is_associative_for_small_dense_strings(
        a in dense_pauli_string_strategy(6),
        b in dense_pauli_string_strategy(6),
        c in dense_pauli_string_strategy(6),
    ) {
        let a = flex(&a);
        let b = flex(&b);
        let c = flex(&c);

        let left = a.multiply(&b).expect("a*b").multiply(&c).expect("(a*b)*c");
        let right = a.multiply(&b.multiply(&c).expect("b*c")).expect("a*(b*c)");
        prop_assert_eq!(left, right);
    }

    #[test]
    fn stabilizers_pauli_commutation_matches_scalar_reference(
        left in bare_pauli_body_strategy(12),
        right in bare_pauli_body_strategy(12),
    ) {
        let left = pauli(&left);
        let right = pauli(&right);
        let expected = scalar_commutes(&left, &right);
        prop_assert_eq!(left.commutes(&right).expect("commutes"), expected);
    }
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse PauliString")
}

fn flex(text: &str) -> FlexPauliString {
    FlexPauliString::from_str(text).expect("parse FlexPauliString")
}

fn cnot_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT tableau")
}

fn upstream_clifford_gate_order() -> Vec<SingleQubitClifford> {
    vec![
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
    ]
}

fn assert_commutes(left: &str, right: &str, expected: bool) {
    assert_eq!(
        pauli(left).commutes(&pauli(right)).expect("commutes"),
        expected
    );
}

fn bare_pauli_body_strategy(max_len: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![Just('_'), Just('X'), Just('Y'), Just('Z')],
        0..=max_len,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn dense_pauli_string_strategy(max_len: usize) -> impl Strategy<Value = String> {
    (
        prop_oneof![
            Just(""),
            Just("+"),
            Just("-"),
            Just("i"),
            Just("+i"),
            Just("-i")
        ],
        bare_pauli_body_strategy(max_len),
    )
        .prop_map(|(prefix, body)| format!("{prefix}{body}"))
}

fn scalar_commutes(left: &PauliString, right: &PauliString) -> bool {
    let mut anticommutes = false;
    for index in 0..left.len().max(right.len()) {
        let left_basis = left.get(index).unwrap_or(PauliBasis::I);
        let right_basis = right.get(index).unwrap_or(PauliBasis::I);
        let anti = left_basis != PauliBasis::I
            && right_basis != PauliBasis::I
            && left_basis != right_basis;
        anticommutes ^= anti;
    }
    !anticommutes
}
