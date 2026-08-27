#![allow(
    clippy::expect_used,
    reason = "resource-contract fixtures use direct setup assertions"
)]

use std::hint::black_box;

use stab_records::{
    DetsLayout, RecordFormat, RecordResult, for_each_dets_packed_record, for_each_dets_record,
    for_each_dets_sparse_shot, for_each_dets_token_record, for_each_packed_record,
    for_each_ptb64_record_all, for_each_record, for_each_sparse_record,
    write_ptb64_records_checked, write_records,
};

#[test]
fn dense_and_packed_streaming_allocations_follow_width_not_record_count() -> RecordResult<()> {
    const WIDTH: usize = 193;
    const RECORDS: usize = 257;
    let record = (0usize..WIDTH)
        .map(|bit| (bit * 17).is_multiple_of(43))
        .collect::<Vec<_>>();

    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
    ] {
        let one = write_records(std::slice::from_ref(&record), format)?;
        let many = one.repeat(RECORDS);
        assert_width_bounded(
            &format_name(format, "dense"),
            WIDTH,
            || {
                for_each_record(&one, format, WIDTH, |record| {
                    black_box(record.len());
                    Ok(())
                })
            },
            || {
                for_each_record(&many, format, WIDTH, |record| {
                    black_box(record.len());
                    Ok(())
                })
            },
        );
        assert_width_bounded(
            &format_name(format, "packed"),
            WIDTH,
            || {
                for_each_packed_record(&one, format, WIDTH, |record| {
                    black_box(record.len());
                    Ok(())
                })
            },
            || {
                for_each_packed_record(&many, format, WIDTH, |record| {
                    black_box(record.len());
                    Ok(())
                })
            },
        );
    }

    let mut duplicate_hits = b"0".to_vec();
    for _ in 1..16_384 {
        duplicate_hits.extend_from_slice(b",0");
    }
    duplicate_hits.push(b'\n');
    let one_hit = b"0\n";
    assert_width_bounded(
        "HITS duplicate-heavy dense",
        1,
        || for_each_record(one_hit, RecordFormat::Hits, 1, |_| Ok(())),
        || for_each_record(&duplicate_hits, RecordFormat::Hits, 1, |_| Ok(())),
    );
    assert_width_bounded(
        "HITS duplicate-heavy packed",
        1,
        || for_each_packed_record(one_hit, RecordFormat::Hits, 1, |_| Ok(())),
        || for_each_packed_record(&duplicate_hits, RecordFormat::Hits, 1, |_| Ok(())),
    );

    let layout = DetsLayout::try_new(128, 64, 32)?;
    let one_dets_record = b"shot M0 M127 D0 D63 L0 L31\n".to_vec();
    let many_dets_records = one_dets_record.repeat(RECORDS);
    assert_width_bounded(
        "DETS dense",
        layout.total_bits(),
        || for_each_dets_record(&one_dets_record, layout, |_| Ok(())),
        || for_each_dets_record(&many_dets_records, layout, |_| Ok(())),
    );
    assert_width_bounded(
        "DETS packed",
        layout.total_bits(),
        || for_each_dets_packed_record(&one_dets_record, layout, |_| Ok(())),
        || for_each_dets_packed_record(&many_dets_records, layout, |_| Ok(())),
    );

    let duplicate_dets = duplicate_dets_record(16_384);
    assert_width_bounded(
        "DETS duplicate-heavy dense",
        layout.total_bits(),
        || for_each_dets_record(b"shot D0\n", layout, |_| Ok(())),
        || for_each_dets_record(&duplicate_dets, layout, |_| Ok(())),
    );
    assert_width_bounded(
        "DETS duplicate-heavy packed",
        layout.total_bits(),
        || for_each_dets_packed_record(b"shot D0\n", layout, |_| Ok(())),
        || for_each_dets_packed_record(&duplicate_dets, layout, |_| Ok(())),
    );

    let ptb64_records = (0usize..256)
        .map(|shot| {
            (0usize..WIDTH)
                .map(|bit| (shot * 17 + bit * 31).is_multiple_of(97))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ptb64 = write_ptb64_records_checked(&ptb64_records)?;
    let one_ptb64_group = ptb64
        .get(..WIDTH * 8)
        .expect("four PTB64 groups contain a complete first group");
    assert_width_bounded(
        "PTB64 dense",
        WIDTH,
        || {
            for_each_ptb64_record_all(one_ptb64_group, WIDTH, |record| {
                black_box(record.len());
                Ok(())
            })
        },
        || {
            for_each_ptb64_record_all(&ptb64, WIDTH, |record| {
                black_box(record.len());
                Ok(())
            })
        },
    );
    Ok(())
}

#[test]
fn sparse_and_token_streaming_allocations_follow_largest_record_not_record_count()
-> RecordResult<()> {
    const WIDTH: usize = 193;
    const RECORDS: usize = 257;
    let record = (0usize..WIDTH)
        .map(|bit| (bit * 17).is_multiple_of(43))
        .collect::<Vec<_>>();

    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let one = write_records(std::slice::from_ref(&record), format)?;
        let many = one.repeat(RECORDS);
        assert_largest_record_bounded(
            &format_name(format, "sparse"),
            one.len(),
            || {
                for_each_sparse_record(&one, format, WIDTH, |hits| {
                    black_box(hits.len());
                    Ok(())
                })
            },
            || {
                for_each_sparse_record(&many, format, WIDTH, |hits| {
                    black_box(hits.len());
                    Ok(())
                })
            },
        );
    }

    let mut duplicate_hits = b"0".to_vec();
    for _ in 1..4_096 {
        duplicate_hits.extend_from_slice(b",0");
    }
    duplicate_hits.push(b'\n');
    let many_duplicate_hits = duplicate_hits.repeat(64);
    assert_largest_record_bounded(
        "HITS duplicate-heavy sparse",
        duplicate_hits.len(),
        || for_each_sparse_record(&duplicate_hits, RecordFormat::Hits, 1, |_| Ok(())),
        || for_each_sparse_record(&many_duplicate_hits, RecordFormat::Hits, 1, |_| Ok(())),
    );

    let layout = DetsLayout::try_new(128, 64, 32)?;
    let one_record = duplicate_dets_record(4_096);
    let many_records = one_record.repeat(64);
    assert_largest_record_bounded(
        "DETS tokens",
        one_record.len(),
        || for_each_dets_token_record(&one_record, layout, |_| Ok(())),
        || for_each_dets_token_record(&many_records, layout, |_| Ok(())),
    );
    assert_largest_record_bounded(
        "DETS sparse",
        one_record.len(),
        || for_each_dets_sparse_shot(&one_record, layout, |_| Ok(())),
        || for_each_dets_sparse_shot(&many_records, layout, |_| Ok(())),
    );
    Ok(())
}

fn assert_largest_record_bounded(
    reader: &str,
    largest_record_bytes: usize,
    mut read_one: impl FnMut() -> RecordResult<()>,
    mut read_many: impl FnMut() -> RecordResult<()>,
) {
    read_one().expect("warm one-record streaming reader");
    read_many().expect("warm many-record streaming reader");
    let one = allocation_counter::measure(|| read_one().expect("measure one-record reader"));
    let many = allocation_counter::measure(|| read_many().expect("measure many-record reader"));
    let record_bound = u64::try_from(largest_record_bytes)
        .expect("test record length fits u64")
        .saturating_mul(64)
        .saturating_add(4_096);
    assert_eq!(
        many.count_total, one.count_total,
        "{reader} allocation count grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "{reader} allocated bytes grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_current, 0,
        "{reader} retained scratch after returning: {many:?}"
    );
    assert!(
        many.bytes_max <= record_bound,
        "{reader} exceeded largest-record-derived scratch bound {record_bound}: {many:?}"
    );
}

fn duplicate_dets_record(repetitions: usize) -> Vec<u8> {
    let mut record = b"shot".to_vec();
    for index in 0..repetitions {
        record.extend_from_slice(match index % 3 {
            0 => b" M0",
            1 => b" D0",
            _ => b" L0",
        });
    }
    record.push(b'\n');
    record
}

fn assert_width_bounded(
    reader: &str,
    width: usize,
    mut read_one: impl FnMut() -> RecordResult<()>,
    mut read_many: impl FnMut() -> RecordResult<()>,
) {
    read_one().expect("warm one-record streaming reader");
    read_many().expect("warm many-record streaming reader");
    let one = allocation_counter::measure(|| read_one().expect("measure one-record reader"));
    let many = allocation_counter::measure(|| read_many().expect("measure many-record reader"));
    let width_bound = u64::try_from(width)
        .expect("test width fits u64")
        .saturating_mul(16)
        .saturating_add(4_096);
    assert_eq!(
        many.count_total, one.count_total,
        "{reader} allocation count grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "{reader} allocated bytes grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_current, 0,
        "{reader} retained scratch after returning: {many:?}"
    );
    assert!(
        many.bytes_max <= width_bound,
        "{reader} exceeded width-derived scratch bound {width_bound}: {many:?}"
    );
}

fn format_name(format: RecordFormat, representation: &str) -> String {
    format!("{format:?} {representation}")
}
