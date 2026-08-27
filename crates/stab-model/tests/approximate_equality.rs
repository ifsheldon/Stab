#![allow(
    clippy::expect_used,
    reason = "semantic comparison tests use direct fixture assertions"
)]

use stab_model::{AbsoluteTolerance, Circuit, DetectorErrorModel};

fn tolerance(value: f64) -> AbsoluteTolerance {
    AbsoluteTolerance::try_new(value).expect("valid test tolerance")
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid test circuit")
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

#[test]
fn circuit_approximate_equality_matches_stim_structure_and_tolerance() {
    let base = circuit("H[tag] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\n");
    let cases = [
        (
            "identical",
            "H[tag] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\n",
            0.0,
            true,
        ),
        (
            "arguments within tolerance",
            "H[tag] 0\nX_ERROR(0.021) 0\nQUBIT_COORDS(0.081, 0.06) 0\n",
            0.01,
            true,
        ),
        (
            "probability outside tolerance",
            "H[tag] 0\nX_ERROR(0.021) 0\nQUBIT_COORDS(0.08, 0.06) 0\n",
            0.0001,
            false,
        ),
        (
            "coordinate outside tolerance",
            "H[tag] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.081, 0.06) 0\n",
            0.0001,
            false,
        ),
        (
            "different gate",
            "H[tag] 0\nDEPOLARIZE1(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\n",
            999.0,
            false,
        ),
        (
            "different target",
            "H[tag] 1\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\n",
            999.0,
            false,
        ),
        (
            "different tag",
            "H[other] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\n",
            999.0,
            false,
        ),
        (
            "different argument count",
            "H[tag] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08) 0\n",
            999.0,
            false,
        ),
        (
            "different order",
            "H[tag] 0\nQUBIT_COORDS(0.08, 0.06) 0\nX_ERROR(0.02) 0\n",
            999.0,
            false,
        ),
        (
            "different length",
            "H[tag] 0\nX_ERROR(0.02) 0\nQUBIT_COORDS(0.08, 0.06) 0\nTICK\n",
            999.0,
            false,
        ),
    ];
    for (label, other, absolute_tolerance, expected) in cases {
        assert_eq!(
            base.approx_equals(&circuit(other), tolerance(absolute_tolerance)),
            expected,
            "{label}"
        );
    }

    let repeated = circuit("REPEAT[loop] 2 {\nX_ERROR(0.1) 0\n}\n");
    assert!(repeated.approx_equals(
        &circuit("REPEAT[loop] 2 {\nX_ERROR(0.101) 0\n}\n"),
        tolerance(0.01)
    ));
    for other in [
        "REPEAT[loop] 3 {\nX_ERROR(0.1) 0\n}\n",
        "REPEAT[loop] 2 {\nX_ERROR(0.1) 1\n}\n",
        "X_ERROR(0.1) 0\n",
    ] {
        assert!(!repeated.approx_equals(&circuit(other), tolerance(999.0)));
    }

    assert!(
        circuit("X_ERROR(0.25) 0\n").approx_equals(&circuit("X_ERROR(0.5) 0\n"), tolerance(0.25))
    );
    assert!(
        circuit("M 0\nOBSERVABLE_INCLUDE(1) rec[-1]\n").approx_equals(
            &circuit("M 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n"),
            tolerance(1.0)
        )
    );
    assert!(circuit("cnot 0 1\n").approx_equals(&circuit("CX 0 1\n"), tolerance(0.0)));
    assert!(!circuit("X_ERROR(0.1) 0\nX_ERROR(0.1) 1\n").approx_equals(
        &circuit("X_ERROR(0.1) 0\nX_ERROR(0.101) 1\n"),
        tolerance(0.01)
    ));
}

#[test]
fn circuit_equality_compares_repeat_tags_despite_pinned_stim_bug() {
    let left = circuit("REPEAT[left] 2 {\nX_ERROR(0.1) 0\n}\n");
    let right = circuit("REPEAT[right] 2 {\nX_ERROR(0.1) 0\n}\n");

    assert_ne!(left, right);
    assert!(!left.approx_equals(&right, tolerance(999.0)));
}

#[test]
fn circuit_equality_discards_popped_repeat_storage() {
    let mut popped = circuit("REPEAT 2 {\nX_ERROR(0.1) 0\n}\n");
    popped.pop_last_item().expect("repeat item exists");
    let empty = Circuit::new();

    assert_eq!(popped, empty);
    assert!(popped.approx_equals(&empty, tolerance(0.0)));
}

#[test]
fn dem_approximate_equality_matches_stim_structure_and_tolerance() {
    let base = dem("error[tag](0.099) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n");
    let cases = [
        (
            "identical",
            "error[tag](0.099) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n",
            0.0,
            true,
        ),
        (
            "arguments within tolerance",
            "error[tag](0.101) D0 D1\ndetector(1.001, 2) D2\nshift_detectors(3, 4.001) 5\n",
            0.01,
            true,
        ),
        (
            "probability outside tolerance",
            "error[tag](0.101) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n",
            0.0001,
            false,
        ),
        (
            "coordinate outside tolerance",
            "error[tag](0.099) D0 D1\ndetector(1.001, 2) D2\nshift_detectors(3, 4) 5\n",
            0.0001,
            false,
        ),
        (
            "different target",
            "error[tag](0.099) D0 D2\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different separator",
            "error[tag](0.099) D0 ^ D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different tag",
            "error[other](0.099) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different argument count",
            "error[tag](0.099) D0 D1\ndetector(1) D2\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different instruction kind",
            "error[tag](0.099) D0 D1\nshift_detectors(1, 2) 2\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different detector shift",
            "error[tag](0.099) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 6\n",
            999.0,
            false,
        ),
        (
            "different order",
            "detector(1, 2) D2\nerror[tag](0.099) D0 D1\nshift_detectors(3, 4) 5\n",
            999.0,
            false,
        ),
        (
            "different length",
            "error[tag](0.099) D0 D1\ndetector(1, 2) D2\nshift_detectors(3, 4) 5\nlogical_observable L0\n",
            999.0,
            false,
        ),
    ];
    for (label, other, absolute_tolerance, expected) in cases {
        assert_eq!(
            base.approx_equals(&dem(other), tolerance(absolute_tolerance)),
            expected,
            "{label}"
        );
    }

    assert!(dem("error(0.25) D0\n").approx_equals(&dem("error(0.5) D0\n"), tolerance(0.25)));

    let repeated = dem("repeat[loop] 2 {\nerror(0.1) D0 ^ L0\n}\n");
    assert!(repeated.approx_equals(
        &dem("repeat[loop] 2 {\nerror(0.101) D0 ^ L0\n}\n"),
        tolerance(0.01)
    ));
    for other in [
        "repeat[loop] 3 {\nerror(0.1) D0 ^ L0\n}\n",
        "repeat[other] 2 {\nerror(0.1) D0 ^ L0\n}\n",
        "repeat[loop] 2 {\nerror(0.1) D1 ^ L0\n}\n",
        "error(0.1) D0 ^ L0\n",
    ] {
        assert!(!repeated.approx_equals(&dem(other), tolerance(999.0)));
    }
}

#[test]
fn absolute_tolerance_rejects_invalid_numeric_domains() {
    assert_eq!(
        AbsoluteTolerance::try_new(0.0).map(AbsoluteTolerance::get),
        Ok(0.0)
    );
    assert_eq!(
        AbsoluteTolerance::try_new(9999.0).map(AbsoluteTolerance::get),
        Ok(9999.0)
    );
    for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(AbsoluteTolerance::try_new(invalid).is_err(), "{invalid}");
    }
}
