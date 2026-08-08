#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "result-format unit tests use direct fixture assertions for compact diagnostics"
)]

use super::*;

#[test]
fn measure_record_records_lookback_and_writes_unwritten_results() {
    let mut record = MeasureRecord::new(20);
    record.record_result(true);
    assert_eq!(record.lookback(1), Some(true));
    record.record_result(false);
    assert_eq!(record.lookback(1), Some(false));
    assert_eq!(record.lookback(2), Some(true));
    for _ in 0..50 {
        record.record_result(true);
        record.record_result(false);
    }
    assert_eq!(record.storage_len(), 102);

    let mut writer = MeasureRecordWriter::new(SampleFormat::ZeroOne);
    record
        .write_unwritten_results_to(&mut writer)
        .expect("write unwritten results");
    assert_eq!(
        writer.into_bytes(),
        (0..102)
            .map(|index| if index % 2 == 0 { b'1' } else { b'0' })
            .collect::<Vec<_>>()
    );
    assert!(record.storage_len() <= 40);
}

#[test]
fn measure_record_writer_handles_empty_dets_records_and_long_r8_gaps() {
    let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
    writer.write_end();
    writer.write_end();
    writer.write_end();
    assert_eq!(writer.into_bytes(), b"shot\nshot\nshot\n");

    let mut writer = MeasureRecordWriter::new(SampleFormat::R8);
    for _ in 0..(8 * 64) {
        writer.write_bit(false);
    }
    writer.write_bit(true);
    for _ in 0..32 {
        writer.write_bit(false);
    }
    writer.write_end();
    assert_eq!(writer.into_bytes(), [255, 255, 2, 32]);
}

/// Ported from Stim's `MeasureRecordWriter` writer-contract tests
/// (`vendor/stim/src/stim/io/measure_record_writer.test.cc`): the base-class
/// `begin_result_type` is a no-op, so only the DETS writer reacts to it.
#[test]
fn begin_result_type_is_a_no_op_on_non_dets_writers_like_stim() {
    let bytes = [0xF8_u8];
    for (format, expected) in [
        (SampleFormat::ZeroOne, b"000111110000111111\n".to_vec()),
        (SampleFormat::B8, vec![0xF8, 0xF0, 0x03]),
        (
            SampleFormat::Hits,
            b"3,4,5,6,7,12,13,14,15,16,17\n".to_vec(),
        ),
        (SampleFormat::R8, vec![3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]),
    ] {
        let mut writer = MeasureRecordWriter::new(format);
        writer.begin_result_type(b'D');
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.begin_result_type(b'L');
        writer.write_bytes(&bytes);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(writer.into_bytes(), expected, "{format:?}");
    }
}

/// Regression for the pre-consolidation trap: `begin_result_type` unconditionally reset the
/// in-record bit index, so a mid-record call on a HITS writer restarted hit indexes at zero
/// (producing `3,4,5,6,7,3,4,5,6,7,8`) where upstream Stim keeps counting.
#[test]
fn begin_result_type_does_not_reset_hits_bit_position_mid_record() {
    let bytes = [0xF8_u8];
    let mut writer = MeasureRecordWriter::new(SampleFormat::Hits);
    writer.write_bytes(&bytes);
    writer.write_bit(false);
    writer.begin_result_type(b'D');
    writer.write_bytes(&bytes);
    writer.write_bit(true);
    writer.write_end();
    assert_eq!(writer.into_bytes(), b"3,4,5,6,7,12,13,14,15,16,17\n");
}

/// Ported from Stim's `MeasureRecordWriter.FormatDets` contract: on a DETS writer,
/// `begin_result_type` still switches the namespace and restarts its position at zero.
#[test]
fn begin_result_type_switches_namespace_and_resets_position_on_dets_writers() {
    let bytes = [0xF8_u8];
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
        b"shot D3 D4 D5 D6 D7 D12 D13 D14 D15 D16 L1\n".to_vec()
    );
}

#[test]
fn fallible_reservation_constructor_reserves_and_encodes_like_the_plain_writer() {
    let record = [true, false, true, true];
    for format in [
        SampleFormat::ZeroOne,
        SampleFormat::B8,
        SampleFormat::R8,
        SampleFormat::Hits,
        SampleFormat::Dets,
    ] {
        let mut writer =
            MeasureRecordWriter::try_with_capacity(format, 64).expect("reserve output");
        writer.write_bits(&record);
        writer.write_end();
        assert_eq!(
            writer.into_bytes(),
            write_records(std::slice::from_ref(&record.to_vec()), format),
            "{format:?}"
        );
    }
}

#[test]
fn measure_record_reader_loads_all_supported_record_formats() {
    let expected = [
        false, false, false, true, true, true, true, true, false, false, false, false, true, true,
        true, true, true, true,
    ]
    .to_vec();

    for (format, input) in [
        (SampleFormat::ZeroOne, b"000111110000111111\n".as_slice()),
        (SampleFormat::B8, &[0xF8, 0xF0, 0x03]),
        (
            SampleFormat::Hits,
            b"3,4,5,6,7,12,13,14,15,16,17\n".as_slice(),
        ),
        (SampleFormat::R8, &[3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]),
    ] {
        assert_eq!(
            read_records(input, format, 18).unwrap(),
            vec![expected.clone()]
        );
    }

    assert!(read_records(&[], SampleFormat::B8, 0).is_err());
}

#[test]
fn measure_record_reader_round_trips_writer_output() {
    let source = [0, 1, 2, 3, 4, 0xFF, 0xBF, 0xFE, 80, 0, 0, 1, 20];
    let bits = unpack_b8_chunk(&source, source.len() * 8);
    for format in [
        SampleFormat::ZeroOne,
        SampleFormat::B8,
        SampleFormat::R8,
        SampleFormat::Hits,
        SampleFormat::Dets,
    ] {
        let encoded = write_records(std::slice::from_ref(&bits), format);
        let width = if matches!(format, SampleFormat::Hits | SampleFormat::Dets) {
            bits.len() - 1
        } else {
            bits.len()
        };
        assert_eq!(
            read_records(&encoded, format, width).unwrap(),
            vec![bits[..width].to_vec()]
        );
    }
}

#[test]
fn ptb64_reader_round_trips_writer_output() {
    let records = (0..64)
        .map(|shot_index| {
            (0..17)
                .map(|bit_index| (shot_index * 7 + bit_index * 11) % 13 == 0)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let encoded = write_ptb64_records_checked(&records).unwrap();

    assert_eq!(read_ptb64_records(&encoded, 17, 64).unwrap(), records);
    assert_eq!(read_ptb64_records_all(&encoded, 17).unwrap(), records);
    assert_eq!(ptb64_record_count(&encoded, 17).unwrap(), 64);
}

#[test]
fn measure_record_reader_handles_multiple_records() {
    let records = read_records(
        b"111011001\n010000000\n101100011\n",
        SampleFormat::ZeroOne,
        9,
    )
    .unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(
        read_records(b"shot M0\nshot M1\nshot M0\nshot\n", SampleFormat::Dets, 2).unwrap(),
        vec![
            vec![true, false],
            vec![false, true],
            vec![true, false],
            vec![false, false],
        ]
    );
    assert_eq!(
        read_measurement_records(b"shot M0\nshot\n", SampleFormat::Dets, 2).unwrap(),
        vec![vec![true, false], vec![false, false]]
    );
    assert!(read_measurement_records(b"shot D0\n", SampleFormat::Dets, 2).is_err());
    assert!(read_measurement_records(b"shot L0\n", SampleFormat::Dets, 2).is_err());
}

#[test]
fn measure_record_reader_accepts_stim_windows_newline_text_records() {
    assert_eq!(
        read_records(b"01\r\n01\r\n", SampleFormat::ZeroOne, 2).unwrap(),
        vec![vec![false, true], vec![false, true]]
    );
    assert_eq!(
        read_records(b"3\r\n1\r\n", SampleFormat::Hits, 4).unwrap(),
        vec![
            vec![false, false, false, true],
            vec![false, true, false, false],
        ]
    );
    assert_eq!(
        read_measurement_records(b"shot M3\r\n\r\n\n   shot M1\r\n\n", SampleFormat::Dets, 4,)
            .unwrap(),
        vec![
            vec![false, false, false, true],
            vec![false, true, false, false],
        ]
    );
}

#[test]
fn measure_record_reader_rejects_unterminated_01_records_and_non_bits() {
    assert!(read_records(b"10", SampleFormat::ZeroOne, 2).is_err());
    assert!(read_records(&[b'0', 0xFF], SampleFormat::ZeroOne, 2).is_err());
}

#[test]
fn measure_record_batch_writes_shot_major_01_records() {
    let s0 = vec![true, false, true, false, true];
    let s1 = vec![false, true, false, true, false];
    let mut batch = MeasureRecordBatch::new(5, 20);
    assert_eq!(batch.stored(), 0);
    batch.record_result(s0.clone()).unwrap();
    assert_eq!(batch.lookback(1), Some(s0.as_slice()));
    batch.record_result(s1.clone()).unwrap();
    assert_eq!(batch.lookback(1), Some(s1.as_slice()));
    assert_eq!(batch.lookback(2), Some(s0.as_slice()));
    for _ in 0..50 {
        batch.record_result(s0.clone()).unwrap();
        batch.record_result(s1.clone()).unwrap();
    }
    assert_eq!(batch.unwritten(), 102);

    let mut writer = MeasureRecordBatchWriter::new(5, SampleFormat::ZeroOne);
    batch
        .final_write_unwritten_results_to(&mut writer, &[false; 5])
        .unwrap();
    let output = writer.write_end();
    for shot_index in 0..5 {
        for sample_index in 0..102 {
            assert_eq!(
                output[shot_index * 103 + sample_index],
                b'0' + u8::from((shot_index + sample_index + 1) % 2 == 1)
            );
        }
        assert_eq!(output[shot_index * 103 + 102], b'\n');
    }
    assert!(batch.stored() <= 20);
}

#[test]
fn measure_record_batch_records_zero_result_to_edit() {
    let mut batch = MeasureRecordBatch::new(5, 2);
    batch.record_zero_result_to_edit()[2] = true;
    assert_eq!(batch.stored(), 1);
    assert_eq!(
        batch.lookback(1),
        Some([false, false, true, false, false].as_slice())
    );
    batch.record_zero_result_to_edit()[3] = true;
    assert_eq!(
        batch.lookback(1),
        Some([false, false, false, true, false].as_slice())
    );
}

#[test]
fn sparse_shot_matches_upstream_equality_string_and_mask_behavior() {
    assert_eq!(
        SparseShot::new(Vec::new(), vec![false; 64]),
        SparseShot::new(Vec::new(), vec![false; 64])
    );
    assert_ne!(
        SparseShot::new(Vec::new(), vec![false; 64]),
        SparseShot::new(vec![2], vec![false; 64])
    );
    let mut mask = vec![false; 64];
    mask[2] = true;
    let shot = SparseShot::new(vec![1, 2, 3], mask.clone());
    assert_eq!(
        shot.stim_debug_string(),
        "SparseShot{{1, 2, 3}, __1_____________________________________________________________}"
    );
    assert_eq!(shot.obs_mask_as_u64(), 4);

    let mut wide_mask = vec![false; 125];
    wide_mask[1] = true;
    wide_mask[64] = true;
    assert_eq!(SparseShot::new(Vec::new(), wide_mask).obs_mask_as_u64(), 2);
}

#[test]
fn ptb64_records_are_measurement_major_over_64_shot_groups() {
    let mut records = vec![vec![false, false, false, false]; 64];
    for record in records.iter_mut().take(5) {
        record[1] = true;
    }
    assert_eq!(
        write_ptb64_records(&records),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0x1F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]
    );
    let encoded = write_ptb64_records_checked(&records).unwrap();
    assert_eq!(read_ptb64_records(&encoded, 4, 64).unwrap(), records);

    let mut encoded_with_extra_group = encoded.clone();
    encoded_with_extra_group.extend_from_slice(&encoded);
    assert_eq!(
        read_ptb64_records(&encoded_with_extra_group, 4, 64).unwrap(),
        records
    );
    assert!(write_ptb64_records_checked(&records[..63]).is_err());
    assert!(read_ptb64_records(&encoded[..31], 4, 64).is_err());
    assert!(read_ptb64_records(&encoded, 0, 64).is_err());
    assert_eq!(read_ptb64_records_all(&encoded, 4).unwrap(), records);
    assert!(read_ptb64_records_all(&encoded[..31], 4).is_err());
    assert!(read_ptb64_records_all(&encoded, 0).is_err());
    assert!(read_ptb64_records_all(&[], 0).is_err());
    assert_eq!(ptb64_record_count(&encoded, 4).unwrap(), 64);
}
