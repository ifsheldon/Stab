#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "CQ2 compatibility tests use direct fixture assertions for precise failures"
)]

use std::collections::{BTreeMap, BTreeSet, HashSet};

use stab_core::{
    CircuitResult, DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemObservableId,
    DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel, Probability,
};

#[test]
fn cq2_dem_target_value_and_parse_contract_matches_stim() {
    let detector = DemDetectorId::try_new(5).expect("D5");
    let observable = DemObservableId::try_new(6).expect("L6");
    assert_eq!(detector.get(), 5);
    assert_eq!(observable.get(), 6);
    assert_eq!(detector, detector.clone());
    assert_eq!(observable, observable.clone());
    assert!(format!("{detector:?}").contains('5'));
    assert!(format!("{observable:?}").contains('6'));
    assert_eq!(
        BTreeSet::from([detector]).into_iter().next(),
        Some(detector)
    );
    assert_eq!(HashSet::from([detector, detector]).len(), 1);
    assert_eq!(HashSet::from([observable, observable]).len(), 1);

    let targets = [
        (DemTarget::relative_detector(5).expect("D5"), "D5"),
        (DemTarget::logical_observable(6).expect("L6"), "L6"),
        (DemTarget::separator(), "^"),
    ];
    for (target, text) in targets {
        assert_eq!(target.to_string(), text);
        assert_eq!(text.parse::<DemTarget>().expect("parse target"), target);
        assert_eq!(target, target.clone());
        assert!(!format!("{target:?}").is_empty());
    }
    assert_ne!(
        DemTarget::relative_detector(5).expect("D5"),
        DemTarget::logical_observable(5).expect("L5")
    );
    assert!(
        DemTarget::relative_detector(4).expect("D4") < DemTarget::relative_detector(5).expect("D5")
    );

    assert_eq!(
        format!("{}", DemTarget::numeric(10)),
        "10",
        "numeric targets are the explicit Rust representation used by shift_detectors"
    );
    assert!(
        "10".parse::<DemTarget>().is_err(),
        "Stim's standalone DemTarget text parser rejects bare numerics"
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str("shift_detectors 10\n")
            .expect("numeric target inside full DEM syntax")
            .to_dem_string(),
        "shift_detectors 10\n"
    );
    assert_eq!(
        "D1152921504606846975"
            .parse::<DemTarget>()
            .expect("maximum textual detector target"),
        DemTarget::relative_detector((1_u64 << 60) - 1).expect("maximum textual detector target")
    );

    assert!(DemDetectorId::try_new((1_u64 << 62) - 1).is_ok());
    assert!(DemDetectorId::try_new(1_u64 << 62).is_err());
    assert!(DemObservableId::try_new(u64::from(u32::MAX)).is_ok());
    assert!(DemObservableId::try_new(u64::from(u32::MAX) + 1).is_err());
    for rejected in [
        "",
        "5",
        "d5",
        "l6",
        "D-1",
        "L-1",
        "X5",
        "D1152921504606846976",
        "L1152921504606846976",
        "D4611686018427387904",
        "L4294967296",
    ] {
        assert!(rejected.parse::<DemTarget>().is_err(), "{rejected:?}");
    }
}

#[test]
fn cq2_dem_instruction_value_validation_and_print_contract_matches_stim() {
    let error = DemInstruction::error(
        Probability::try_new(0.125).expect("probability"),
        vec![
            DemTarget::relative_detector(3).expect("D3"),
            DemTarget::logical_observable(6).expect("L6"),
        ],
        Some("err".to_string()),
    )
    .expect("error instruction");
    let detector = DemInstruction::detector(
        vec![1.5, 2.5],
        DemTarget::relative_detector(5).expect("D5"),
        Some("det".to_string()),
    )
    .expect("detector instruction");
    let logical = DemInstruction::logical_observable(
        DemTarget::logical_observable(4).expect("L4"),
        Some("obs".to_string()),
    )
    .expect("logical-observable instruction");
    let shift = DemInstruction::shift_detectors(vec![3.5], 7, Some("shift".to_string()))
        .expect("shift instruction");

    assert_eq!(error.kind(), DemInstructionKind::Error);
    assert_eq!(error.args(), &[0.125]);
    assert_eq!(
        error.targets(),
        &[
            DemTarget::relative_detector(3).expect("D3"),
            DemTarget::logical_observable(6).expect("L6"),
        ]
    );
    assert_eq!(error.tag(), Some("err"));
    assert_eq!(error, error.clone());
    assert!(!format!("{error:?}").is_empty());

    let kinds = [
        DemInstructionKind::Error,
        DemInstructionKind::Detector,
        DemInstructionKind::LogicalObservable,
        DemInstructionKind::ShiftDetectors,
    ];
    assert_eq!(kinds[0], kinds[0].clone());
    assert_eq!(format!("{:?}", kinds[3]), "ShiftDetectors");

    let mut model = DetectorErrorModel::new();
    for instruction in [
        error.clone(),
        detector.clone(),
        logical.clone(),
        shift.clone(),
    ] {
        model.push_instruction(instruction);
    }
    assert_eq!(
        model.to_dem_string(),
        concat!(
            "error[err](0.125) D3 L6\n",
            "detector[det](1.5, 2.5) D5\n",
            "logical_observable[obs] L4\n",
            "shift_detectors[shift](3.5) 7\n",
        )
    );

    assert_eq!(
        DemInstruction::new(
            DemInstructionKind::ShiftDetectors,
            vec![1.0, 2.0],
            vec![DemTarget::numeric(3)],
            None,
        )
        .expect("generic constructor"),
        DemInstruction::shift_detectors(vec![1.0, 2.0], 3, None).expect("shift constructor")
    );

    let d0 = DemTarget::relative_detector(0).expect("D0");
    let l0 = DemTarget::logical_observable(0).expect("L0");
    for (kind, args, targets) in [
        (DemInstructionKind::Error, vec![], vec![d0]),
        (DemInstructionKind::Error, vec![0.25, 0.5], vec![d0]),
        (DemInstructionKind::Error, vec![-0.1], vec![d0]),
        (DemInstructionKind::Error, vec![1.1], vec![d0]),
        (
            DemInstructionKind::Error,
            vec![0.25],
            vec![DemTarget::separator()],
        ),
        (
            DemInstructionKind::Error,
            vec![0.25],
            vec![d0, DemTarget::separator()],
        ),
        (
            DemInstructionKind::Error,
            vec![0.25],
            vec![d0, DemTarget::separator(), DemTarget::separator(), l0],
        ),
        (
            DemInstructionKind::Error,
            vec![0.25],
            vec![DemTarget::numeric(3)],
        ),
        (DemInstructionKind::Detector, vec![], vec![l0]),
        (DemInstructionKind::Detector, vec![], vec![d0, d0]),
        (DemInstructionKind::Detector, vec![f64::INFINITY], vec![d0]),
        (DemInstructionKind::LogicalObservable, vec![1.0], vec![l0]),
        (DemInstructionKind::LogicalObservable, vec![], vec![d0]),
        (DemInstructionKind::LogicalObservable, vec![], vec![l0, l0]),
        (DemInstructionKind::ShiftDetectors, vec![], vec![d0]),
        (
            DemInstructionKind::ShiftDetectors,
            vec![f64::NAN],
            vec![DemTarget::numeric(1)],
        ),
    ] {
        assert!(
            DemInstruction::new(kind, args, targets, None).is_err(),
            "expected invalid {kind:?} instruction"
        );
    }
}

#[test]
fn cq2_dem_instruction_target_groups_match_stim() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.1) D0 ^ D2 L0 ^ D1 D2 D3\nerror(0.2) D4\nerror(0.3)\n",
    )
    .expect("target-group DEM");
    let instructions = model
        .items()
        .iter()
        .filter_map(DemItem::as_instruction)
        .collect::<Vec<_>>();
    let groups = instructions[0]
        .target_groups()
        .into_iter()
        .map(<[DemTarget]>::to_vec)
        .collect::<Vec<_>>();
    assert_eq!(
        groups,
        vec![
            vec![DemTarget::relative_detector(0).expect("D0")],
            vec![
                DemTarget::relative_detector(2).expect("D2"),
                DemTarget::logical_observable(0).expect("L0"),
            ],
            vec![
                DemTarget::relative_detector(1).expect("D1"),
                DemTarget::relative_detector(2).expect("D2"),
                DemTarget::relative_detector(3).expect("D3"),
            ],
        ]
    );
    assert_eq!(
        instructions[1].target_groups(),
        vec![&[DemTarget::relative_detector(4).expect("D4")][..]]
    );
    assert_eq!(instructions[2].target_groups(), vec![&[][..]]);
}

#[test]
fn cq2_dem_model_parse_print_tag_and_newline_contract_matches_stim() {
    let canonical = concat!(
        "error[first](0.125) D0\n",
        "repeat[outer] 2 {\n",
        "    error[test\\Ctag](0.25) D0 D1 L0 ^ D2\n",
        "    shift_detectors[step](1.5, 3) 10\n",
        "    detector[coords](0.5) D0\n",
        "}\n",
        "logical_observable[obs] L0\n",
    );
    let source = concat!(
        "# comment\r\n",
        "ERROR[first](.125) D0\r\n",
        "REPEAT[outer] 2 {\r\n",
        "error[test\\Ctag](.25) D0 D1 L0 ^ D2\r\n",
        "SHIFT_DETECTORS[step](1.5,3) 10\r\n",
        "DETECTOR[coords](.5) D0\r\n",
        "}\r\n",
        "LOGICAL_OBSERVABLE[obs] L0\r\n",
    );
    let model = DetectorErrorModel::from_dem_str(source).expect("parse canonical DEM corpus");
    assert_eq!(model.to_dem_string(), canonical);
    assert_eq!(model.to_string(), canonical);
    assert_eq!(
        DetectorErrorModel::from_dem_str(&model.to_dem_string()).expect("parse printed DEM"),
        model
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str("# empty\r\n").unwrap(),
        DetectorErrorModel::new()
    );

    let empty_arguments =
        DetectorErrorModel::from_dem_str("error() D0\ndetector() D1\nshift_detectors() 2\n")
            .expect("parse Stim-compatible empty argument tokens");
    assert_eq!(
        empty_arguments.to_dem_string(),
        "error(0) D0\ndetector(0) D1\nshift_detectors(0) 2\n"
    );

    let zero_repeat_source = concat!(
        "repeat 0 {\n",
        "    error(1) D9 L7\n",
        "    shift_detectors(2) 10\n",
        "    detector(3) D0\n",
        "    logical_observable L7\n",
        "}\n",
    );
    let zero_repeat = DetectorErrorModel::from_dem_str(zero_repeat_source)
        .expect("Stim accepts zero-count DEM repeats");
    assert_eq!(zero_repeat.to_dem_string(), zero_repeat_source);
    assert_eq!(zero_repeat.total_detector_shift(), Ok(0));
    assert_eq!(zero_repeat.count_detectors(), Ok(0));
    assert_eq!(zero_repeat.count_observables(), Ok(8));
    assert_eq!(zero_repeat.count_errors(), Ok(0));
    assert_eq!(zero_repeat.final_coordinate_shift(), Ok(vec![0.0]));
    assert_eq!(
        zero_repeat
            .flattened()
            .expect("flatten zero-count repeat")
            .to_dem_string(),
        ""
    );

    let max_text_integer = (1_u64 << 60) - 1;
    let maximum_text = format!(
        "shift_detectors {max_text_integer}\nerror(0.25) D{max_text_integer}\nrepeat {max_text_integer} {{\n}}\n"
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str(&maximum_text)
            .expect("parse Stim's maximum textual DEM integer")
            .to_dem_string(),
        maximum_text
    );

    for rejected in [
        "unknown D0\n",
        "error D0\n",
        "error(0.25) 5\n",
        "detector D0 D1\n",
        "logical_observable L0 L1\n",
        "shift_detectors D0\n",
        "repeat nope {\n}\n",
        "logical_observable() L0\n",
        "shift_detectors 1152921504606846976\n",
        "error(0.25) D1152921504606846976\n",
        "repeat 1152921504606846976 {\n}\n",
        "detector [tag] D0\n",
        "detector (1) D0\n",
        "error(0.25) D0\u{2003}D1\n",
        "\u{2003}error(0.25) D0\n",
        "error[tag\n](0.25) D0\n",
        "}\n",
    ] {
        assert!(
            DetectorErrorModel::from_dem_str(rejected).is_err(),
            "expected parser rejection: {rejected:?}"
        );
    }
}

#[test]
fn cq2_dem_model_value_mutation_and_repeat_contract_matches_stim() {
    let mut body = DetectorErrorModel::new();
    body.push_instruction(
        DemInstruction::shift_detectors(Vec::new(), 3, Some("step".to_string()))
            .expect("body shift"),
    );
    let repeat = DemRepeatBlock::new(
        DemRepeatCount::new(5),
        body.clone(),
        Some("loop".to_string()),
    );
    assert_eq!(repeat.repeat_count().get(), 5);
    assert_eq!(repeat.body(), &body);
    assert_eq!(repeat.tag(), Some("loop"));
    assert_eq!(repeat, repeat.clone());
    assert!(!format!("{repeat:?}").is_empty());

    let zero_repeat_count = DemRepeatCount::new(0);
    assert_eq!(zero_repeat_count.get(), 0);
    let zero_repeat = DemRepeatBlock::new(zero_repeat_count, body.clone(), None);
    assert_eq!(zero_repeat.repeat_count(), zero_repeat_count);
    assert_eq!(
        DetectorErrorModel::from_dem_str("repeat 0 {\n    shift_detectors[step] 3\n}\n")
            .expect("parse zero repeat"),
        {
            let mut expected = DetectorErrorModel::new();
            expected.push_repeat_block(zero_repeat);
            expected
        }
    );

    let mut model = DetectorErrorModel::default();
    assert_eq!(model, DetectorErrorModel::new());
    assert!(model.is_empty());
    model.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.2).expect("probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            Some("head".to_string()),
        )
        .expect("head error"),
    );
    model.push_repeat_block(repeat.clone());
    model
        .append_from_dem_text("logical_observable[tail] L2\n")
        .expect("append text");
    assert_eq!(model.len(), 3);
    assert_eq!(model.items().len(), 3);
    assert_eq!(model.iter_items().count(), 3);
    assert!(model.items()[0].as_instruction().is_some());
    assert!(model.items()[0].as_repeat_block().is_none());
    assert!(model.items()[1].as_instruction().is_none());
    assert_eq!(model.items()[1].as_repeat_block(), Some(&repeat));
    assert_eq!(model.item_range(1..).expect("item range").count(), 2);
    assert_eq!(
        model
            .instruction_range(..1)
            .expect("instruction-only range")
            .count(),
        1
    );
    assert!(model.instruction_range(..2).is_err());
    assert_eq!(model, model.clone());
    assert!(!format!("{model:?}").is_empty());
    assert_eq!(
        model.to_string(),
        concat!(
            "error[head](0.2000000000000000111022302462515654) D0\n",
            "repeat[loop] 5 {\n",
            "    shift_detectors[step] 3\n",
            "}\n",
            "logical_observable[tail] L2\n",
        )
    );

    let before = model.clone();
    assert!(model.append_from_dem_text("detector L0\n").is_err());
    assert_eq!(model, before, "failed append must be atomic");
    model.clear();
    assert!(model.is_empty());
    assert_eq!(model.len(), 0);
}

#[test]
fn cq2_dem_counts_and_shift_contract_matches_stim() {
    let model = DetectorErrorModel::from_dem_str(
        "shift_detectors 50\n\
         repeat 3 {\n\
             detector(1) D0\n\
             error(0.125) D0 D2 L6\n\
             shift_detectors(2, 3) 4\n\
         }\n\
         logical_observable L5\n",
    )
    .expect("counting DEM");
    assert_eq!(model.total_detector_shift().expect("detector shift"), 62);
    assert_eq!(model.count_detectors().expect("detector count"), 61);
    assert_eq!(model.count_observables().expect("observable count"), 7);
    assert_eq!(model.count_errors().expect("error count"), 3);
    assert_eq!(
        model.final_coordinate_shift().expect("coordinate shift"),
        vec![6.0, 9.0]
    );

    let mut overflow = DetectorErrorModel::new();
    for shift in [u64::MAX, 1] {
        overflow.push_instruction(
            DemInstruction::shift_detectors(Vec::new(), shift, None)
                .expect("construct programmatic high shift"),
        );
    }
    assert!(overflow.total_detector_shift().is_err());
}

#[test]
fn cq2_dem_coordinate_query_contract_matches_stim() {
    let model = DetectorErrorModel::from_dem_str(
        "detector(1, 2, 3) D0\n\
         shift_detectors 1\n\
         repeat 3 {\n\
             detector(2) D0\n\
             shift_detectors(5) 1\n\
         }\n",
    )
    .expect("coordinate DEM");
    let expected = BTreeMap::from([
        (DemDetectorId::try_new(0).expect("D0"), vec![1.0, 2.0, 3.0]),
        (DemDetectorId::try_new(1).expect("D1"), vec![2.0]),
        (DemDetectorId::try_new(2).expect("D2"), vec![7.0]),
        (DemDetectorId::try_new(3).expect("D3"), vec![12.0]),
    ]);
    assert_eq!(
        model.detector_coordinates().expect("all coordinates"),
        expected
    );
    assert_eq!(
        model
            .detector_coordinates_for([
                DemDetectorId::try_new(1).expect("D1"),
                DemDetectorId::try_new(3).expect("D3"),
            ])
            .expect("selected coordinates"),
        BTreeMap::from([
            (DemDetectorId::try_new(1).expect("D1"), vec![2.0]),
            (DemDetectorId::try_new(3).expect("D3"), vec![12.0]),
        ])
    );
    assert_eq!(
        model
            .coordinates_of_detector(DemDetectorId::try_new(2).expect("D2"))
            .expect("single coordinate"),
        vec![7.0]
    );
    assert!(
        model
            .coordinates_of_detector(DemDetectorId::try_new(4).expect("D4"))
            .is_err()
    );
}

#[test]
fn cq2_dem_flattened_iteration_contract_matches_stim() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.125) D0\n\
         repeat[tag] 2 {\n\
             shift_detectors(3) 2\n\
             detector(1, 2) D0\n\
             error(0.25) D0 L0\n\
         }\n\
         logical_observable L0\n",
    )
    .expect("flattened DEM");
    let instructions = model
        .iter_flattened_instructions()
        .collect::<CircuitResult<Vec<_>>>()
        .expect("lazy flattening");
    assert_eq!(instructions.len(), 6);
    let mut from_iterator = DetectorErrorModel::new();
    for instruction in instructions {
        from_iterator.push_instruction(instruction);
    }
    let expected = concat!(
        "error(0.125) D0\n",
        "detector(4, 2) D2\n",
        "error(0.25) D2 L0\n",
        "detector(7, 2) D4\n",
        "error(0.25) D4 L0\n",
        "logical_observable L0\n",
    );
    assert_eq!(from_iterator.to_dem_string(), expected);
    assert_eq!(
        model
            .flattened()
            .expect("materialized flattening")
            .to_dem_string(),
        expected
    );

    let huge = DetectorErrorModel::from_dem_str("repeat 1000000000000 {\n    error(0.1) D0\n}\n")
        .expect("large lazy DEM");
    let first = huge
        .iter_flattened_instructions()
        .take(3)
        .collect::<CircuitResult<Vec<_>>>()
        .expect("bounded lazy prefix");
    assert_eq!(first.len(), 3);
    assert!(huge.flattened().is_err());
}

#[test]
fn cq2_dem_compact_transform_contract_matches_stim() {
    let model = DetectorErrorModel::from_dem_str(
        "error[first](0.01000002) D0 D1\n\
         repeat[outer] 2 {\n\
             error[inner](0.123456789) D1 D2 L3\n\
             detector[coords](0.0200000334, 0.12345) D0\n\
         }\n",
    )
    .expect("transform DEM");
    assert_eq!(
        model.rounded(2).expect("rounded DEM"),
        DetectorErrorModel::from_dem_str(
            "error[first](0.01) D0 D1\n\
             repeat[outer] 2 {\n\
                 error[inner](0.12) D1 D2 L3\n\
                 detector[coords](0.0200000334, 0.12345) D0\n\
             }\n",
        )
        .expect("rounded reference")
    );
    let stripped = model.without_tags();
    assert_eq!(
        stripped,
        DetectorErrorModel::from_dem_str(
            "error(0.01000002) D0 D1\n\
             repeat 2 {\n\
                 error(0.123456789) D1 D2 L3\n\
                 detector(0.0200000334, 0.12345) D0\n\
             }\n",
        )
        .expect("tag-free reference")
    );
    assert!(!stripped.to_dem_string().contains('['));
    assert!(model.to_dem_string().contains("[first]"));
    assert_eq!(
        DetectorErrorModel::from_dem_str("error(0.000001) D0\n")
            .expect("tiny error")
            .rounded(2)
            .expect("round tiny error")
            .to_dem_string(),
        "error(0) D0\n"
    );
}

#[test]
fn cq2_dem_instruction_source_matrix_matches_stim() {
    cq2_dem_target_value_and_parse_contract_matches_stim();
    cq2_dem_instruction_value_validation_and_print_contract_matches_stim();
    cq2_dem_instruction_target_groups_match_stim();
}

#[test]
fn cq2_dem_model_source_matrix_matches_stim() {
    cq2_dem_model_value_mutation_and_repeat_contract_matches_stim();
}

#[test]
fn cq2_dem_materialized_transform_matrix_matches_stim() {
    cq2_dem_flattened_iteration_contract_matches_stim();
    cq2_dem_compact_transform_contract_matches_stim();
}

#[test]
fn cq2_dem_validation_matrix_matches_stim() {
    cq2_dem_target_value_and_parse_contract_matches_stim();
    cq2_dem_instruction_value_validation_and_print_contract_matches_stim();
    cq2_dem_model_parse_print_tag_and_newline_contract_matches_stim();
}
