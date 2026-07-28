#![allow(
    clippy::expect_used,
    reason = "resource-contract fixtures use direct setup assertions"
)]

use stab_records::{
    DetsLayout, RecordResult, SampleFormat, for_each_dets_packed_record, for_each_dets_record,
    for_each_dets_sparse_shot, for_each_dets_token_record, for_each_packed_record,
    for_each_ptb64_record_all, for_each_record, write_ptb64_records_checked,
};

#[test]
fn dense_text_readers_bound_scratch_by_width_not_duplicate_count() -> RecordResult<()> {
    const DUPLICATES: usize = 16_384;
    let mut duplicate_hits = b"0".to_vec();
    let mut duplicate_dets = b"shot M0".to_vec();
    for _ in 1..DUPLICATES {
        duplicate_hits.extend_from_slice(b",0");
        duplicate_dets.extend_from_slice(b" M0");
    }
    duplicate_hits.push(b'\n');
    duplicate_dets.push(b'\n');

    assert_text_duplicate_count_independent(
        "HITS dense",
        || for_each_record(b"0\n", SampleFormat::Hits, 1, |_| Ok(())),
        || for_each_record(&duplicate_hits, SampleFormat::Hits, 1, |_| Ok(())),
    );
    assert_text_duplicate_count_independent(
        "HITS packed",
        || for_each_packed_record(b"0\n", SampleFormat::Hits, 1, |_| Ok(())),
        || for_each_packed_record(&duplicate_hits, SampleFormat::Hits, 1, |_| Ok(())),
    );

    let layout = DetsLayout::measurement_only(1);
    assert_text_duplicate_count_independent(
        "DETS dense",
        || for_each_dets_record(b"shot M0\n", layout, |_| Ok(())),
        || for_each_dets_record(&duplicate_dets, layout, |_| Ok(())),
    );
    assert_text_duplicate_count_independent(
        "DETS packed",
        || for_each_dets_packed_record(b"shot M0\n", layout, |_| Ok(())),
        || for_each_dets_packed_record(&duplicate_dets, layout, |_| Ok(())),
    );
    Ok(())
}

#[test]
fn dets_visitors_keep_allocation_bounded_by_width_not_record_count() -> RecordResult<()> {
    let layout = DetsLayout::try_new(128, 64, 32)?;
    let one_record = b"shot M0 M127 D0 D63 L0 L31\n".to_vec();
    let many_records = one_record.repeat(256);

    let dense_one = allocation_counter::measure(|| {
        for_each_dets_record(&one_record, layout, |_| Ok(())).expect("dense one");
    });
    let dense_many = allocation_counter::measure(|| {
        for_each_dets_record(&many_records, layout, |_| Ok(())).expect("dense many");
    });
    assert_record_count_independent("dense", dense_one, dense_many, layout.total_bits());

    let packed_one = allocation_counter::measure(|| {
        for_each_dets_packed_record(&one_record, layout, |_| Ok(())).expect("packed one");
    });
    let packed_many = allocation_counter::measure(|| {
        for_each_dets_packed_record(&many_records, layout, |_| Ok(())).expect("packed many");
    });
    assert_record_count_independent("packed", packed_one, packed_many, layout.total_bits());

    let token_one = allocation_counter::measure(|| {
        for_each_dets_token_record(&one_record, layout, |_| Ok(())).expect("token one");
    });
    let token_many = allocation_counter::measure(|| {
        for_each_dets_token_record(&many_records, layout, |_| Ok(())).expect("token many");
    });
    assert_record_count_independent("token", token_one, token_many, layout.total_bits());

    let sparse_one = allocation_counter::measure(|| {
        for_each_dets_sparse_shot(&one_record, layout, |_| Ok(())).expect("sparse one");
    });
    let sparse_many = allocation_counter::measure(|| {
        for_each_dets_sparse_shot(&many_records, layout, |_| Ok(())).expect("sparse many");
    });
    assert_record_count_independent("sparse", sparse_one, sparse_many, layout.total_bits());
    Ok(())
}

#[test]
fn ptb64_streaming_allocation_is_independent_of_group_count() -> RecordResult<()> {
    const WIDTH: usize = 193;
    let records = (0..256)
        .map(|shot| {
            (0..WIDTH)
                .map(|bit| (shot * 17 + bit * 31).is_multiple_of(97))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let one_group = write_ptb64_records_checked(
        records
            .get(..64)
            .expect("fixture contains one complete PTB64 group"),
    )?;
    let four_groups = write_ptb64_records_checked(&records)?;

    let one = allocation_counter::measure(|| {
        for_each_ptb64_record_all(&one_group, WIDTH, |_| Ok(())).expect("one PTB64 group");
    });
    let four = allocation_counter::measure(|| {
        for_each_ptb64_record_all(&four_groups, WIDTH, |_| Ok(())).expect("four PTB64 groups");
    });

    assert_record_count_independent("ptb64", one, four, WIDTH);
    Ok(())
}

fn assert_record_count_independent(
    reader: &str,
    one: allocation_counter::AllocationInfo,
    many: allocation_counter::AllocationInfo,
    width: usize,
) {
    assert_eq!(
        many.count_total, one.count_total,
        "{reader} allocation count grew with records: one={one:?}, many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "{reader} allocated bytes grew with records: one={one:?}, many={many:?}"
    );
    let generous_width_bound = u64::try_from(width)
        .expect("test width fits u64")
        .saturating_mul(64)
        .saturating_add(4_096);
    assert!(
        many.bytes_max <= generous_width_bound,
        "{reader} peak allocation exceeded width-derived bound {generous_width_bound}: {many:?}"
    );
}

fn assert_text_duplicate_count_independent(
    reader: &str,
    short: impl FnOnce() -> RecordResult<()>,
    duplicate_heavy: impl FnOnce() -> RecordResult<()>,
) {
    let short = allocation_counter::measure(|| short().expect("short text record"));
    let duplicate_heavy =
        allocation_counter::measure(|| duplicate_heavy().expect("duplicate-heavy text record"));
    assert_record_count_independent(reader, short, duplicate_heavy, 1);
}
