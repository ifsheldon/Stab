#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "semantic parity failures need direct fixture context"
)]

use std::str::FromStr;

use stab_algebra::{PauliBasis, PauliString, Tableau, TableauIterator};
use stab_analysis::{
    ResourceKind, ResourceOperation, circuit_to_tableau, pauli_after_circuit, pauli_before_circuit,
    tableau_to_circuit,
};
use stab_model::{Circuit, CircuitItem};

#[test]
fn algebra_circuit_conversions_match_stim() {
    let empty = Tableau::identity(0).expect("empty tableau");
    assert_synthesis_round_trip(&empty);

    for tableau in TableauIterator::new(1, true).expect("signed one-qubit tableaus") {
        assert_synthesis_round_trip(&tableau);
    }

    let mut signed_two_qubit_count = 0;
    for tableau in TableauIterator::new(2, true).expect("signed two-qubit tableaus") {
        assert_synthesis_round_trip(&tableau);
        signed_two_qubit_count += 1;
    }
    assert_eq!(signed_two_qubit_count, 11_520);

    for source in [
        "H 0\nCX 0 1\nS 1\n",
        "ISWAP 0 1\n",
        "H 0 1 2\nCX 0 1 1 2\nS 0 2\nCX 2 0\n",
    ] {
        let tableau = circuit_to_tableau(&circuit(source), false, false, false)
            .expect("source circuit tableau");
        assert_synthesis_round_trip(&tableau);
    }
    assert_pauli_circuit_conjugation();
}

#[test]
fn tableau_synthesis_round_trips_representative_wide_cliffords() {
    for width in [16, 64, 256] {
        let source = structured_clifford(width);
        let tableau = circuit_to_tableau(&source, false, false, false)
            .expect("structured source circuit tableau");
        assert_synthesis_round_trip(&tableau);
    }
}

fn assert_pauli_circuit_conjugation() {
    // Adapted from Stim v1.16.0 pauli_string.after_circuit and before_circuit tests.
    let unitary = circuit("H 1\nCX 1 2\nS 2\n");
    assert_eq!(
        pauli_after_circuit(&pauli("+_XYZ"), &unitary).expect("unitary after"),
        pauli("-__XZ")
    );
    assert_eq!(
        pauli_before_circuit(&pauli("-__XZ"), &unitary).expect("unitary before"),
        pauli("+_XYZ")
    );

    let annotated = circuit(
        "QUBIT_COORDS(2, 3) 5\nREPEAT 5 {\n    DETECTOR rec[-1]\n}\nH 1\nTICK\nOBSERVABLE_INCLUDE(0) rec[-1]\nCX 1 2\nS 2\nSHIFT_COORDS(1, 2, 3)\n",
    );
    assert_eq!(
        pauli_after_circuit(&pauli("+_XYZ"), &annotated).expect("ignore annotations"),
        pauli("-__XZ")
    );

    assert_eq!(
        pauli_before_circuit(&pauli("+Z"), &circuit("R 0\n")).expect("undo reset"),
        pauli("+_")
    );
    assert_eq!(
        pauli_after_circuit(&pauli("+_"), &circuit("R 0\n")).expect("avoid reset"),
        pauli("+_")
    );
    assert!(pauli_after_circuit(&pauli("+Z"), &circuit("R 0\n")).is_err());
    assert!(pauli_before_circuit(&pauli("+X"), &circuit("R 0\n")).is_err());

    assert_eq!(
        pauli_after_circuit(&pauli("+Z_"), &circuit("M 0\nH 1\n")).expect("commuting measurement"),
        pauli("+Z_")
    );
    assert!(pauli_after_circuit(&pauli("+X_"), &circuit("M 0\nH 1\n")).is_err());

    let mpp = circuit("MPP X2*Y3*Z4 X5*X6\nH 1\n");
    assert_eq!(
        pauli_after_circuit(&pauli("+_XXYZXX"), &mpp).expect("commuting MPP"),
        pauli("+_ZXYZXX")
    );
    assert!(pauli_after_circuit(&pauli("+__XXYZX"), &mpp).is_err());

    for (gate, commuting, anticommuting) in [
        ("CX rec[-1] 0", "+X_", "+Z_"),
        ("CY rec[-1] 0", "+Y_", "+X_"),
        ("CZ rec[-1] 0", "+Z_", "+X_"),
        ("XCZ 0 rec[-1]", "+X_", "+Z_"),
        ("YCZ 0 rec[-1]", "+Y_", "+X_"),
    ] {
        let controlled = circuit(&format!("M 1\n{gate}\n"));
        assert_eq!(
            pauli_after_circuit(&pauli(commuting), &controlled).expect("commuting feedback"),
            pauli(commuting),
            "{gate} after"
        );
        assert_eq!(
            pauli_before_circuit(&pauli(commuting), &controlled).expect("commuting feedback"),
            pauli(commuting),
            "{gate} before"
        );
        assert!(
            pauli_after_circuit(&pauli(anticommuting), &controlled).is_err(),
            "{gate} after"
        );
        assert!(
            pauli_before_circuit(&pauli(anticommuting), &controlled).is_err(),
            "{gate} before"
        );
    }

    let classical_cz = circuit("M 0\nCZ rec[-1] sweep[0]\n");
    assert_eq!(
        pauli_after_circuit(&pauli("+Z"), &classical_cz).expect("classical CZ no-op"),
        pauli("+Z")
    );

    let mut wide = PauliString::identity(1_024).expect("wide Pauli");
    wide.set(777, PauliBasis::X).expect("wide Pauli support");
    let mut expected = PauliString::identity(1_024).expect("wide expected Pauli");
    expected
        .set(777, PauliBasis::Z)
        .expect("wide expected support");
    assert_eq!(
        pauli_after_circuit(&wide, &circuit("H 777\n")).expect("wide local Clifford"),
        expected
    );
    assert_eq!(
        pauli_before_circuit(&expected, &circuit("H 777\n")).expect("wide local inverse"),
        wide
    );
}

#[test]
fn pauli_circuit_conjugation_keeps_compact_unitaries_and_bounds_nonunitary_work() {
    let folded = circuit("REPEAT 1000000000 {\n    H 0\n}\n");
    assert_eq!(
        pauli_after_circuit(&pauli("+X"), &folded).expect("compact unitary repeat"),
        pauli("+X")
    );

    assert!(pauli_after_circuit(&pauli("+_"), &circuit("X_ERROR(0) 0\n")).is_err());
    assert!(pauli_after_circuit(&pauli("+_"), &circuit("H 1\n")).is_err());
    assert!(pauli_after_circuit(&pauli("+__"), &circuit("MXX 0 1\n")).is_err());

    let excessive = circuit("REPEAT 1000001 {\n    M 0\n}\n");
    let error = pauli_after_circuit(&pauli("+Z"), &excessive)
        .expect_err("nonunitary expanded-work boundary");
    let resource = error
        .resource_limit_error()
        .expect("typed Pauli-conjugation resource error");
    assert_eq!(resource.operation(), ResourceOperation::PauliConjugation);
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);
}

fn assert_synthesis_round_trip(tableau: &Tableau) {
    let circuit = tableau_to_circuit(tableau).expect("synthesize tableau");
    assert_eq!(
        circuit_to_tableau(&circuit, false, false, false).expect("round-trip tableau"),
        *tableau
    );
    for item in circuit.items() {
        let CircuitItem::Instruction(instruction) = item else {
            panic!("tableau synthesis emitted a repeat block");
        };
        assert!(
            matches!(instruction.gate().canonical_name(), "H" | "S" | "CX"),
            "{}",
            instruction.gate().canonical_name()
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn structured_clifford(width: usize) -> Circuit {
    use std::fmt::Write as _;

    let mut text = String::from("H");
    for qubit in 0..width {
        write!(text, " {qubit}").expect("write H target");
    }
    text.push_str("\nS");
    for qubit in (0..width).step_by(2) {
        write!(text, " {qubit}").expect("write S target");
    }
    text.push_str("\nCX");
    for qubit in 0..width.saturating_sub(1) {
        write!(text, " {qubit} {}", qubit + 1).expect("write CX targets");
    }
    text.push('\n');
    circuit(&text)
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse Pauli string")
}
