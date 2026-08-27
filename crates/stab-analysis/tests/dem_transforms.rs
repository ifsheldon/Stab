#![allow(
    clippy::expect_used,
    reason = "DEM transform compatibility tests use direct fixture assertions for precise failures"
)]

use stab_analysis::{
    detector_error_model_without_tags, flattened_detector_error_model, rounded_detector_error_model,
};
use stab_model::{
    DemInstruction, DemItem, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel,
    ModelResult, Probability,
};

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

fn dem_from_bytes(bytes: &[u8]) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_bytes(bytes).expect("parse opaque-tag DEM")
}

fn dem_top_level_tags(model: &DetectorErrorModel) -> Vec<Option<&[u8]>> {
    model
        .items()
        .iter()
        .map(|item| match item {
            DemItem::Instruction(instruction) => instruction.tag_bytes(),
            DemItem::RepeatBlock(repeat) => repeat.tag_bytes(),
        })
        .collect()
}

#[test]
fn pf4_dem_materialized_flattened_matches_pinned_stim_cases() {
    let empty = DetectorErrorModel::new();
    assert_eq!(
        flattened_detector_error_model(&empty).expect("flatten empty"),
        empty
    );

    let zero_repeat = DetectorErrorModel::from_dem_str(
        "repeat 0 {\n    error(1) D9 L7\n    shift_detectors(2) 10\n}\n",
    )
    .expect("parse zero-count repeat");
    assert_eq!(
        flattened_detector_error_model(&zero_repeat)
            .expect("flatten zero-count repeat")
            .to_dem_string(),
        ""
    );

    let shifted = DetectorErrorModel::from_dem_str(
        "shift_detectors 5\n\
         error(0.125) D0 ^ D1 L0\n",
    )
    .expect("parse shifted DEM");
    assert_eq!(
        flattened_detector_error_model(&shifted)
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
        flattened_detector_error_model(&coordinates)
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
        flattened_detector_error_model(&repeated)
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

fn deep_tagged_dem(depth: usize) -> DetectorErrorModel {
    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.125).expect("valid probability"),
            vec![DemTarget::relative_detector(0).expect("D0")],
            Some("leaf".to_string()),
        )
        .expect("valid error instruction"),
    );
    for _ in 0..depth {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            DemRepeatCount::new(1),
            model,
            Some("repeat".to_string()),
        ));
        model = outer;
    }
    model
}

fn deep_transform_matches(
    model: &DetectorErrorModel,
    depth: usize,
    expected_repeat_tag: Option<&[u8]>,
    expected_leaf_tag: Option<&[u8]>,
    expected_probability: f64,
) -> bool {
    let mut current = model;
    for _ in 0..depth {
        let Some(DemItem::RepeatBlock(repeat)) = current.items().first() else {
            return false;
        };
        if repeat.tag_bytes() != expected_repeat_tag {
            return false;
        }
        current = repeat.body();
    }
    let Some(DemItem::Instruction(instruction)) = current.items().first() else {
        return false;
    };
    instruction.tag_bytes() == expected_leaf_tag
        && instruction
            .args()
            .first()
            .is_some_and(|value| (*value - expected_probability).abs() < f64::EPSILON)
}

#[test]
fn deep_folded_dem_transforms_use_bounded_stack() {
    const DEPTH: usize = 10_000;
    let model = deep_tagged_dem(DEPTH);
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let rounded =
                rounded_detector_error_model(&model, 2).map_err(|error| error.to_string())?;
            let stripped = detector_error_model_without_tags(&model);
            let result = (
                deep_transform_matches(&rounded, DEPTH, Some(b"repeat"), Some(b"leaf"), 0.13),
                deep_transform_matches(&stripped, DEPTH, None, None, 0.125),
            );
            drop(stripped);
            drop(rounded);
            drop(model);
            Ok::<_, String>(result)
        })
        .expect("spawn constrained-stack transform regression");

    let (rounded, stripped) = handle
        .join()
        .expect("constrained-stack transform should not panic")
        .expect("deep transforms should succeed");
    assert!(rounded);
    assert!(stripped);
}

#[test]
fn pf4_dem_materialized_flattened_rejects_excessive_repeat() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n\
             error(0.125) D0\n\
         }\n",
    )
    .expect("parse large repeat DEM");

    let error = flattened_detector_error_model(&dem).expect_err("reject excessive flattening");

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
        rounded_detector_error_model(&dem, 0).expect("round 0"),
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
        rounded_detector_error_model(&dem, 2).expect("round 2"),
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
        rounded_detector_error_model(&dem, 3)
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
        rounded_detector_error_model(&dem, 2)
            .expect("round tiny error")
            .to_dem_string(),
        "error(0) D0 D1\n",
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
        .collect::<ModelResult<Vec<_>>>()
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
        flattened_detector_error_model(&model)
            .expect("materialized flattening")
            .to_dem_string(),
        expected
    );

    let huge = DetectorErrorModel::from_dem_str("repeat 1000000000000 {\n    error(0.1) D0\n}\n")
        .expect("large lazy DEM");
    let first = huge
        .iter_flattened_instructions()
        .take(3)
        .collect::<ModelResult<Vec<_>>>()
        .expect("bounded lazy prefix");
    assert_eq!(first.len(), 3);
    assert!(flattened_detector_error_model(&huge).is_err());
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
        rounded_detector_error_model(&model, 2).expect("rounded DEM"),
        DetectorErrorModel::from_dem_str(
            "error[first](0.01) D0 D1\n\
             repeat[outer] 2 {\n\
                 error[inner](0.12) D1 D2 L3\n\
                 detector[coords](0.0200000334, 0.12345) D0\n\
             }\n",
        )
        .expect("rounded reference")
    );
    let stripped = detector_error_model_without_tags(&model);
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
        rounded_detector_error_model(
            &DetectorErrorModel::from_dem_str("error(0.000001) D0\n").expect("tiny error"),
            2,
        )
        .expect("round tiny error")
        .to_dem_string(),
        "error(0) D0\n"
    );
}

#[test]
fn cq2_dem_materialized_transform_matrix_matches_stim() {
    cq2_dem_flattened_iteration_contract_matches_stim();
    cq2_dem_compact_transform_contract_matches_stim();
}

#[test]
fn rounded_dem_preserves_opaque_instruction_and_repeat_tags() {
    let model = dem_from_bytes(
        b"error[\xff](0.49) D0\n\
          repeat[\xfe] 2 {\n    error[\xfd](0.51) D1\n}\n",
    );
    let rounded = rounded_detector_error_model(&model, 0).expect("round tagged DEM");

    assert_eq!(
        rounded.to_dem_bytes(),
        b"error[\xff](0) D0\nrepeat[\xfe] 2 {\n    error[\xfd](1) D1\n}\n"
    );
    assert_eq!(
        dem_top_level_tags(&rounded),
        vec![Some(b"\xff".as_slice()), Some(b"\xfe".as_slice())]
    );
    let repeat = rounded
        .items()
        .iter()
        .find_map(|item| match item {
            DemItem::Instruction(_) => None,
            DemItem::RepeatBlock(repeat) => Some(repeat),
        })
        .expect("rounded repeat block");
    assert_eq!(
        dem_top_level_tags(repeat.body()),
        vec![Some(b"\xfd".as_slice())]
    );
}

#[test]
fn flattened_dem_preserves_opaque_tags_on_materialized_instructions() {
    let model = dem_from_bytes(
        b"error[\xff](0.5) D0\n\
          shift_detectors[\xfc] 3\n\
          repeat[\xfe] 2 {\n    error[\xfd](0.25) D0 L1\n    detector[\xfb] D1\n    shift_detectors[\xfa] 2\n}\n",
    );
    let flattened = flattened_detector_error_model(&model).expect("flatten tagged DEM");

    assert_eq!(
        flattened.to_dem_bytes(),
        b"error[\xff](0.5) D0\n\
          error[\xfd](0.25) D3 L1\n\
          detector[\xfb] D4\n\
          error[\xfd](0.25) D5 L1\n\
          detector[\xfb] D6\n"
    );
    assert_eq!(
        dem_top_level_tags(&flattened),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xfd".as_slice()),
            Some(b"\xfb".as_slice()),
            Some(b"\xfd".as_slice()),
            Some(b"\xfb".as_slice()),
        ]
    );
}

#[test]
fn pfm_b3_folded_traversal_transforms() {
    let source = dem("repeat[outer] 1000000000 {\n\
             error[first](0.123456) D0 L0\n\
             detector[coords](1, 2) D0\n\
             repeat[inner] 3 {\n\
                 error[tiny](0.0004) D1\n\
             }\n\
         }\n");
    let rounded = rounded_detector_error_model(&source, 3).expect("compact rounded transform");
    assert_eq!(
        rounded,
        dem("repeat[outer] 1000000000 {\n\
                 error[first](0.123) D0 L0\n\
                 detector[coords](1, 2) D0\n\
                 repeat[inner] 3 {\n\
                     error[tiny](0) D1\n\
                 }\n\
             }\n")
    );
    let stripped = detector_error_model_without_tags(&source).to_dem_string();
    assert!(stripped.starts_with("repeat 1000000000"), "{stripped}");
    assert!(!stripped.contains('['), "{stripped}");
    let flatten_error = flattened_detector_error_model(&source)
        .expect_err("materialized flattening keeps its explicit cap");
    assert!(
        flatten_error.to_string().contains("supports repeat counts"),
        "{flatten_error}"
    );

    let mut deep = dem("error[tag](0.1234) D0\n");
    for _ in 0..257 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            DemRepeatCount::new(1),
            deep,
            Some("nested".to_string()),
        ));
        deep = outer;
    }
    assert_eq!(deep.count_errors(), Ok(1));
    assert_eq!(
        rounded_detector_error_model(&deep, 2)
            .expect("deep rounded model")
            .count_errors(),
        Ok(1)
    );
    assert!(
        !detector_error_model_without_tags(&deep)
            .to_dem_string()
            .contains('[')
    );
}
