use stab_core::SparseXorVec;

use super::sparse_xor::{
    SPARSE_ITEM_BASE_WORK_ITEMS, SPARSE_ITEM_MAX_WORK_ITEMS, SPARSE_ROW_BASE_WORK_ITEMS,
    SPARSE_ROW_MAX_WORK_ITEMS, SparseXorFixture, SparseXorKind,
};
use super::{WorkerError, semantic_digest};

const ROW_INPUT_DIGEST: &str = "9fdcaf10b6a6437d51afade0e21f39acdd1130ff18255e38c0751261f93df2a2";
const ROW_ODD_OUTPUT_DIGEST: &str =
    "965b7771f81e6f2d3852054cd9f7264c44678c565b110239030ac3f15a0ef466";
const ROW_EVEN_OUTPUT_DIGEST: &str =
    "14da5bd21c8d108e3a62836ad2b609717576519fd1698f2eb54d20e93059bcc9";
const ROW_MAX_OUTPUT_DIGEST: &str =
    "914ae143ba0e910f5a1e82fc71c02d8d71722714b508fa38c7bbfcdf267f78a7";
const ITEM_INPUT_DIGEST: &str = "c2c1749b4bf4c7c355c1d0a8109ea53bba790034d116acea3755b533c1fb1059";
const ITEM_ODD_OUTPUT_DIGEST: &str =
    "ff6a52e2bae9e011bad5033d00625472e3778842ccc1065696939f48d61bba5a";
const ITEM_EVEN_OUTPUT_DIGEST: &str =
    "5eb3fddcc378022892474da3369b0bf92e260efec5305685eb5621bede48fb49";
const ITEM_MAX_OUTPUT_DIGEST: &str =
    "57dec6b5484ba78c84a054c42cb574a8678ff7531f219c8885d4607b7faae8ef";

#[test]
fn sparse_row_fixture_binds_exact_input_scales_and_states() {
    for (work_items, expected_sweeps) in [
        (SPARSE_ROW_BASE_WORK_ITEMS, 1),
        (SPARSE_ROW_BASE_WORK_ITEMS * 64, 64),
        (SPARSE_ROW_MAX_WORK_ITEMS, 4_096),
    ] {
        let fixture = SparseXorFixture::prepare(SparseXorKind::Row, work_items)
            .expect("source-owned row fixture");
        assert_eq!(fixture.sweeps(), expected_sweeps);
        assert_eq!(fixture.input_bytes, 28_008);
        assert_eq!(semantic_digest(fixture.input_digest), ROW_INPUT_DIGEST);
        let table = fixture.row_state().expect("row fixture state");
        assert_eq!(table.len(), 1_000);
        assert_eq!(
            table.first().map(SparseXorVec::items),
            Some([0, 1, 4, 8, 15].as_slice())
        );
        assert_eq!(
            table.get(999).map(SparseXorVec::items),
            Some([999, 1_000, 1_003, 1_007, 1_014].as_slice())
        );
        assert!(table.iter().all(|row| row.items().len() == 5));
    }

    let mut odd = SparseXorFixture::prepare(SparseXorKind::Row, SPARSE_ROW_BASE_WORK_ITEMS)
        .expect("odd row fixture");
    odd.execute(1);
    let table = odd.row_state().expect("row fixture state");
    assert_eq!(
        table.first().map(SparseXorVec::items),
        Some([0, 2, 4, 5, 8, 9, 15, 16].as_slice())
    );
    assert_eq!(
        table.get(1).map(SparseXorVec::items),
        Some([1, 2, 5, 9, 16].as_slice())
    );
    assert_eq!(
        semantic_digest(
            odd.output_digest(1, SPARSE_ROW_BASE_WORK_ITEMS)
                .expect("odd output")
        ),
        ROW_ODD_OUTPUT_DIGEST,
    );

    let mut even = SparseXorFixture::prepare(SparseXorKind::Row, SPARSE_ROW_BASE_WORK_ITEMS)
        .expect("even row fixture");
    even.execute(2);
    assert_eq!(
        semantic_digest(
            even.output_digest(2, SPARSE_ROW_BASE_WORK_ITEMS)
                .expect("even output")
        ),
        ROW_EVEN_OUTPUT_DIGEST,
    );

    let mut maximum = SparseXorFixture::prepare(SparseXorKind::Row, SPARSE_ROW_MAX_WORK_ITEMS)
        .expect("maximum row fixture");
    maximum.execute(1);
    assert_eq!(
        semantic_digest(
            maximum
                .output_digest(1, SPARSE_ROW_MAX_WORK_ITEMS)
                .expect("maximum output")
        ),
        ROW_MAX_OUTPUT_DIGEST,
    );
}

#[test]
fn sparse_item_fixture_binds_exact_input_scales_and_states() {
    for (work_items, expected_sweeps) in [
        (SPARSE_ITEM_BASE_WORK_ITEMS, 1),
        (SPARSE_ITEM_BASE_WORK_ITEMS * 64, 64),
        (SPARSE_ITEM_MAX_WORK_ITEMS, 4_096),
    ] {
        let fixture = SparseXorFixture::prepare(SparseXorKind::Item, work_items)
            .expect("source-owned item fixture");
        assert_eq!(fixture.sweeps(), expected_sweeps);
        assert_eq!(fixture.input_bytes, 36);
        assert_eq!(semantic_digest(fixture.input_digest), ITEM_INPUT_DIGEST);
        let buffer = fixture.item_state().expect("item fixture state");
        assert!(buffer.is_empty());
    }

    let mut odd = SparseXorFixture::prepare(SparseXorKind::Item, SPARSE_ITEM_BASE_WORK_ITEMS)
        .expect("odd item fixture");
    odd.execute(1);
    let buffer = odd.item_state().expect("item fixture state");
    assert_eq!(buffer.items(), [2, 3, 6, 9, 10]);
    assert_eq!(
        semantic_digest(
            odd.output_digest(1, SPARSE_ITEM_BASE_WORK_ITEMS)
                .expect("odd output")
        ),
        ITEM_ODD_OUTPUT_DIGEST,
    );

    let mut even = SparseXorFixture::prepare(SparseXorKind::Item, SPARSE_ITEM_BASE_WORK_ITEMS)
        .expect("even item fixture");
    even.execute(2);
    assert_eq!(
        semantic_digest(
            even.output_digest(2, SPARSE_ITEM_BASE_WORK_ITEMS)
                .expect("even output")
        ),
        ITEM_EVEN_OUTPUT_DIGEST,
    );

    let mut maximum = SparseXorFixture::prepare(SparseXorKind::Item, SPARSE_ITEM_MAX_WORK_ITEMS)
        .expect("maximum item fixture");
    maximum.execute(1);
    assert_eq!(
        semantic_digest(
            maximum
                .output_digest(1, SPARSE_ITEM_MAX_WORK_ITEMS)
                .expect("maximum output")
        ),
        ITEM_MAX_OUTPUT_DIGEST,
    );
}

#[test]
fn sparse_xor_fixtures_reject_partial_and_over_cap_work_before_allocation() {
    assert!(matches!(
        SparseXorFixture::prepare(SparseXorKind::Row, SPARSE_ROW_BASE_WORK_ITEMS + 1),
        Err(WorkerError::SparseXorWorkShape { .. })
    ));
    assert!(matches!(
        SparseXorFixture::prepare(
            SparseXorKind::Row,
            SPARSE_ROW_MAX_WORK_ITEMS + SPARSE_ROW_BASE_WORK_ITEMS,
        ),
        Err(WorkerError::SparseXorWorkLimit { .. })
    ));
    assert!(matches!(
        SparseXorFixture::prepare(SparseXorKind::Item, SPARSE_ITEM_BASE_WORK_ITEMS + 1),
        Err(WorkerError::SparseXorWorkShape { .. })
    ));
    assert!(matches!(
        SparseXorFixture::prepare(
            SparseXorKind::Item,
            SPARSE_ITEM_MAX_WORK_ITEMS + SPARSE_ITEM_BASE_WORK_ITEMS,
        ),
        Err(WorkerError::SparseXorWorkLimit { .. })
    ));
}

#[cfg(feature = "count-allocations")]
#[test]
fn sparse_xor_timed_workloads_allocate_nothing_after_capacity_priming() {
    for (kind, scales) in [
        (
            SparseXorKind::Row,
            [
                SPARSE_ROW_BASE_WORK_ITEMS,
                SPARSE_ROW_BASE_WORK_ITEMS * 64,
                SPARSE_ROW_MAX_WORK_ITEMS,
            ],
        ),
        (
            SparseXorKind::Item,
            [
                SPARSE_ITEM_BASE_WORK_ITEMS,
                SPARSE_ITEM_BASE_WORK_ITEMS * 64,
                SPARSE_ITEM_MAX_WORK_ITEMS,
            ],
        ),
    ] {
        for work_items in scales {
            let mut fixture =
                SparseXorFixture::prepare(kind, work_items).expect("source-owned fixture");
            let allocations = allocation_counter::measure(|| fixture.execute(2));
            assert_eq!(
                allocations.count_total, 0,
                "kind={kind:?} work_items={work_items} {allocations:?}",
            );
            assert_eq!(
                allocations.bytes_total, 0,
                "kind={kind:?} work_items={work_items} {allocations:?}",
            );
        }
    }
}
