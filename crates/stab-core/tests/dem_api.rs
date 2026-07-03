#![allow(
    clippy::expect_used,
    reason = "PF1 DEM API compatibility tests use direct assertions for compact diagnostics"
)]

use stab_core::{DemInstruction, DemInstructionKind, DemTarget, DetectorErrorModel};

#[test]
fn dem_basic_mutation_append_and_tag_stripping() {
    let mut dem = DetectorErrorModel::new();
    assert!(dem.is_empty());
    assert_eq!(dem.len(), 0);

    dem.append_from_dem_text(
        "error[first](0.125) D0\n\
         repeat[outer] 2 {\n\
             detector[inner](1, 2) D0\n\
             shift_detectors[step](3) 1\n\
         }\n",
    )
    .expect("append DEM text");

    assert!(!dem.is_empty());
    assert_eq!(dem.len(), 2);
    let stripped = dem.without_tags();
    assert_eq!(
        stripped.to_dem_string(),
        concat!(
            "error(0.125) D0\n",
            "repeat 2 {\n",
            "    detector(1, 2) D0\n",
            "    shift_detectors(3) 1\n",
            "}\n",
        )
    );
    assert!(
        dem.to_dem_string().contains("error[first]"),
        "without_tags must not mutate the source model"
    );

    dem.clear();
    assert!(dem.is_empty());
    assert_eq!(dem.to_dem_string(), "");
}

#[test]
fn dem_append_from_text_is_atomic_on_parse_error() {
    let mut dem = DetectorErrorModel::from_dem_str("error(0.125) D0\n").expect("parse DEM");
    let before = dem.clone();

    let error = dem
        .append_from_dem_text("detector L0\n")
        .expect_err("reject invalid append");

    assert!(
        error.to_string().contains("detector"),
        "unexpected error: {error}"
    );
    assert_eq!(dem, before);
}

#[test]
fn dem_rejects_multi_target_detector_and_logical_observable() {
    for text in ["detector D0 D1\n", "logical_observable L0 L1\n"] {
        let error =
            DetectorErrorModel::from_dem_str(text).expect_err("reject invalid target count");
        assert!(
            error.to_string().contains("exactly one target"),
            "unexpected error for {text:?}: {error}"
        );
    }

    assert!(
        DemInstruction::new(
            DemInstructionKind::Detector,
            Vec::new(),
            vec![
                DemTarget::relative_detector(0).expect("D0"),
                DemTarget::relative_detector(1).expect("D1"),
            ],
            None,
        )
        .is_err()
    );
    assert!(
        DemInstruction::new(
            DemInstructionKind::LogicalObservable,
            Vec::new(),
            vec![
                DemTarget::logical_observable(0).expect("L0"),
                DemTarget::logical_observable(1).expect("L1"),
            ],
            None,
        )
        .is_err()
    );
}

#[test]
fn dem_final_coordinate_shift_folds_nested_repeats() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000 {\n\
             repeat 2000 {\n\
                 shift_detectors(0, 0, 1) 0\n\
             }\n\
             shift_detectors(1) 0\n\
         }\n\
         shift_detectors(0, 1) 0\n",
    )
    .expect("parse DEM");

    assert_eq!(
        dem.final_coordinate_shift()
            .expect("final coordinate shift"),
        vec![1000.0, 1.0, 2_000_000.0]
    );
}

#[test]
fn dem_final_coordinate_shift_rejects_non_finite_folded_shift() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n\
             shift_detectors(1e308) 0\n\
         }\n",
    )
    .expect("parse DEM");

    let error = dem
        .final_coordinate_shift()
        .expect_err("reject infinite coordinate shift");

    assert!(
        error.to_string().contains("coordinate shift overflowed"),
        "{error}"
    );
}
