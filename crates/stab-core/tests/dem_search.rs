#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    DetectorErrorModel, find_undetectable_logical_error, shortest_error_sat_problem,
    shortest_graphlike_undetectable_logical_error,
};

#[test]
fn pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned() {
    let allowed = DetectorErrorModel::from_dem_str(
        "error(0.1) D0\nrepeat 2 {\n    error(0.1) D0 D1\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();
    let expected = "error(1) D0\nerror(1) D0 D1\nerror(1) D1 D2\nerror(1) D2 L0\n";

    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&allowed, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    assert_eq!(
        find_undetectable_logical_error(&allowed, usize::MAX, usize::MAX, false)
            .unwrap()
            .to_dem_string(),
        expected
    );
    let sat_problem = shortest_error_sat_problem(&allowed).unwrap();
    assert_eq!(
        sat_problem
            .lines()
            .filter(|line| line.starts_with("1 -"))
            .count(),
        4,
        "SAT problem should include one soft clause per expanded shifted-repeat error"
    );

    let hostile = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let graphlike_error = shortest_graphlike_undetectable_logical_error(&hostile, true)
        .expect_err("graphlike search should reject hostile repeat expansion")
        .to_string();
    assert!(
        graphlike_error
            .contains("DEM graphlike search currently supports repeat counts up to 100000"),
        "{graphlike_error}"
    );

    let hyper_error = find_undetectable_logical_error(&hostile, usize::MAX, usize::MAX, false)
        .expect_err("hypergraph search should reject hostile repeat expansion")
        .to_string();
    assert!(
        hyper_error.contains("DEM hypergraph search currently supports repeat counts up to 100000"),
        "{hyper_error}"
    );

    let sat_error = shortest_error_sat_problem(&hostile)
        .expect_err("SAT problem generation should reject hostile repeat expansion")
        .to_string();
    assert!(
        sat_error
            .contains("DEM SAT problem generation currently supports repeat counts up to 100000"),
        "{sat_error}"
    );
}
