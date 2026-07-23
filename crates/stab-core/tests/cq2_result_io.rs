#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "qualification tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    BitSlice, CircuitError, CircuitResult, SampleFormat,
    result_formats::{
        MeasureRecordBatchWriter, MeasureRecordWriter, SparseShot, ptb64_record_count,
        read_measurement_records, read_ptb64_records, read_ptb64_records_all, read_records,
        validate_ptb64_shot_count, write_ptb64_records_checked, write_records,
    },
    result_streaming::{
        for_each_packed_record, for_each_ptb64_record, for_each_ptb64_record_all, for_each_record,
        for_each_sparse_record, ptb64_record_count as streaming_ptb64_record_count,
    },
};

fn unpack_bytes(bytes: &[u8], width: usize) -> Vec<bool> {
    (0..width)
        .map(|index| bytes[index / 8] & (1 << (index % 8)) != 0)
        .collect()
}

fn deterministic_records(shots: usize, width: usize) -> Vec<Vec<bool>> {
    (0..shots)
        .map(|shot| {
            (0..width)
                .map(|bit| (shot * 17 + bit * 29 + shot * bit) % 31 < 7)
                .collect()
        })
        .collect()
}

fn bitslice_to_vec(bits: BitSlice<'_>) -> Vec<bool> {
    (0..bits.len())
        .map(|index| bits.get(index).unwrap())
        .collect()
}

fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

#[test]
fn cq_result_sample_format_value_contract_matches_stim() {
    let module_path_value: stab_core::result_formats::SampleFormat = SampleFormat::ZeroOne;
    assert_eq!(module_path_value, SampleFormat::ZeroOne);
    let formats = [
        SampleFormat::ZeroOne,
        SampleFormat::B8,
        SampleFormat::R8,
        SampleFormat::Hits,
        SampleFormat::Dets,
    ];
    let copied = formats;
    let cloned = formats.map(|format| clone_value(&format));
    assert_eq!(formats, copied);
    assert_eq!(formats, cloned);
    assert_eq!(
        formats.map(|format| format!("{format:?}")),
        ["ZeroOne", "B8", "R8", "Hits", "Dets"]
    );
    assert_ne!(SampleFormat::ZeroOne, SampleFormat::B8);
}

#[test]
fn cq_result_writer_exact_format_bytes_match_stim() {
    let bytes = [0xF8];

    let value = MeasureRecordWriter::new(SampleFormat::ZeroOne);
    assert_eq!(value, value.clone());
    assert!(format!("{value:?}").contains("MeasureRecordWriter"));

    let mut writer = MeasureRecordWriter::with_capacity(SampleFormat::ZeroOne, 18);
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.write_bytes(&bytes);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(writer.into_bytes(), b"000111110000111111\n");

    let mut writer = MeasureRecordWriter::new(SampleFormat::B8);
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.write_bytes(&bytes);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(writer.into_bytes(), [0xF8, 0xF0, 0x03]);

    let mut writer = MeasureRecordWriter::new(SampleFormat::Hits);
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.write_bytes(&bytes);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(writer.into_bytes(), b"3,4,5,6,7,12,13,14,15,16,17\n");

    let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
    writer.begin_result_type(b'D');
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.write_bytes(&bytes);
    writer.begin_result_type(b'L');
    writer.write_bit(false);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(
        writer.into_bytes(),
        b"shot D3 D4 D5 D6 D7 D12 D13 D14 D15 D16 L1\n"
    );

    let mut writer = MeasureRecordWriter::new(SampleFormat::R8);
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.write_bytes(&bytes);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(writer.into_bytes(), [3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn cq_result_writer_record_boundaries_and_bit_slices_match_stim() {
    let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
    writer.write_end();
    writer.write_end();
    writer.begin_result_type(b'D');
    writer.write_bits(&[false, false, true]);
    writer.begin_result_type(b'L');
    writer.write_bits(&[false, true]);
    writer.write_end();
    writer.begin_result_type(b'D');
    writer.write_bits(&[true, false, false, true]);
    writer.begin_result_type(b'L');
    writer.write_bits(&[true, false, true]);
    writer.write_end();
    assert_eq!(
        writer.into_bytes(),
        b"shot\nshot\nshot D2 L1\nshot D0 D3 L0 L2\n"
    );

    let mut writer = MeasureRecordWriter::new(SampleFormat::R8);
    writer.write_bits(&vec![false; 512]);
    writer.write_bit(true);
    writer.write_bits(&[false; 32]);
    writer.write_end();
    assert_eq!(writer.into_bytes(), [255, 255, 2, 32]);

    for (bits, expected_01, expected_b8, expected_r8) in [
        (
            unpack_bytes(&[0x00, 0xFF], 11),
            b"00000000111\n".as_slice(),
            [0x00, 0x07].as_slice(),
            [8, 0, 0, 0].as_slice(),
        ),
        (
            unpack_bytes(&[0xFF, 0x00], 11),
            b"11111111000\n".as_slice(),
            [0xFF, 0x00].as_slice(),
            [0, 0, 0, 0, 0, 0, 0, 0, 3].as_slice(),
        ),
    ] {
        assert_eq!(
            write_records(std::slice::from_ref(&bits), SampleFormat::ZeroOne),
            expected_01
        );
        assert_eq!(
            write_records(std::slice::from_ref(&bits), SampleFormat::B8),
            expected_b8
        );
        assert_eq!(
            write_records(std::slice::from_ref(&bits), SampleFormat::R8),
            expected_r8
        );
    }
}

#[test]
fn cq_result_batch_writer_small_table_contract_matches_stim() {
    let columns = [
        [false, false, false, false, false],
        [true, true, true, true, true],
        [false, false, false, false, false],
        [false, false, false, false, false],
    ];

    for (format, expected) in [
        (
            SampleFormat::ZeroOne,
            b"0100\n0100\n0100\n0100\n0100\n".as_slice(),
        ),
        (SampleFormat::Hits, b"1\n1\n1\n1\n1\n".as_slice()),
        (
            SampleFormat::Dets,
            b"shot M1\nshot M1\nshot M1\nshot M1\nshot M1\n".as_slice(),
        ),
        (SampleFormat::R8, [1, 2, 1, 2, 1, 2, 1, 2, 1, 2].as_slice()),
        (SampleFormat::B8, [2, 2, 2, 2, 2].as_slice()),
    ] {
        let mut writer = MeasureRecordBatchWriter::new(5, format);
        for column in columns {
            writer.batch_write_bit(&column).unwrap();
        }
        let cloned = writer.clone();
        assert_eq!(writer, cloned);
        assert!(format!("{writer:?}").contains("MeasureRecordBatchWriter"));
        assert_eq!(writer.write_end(), expected, "{format:?}");
    }

    let records = (0..64)
        .map(|shot| {
            columns
                .map(|column| column.get(shot).copied().unwrap_or(false))
                .to_vec()
        })
        .collect::<Vec<_>>();
    let encoded = write_ptb64_records_checked(&records).unwrap();
    assert_eq!(encoded.len(), 32);
    assert_eq!(&encoded[8..16], &0x1F_u64.to_le_bytes());
}

#[test]
fn cq_result_large_table_reference_and_format_contract_matches_stim() {
    let mut reference = vec![false; 100];
    for index in [2, 3, 5, 7, 11] {
        reference[index] = true;
    }
    let mut records = vec![reference.clone(), reference.clone()];
    records[1][7] = false;

    assert_eq!(
        write_records(&records, SampleFormat::Hits),
        b"2,3,5,7,11\n2,3,5,11\n"
    );
    assert_eq!(
        write_records(&records, SampleFormat::B8),
        [
            0xAC, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x2C, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0,
        ]
    );
    assert_eq!(
        write_records(&records, SampleFormat::R8),
        [2, 0, 1, 1, 3, 88, 2, 0, 1, 5, 88]
    );

    let mut dets = MeasureRecordWriter::new(SampleFormat::Dets);
    for record in &records {
        dets.begin_result_type(b'D');
        dets.write_bits(&record[..5]);
        dets.begin_result_type(b'L');
        dets.write_bits(&record[5..]);
        dets.write_end();
    }
    assert_eq!(
        dets.into_bytes(),
        b"shot D2 D3 L0 L2 L6\nshot D2 D3 L0 L6\n"
    );

    let mut ptb64_records = vec![reference; 64];
    ptb64_records[1][7] = false;
    let ptb64 = write_ptb64_records_checked(&ptb64_records).unwrap();
    for (bit, word) in ptb64.chunks_exact(8).enumerate() {
        let word = u64::from_le_bytes(word.try_into().unwrap());
        let expected = match bit {
            2 | 3 | 5 | 11 => u64::MAX,
            7 => u64::MAX ^ 2,
            _ => 0,
        };
        assert_eq!(word, expected, "bit {bit}");
    }
}

#[test]
fn cq_result_reader_exact_format_records_match_stim() {
    let expected = unpack_bytes(&[0xF8, 0xF0, 0x03], 18);
    for (format, input) in [
        (SampleFormat::ZeroOne, b"000111110000111111\n".as_slice()),
        (SampleFormat::B8, [0xF8, 0xF0, 0x03].as_slice()),
        (
            SampleFormat::Hits,
            b"3,4,5,6,7,12,13,14,15,16,17\n".as_slice(),
        ),
        (
            SampleFormat::R8,
            [3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0].as_slice(),
        ),
    ] {
        assert_eq!(
            read_records(input, format, 18).unwrap(),
            vec![expected.clone()]
        );

        let mut dense = Vec::new();
        for_each_record(input, format, 18, |record| {
            dense.push(record.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(dense, vec![expected.clone()]);

        let mut packed = Vec::new();
        for_each_packed_record(input, format, 18, |record| {
            packed.push(bitslice_to_vec(record));
            Ok(())
        })
        .unwrap();
        assert_eq!(packed, vec![expected.clone()]);

        let expected_hits = expected
            .iter()
            .enumerate()
            .filter_map(|(index, bit)| bit.then_some(index as u64))
            .collect::<Vec<_>>();
        let mut sparse = Vec::new();
        for_each_sparse_record(input, format, 18, |hits| {
            sparse.push(hits.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(sparse, vec![expected_hits]);
    }
}

#[test]
fn cq_result_reader_round_trip_width_matrix_matches_stim() {
    for (shots, width) in [(3, 64), (3, 128), (3, 256), (100, 504)] {
        let records = deterministic_records(shots, width);
        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let encoded = write_records(&records, format);
            assert_eq!(
                read_records(&encoded, format, width).unwrap(),
                records,
                "{width} {format:?}"
            );

            let mut streamed = Vec::new();
            for_each_record(&encoded, format, width, |record| {
                streamed.push(record.to_vec());
                Ok(())
            })
            .unwrap();
            assert_eq!(streamed, records, "streamed {width} {format:?}");

            let mut packed = Vec::new();
            for_each_packed_record(&encoded, format, width, |record| {
                packed.push(bitslice_to_vec(record));
                Ok(())
            })
            .unwrap();
            assert_eq!(packed, records, "packed {width} {format:?}");

            let mut sparse = Vec::new();
            for_each_sparse_record(&encoded, format, width, |hits| {
                let mut record = vec![false; width];
                for hit in hits {
                    record[usize::try_from(*hit).unwrap()] = true;
                }
                sparse.push(record);
                Ok(())
            })
            .unwrap();
            assert_eq!(sparse, records, "sparse {width} {format:?}");
        }
    }
}

#[test]
fn cq_result_large_all_format_table_round_trip_matches_stim() {
    let records = deterministic_records(576, 1000);
    for format in [
        SampleFormat::ZeroOne,
        SampleFormat::B8,
        SampleFormat::R8,
        SampleFormat::Hits,
        SampleFormat::Dets,
    ] {
        let encoded = write_records(&records, format);
        assert_eq!(
            read_records(&encoded, format, 1000).unwrap(),
            records,
            "{format:?}"
        );
    }
    let ptb64 = write_ptb64_records_checked(&records).unwrap();
    assert_eq!(read_ptb64_records_all(&ptb64, 1000).unwrap(), records);
}

#[test]
fn cq_result_reader_record_boundaries_types_and_crlf_match_stim() {
    assert_eq!(
        read_records(
            b"111011001\r\n010000000\n101100011\n",
            SampleFormat::ZeroOne,
            9,
        )
        .unwrap()
        .len(),
        3
    );
    assert_eq!(
        read_measurement_records(
            b"shot M0\r\nshot M1\nshot M0\nshot\n",
            SampleFormat::Dets,
            2,
        )
        .unwrap(),
        vec![
            vec![true, false],
            vec![false, true],
            vec![true, false],
            vec![false, false],
        ]
    );
    assert!(read_measurement_records(b"shot D0\n", SampleFormat::Dets, 1).is_err());
    assert!(read_measurement_records(b"shot L0\n", SampleFormat::Dets, 1).is_err());
    assert_eq!(
        read_records(b"3\r\n1\r\n", SampleFormat::Hits, 4).unwrap(),
        vec![
            vec![false, false, false, true],
            vec![false, true, false, false],
        ]
    );
    assert_eq!(
        read_records(b"shot M3\r\n\r\n\n   shot M1\r\n\n", SampleFormat::Dets, 4,).unwrap(),
        vec![
            vec![false, false, false, true],
            vec![false, true, false, false],
        ]
    );
}

#[test]
fn cq_result_reader_rejects_malformed_widths_and_indices() {
    let parsed = read_records(b"105\n", SampleFormat::Hits, 106).unwrap();
    assert!(parsed[0][105]);

    let cases = [
        (SampleFormat::ZeroOne, b"012\n".as_slice(), 3),
        (SampleFormat::ZeroOne, b"01\n".as_slice(), 3),
        (SampleFormat::B8, [0].as_slice(), 9),
        (SampleFormat::R8, [255].as_slice(), 300),
        (SampleFormat::R8, [4].as_slice(), 3),
        (SampleFormat::Hits, b"100,1\n".as_slice(), 3),
        (SampleFormat::Hits, b"18446744073709551616\n".as_slice(), 3),
        (SampleFormat::Dets, b"D2\n".as_slice(), 3),
        (SampleFormat::Dets, b"shot X2\n".as_slice(), 3),
    ];
    for (format, input, width) in cases {
        assert!(
            read_records(input, format, width).is_err(),
            "materialized {format:?}"
        );
        assert!(
            for_each_record(input, format, width, |_| Ok(())).is_err(),
            "dense {format:?}"
        );
        assert!(
            for_each_packed_record(input, format, width, |_| Ok(())).is_err(),
            "packed {format:?}"
        );
        assert!(
            for_each_sparse_record(input, format, width, |_| Ok(())).is_err(),
            "sparse {format:?}"
        );
    }
}

#[test]
fn cq_result_ptb64_dense_sparse_prefix_and_validation_match_stim() {
    let records = deterministic_records(128, 71);
    let encoded = write_ptb64_records_checked(&records).unwrap();
    assert_eq!(ptb64_record_count(&encoded, 71).unwrap(), 128);
    assert_eq!(streaming_ptb64_record_count(&encoded, 71).unwrap(), 128);
    assert_eq!(read_ptb64_records(&encoded, 71, 64).unwrap(), records[..64]);
    assert_eq!(read_ptb64_records_all(&encoded, 71).unwrap(), records);

    let mut dense = Vec::new();
    for_each_ptb64_record_all(&encoded, 71, |record| {
        dense.push(record.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(dense, records);

    let mut prefix = Vec::new();
    for_each_ptb64_record(&encoded, 71, 64, |record| {
        prefix.push(record.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(prefix, records[..64]);

    assert!(validate_ptb64_shot_count(63).is_err());
    assert!(write_ptb64_records_checked(&records[..63]).is_err());
    let mut mixed_widths = vec![vec![false; 2]; 64];
    mixed_widths[63] = vec![false; 3];
    assert!(write_ptb64_records_checked(&mixed_widths).is_err());
    assert!(read_ptb64_records(&encoded[..encoded.len() - 1], 71, 128).is_err());
    assert!(read_ptb64_records_all(&encoded[..encoded.len() - 1], 71).is_err());
    assert!(read_ptb64_records_all(&encoded, 0).is_err());
}

#[test]
fn cq_result_streaming_visitors_stop_at_first_error() {
    fn stop() -> CircuitResult<()> {
        Err(CircuitError::InvalidResultFormat {
            message: "stop".to_string(),
        })
    }

    let mut visits = 0;
    let result = for_each_record(b"00\n11\n", SampleFormat::ZeroOne, 2, |_| {
        visits += 1;
        stop()
    });
    assert!(result.is_err());
    assert_eq!(visits, 1);

    visits = 0;
    let result = for_each_packed_record(b"00\n11\n", SampleFormat::ZeroOne, 2, |_| {
        visits += 1;
        stop()
    });
    assert!(result.is_err());
    assert_eq!(visits, 1);

    visits = 0;
    let result = for_each_sparse_record(b"0\n1\n", SampleFormat::Hits, 2, |_| {
        visits += 1;
        stop()
    });
    assert!(result.is_err());
    assert_eq!(visits, 1);

    let encoded = write_ptb64_records_checked(&deterministic_records(64, 2)).unwrap();
    visits = 0;
    let result = for_each_ptb64_record_all(&encoded, 2, |_| {
        visits += 1;
        stop()
    });
    assert!(result.is_err());
    assert_eq!(visits, 1);
}

#[test]
fn cq_result_sparse_shot_value_string_and_mask_match_stim() {
    let empty = SparseShot::new(Vec::new(), vec![false; 64]);
    assert_eq!(empty, empty.clone());
    assert_ne!(empty, SparseShot::new(vec![2], vec![false; 64]));
    assert!(format!("{empty:?}").contains("SparseShot"));

    let mut mask = vec![false; 125];
    mask[2] = true;
    mask[64] = true;
    let shot = SparseShot::new(vec![1, 2, 3], mask);
    assert_eq!(shot.hits, [1, 2, 3]);
    assert!(shot.obs_mask[64]);
    assert_eq!(shot.obs_mask_as_u64(), 4);
    let mut expected_mask = "_".repeat(125);
    expected_mask.replace_range(2..3, "1");
    expected_mask.replace_range(64..65, "1");
    let expected = ["SparseShot{{1, 2, 3}, ", expected_mask.as_str(), "}"].concat();
    assert_eq!(shot.stim_debug_string(), expected);
}
