#![allow(
    clippy::expect_used,
    reason = "parser fast-path tests use direct fixture assertions for compact diagnostics"
)]

use stab_model::{Circuit, CircuitItem};

#[test]
fn common_phase_and_annotation_paths_preserve_public_semantics() {
    let exact = Circuit::from_stim_str("S 1\nTICK\nDETECTOR rec[-1]\n")
        .expect("parse exact common instructions");
    let generic = Circuit::from_stim_str("s    1\n tick\n detector  rec[-1]\n")
        .expect("parse generic common instructions");

    assert_eq!(exact, generic);
    assert_eq!(exact.to_stim_string(), "S 1\nTICK\nDETECTOR rec[-1]\n");

    let decorated =
        Circuit::from_stim_str("S[tag] 1\nTICK[tag]\nDETECTOR[tag](1, 2) rec[-1] rec[-2]\n")
            .expect("parse decorated generic fallbacks");
    assert_eq!(
        decorated.to_stim_string(),
        "S[tag] 1\nTICK[tag]\nDETECTOR[tag](1, 2) rec[-1] rec[-2]\n"
    );
}

#[test]
fn detector_fast_path_rejects_non_stim_unicode_whitespace() {
    for separator in ['\u{a0}', '\u{2003}'] {
        for name in ["DETECTOR", "detector"] {
            Circuit::from_stim_str(&format!("{name} rec[-1]{separator}rec[-2]\n"))
                .expect_err("reject non-Stim Unicode target separator");
        }
    }
}

#[test]
fn qualification_cycle_avoids_per_instruction_item_allocation() {
    const INSTRUCTION_COUNT: usize = 4_096;
    const MAX_ITEM_ALLOCATIONS: u64 = 12;
    const CYCLE: [&str; 6] = [
        "H 0\n",
        "S 1\n",
        "CX 0 1\n",
        "M 0\n",
        "DETECTOR rec[-1]\n",
        "TICK\n",
    ];

    let mut input = String::with_capacity(INSTRUCTION_COUNT * 12);
    for instruction in CYCLE.iter().cycle().take(INSTRUCTION_COUNT) {
        input.push_str(instruction);
    }
    let parsed = Circuit::from_stim_str(&input).expect("warm qualification-cycle parse");
    assert_eq!(parsed.items().len(), INSTRUCTION_COUNT);
    std::hint::black_box(parsed);

    let allocations = allocation_counter::measure(|| {
        let parsed = Circuit::from_stim_str(&input).expect("measured qualification-cycle parse");
        std::hint::black_box(parsed.items().len());
    });
    let expected_bytes = u64::try_from(std::mem::size_of::<CircuitItem>() * INSTRUCTION_COUNT)
        .expect("qualification-cycle allocation size fits u64");
    assert!(
        allocations.count_total <= MAX_ITEM_ALLOCATIONS,
        "parser performed too many item-vector allocations: {allocations:?}"
    );
    assert!(
        allocations.count_max <= 2,
        "item-vector growth retained too many allocations: {allocations:?}"
    );
    assert!(
        allocations.bytes_total <= expected_bytes.saturating_mul(2),
        "item-vector growth exceeded a geometric bound: {allocations:?}"
    );
    assert!(
        allocations.bytes_max <= expected_bytes.saturating_mul(2),
        "item-vector peak storage exceeded a geometric bound: {allocations:?}"
    );

    let byte_allocations = allocation_counter::measure(|| {
        let parsed = Circuit::from_stim_bytes(input.as_bytes()).expect("measured byte-entry parse");
        std::hint::black_box(parsed.items().len());
    });
    assert_eq!(
        byte_allocations.count_total, allocations.count_total,
        "{byte_allocations:?}"
    );
    assert_eq!(
        byte_allocations.bytes_total, allocations.bytes_total,
        "{byte_allocations:?}"
    );
}

#[test]
fn exact_detector_fast_candidates_preserve_target_boundaries() {
    for invalid in [
        "DETECTOR rec[-16777216]\n",
        "DETECTOR rec[-999999999999999999999]\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}
