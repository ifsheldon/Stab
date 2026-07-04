#![allow(
    clippy::expect_used,
    reason = "PF1 DEM API compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeMap;
use std::ops::Bound;

use stab_core::{
    CircuitResult, DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock,
    DemTarget, DetectorErrorModel, Probability, RepeatCount,
};

#[test]
fn pf1_dem_basic_mutation_append_and_tag_stripping() {
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
fn pf1_dem_basic_programmatic_constructors_push_and_clone() {
    let mut body = DetectorErrorModel::new();
    body.push_instruction(
        DemInstruction::detector(
            vec![1.0, 2.0],
            DemTarget::relative_detector(0).expect("D0"),
            Some("inner".to_string()),
        )
        .expect("detector instruction"),
    );
    body.push_instruction(
        DemInstruction::shift_detectors(vec![3.0], 2, Some("step".to_string()))
            .expect("shift instruction"),
    );

    let mut dem = DetectorErrorModel::new();
    dem.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.125).expect("probability"),
            vec![
                DemTarget::relative_detector(0).expect("D0"),
                DemTarget::separator(),
                DemTarget::relative_detector(1).expect("D1"),
                DemTarget::logical_observable(2).expect("L2"),
            ],
            Some("err".to_string()),
        )
        .expect("error instruction"),
    );
    dem.push_repeat_block(DemRepeatBlock::new(
        RepeatCount::try_new(2).expect("repeat count"),
        body,
        Some("loop".to_string()),
    ));
    dem.push_instruction(
        DemInstruction::logical_observable(
            DemTarget::logical_observable(4).expect("L4"),
            Some("obs".to_string()),
        )
        .expect("logical observable"),
    );

    assert_eq!(
        dem.to_dem_string(),
        concat!(
            "error[err](0.125) D0 ^ D1 L2\n",
            "repeat[loop] 2 {\n",
            "    detector[inner](1, 2) D0\n",
            "    shift_detectors[step](3) 2\n",
            "}\n",
            "logical_observable[obs] L4\n",
        )
    );
    assert_eq!(dem.clone(), dem);
    assert_eq!(dem.len(), 3);
    assert!(
        dem.items()
            .get(1)
            .and_then(DemItem::as_repeat_block)
            .is_some()
    );
    assert_eq!(dem.count_detectors().expect("detectors"), 3);
    assert_eq!(dem.count_observables().expect("observables"), 5);
}

#[test]
fn pf1_dem_basic_append_from_text_is_atomic_on_parse_error() {
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
fn pf1_dem_basic_rejects_multi_target_detector_and_logical_observable() {
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
fn pf4_dem_introspection_transform_queries_cover_without_tags_and_final_counts() {
    let dem = DetectorErrorModel::from_dem_str(
        "error[first](0.125) D0 ^ D1 L2\n\
         repeat[outer] 3 {\n\
             detector[inside](5, 6) D0\n\
             logical_observable[log] L4\n\
             shift_detectors[step](1, 2) 2\n\
         }\n\
         shift_detectors[tail](10, 20, 30) 5\n",
    )
    .expect("parse DEM");

    let stripped = dem.without_tags();
    assert_eq!(
        stripped.to_dem_string(),
        concat!(
            "error(0.125) D0 ^ D1 L2\n",
            "repeat 3 {\n",
            "    detector(5, 6) D0\n",
            "    logical_observable L4\n",
            "    shift_detectors(1, 2) 2\n",
            "}\n",
            "shift_detectors(10, 20, 30) 5\n",
        )
    );
    assert!(
        dem.to_dem_string().contains("error[first]"),
        "without_tags must leave the source model tagged"
    );

    assert_eq!(dem.count_errors().expect("error count"), 1);
    assert_eq!(dem.count_detectors().expect("detector count"), 5);
    assert_eq!(dem.count_observables().expect("observable count"), 5);
    assert_eq!(dem.total_detector_shift().expect("detector shift"), 11);
    assert_eq!(
        dem.final_coordinate_shift()
            .expect("final coordinate shift"),
        vec![13.0, 26.0, 30.0]
    );
    assert_eq!(
        dem.detector_coordinates_for([
            DemDetectorId::try_new(0).expect("D0"),
            DemDetectorId::try_new(2).expect("D2"),
            DemDetectorId::try_new(4).expect("D4"),
        ])
        .expect("selected detector coordinates"),
        BTreeMap::from([
            (DemDetectorId::try_new(0).expect("D0"), vec![5.0, 6.0]),
            (DemDetectorId::try_new(2).expect("D2"), vec![6.0, 8.0]),
            (DemDetectorId::try_new(4).expect("D4"), vec![7.0, 10.0]),
        ])
    );
}

#[test]
fn pf4_dem_public_validation_rejects_malformed_inputs() {
    for (text, expected) in [
        ("error(1.5) D0\n", "probability"),
        ("error(0.25) ^ D0\n", "separators cannot be first"),
        ("error(0.25) D0 ^\n", "separators cannot be last"),
        (
            "error(0.25) D0 ^ ^ D1\n",
            "separators cannot be first or consecutive",
        ),
        ("error(0.25) 5\n", "raw numbers"),
        ("detector L0\n", "detector received invalid target"),
        (
            "logical_observable D0\n",
            "logical_observable received invalid target",
        ),
        (
            "shift_detectors D0\n",
            "shift_detectors requires exactly one numeric target",
        ),
        ("repeat nope {\n}\n", "invalid repeat count"),
        ("error[tag\n](0.25) D0\n", "unterminated tag"),
    ] {
        let error = DetectorErrorModel::from_dem_str(text).expect_err("reject malformed DEM");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} for {text:?}, got {error}"
        );
    }

    let programmatic_error = DemInstruction::new(
        DemInstructionKind::Error,
        vec![0.25],
        vec![
            DemTarget::separator(),
            DemTarget::relative_detector(0).expect("D0"),
        ],
        None,
    )
    .expect_err("reject programmatic leading separator");
    assert!(
        programmatic_error
            .to_string()
            .contains("separators cannot be first"),
        "{programmatic_error}"
    );

    let programmatic_detector = DemInstruction::new(
        DemInstructionKind::Detector,
        vec![f64::INFINITY],
        vec![DemTarget::relative_detector(0).expect("D0")],
        None,
    )
    .expect_err("reject non-finite detector coordinates");
    assert!(
        programmatic_detector.to_string().contains("not finite"),
        "{programmatic_detector}"
    );
}

#[test]
fn pf4_dem_public_validation_rejects_high_ids_and_unsupported_ranges() {
    let detector_error =
        DemTarget::relative_detector(1_u64 << 62).expect_err("reject high detector id");
    assert!(
        detector_error.to_string().contains("detector id"),
        "{detector_error}"
    );

    let observable_error =
        DemTarget::logical_observable(u64::from(u32::MAX) + 1).expect_err("reject high observable");
    assert!(
        observable_error.to_string().contains("observable id"),
        "{observable_error}"
    );

    let shift_overflow = DetectorErrorModel::from_dem_str(
        "shift_detectors 18446744073709551615\nshift_detectors 1\n",
    )
    .expect("parse high shifts");
    let error = shift_overflow
        .total_detector_shift()
        .expect_err("reject shift overflow")
        .to_string();
    assert!(error.contains("detector shift overflowed"), "{error}");

    let repeated =
        DetectorErrorModel::from_dem_str("error(0.125) D0\nrepeat 2 {\n    detector D0\n}\n")
            .expect("parse repeat");
    let error = repeated
        .instruction_range(..)
        .err()
        .map(|error| error.to_string());
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.contains("repeat block")),
        "{error:?}"
    );
}

#[test]
fn pf1_dem_counts_final_coordinate_shift_folds_nested_repeats() {
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
fn pf1_dem_counts_final_coordinate_shift_rejects_non_finite_folded_shift() {
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
fn pf1_dem_counts_errors_and_coordinates_through_repeats() {
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
fn pf4_dem_coordinates_reject_huge_all_map_but_allow_selected_queries() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000001 {\n\
             detector(1, 2) D0\n\
             shift_detectors(3, 4) 1\n\
         }\n",
    )
    .expect("parse huge coordinate DEM");

    let error = dem
        .detector_coordinates()
        .expect_err("reject huge all-detector coordinate map");

    assert!(
        error
            .to_string()
            .contains("detector_coordinates currently supports at most 1000000"),
        "{error}"
    );
    assert_eq!(
        dem.detector_coordinates_for([
            DemDetectorId::try_new(0).expect("D0"),
            DemDetectorId::try_new(1).expect("D1"),
        ])
        .expect("selected huge-repeat coordinates"),
        BTreeMap::from([
            (DemDetectorId::try_new(0).expect("D0"), vec![1.0, 2.0]),
            (DemDetectorId::try_new(1).expect("D1"), vec![4.0, 6.0]),
        ])
    );
    assert_eq!(
        dem.coordinates_of_detector(DemDetectorId::try_new(1).expect("D1"))
            .expect("single detector coordinates"),
        vec![4.0, 6.0]
    );
}

#[test]
fn pf4_dem_coordinates_fold_late_selected_detector_lookup() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000000000 {\n\
             detector(1, 2) D0\n\
             shift_detectors(3, 4) 1\n\
         }\n",
    )
    .expect("parse huge coordinate DEM");
    let late_detector = DemDetectorId::try_new(999_999_999).expect("late detector id");

    assert_eq!(
        dem.detector_coordinates_for([DemDetectorId::try_new(0).expect("D0"), late_detector,])
            .expect("fold selected detector lookup through huge repeat"),
        BTreeMap::from([
            (DemDetectorId::try_new(0).expect("D0"), vec![1.0, 2.0]),
            (late_detector, vec![2_999_999_998.0, 3_999_999_998.0]),
        ])
    );
}

#[test]
fn pf4_dem_coordinates_preserve_first_overlapping_repeat_declaration() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 10 {\n\
             detector(100) D2\n\
             detector(0) D0\n\
             shift_detectors(1) 1\n\
         }\n",
    )
    .expect("parse overlapping coordinate DEM");
    let detector = DemDetectorId::try_new(9).expect("D9");

    assert_eq!(
        dem.coordinates_of_detector(detector)
            .expect("fold selected overlapping detector lookup"),
        vec![107.0]
    );
}

#[test]
fn pf1_dem_iterators_item_ranges_and_flattened_iterator_are_typed() {
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

#[test]
fn pf4_dem_materialized_flattened_matches_pinned_stim_cases() {
    let empty = DetectorErrorModel::new();
    assert_eq!(empty.flattened().expect("flatten empty"), empty);

    let shifted = DetectorErrorModel::from_dem_str(
        "shift_detectors 5\n\
         error(0.125) D0 ^ D1 L0\n",
    )
    .expect("parse shifted DEM");
    assert_eq!(
        shifted
            .flattened()
            .expect("flatten shifted DEM")
            .to_dem_string(),
        "error(0.125) D5 ^ D6 L0\n",
    );

    let coordinates = DetectorErrorModel::from_dem_str(
        "detector(10, 20) D0\n\
         detector(10, 20, 30, 40) D1\n\
         logical_observable L0\n\
         shift_detectors(1, 2, 3) 5\n\
         detector(10, 20) D0\n\
         detector(10, 20, 30, 40) D1\n\
         logical_observable L1\n",
    )
    .expect("parse coordinate DEM");
    assert_eq!(
        coordinates
            .flattened()
            .expect("flatten coordinate DEM")
            .to_dem_string(),
        concat!(
            "detector(10, 20) D0\n",
            "detector(10, 20, 30, 40) D1\n",
            "logical_observable L0\n",
            "detector(11, 22) D5\n",
            "detector(11, 22, 33, 40) D6\n",
            "logical_observable L1\n",
        )
    );

    let repeated = DetectorErrorModel::from_dem_str(
        "repeat[drop-me] 5 {\n\
             error[tag](0.125) D0\n\
             shift_detectors(3) 2\n\
         }\n\
         detector(10, 20, 30, 40) D0\n",
    )
    .expect("parse repeated DEM");
    assert_eq!(
        repeated
            .flattened()
            .expect("flatten repeated DEM")
            .to_dem_string(),
        concat!(
            "error[tag](0.125) D0\n",
            "error[tag](0.125) D2\n",
            "error[tag](0.125) D4\n",
            "error[tag](0.125) D6\n",
            "error[tag](0.125) D8\n",
            "detector(25, 20, 30, 40) D10\n",
        )
    );
}

#[test]
fn pf4_dem_materialized_flattened_rejects_excessive_repeat() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n\
             error(0.125) D0\n\
         }\n",
    )
    .expect("parse large repeat DEM");

    let error = dem.flattened().expect_err("reject excessive flattening");

    assert!(
        error
            .to_string()
            .contains("DEM flattened currently supports repeat counts up to 100000"),
        "{error}"
    );
}

#[test]
fn pf4_dem_materialized_rounded_matches_pinned_stim_probability_cases() {
    let dem = DetectorErrorModel::from_dem_str(
        "error[first](0.01000002) D0 D1\n\
         repeat[outer] 2 {\n\
             error[inner](0.123456789) D1 D2 L3\n\
         }\n\
         detector(0.0200000334, 0.12345) D0\n\
         shift_detectors(5.0300004, 0.12345) 3\n",
    )
    .expect("parse DEM");

    assert_eq!(
        dem.rounded(0).expect("round 0"),
        DetectorErrorModel::from_dem_str(
            "error[first](0) D0 D1\n\
             repeat[outer] 2 {\n\
                 error[inner](0) D1 D2 L3\n\
             }\n\
             detector(0.0200000334, 0.12345) D0\n\
             shift_detectors(5.0300004, 0.12345) 3\n",
        )
        .expect("parse round 0 expected"),
    );
    assert_eq!(
        dem.rounded(2).expect("round 2"),
        DetectorErrorModel::from_dem_str(
            "error[first](0.01) D0 D1\n\
             repeat[outer] 2 {\n\
                 error[inner](0.12) D1 D2 L3\n\
             }\n\
             detector(0.0200000334, 0.12345) D0\n\
             shift_detectors(5.0300004, 0.12345) 3\n",
        )
        .expect("parse round 2 expected"),
    );
    assert_eq!(
        dem.rounded(3)
            .expect("round 3")
            .items()
            .iter()
            .filter_map(DemItem::as_instruction)
            .next()
            .expect("first instruction")
            .args(),
        &[0.01],
    );
}

#[test]
fn pf4_dem_materialized_rounded_keeps_zero_probability_errors() {
    let dem = DetectorErrorModel::from_dem_str("error(0.000001) D0 D1\n").expect("parse DEM");

    assert_eq!(
        dem.rounded(2).expect("round tiny error").to_dem_string(),
        "error(0) D0 D1\n",
    );
}
