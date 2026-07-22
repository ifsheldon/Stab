#![allow(
    clippy::expect_used,
    reason = "parser regression tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::DetectorErrorModel;

#[test]
fn generic_dem_parser_preserves_case_whitespace_comments_tags_and_spills() {
    let input = concat!(
        "ErRoR[edge\\Ctag](0.25) D0\u{2003}D1 ^ L2 D3 D4\n",
        "DeTeCtOr(1, 2, 3, 4) D7 # trailing comment\n",
        "detector[tag#value] D8 # hash inside the tag is not a comment\n",
        "LoGiCaL_ObSeRvAbLe L3\n",
        "ShIfT_DeTeCtOrS(1, 2, 3) 9\n",
        "error[this-tag-is-longer-than-inline](0.5) D9\n",
        "detector[ééééééééé] D10\n",
    );
    let expected = concat!(
        "error[edge\\Ctag](0.25) D0 D1 ^ L2 D3 D4\n",
        "detector(1, 2, 3, 4) D7\n",
        "detector[tag#value] D8\n",
        "logical_observable L3\n",
        "shift_detectors(1, 2, 3) 9\n",
        "error[this-tag-is-longer-than-inline](0.5) D9\n",
        "detector[ééééééééé] D10\n",
    );

    let model = DetectorErrorModel::from_dem_str(input).expect("parse generic DEM forms");

    assert_eq!(model.to_dem_string(), expected);
    assert_eq!(
        DetectorErrorModel::from_dem_str(expected).expect("reparse canonical DEM"),
        model
    );
}

#[test]
fn qualification_cycle_has_bounded_parser_allocations() {
    const TOP_LEVEL_ITEMS: usize = 4_096;
    const CYCLE_ITEMS: usize = 8;
    const CYCLES: usize = TOP_LEVEL_ITEMS / CYCLE_ITEMS;
    const MAX_ALLOCATIONS_PER_CYCLE: u64 = 4;
    const FIXED_ALLOCATIONS: u64 = 2;
    const CYCLE: &str = concat!(
        "error(0.125) D0\n",
        "error[edge](0.25) D1 D2 L0 ^ D3\n",
        "detector(0.5, 1) D4\n",
        "logical_observable L1\n",
        "shift_detectors(1.5, 3) 5\n",
        "detector[tagged] D2\n",
        "repeat[loop] 3 {\n",
        "    error(0.375) D0 D1\n",
        "    shift_detectors 2\n",
        "}\n",
        "error(0.0625) D5 ^ L2\n",
    );

    let input = CYCLE.repeat(CYCLES);
    let warm = DetectorErrorModel::from_dem_str(&input).expect("warm qualification parse");
    assert_eq!(warm.items().len(), TOP_LEVEL_ITEMS);
    std::hint::black_box(warm);

    let allocations = allocation_counter::measure(|| {
        let model = DetectorErrorModel::from_dem_str(&input).expect("measured qualification parse");
        std::hint::black_box(model.items().len());
    });
    let maximum = u64::try_from(CYCLES).expect("cycle count fits u64") * MAX_ALLOCATIONS_PER_CYCLE
        + FIXED_ALLOCATIONS;

    assert!(
        allocations.count_total <= maximum,
        "qualification parser exceeded {maximum} allocations: {allocations:?}"
    );
}
