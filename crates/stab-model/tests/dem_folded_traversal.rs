#![allow(
    clippy::expect_used,
    reason = "integration tests use direct assertions for compact parity diagnostics"
)]

use std::collections::BTreeMap;

use proptest::prelude::*;
use stab_model::{DemDetectorId, DemRepeatBlock, DemRepeatCount, DetectorErrorModel};

#[path = "dem_folded_traversal/generated.rs"]
mod generated;

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

fn detector(id: u64) -> DemDetectorId {
    DemDetectorId::try_new(id).expect("valid detector id")
}

fn assert_generated_folded_model_semantics() {
    let mut runner = generated::generated_dem_runner();
    runner
        .run(&generated::generated_dem_strategy(), |items| {
            let compact_text = generated::render_generated_dem(&items);
            let compact = DetectorErrorModel::from_dem_str(&compact_text).map_err(|error| {
                TestCaseError::fail(format!(
                    "generated folded DEM did not parse: {error}\n{compact_text}"
                ))
            })?;
            let materialized_text =
                generated::render_generated_dem(&generated::expand_generated_dem(&items));
            let materialized =
                DetectorErrorModel::from_dem_str(&materialized_text).map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated materialized DEM did not parse: {error}\n{materialized_text}"
                    ))
                })?;

            prop_assert_eq!(
                compact.total_detector_shift(),
                materialized.total_detector_shift()
            );
            prop_assert_eq!(compact.count_detectors(), materialized.count_detectors());
            prop_assert_eq!(
                compact.count_observables(),
                materialized.count_observables()
            );
            prop_assert_eq!(compact.count_errors(), materialized.count_errors());
            prop_assert_eq!(
                compact.final_coordinate_shift(),
                materialized.final_coordinate_shift()
            );
            prop_assert_eq!(
                compact.detector_coordinates(),
                materialized.detector_coordinates()
            );
            Ok(())
        })
        .expect("deterministic generated folded DEM model corpus");
}

#[test]
fn pfm_b3_folded_traversal_counts() {
    let huge = dem("repeat 1000000000 {\n\
             repeat 1 {\n\
                 error(0) D3 ^ D1 L5\n\
                 detector(1, 2) D1\n\
                 shift_detectors(3, 4) 2\n\
             }\n\
             logical_observable L7\n\
         }\n");
    assert_eq!(
        huge.total_detector_shift().expect("detector shift"),
        2_000_000_000
    );
    assert_eq!(
        huge.count_detectors().expect("detector count"),
        2_000_000_002
    );
    assert_eq!(huge.count_observables().expect("observable count"), 8);
    assert_eq!(huge.count_errors().expect("error count"), 1_000_000_000);
    assert_eq!(
        huge.final_coordinate_shift().expect("coordinate shift"),
        vec![3_000_000_000.0, 4_000_000_000.0]
    );

    assert_generated_folded_model_semantics();

    let overflow = dem("repeat 17 {\n    shift_detectors 1152921504606846975\n}\n")
        .total_detector_shift()
        .expect_err("checked repeat shift overflow");
    assert!(overflow.to_string().contains("overflowed"), "{overflow}");

    let coordinate_overflow = dem("repeat 2 {\n    shift_detectors(1e308) 0\n}\n");
    assert_eq!(coordinate_overflow.count_detectors(), Ok(0));
    let error = coordinate_overflow
        .final_coordinate_shift()
        .expect_err("coordinate overflow belongs only to coordinate queries");
    assert!(
        error.to_string().contains("coordinate shift overflowed"),
        "{error}"
    );

    let wide_coordinates = std::iter::repeat_n("1", 32_000)
        .collect::<Vec<_>>()
        .join(",");
    let mut deep_coordinate = dem(&format!("shift_detectors({wide_coordinates}) 0\n"));
    for _ in 0..256 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            DemRepeatCount::new(1),
            deep_coordinate,
            None,
        ));
        deep_coordinate = outer;
    }
    assert_eq!(deep_coordinate.count_detectors(), Ok(0));
    let error = deep_coordinate
        .final_coordinate_shift()
        .expect_err("aggregate coordinate scalar work must be bounded");
    assert!(
        error.to_string().contains("coordinate scalar updates"),
        "{error}"
    );
}

#[test]
fn pfm_b3_folded_traversal_coordinates() {
    let compact = dem("repeat 3 {\n\
             detector(10) D2\n\
             shift_detectors(1) 1\n\
             repeat 2 {\n\
                 detector(20) D0\n\
                 shift_detectors(2) 2\n\
             }\n\
         }\n\
         error(0.1) D1\n");
    assert_eq!(
        compact.detector_coordinates().expect("compact coordinates"),
        BTreeMap::from([
            (detector(0), Vec::new()),
            (detector(1), vec![21.0]),
            (detector(2), vec![10.0]),
            (detector(3), vec![23.0]),
            (detector(4), Vec::new()),
            (detector(5), Vec::new()),
            (detector(6), vec![26.0]),
            (detector(7), vec![15.0]),
            (detector(8), vec![28.0]),
            (detector(9), Vec::new()),
            (detector(10), Vec::new()),
            (detector(11), vec![31.0]),
            (detector(12), vec![20.0]),
            (detector(13), vec![33.0]),
            (detector(14), Vec::new()),
            (detector(15), Vec::new()),
            (detector(16), Vec::new()),
        ])
    );
    assert_eq!(
        compact
            .detector_coordinates_for([detector(0), detector(2), detector(7), detector(15)])
            .expect("selected compact coordinates"),
        BTreeMap::from([
            (detector(0), Vec::new()),
            (detector(2), vec![10.0]),
            (detector(7), vec![15.0]),
            (detector(15), Vec::new()),
        ])
    );

    let huge_sparse = dem("repeat 4000000 {\n\
             repeat 1 {\n\
                 detector(7) D0\n\
             }\n\
             detector(99) D2000000\n\
             shift_detectors(1) 1\n\
         }\n");
    assert_eq!(
        huge_sparse
            .coordinates_of_detector(detector(1_500_000))
            .expect("folded sparse coordinate"),
        vec![1_500_007.0]
    );
    let ambiguous =
        dem("repeat 10 {\n    detector(100) D2\n    detector(0) D0\n    shift_detectors(1) 1\n}\n");
    assert_eq!(
        ambiguous
            .coordinates_of_detector(detector(9))
            .expect("first repeated declaration"),
        vec![107.0]
    );

    let huge_full = dem("repeat 1000001 {\n    detector(1) D0\n    shift_detectors 1\n}\n");
    let error = huge_full
        .detector_coordinates()
        .expect_err("full coordinate map has inherently expanded output");
    assert!(
        error.to_string().contains("at most 1000000 detectors"),
        "{error}"
    );

    let declaration_overflow = dem(
        "error(0) D1\nrepeat 5 {\n    repeat 5 {\n        repeat 1152921504606846975 {\n            detector(5) D0\n        }\n    }\n}\n",
    );
    assert_eq!(declaration_overflow.count_detectors(), Ok(2));
    assert_eq!(
        declaration_overflow
            .coordinates_of_detector(detector(0))
            .expect("selected declaration survives irrelevant count overflow"),
        vec![5.0]
    );
    assert_eq!(
        declaration_overflow
            .coordinates_of_detector(detector(1))
            .expect("selected sparse hole survives irrelevant count overflow"),
        Vec::<f64>::new()
    );

    let fractional = dem("repeat 100 {\n    detector(0) D0\n    shift_detectors(0.1) 1\n}\n");
    let coordinate = fractional
        .coordinates_of_detector(detector(99))
        .expect("fractional selected coordinate");
    assert_eq!(coordinate.len(), 1);
    let coordinate = *coordinate.first().expect("one fractional coordinate");
    assert!(
        (coordinate - 9.899_999_999_999_98).abs() <= 1e-12,
        "coordinate must be semantically equivalent to pinned Stim accumulation: {coordinate}"
    );
}
