#![allow(
    clippy::expect_used,
    reason = "PF1 DEM API compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeMap;
use std::ops::Bound;

use stab_core::{
    CircuitResult, DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemTarget,
    DetectorErrorModel,
};

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

#[test]
fn dem_counts_errors_and_coordinates_through_repeats() {
    let dem = DetectorErrorModel::from_dem_str(
        "logical_observable L100\n\
         detector D100\n\
         shift_detectors(100, 100, 100) 100\n\
         error(0.125) D100\n\
         repeat 100 {\n\
             repeat 5 {\n\
                 error(0.25) D1\n\
             }\n\
         }\n",
    )
    .expect("parse counting DEM");
    assert_eq!(dem.count_errors().expect("error count"), 501);

    let dem = DetectorErrorModel::from_dem_str(
        "error(0.25) D0 D1\n\
         detector(1, 2, 3) D1\n\
         shift_detectors(5) 1\n\
         detector(1, 2) D2\n",
    )
    .expect("parse coordinate DEM");

    let expected = BTreeMap::from([
        (DemDetectorId::try_new(0).expect("D0"), vec![]),
        (DemDetectorId::try_new(1).expect("D1"), vec![1.0, 2.0, 3.0]),
        (DemDetectorId::try_new(2).expect("D2"), vec![]),
        (DemDetectorId::try_new(3).expect("D3"), vec![6.0, 2.0]),
    ]);
    assert_eq!(
        dem.detector_coordinates().expect("all coordinates"),
        expected
    );
    assert_eq!(
        dem.detector_coordinates_for([
            DemDetectorId::try_new(1).expect("D1"),
            DemDetectorId::try_new(3).expect("D3"),
        ])
        .expect("selected coordinates"),
        BTreeMap::from([
            (DemDetectorId::try_new(1).expect("D1"), vec![1.0, 2.0, 3.0]),
            (DemDetectorId::try_new(3).expect("D3"), vec![6.0, 2.0]),
        ])
    );
    assert_eq!(
        dem.coordinates_of_detector(DemDetectorId::try_new(3).expect("D3"))
            .expect("single detector coordinates"),
        vec![6.0, 2.0]
    );

    let error = dem
        .coordinates_of_detector(DemDetectorId::try_new(4).expect("D4"))
        .expect_err("reject out-of-range detector id");
    assert!(error.to_string().contains("too big"), "{error}");
}

#[test]
fn dem_item_ranges_and_flattened_iterator_are_typed() {
    let dem = DetectorErrorModel::from_dem_str(
        "error(0.125) D0\n\
         repeat[tag] 2 {\n\
             shift_detectors(3) 2\n\
             detector(1, 2) D0\n\
             error(0.25) D0 L0\n\
         }\n\
         logical_observable L0\n",
    )
    .expect("parse flattened DEM");

    assert_eq!(
        dem.iter_items()
            .map(|item| match item {
                DemItem::Instruction(instruction) => format!("{:?}", instruction.kind()),
                DemItem::RepeatBlock(_) => "RepeatBlock".to_string(),
            })
            .collect::<Vec<_>>(),
        vec!["Error", "RepeatBlock", "LogicalObservable"]
    );
    assert!(
        dem.items()
            .get(1)
            .and_then(DemItem::as_repeat_block)
            .is_some()
    );
    assert!(
        dem.items()
            .first()
            .and_then(DemItem::as_instruction)
            .is_some()
    );
    assert_eq!(dem.item_range(1..).expect("item range").count(), 2);
    assert_eq!(
        dem.instruction_range(0..1)
            .expect("instruction range")
            .map(DemInstruction::kind)
            .collect::<Vec<_>>(),
        vec![DemInstructionKind::Error]
    );

    let repeat_error = dem
        .instruction_range(0..2)
        .err()
        .expect("repeat blocks are not instruction-only items");
    assert!(
        repeat_error.to_string().contains("DEM instruction range")
            && repeat_error.to_string().contains("repeat block"),
        "{repeat_error}"
    );

    let range_error = dem
        .item_range((Bound::Excluded(usize::MAX), Bound::Unbounded))
        .err()
        .expect("reject overflowing range bound");
    assert!(
        range_error
            .to_string()
            .contains("excluded start index overflowed"),
        "{range_error}"
    );

    let flattened = dem
        .iter_flattened_instructions()
        .collect::<CircuitResult<Vec<_>>>()
        .expect("flatten instructions");
    let mut flattened_dem = DetectorErrorModel::new();
    for instruction in flattened {
        flattened_dem.push_instruction(instruction);
    }
    assert_eq!(
        flattened_dem.to_dem_string(),
        "error(0.125) D0\n\
         detector(4, 2) D2\n\
         error(0.25) D2 L0\n\
         detector(7, 2) D4\n\
         error(0.25) D4 L0\n\
         logical_observable L0\n"
    );

    let huge_repeat =
        DetectorErrorModel::from_dem_str("repeat 1000000000000 {\n    error(0.1) D0\n}\n")
            .expect("parse huge repeat");
    let first_three = huge_repeat
        .iter_flattened_instructions()
        .take(3)
        .collect::<CircuitResult<Vec<_>>>()
        .expect("first flattened instructions");
    assert_eq!(first_three.len(), 3);
    assert!(first_three.iter().all(|instruction| {
        instruction.kind() == DemInstructionKind::Error
            && instruction.targets() == [DemTarget::relative_detector(0).expect("D0")]
    }));
}
