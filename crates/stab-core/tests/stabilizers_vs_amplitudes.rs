#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M6 unitary/tableau parity tests mirror compact upstream examples"
)]

use std::collections::BTreeSet;

use num_complex::Complex32;
use stab_core::{Gate, GateUnitaryMatrix, Tableau, unitary_to_tableau};

#[test]
fn unitary_to_tableau_matches_known_gate_data_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc.
    // Gate matrices and expected tableau outputs come from src/stim/gates/gate_data_*.cc.
    let cases = known_unitary_gate_cases();
    assert_eq!(cases.len(), 46);
    for case in cases {
        let expected = case.expected_tableau();
        assert_eq!(
            unitary_to_tableau(&case.matrix, true).expect(case.name),
            expected,
            "{}",
            case.name
        );
        if case.matrix.len() == 2 {
            assert_eq!(
                unitary_to_tableau(&case.matrix, false).expect(case.name),
                expected,
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn gate_unitary_matrix_metadata_matches_known_gate_data_like_stim() {
    // Uses the same Stim v1.16.0 gate_data rows as the unitary-to-tableau parity test, but checks
    // exact matrix metadata instead of the global-phase-insensitive tableau action.
    let cases = known_unitary_gate_cases();
    assert_eq!(cases.len(), 46);
    for case in cases {
        let gate = Gate::from_name(case.name).expect("known gate");
        assert_eq!(
            gate.unitary_matrix().expect("known gate unitary"),
            case.expected_gate_unitary_matrix(),
            "{}",
            case.name
        );
    }
}

#[test]
fn known_unitary_gate_case_count_tracks_upstream_scope() {
    let cases = known_unitary_gate_cases();
    let one_qubit = cases
        .iter()
        .filter(|case| matches!(case.outputs, GateOutputs::One(_)))
        .count();
    let two_qubit = cases
        .iter()
        .filter(|case| matches!(case.outputs, GateOutputs::Two(_)))
        .count();

    assert_eq!(one_qubit, 24);
    assert_eq!(two_qubit, 22);

    let actual_names = cases.iter().map(|case| case.name).collect::<BTreeSet<_>>();
    let expected_names = BTreeSet::from([
        "C_NXYZ",
        "C_NZYX",
        "C_XNYZ",
        "C_XYNZ",
        "C_XYZ",
        "C_ZNYX",
        "C_ZYNX",
        "C_ZYX",
        "CX",
        "CXSWAP",
        "CY",
        "CZ",
        "CZSWAP",
        "H",
        "H_NXY",
        "H_NXZ",
        "H_NYZ",
        "H_XY",
        "H_YZ",
        "I",
        "II",
        "ISWAP",
        "ISWAP_DAG",
        "S",
        "S_DAG",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SWAP",
        "SWAPCX",
        "X",
        "XCX",
        "XCY",
        "XCZ",
        "Y",
        "YCX",
        "YCY",
        "YCZ",
        "Z",
    ]);
    assert_eq!(actual_names, expected_names);
    assert_eq!(actual_names.len(), cases.len());
}

#[test]
fn unitary_to_tableau_handles_controlled_gate_endianness_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc.
    for (gate, matrix, expected_little, expected_big) in [
        ("XCZ", xcz_unitary(), xcz_tableau(), zcx_tableau()),
        ("ZCX", zcx_unitary(), zcx_tableau(), xcz_tableau()),
        ("XCY", xcy_unitary(), xcy_tableau(), ycx_tableau()),
        ("YCX", ycx_unitary(), ycx_tableau(), xcy_tableau()),
    ] {
        let matrix = matrix_from_rows(matrix);
        assert_eq!(
            unitary_to_tableau(&matrix, true).expect(gate),
            expected_little,
            "{gate} little endian"
        );
        assert_eq!(
            unitary_to_tableau(&matrix, false).expect(gate),
            expected_big,
            "{gate} big endian"
        );
    }
}

#[test]
fn unitary_to_tableau_rejects_non_clifford_or_malformed_matrices_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_vs_amplitudes.test.cc.
    let eighth_turn_phase = f32::sqrt(0.5);
    assert!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(eighth_turn_phase, eighth_turn_phase)],
            ],
            false,
        )
        .is_err()
    );

    assert!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 1.0)],
            ],
            false,
        )
        .is_err()
    );

    assert!(
        unitary_to_tableau(
            &[
                vec![
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                ],
                vec![
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(0.0, 0.0),
                    c(1.0, 0.0),
                    c(0.0, 0.0),
                ],
            ],
            false,
        )
        .is_err()
    );

    assert!(unitary_to_tableau(&[vec![c(1.0, 0.0)], vec![c(0.0, 0.0)]], false).is_err());
}

#[test]
fn unitary_to_tableau_snaps_near_clifford_phases_like_stim() {
    // Stim v1.16.0 smooths stabilizer-state ratios within squared distance 0.125 of 0, +/-1,
    // and +/-i. This near-identity phase is therefore accepted as the identity tableau.
    let theta = 0.01_f32;
    let near_identity_phase = matrix(&[
        &[(1.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (theta.cos(), theta.sin())],
    ]);
    assert_eq!(
        unitary_to_tableau(&near_identity_phase, true).expect("near identity phase"),
        Tableau::identity(1).expect("Tableau identity")
    );

    let theta = 0.5_f32;
    let non_clifford_phase = matrix(&[
        &[(1.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (theta.cos(), theta.sin())],
    ]);
    assert!(unitary_to_tableau(&non_clifford_phase, true).is_err());
}

#[derive(Clone, Copy)]
enum GateOutputs {
    One([&'static str; 2]),
    Two([&'static str; 4]),
}

struct GateCase {
    name: &'static str,
    matrix: Vec<Vec<Complex32>>,
    outputs: GateOutputs,
}

impl GateCase {
    fn expected_tableau(&self) -> Tableau {
        match self.outputs {
            GateOutputs::One([x, z]) => Tableau::gate1(x, z).expect("gate1 tableau"),
            GateOutputs::Two([x1, z1, x2, z2]) => {
                Tableau::gate2(x1, z1, x2, z2).expect("gate2 tableau")
            }
        }
    }

    fn expected_gate_unitary_matrix(&self) -> GateUnitaryMatrix {
        match self.outputs {
            GateOutputs::One(_) => GateUnitaryMatrix::One([
                [self.matrix[0][0], self.matrix[0][1]],
                [self.matrix[1][0], self.matrix[1][1]],
            ]),
            GateOutputs::Two(_) => GateUnitaryMatrix::Two([
                [
                    self.matrix[0][0],
                    self.matrix[0][1],
                    self.matrix[0][2],
                    self.matrix[0][3],
                ],
                [
                    self.matrix[1][0],
                    self.matrix[1][1],
                    self.matrix[1][2],
                    self.matrix[1][3],
                ],
                [
                    self.matrix[2][0],
                    self.matrix[2][1],
                    self.matrix[2][2],
                    self.matrix[2][3],
                ],
                [
                    self.matrix[3][0],
                    self.matrix[3][1],
                    self.matrix[3][2],
                    self.matrix[3][3],
                ],
            ]),
        }
    }
}

fn known_unitary_gate_cases() -> Vec<GateCase> {
    let h = f32::sqrt(0.5);
    vec![
        gate1_case(
            "I",
            [[(1.0, 0.0), (0.0, 0.0)], [(0.0, 0.0), (1.0, 0.0)]],
            ["+X", "+Z"],
        ),
        gate1_case(
            "X",
            [[(0.0, 0.0), (1.0, 0.0)], [(1.0, 0.0), (0.0, 0.0)]],
            ["+X", "-Z"],
        ),
        gate1_case(
            "Y",
            [[(0.0, 0.0), (0.0, -1.0)], [(0.0, 1.0), (0.0, 0.0)]],
            ["-X", "-Z"],
        ),
        gate1_case(
            "Z",
            [[(1.0, 0.0), (0.0, 0.0)], [(0.0, 0.0), (-1.0, 0.0)]],
            ["-X", "+Z"],
        ),
        gate1_case(
            "H",
            [[(h, 0.0), (h, 0.0)], [(h, 0.0), (-h, 0.0)]],
            ["+Z", "+X"],
        ),
        gate1_case(
            "H_XY",
            [[(0.0, 0.0), (h, -h)], [(h, h), (0.0, 0.0)]],
            ["+Y", "-Z"],
        ),
        gate1_case(
            "H_YZ",
            [[(h, 0.0), (0.0, -h)], [(0.0, h), (-h, 0.0)]],
            ["-X", "+Y"],
        ),
        gate1_case(
            "H_NXY",
            [[(0.0, 0.0), (h, h)], [(h, -h), (0.0, 0.0)]],
            ["-Y", "-Z"],
        ),
        gate1_case(
            "H_NXZ",
            [[(-h, 0.0), (h, 0.0)], [(h, 0.0), (h, 0.0)]],
            ["-Z", "-X"],
        ),
        gate1_case(
            "H_NYZ",
            [[(-h, 0.0), (0.0, -h)], [(0.0, h), (h, 0.0)]],
            ["-X", "-Y"],
        ),
        gate1_case(
            "SQRT_X",
            [[(0.5, 0.5), (0.5, -0.5)], [(0.5, -0.5), (0.5, 0.5)]],
            ["+X", "-Y"],
        ),
        gate1_case(
            "SQRT_X_DAG",
            [[(0.5, -0.5), (0.5, 0.5)], [(0.5, 0.5), (0.5, -0.5)]],
            ["+X", "+Y"],
        ),
        gate1_case(
            "SQRT_Y",
            [[(0.5, 0.5), (-0.5, -0.5)], [(0.5, 0.5), (0.5, 0.5)]],
            ["-Z", "+X"],
        ),
        gate1_case(
            "SQRT_Y_DAG",
            [[(0.5, -0.5), (0.5, -0.5)], [(-0.5, 0.5), (0.5, -0.5)]],
            ["+Z", "-X"],
        ),
        gate1_case(
            "S",
            [[(1.0, 0.0), (0.0, 0.0)], [(0.0, 0.0), (0.0, 1.0)]],
            ["+Y", "+Z"],
        ),
        gate1_case(
            "S_DAG",
            [[(1.0, 0.0), (0.0, 0.0)], [(0.0, 0.0), (0.0, -1.0)]],
            ["-Y", "+Z"],
        ),
        gate1_case(
            "C_XYZ",
            [[(0.5, -0.5), (-0.5, -0.5)], [(0.5, -0.5), (0.5, 0.5)]],
            ["+Y", "+X"],
        ),
        gate1_case(
            "C_NXYZ",
            [[(0.5, 0.5), (0.5, -0.5)], [(-0.5, -0.5), (0.5, -0.5)]],
            ["-Y", "-X"],
        ),
        gate1_case(
            "C_XNYZ",
            [[(0.5, 0.5), (-0.5, 0.5)], [(0.5, 0.5), (0.5, -0.5)]],
            ["-Y", "+X"],
        ),
        gate1_case(
            "C_XYNZ",
            [[(0.5, -0.5), (0.5, 0.5)], [(-0.5, 0.5), (0.5, 0.5)]],
            ["+Y", "-X"],
        ),
        gate1_case(
            "C_ZYX",
            [[(0.5, 0.5), (0.5, 0.5)], [(-0.5, 0.5), (0.5, -0.5)]],
            ["+Z", "+Y"],
        ),
        gate1_case(
            "C_ZYNX",
            [[(0.5, -0.5), (-0.5, 0.5)], [(0.5, 0.5), (0.5, 0.5)]],
            ["-Z", "+Y"],
        ),
        gate1_case(
            "C_ZNYX",
            [[(0.5, -0.5), (0.5, -0.5)], [(-0.5, -0.5), (0.5, 0.5)]],
            ["+Z", "-Y"],
        ),
        gate1_case(
            "C_NZYX",
            [[(0.5, 0.5), (-0.5, -0.5)], [(0.5, -0.5), (0.5, -0.5)]],
            ["-Z", "-Y"],
        ),
        gate2_case(
            "II",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
            ],
            ["+XI", "+ZI", "+IX", "+IZ"],
        ),
        gate2_case(
            "SQRT_XX",
            [
                [(0.5, 0.5), (0.0, 0.0), (0.0, 0.0), (0.5, -0.5)],
                [(0.0, 0.0), (0.5, 0.5), (0.5, -0.5), (0.0, 0.0)],
                [(0.0, 0.0), (0.5, -0.5), (0.5, 0.5), (0.0, 0.0)],
                [(0.5, -0.5), (0.0, 0.0), (0.0, 0.0), (0.5, 0.5)],
            ],
            ["+XI", "-YX", "+IX", "-XY"],
        ),
        gate2_case(
            "SQRT_XX_DAG",
            [
                [(0.5, -0.5), (0.0, 0.0), (0.0, 0.0), (0.5, 0.5)],
                [(0.0, 0.0), (0.5, -0.5), (0.5, 0.5), (0.0, 0.0)],
                [(0.0, 0.0), (0.5, 0.5), (0.5, -0.5), (0.0, 0.0)],
                [(0.5, 0.5), (0.0, 0.0), (0.0, 0.0), (0.5, -0.5)],
            ],
            ["+XI", "+YX", "+IX", "+XY"],
        ),
        gate2_case(
            "SQRT_YY",
            [
                [(0.5, 0.5), (0.0, 0.0), (0.0, 0.0), (-0.5, 0.5)],
                [(0.0, 0.0), (0.5, 0.5), (0.5, -0.5), (0.0, 0.0)],
                [(0.0, 0.0), (0.5, -0.5), (0.5, 0.5), (0.0, 0.0)],
                [(-0.5, 0.5), (0.0, 0.0), (0.0, 0.0), (0.5, 0.5)],
            ],
            ["-ZY", "+XY", "-YZ", "+YX"],
        ),
        gate2_case(
            "SQRT_YY_DAG",
            [
                [(0.5, -0.5), (0.0, 0.0), (0.0, 0.0), (-0.5, -0.5)],
                [(0.0, 0.0), (0.5, -0.5), (0.5, 0.5), (0.0, 0.0)],
                [(0.0, 0.0), (0.5, 0.5), (0.5, -0.5), (0.0, 0.0)],
                [(-0.5, -0.5), (0.0, 0.0), (0.0, 0.0), (0.5, -0.5)],
            ],
            ["+ZY", "-XY", "+YZ", "-YX"],
        ),
        gate2_case(
            "SQRT_ZZ",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
            ],
            ["+YZ", "+ZI", "+ZY", "+IZ"],
        ),
        gate2_case(
            "SQRT_ZZ_DAG",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, -1.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, -1.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
            ],
            ["-YZ", "+ZI", "-ZY", "+IZ"],
        ),
        gate2_case("SWAP", swap_unitary(), ["+IX", "+IZ", "+XI", "+ZI"]),
        gate2_case(
            "ISWAP",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
            ],
            ["+ZY", "+IZ", "+YZ", "+ZI"],
        ),
        gate2_case(
            "ISWAP_DAG",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, -1.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, -1.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
            ],
            ["-ZY", "+IZ", "-YZ", "+ZI"],
        ),
        gate2_case(
            "CXSWAP",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
                [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
            ],
            ["+XX", "+IZ", "+XI", "+ZZ"],
        ),
        gate2_case(
            "SWAPCX",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
                [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
            ],
            ["+IX", "+ZZ", "+XX", "+ZI"],
        ),
        gate2_case("CZSWAP", czswap_unitary(), ["+ZX", "+IZ", "+XZ", "+ZI"]),
        gate2_case(
            "XCX",
            [
                [(0.5, 0.0), (0.5, 0.0), (0.5, 0.0), (-0.5, 0.0)],
                [(0.5, 0.0), (0.5, 0.0), (-0.5, 0.0), (0.5, 0.0)],
                [(0.5, 0.0), (-0.5, 0.0), (0.5, 0.0), (0.5, 0.0)],
                [(-0.5, 0.0), (0.5, 0.0), (0.5, 0.0), (0.5, 0.0)],
            ],
            ["+XI", "+ZX", "+IX", "+XZ"],
        ),
        gate2_case("XCY", xcy_unitary(), ["+XI", "+ZY", "+XX", "+XZ"]),
        gate2_case("XCZ", xcz_unitary(), ["+XI", "+ZZ", "+XX", "+IZ"]),
        gate2_case("YCX", ycx_unitary(), ["+XX", "+ZX", "+IX", "+YZ"]),
        gate2_case(
            "YCY",
            [
                [(0.5, 0.0), (0.0, -0.5), (0.0, -0.5), (0.5, 0.0)],
                [(0.0, 0.5), (0.5, 0.0), (-0.5, 0.0), (0.0, -0.5)],
                [(0.0, 0.5), (-0.5, 0.0), (0.5, 0.0), (0.0, -0.5)],
                [(0.5, 0.0), (0.0, 0.5), (0.0, 0.5), (0.5, 0.0)],
            ],
            ["+XY", "+ZY", "+YX", "+YZ"],
        ),
        gate2_case(
            "YCZ",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, -1.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
            ],
            ["+XZ", "+ZZ", "+YX", "+IZ"],
        ),
        gate2_case("CX", zcx_unitary(), ["+XX", "+ZI", "+IX", "+ZZ"]),
        gate2_case(
            "CY",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, -1.0)],
                [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.0, 0.0)],
            ],
            ["+XY", "+ZI", "+ZX", "+ZZ"],
        ),
        gate2_case(
            "CZ",
            [
                [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
                [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (-1.0, 0.0)],
            ],
            ["+XZ", "+ZI", "+ZX", "+IZ"],
        ),
    ]
}

fn gate1_case(
    name: &'static str,
    rows: [[(f32, f32); 2]; 2],
    outputs: [&'static str; 2],
) -> GateCase {
    GateCase {
        name,
        matrix: matrix_from_rows(rows),
        outputs: GateOutputs::One(outputs),
    }
}

fn gate2_case(
    name: &'static str,
    rows: [[(f32, f32); 4]; 4],
    outputs: [&'static str; 4],
) -> GateCase {
    GateCase {
        name,
        matrix: matrix_from_rows(rows),
        outputs: GateOutputs::Two(outputs),
    }
}

fn matrix_from_rows<const N: usize>(rows: [[(f32, f32); N]; N]) -> Vec<Vec<Complex32>> {
    rows.into_iter()
        .map(|row| row.into_iter().map(|(real, imag)| c(real, imag)).collect())
        .collect()
}

fn swap_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
    ]
}

fn czswap_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (-1.0, 0.0)],
    ]
}

fn xcz_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
    ]
}

fn zcx_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        [(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (0.0, 0.0)],
        [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
    ]
}

fn xcy_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(0.5, 0.0), (0.5, 0.0), (0.0, -0.5), (0.0, 0.5)],
        [(0.5, 0.0), (0.5, 0.0), (0.0, 0.5), (0.0, -0.5)],
        [(0.0, 0.5), (0.0, -0.5), (0.5, 0.0), (0.5, 0.0)],
        [(0.0, -0.5), (0.0, 0.5), (0.5, 0.0), (0.5, 0.0)],
    ]
}

fn ycx_unitary() -> [[(f32, f32); 4]; 4] {
    [
        [(0.5, 0.0), (0.0, -0.5), (0.5, 0.0), (0.0, 0.5)],
        [(0.0, 0.5), (0.5, 0.0), (0.0, -0.5), (0.5, 0.0)],
        [(0.5, 0.0), (0.0, 0.5), (0.5, 0.0), (0.0, -0.5)],
        [(0.0, -0.5), (0.5, 0.0), (0.0, 0.5), (0.5, 0.0)],
    ]
}

fn xcz_tableau() -> Tableau {
    Tableau::gate2("+X_", "+ZZ", "+XX", "+_Z").expect("XCZ tableau")
}

fn zcx_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("ZCX tableau")
}

fn xcy_tableau() -> Tableau {
    Tableau::gate2("+X_", "+ZY", "+XX", "+XZ").expect("XCY tableau")
}

fn ycx_tableau() -> Tableau {
    Tableau::gate2("+XX", "+ZX", "+_X", "+YZ").expect("YCX tableau")
}

fn matrix(rows: &[&[(f32, f32)]]) -> Vec<Vec<Complex32>> {
    rows.iter()
        .map(|row| row.iter().map(|(real, imag)| c(*real, *imag)).collect())
        .collect()
}

fn c(real: f32, imag: f32) -> Complex32 {
    Complex32::new(real, imag)
}
