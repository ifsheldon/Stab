#![allow(
    clippy::expect_used,
    reason = "integration tests use direct assertions for compact parity diagnostics"
)]

use stab_analysis::{
    ResourceKind, ResourceOperation, find_undetectable_logical_error, likeliest_error_sat_problem,
    shortest_error_sat_problem, shortest_graphlike_undetectable_logical_error,
};
use stab_model::DetectorErrorModel;

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

#[test]
fn pfm_b3_folded_traversal_search() {
    let repeated = dem("repeat 100001 {\n\
             detector(1) D0\n\
             logical_observable L2\n\
             error(0) D999999 L999\n\
             error(0.1) D0\n\
             repeat 17 {\n\
                 shift_detectors 0\n\
                 error(0.2) D0 L0\n\
             }\n\
             error(0.3) D0 ^ D1\n\
         }\n");
    let compact = dem("detector(1) D0\n\
         logical_observable L2\n\
         error(0) D999999 L999\n\
         error(0.1) D0\n\
         shift_detectors 0\n\
         error(0.2) D0 L0\n\
         error(0.3) D0 ^ D1\n");
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&repeated, false)
            .expect("folded graphlike search")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&compact, false)
            .expect("compact graphlike search")
            .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&repeated, usize::MAX, usize::MAX, false)
            .expect("folded hypergraph search")
            .to_dem_string(),
        find_undetectable_logical_error(&compact, usize::MAX, usize::MAX, false)
            .expect("compact hypergraph search")
            .to_dem_string()
    );

    let neutral_nested = dem(
        "repeat 100001 {\n    error(0.1) D0\n    error(0.1) D0 L0\n    repeat 100001 {\n    }\n}\n",
    );
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&neutral_nested, false)
            .expect("nested neutral repeat is skipped")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            false,
        )
        .expect("compact neutral-repeat reference")
        .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&neutral_nested, usize::MAX, usize::MAX, false)
            .expect("nested neutral repeat is skipped by hypergraph collection")
            .to_dem_string(),
        find_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            usize::MAX,
            usize::MAX,
            false,
        )
        .expect("compact neutral hypergraph reference")
        .to_dem_string()
    );

    let wide_shift = std::iter::repeat_n("1", 4096).collect::<Vec<_>>().join(",");
    let coordinate_irrelevant = dem(&format!(
        "repeat 100001 {{\n    error(0.1) D0\n    error(0.1) D0 L0\n    shift_detectors({wide_shift}) 0\n}}\n"
    ));
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&coordinate_irrelevant, false)
            .expect("search does not allocate or update irrelevant coordinate state")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            false,
        )
        .expect("compact coordinate-irrelevant reference")
        .to_dem_string()
    );

    let shifted = dem("repeat 100001 {\n    error(0.1) D0 L0\n    shift_detectors 1\n}\n");
    let error = shortest_graphlike_undetectable_logical_error(&shifted, false)
        .expect_err("shifted active repeat exceeds bounded search traversal");
    assert!(
        error.to_string().contains("supports repeat counts"),
        "{error}"
    );
}

#[test]
fn pfm_b3_folded_traversal_sat() {
    let repeated = dem("repeat 100001 {\n\
             detector D0\n\
             error(0.1) D0 L0\n\
             repeat 2 {\n\
                 error(0.9) D0\n\
                 shift_detectors 0\n\
             }\n\
             logical_observable L0\n\
         }\n");
    for error in [
        shortest_error_sat_problem(&repeated).expect_err("folded SAT materialization limit"),
        likeliest_error_sat_problem(&repeated, 100).expect_err("folded WCNF materialization limit"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("typed SAT materialization error");
        assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
        assert_eq!(resource.resource(), ResourceKind::ErrorMechanisms);
        assert_eq!(resource.actual(), 250_001);
        assert_eq!(resource.limit(), 250_000);
    }

    let neutral = dem("repeat 100001 {\n}\n");
    assert_eq!(
        shortest_error_sat_problem(&neutral).expect("neutral SAT"),
        shortest_error_sat_problem(&DetectorErrorModel::new()).expect("empty SAT reference")
    );
}
