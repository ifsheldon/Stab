#![allow(
    clippy::expect_used,
    reason = "parser regression tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{DemInstruction, DemItem, DemTarget, DetectorErrorModel};

#[test]
fn dem_model_layout_stays_bounded_for_large_parse_workloads() {
    assert!(
        std::mem::size_of::<DemTarget>() <= 16,
        "DEM target layout unexpectedly grew"
    );
    assert!(
        std::mem::size_of::<DemInstruction>() <= 160,
        "DEM instruction layout unexpectedly grew"
    );
    assert!(
        std::mem::size_of::<DemItem>() <= 160,
        "DEM item layout unexpectedly grew"
    );
}

#[test]
fn generic_dem_parser_preserves_case_whitespace_comments_tags_and_spills() {
    let input = concat!(
        "ErRoR[edge\\Ctag](0.25) d0 D1 ^ l2 D3 D4\n",
        " \tDeTeCtOr( \t1, 2, 3, 4\t ) D7\t  # trailing comment\n",
        "detector[tag#value] D8 # hash inside the tag is not a comment\n",
        "LoGiCaL_ObSeRvAbLe l3\n",
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
fn fast_target_parser_preserves_numeric_boundaries_and_rejections() {
    let maximum = DetectorErrorModel::from_dem_str(
        "shift_detectors 1152921504606846975\nerror(0.25) d0 l1\n",
    )
    .expect("parse maximum textual integer and lowercase typed targets");
    assert_eq!(
        maximum.to_dem_string(),
        "shift_detectors 1152921504606846975\nerror(0.25) D0 L1\n"
    );

    for invalid in [
        "shift_detectors 1152921504606846976\n",
        "shift_detectors 18446744073709551616\n",
        "error(0.25) D1152921504606846976\n",
        "repeat 1152921504606846976 {\n}\n",
        "error(0.25) D\n",
        "error(0.25) L\n",
        "error(0.25) 1D\n",
    ] {
        assert!(
            DetectorErrorModel::from_dem_str(invalid).is_err(),
            "accepted invalid DEM target in {invalid:?}"
        );
    }
}

#[test]
fn generic_dem_parser_rejects_unicode_separators_and_detached_modifiers() {
    for invalid in [
        "error(0.25) D0\u{2003}D1\n",
        "\u{2003}error(0.25) D0\n",
        "detector [tag] D0\n",
        "detector (1) D0\n",
        "repeat [tag] 2 {\n}\n",
    ] {
        assert!(
            DetectorErrorModel::from_dem_str(invalid).is_err(),
            "accepted non-Stim DEM whitespace in {invalid:?}"
        );
    }
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

#[test]
fn representative_flat_and_coordinate_families_avoid_per_instruction_allocations() {
    const TOP_LEVEL_ITEMS: usize = 4_096;
    const CYCLE_ITEMS: usize = 8;
    const MAX_PARSE_ALLOCATIONS: u64 = 2;
    const FLAT_ERRORS_CYCLE: &str = concat!(
        "error(0.125) D0\n",
        "error(0.25) D1 D2\n",
        "error(0.375) D3 L0\n",
        "error(0.0625) D4 ^ D5\n",
        "error(0.5) D6 D7 D8\n",
        "error(0.03125) D9 L1 ^ D10\n",
        "error(0.75) D11 D12 L2\n",
        "error(0.875) D13 ^ D14 L3\n",
    );
    const COORDINATE_SPARSE_CYCLE: &str = concat!(
        "detector[tag-a](0.5, 1) D1000000\n",
        "logical_observable L100000\n",
        "shift_detectors(1.5, -2, 3) 1000001\n",
        "error[edge](0.25) D0 D1000000 L0 ^ D7\n",
        "detector(2, 3.5) D42\n",
        "error(0.125) D999999 L99999\n",
        "shift_detectors 17\n",
        "detector[tag-b] D1000017\n",
    );

    for (family, cycle) in [
        ("flat-errors", FLAT_ERRORS_CYCLE),
        ("coordinate-sparse", COORDINATE_SPARSE_CYCLE),
    ] {
        let input = cycle.repeat(TOP_LEVEL_ITEMS / CYCLE_ITEMS);
        let warm = DetectorErrorModel::from_dem_str(&input).expect("warm family parse");
        assert_eq!(warm.items().len(), TOP_LEVEL_ITEMS);
        std::hint::black_box(warm);

        let allocations = allocation_counter::measure(|| {
            let model = DetectorErrorModel::from_dem_str(&input).expect("measured family parse");
            std::hint::black_box(model.items().len());
        });
        assert!(
            allocations.count_total <= MAX_PARSE_ALLOCATIONS,
            "{family} parser performed per-instruction allocations: {allocations:?}"
        );
    }
}
