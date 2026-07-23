#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "compatibility tests use Result propagation for setup and direct assertions for contract diagnostics"
)]

use stab_core::{
    CircuitError, CircuitResult,
    bits::BitSlice,
    result_formats::{
        DetsLayout, DetsResultType, DetsToken, SampleFormat, SparseShot, read_dets_records,
        read_measurement_records, read_records,
    },
    result_streaming::{
        for_each_dets_packed_record, for_each_dets_record, for_each_dets_sparse_shot,
        for_each_dets_token_record, for_each_packed_record, for_each_record,
        for_each_sparse_record,
    },
};

#[test]
fn dets_public_layout_and_token_value_contract() -> CircuitResult<()> {
    let layout = DetsLayout::try_new(2, 3, 5)?;
    assert_eq!(layout.measurements(), 2);
    assert_eq!(layout.detectors(), 3);
    assert_eq!(layout.observables(), 5);
    assert_eq!(layout.total_bits(), 10);
    assert_eq!(DetsLayout::measurement_only(7).total_bits(), 7);
    assert!(DetsLayout::try_new(usize::MAX, 1, 0).is_err());

    for (result_type, prefix) in [
        (DetsResultType::Measurement, b'M'),
        (DetsResultType::Detector, b'D'),
        (DetsResultType::Observable, b'L'),
    ] {
        let token = DetsToken::new(result_type, 4);
        assert_eq!(token.result_type(), result_type);
        assert_eq!(token.index(), 4);
        assert_eq!(result_type.prefix(), prefix);
    }
    Ok(())
}

#[test]
fn zero_one_requires_lf_or_crlf_after_every_record() -> CircuitResult<()> {
    for input in [b"01".as_slice(), b"01\r", b"01\rX"] {
        assert_all_width_readers_reject(input, SampleFormat::ZeroOne, 2);
    }

    assert_eq!(
        read_records(b"01\n10\r\n", SampleFormat::ZeroOne, 2)?,
        vec![vec![false, true], vec![true, false]]
    );
    assert_eq!(
        read_records(b"\n\r\n", SampleFormat::ZeroOne, 0)?,
        vec![Vec::<bool>::new(), Vec::new()]
    );
    assert_eq!(
        read_records(b"", SampleFormat::ZeroOne, 0)?,
        Vec::<Vec<bool>>::new()
    );
    Ok(())
}

#[test]
fn hits_requires_strict_commas_and_terminated_records() -> CircuitResult<()> {
    for input in [
        b"1,,2\n".as_slice(),
        b"1,\n",
        b",1\n",
        b"1, 2\n",
        b"1,\t2\n",
        b"1,2",
        b"1\r",
        b"18446744073709551616\n",
        b"3\n",
    ] {
        assert_all_width_readers_reject(input, SampleFormat::Hits, 3);
    }

    assert_eq!(
        read_records(b"1,1\r\n\n", SampleFormat::Hits, 3)?,
        vec![vec![false, false, false], vec![false, false, false]]
    );
    assert_eq!(
        read_records(b"1\r,3\n", SampleFormat::Hits, 4)?,
        vec![vec![false, true, false, true]]
    );
    let mut sparse = Vec::new();
    for_each_sparse_record(b"1,1\n", SampleFormat::Hits, 3, |hits| {
        sparse.push(hits.to_vec());
        Ok(())
    })?;
    assert_eq!(sparse, vec![vec![1, 1]]);
    Ok(())
}

#[test]
fn dets_layout_keeps_namespaces_distinct_and_dense_duplicates_set() -> CircuitResult<()> {
    let layout = DetsLayout::try_new(2, 2, 2)?;
    let records = read_dets_records(b"shot M0 D0 L0 D0 L0\n", layout)?;
    assert_eq!(records, vec![vec![true, false, true, false, true, false]]);

    let mut dense = Vec::new();
    for_each_dets_record(b"shot M0 D0 L0 D0 L0\n", layout, |record| {
        dense.push(record.to_vec());
        Ok(())
    })?;
    assert_eq!(dense, records);

    let mut packed = Vec::new();
    for_each_dets_packed_record(b"shot M0 D0 L0 D0 L0\n", layout, |record| {
        packed.push(bits(record)?);
        Ok(())
    })?;
    assert_eq!(packed, records);
    Ok(())
}

#[test]
fn typed_dets_visitors_preserve_duplicates_and_sparse_shot_observable_parity() -> CircuitResult<()>
{
    let layout = DetsLayout::try_new(2, 2, 2)?;
    let input = b"shot M1 M1 D0 D0 L1 L1 L0\n";
    let mut tokens = Vec::new();
    for_each_dets_token_record(input, layout, |record| {
        tokens.push(record.to_vec());
        Ok(())
    })?;
    assert_eq!(
        tokens,
        vec![vec![
            DetsToken::new(DetsResultType::Measurement, 1),
            DetsToken::new(DetsResultType::Measurement, 1),
            DetsToken::new(DetsResultType::Detector, 0),
            DetsToken::new(DetsResultType::Detector, 0),
            DetsToken::new(DetsResultType::Observable, 1),
            DetsToken::new(DetsResultType::Observable, 1),
            DetsToken::new(DetsResultType::Observable, 0),
        ]]
    );

    let mut shots = Vec::new();
    for_each_dets_sparse_shot(input, layout, |shot| {
        shots.push(shot.clone());
        Ok(())
    })?;
    assert_eq!(
        shots,
        vec![SparseShot::new(vec![1, 1, 2, 2], vec![true, false])]
    );
    Ok(())
}

#[test]
fn width_based_dets_readers_are_measurement_only() -> CircuitResult<()> {
    assert_eq!(
        read_records(b"shot M0 M0\n", SampleFormat::Dets, 1)?,
        vec![vec![true]]
    );
    for input in [b"shot D0\n".as_slice(), b"shot L0\n"] {
        assert_all_width_readers_reject(input, SampleFormat::Dets, 1);
    }
    Ok(())
}

#[test]
fn dets_uses_exact_separators_and_pinned_eof_rule() -> CircuitResult<()> {
    let layout = DetsLayout::try_new(1, 1, 1)?;
    for input in [
        b"shotM0\n".as_slice(),
        b"shot  M0\n",
        b"shot\tM0\n",
        b"shot M0 \n",
        b"shot Q0\n",
        b"shot M\n",
        b"shot M1\n",
        b"shot D1\n",
        b"shot L1\n",
        b"shot M18446744073709551616\n",
    ] {
        assert!(read_dets_records(input, layout).is_err(), "{input:?}");
    }

    assert_eq!(
        read_dets_records(b" \r\n\tshot M0 D0 L0", layout)?,
        vec![vec![true, true, true]]
    );
    assert_eq!(
        read_dets_records(b"shot M0\r", layout)?,
        vec![vec![true, false, false]]
    );
    assert_eq!(
        read_dets_records(b"shot\r M0\r D0\r L0\n", layout)?,
        vec![vec![true, true, true]]
    );
    Ok(())
}

#[test]
fn dets_visitors_stop_immediately_on_visitor_error() -> CircuitResult<()> {
    let layout = DetsLayout::try_new(2, 1, 1)?;
    let input = b"shot M0 D0\nshot M1 L0\n";
    let mut dense_calls = 0usize;
    let result = for_each_dets_record(input, layout, |record| {
        assert_eq!(record.len(), 4);
        dense_calls += 1;
        Err(CircuitError::InvalidResultFormat {
            message: "stop".to_string(),
        })
    });
    assert!(result.is_err());
    assert_eq!(dense_calls, 1);

    let mut packed_calls = 0usize;
    let result = for_each_dets_packed_record(input, layout, |record| {
        assert_eq!(record.len(), 4);
        packed_calls += 1;
        Err(CircuitError::InvalidResultFormat {
            message: "stop".to_string(),
        })
    });
    assert!(result.is_err());
    assert_eq!(packed_calls, 1);

    let mut token_calls = 0usize;
    let result = for_each_dets_token_record(input, layout, |record| {
        assert_eq!(record.len(), 2);
        token_calls += 1;
        Err(CircuitError::InvalidResultFormat {
            message: "stop".to_string(),
        })
    });
    assert!(result.is_err());
    assert_eq!(token_calls, 1);

    let mut sparse_calls = 0usize;
    let result = for_each_dets_sparse_shot(input, layout, |shot| {
        assert_eq!(shot.hits, [0, 2]);
        assert_eq!(shot.obs_mask, [false]);
        sparse_calls += 1;
        Err(CircuitError::InvalidResultFormat {
            message: "stop".to_string(),
        })
    });
    assert!(result.is_err());
    assert_eq!(sparse_calls, 1);
    Ok(())
}

#[test]
fn dets_visitors_keep_allocation_bounded_by_width_not_record_count() -> CircuitResult<()> {
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

fn assert_all_width_readers_reject(input: &[u8], format: SampleFormat, width: usize) {
    assert!(read_records(input, format, width).is_err());
    assert!(read_measurement_records(input, format, width).is_err());
    assert!(for_each_record(input, format, width, |_| Ok(())).is_err());
    assert!(for_each_packed_record(input, format, width, |_| Ok(())).is_err());
    assert!(for_each_sparse_record(input, format, width, |_| Ok(())).is_err());
}

fn bits(record: BitSlice<'_>) -> CircuitResult<Vec<bool>> {
    (0..record.len())
        .map(|index| {
            record
                .get(index)
                .ok_or_else(|| CircuitError::InvalidResultFormat {
                    message: format!("packed test record index {index} was out of range"),
                })
        })
        .collect()
}
