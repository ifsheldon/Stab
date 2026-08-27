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

    let mut writer =
        MeasureRecordWriter::try_new(RecordFormat::ZeroOne).expect("per-record format");
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
    let mut writer = MeasureRecordWriter::try_new(RecordFormat::Dets).expect("per-record format");
    writer.write_end();
    writer.write_end();
    writer.write_end();
    assert_eq!(writer.into_bytes(), b"shot\nshot\nshot\n");

    let mut writer = MeasureRecordWriter::try_new(RecordFormat::R8).expect("per-record format");
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
        (RecordFormat::ZeroOne, b"000111110000111111\n".to_vec()),
        (RecordFormat::B8, vec![0xF8, 0xF0, 0x03]),
        (
            RecordFormat::Hits,
            b"3,4,5,6,7,12,13,14,15,16,17\n".to_vec(),
        ),
        (RecordFormat::R8, vec![3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]),
    ] {
        let mut writer = MeasureRecordWriter::try_new(format).expect("per-record format");
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
    let mut writer = MeasureRecordWriter::try_new(RecordFormat::Hits).expect("per-record format");
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
    let mut writer = MeasureRecordWriter::try_new(RecordFormat::Dets).expect("per-record format");
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
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let mut writer =
            MeasureRecordWriter::try_with_capacity(format, 64).expect("reserve output");
        writer.write_bits(&record);
        writer.write_end();
        assert_eq!(
            writer.into_bytes(),
            write_records(std::slice::from_ref(&record.to_vec()), format).expect("encode record"),
            "{format:?}"
        );
    }

    let error = MeasureRecordWriter::try_new(RecordFormat::Ptb64)
        .expect_err("ptb64 cannot be emitted one record at a time");
    assert!(error.message().contains("64-record group"));
}

#[test]
fn codecs_and_strict_grammars_contract() {
    let records = vec![
        vec![true, false, true, false, true, false, true, false, true],
        vec![false, true, false, true, false, true, false, true, false],
    ];
    for (format, expected) in [
        (RecordFormat::ZeroOne, b"101010101\n010101010\n".as_slice()),
        (RecordFormat::B8, &[0x55, 0x01, 0xAA, 0x00]),
        (RecordFormat::R8, &[0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1]),
        (RecordFormat::Hits, b"0,2,4,6,8\n1,3,5,7\n".as_slice()),
        (
            RecordFormat::Dets,
            b"shot M0 M2 M4 M6 M8\nshot M1 M3 M5 M7\n".as_slice(),
        ),
    ] {
        let encoded = write_records(&records, format).expect("encode records");
        assert_eq!(encoded, expected, "{format:?} byte contract");
        assert_eq!(
            read_records(&encoded, format, 9).unwrap(),
            records,
            "{format:?} decode contract"
        );
    }

    let ptb64_records = (0usize..64)
        .map(|shot| vec![shot.is_multiple_of(2), shot < 5, shot == 63, false])
        .collect::<Vec<_>>();
    let ptb64 = write_records(&ptb64_records, RecordFormat::Ptb64).unwrap();
    assert_eq!(
        ptb64,
        [
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x1F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(ptb64, write_ptb64_records_checked(&ptb64_records).unwrap());
    assert_eq!(
        read_records(&ptb64, RecordFormat::Ptb64, 4).unwrap(),
        ptb64_records
    );
    assert_eq!(read_ptb64_records(&ptb64, 4, 64).unwrap(), ptb64_records);
    assert_eq!(read_ptb64_records_all(&ptb64, 4).unwrap(), ptb64_records);
    assert_eq!(ptb64_record_count(&ptb64, 4).unwrap(), 64);

    assert_eq!(
        read_records(b"01\r\n10\r\n", RecordFormat::ZeroOne, 2).unwrap(),
        vec![vec![false, true], vec![true, false]]
    );
    assert_eq!(
        read_records(b"1,1\r\n2\r\n", RecordFormat::Hits, 3).unwrap(),
        vec![vec![false, false, false], vec![false, false, true]]
    );
    assert_eq!(
        read_measurement_records(b"   shot M1\r\nshot\r\n", RecordFormat::Dets, 2).unwrap(),
        vec![vec![false, true], vec![false, false]]
    );
    assert_eq!(
        read_measurement_records(b"shot M0 M0\n", RecordFormat::Dets, 1).unwrap(),
        vec![vec![true]],
        "duplicate DETS tokens set dense bits instead of toggling them"
    );

    for (format, input, width) in [
        (RecordFormat::ZeroOne, b"10".as_slice(), 2),
        (RecordFormat::ZeroOne, b"0x\n".as_slice(), 2),
        (RecordFormat::ZeroOne, b"0\n".as_slice(), 2),
        (RecordFormat::B8, &[0x01], 9),
        (RecordFormat::B8, &[], 0),
        (RecordFormat::R8, &[3], 2),
        (RecordFormat::Hits, b"1,,2\n".as_slice(), 3),
        (RecordFormat::Hits, b"1,\n".as_slice(), 3),
        (RecordFormat::Hits, b",1\n".as_slice(), 3),
        (RecordFormat::Hits, b"1,2".as_slice(), 3),
        (RecordFormat::Dets, b"shot  M0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot M0 \n".as_slice(), 1),
        (RecordFormat::Dets, b"shot\tM0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot D0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot L0\n".as_slice(), 1),
    ] {
        assert!(
            read_records(input, format, width).is_err(),
            "{format:?} accepted malformed fixture {input:?}"
        );
    }

    assert!(write_ptb64_records_checked(&ptb64_records[..63]).is_err());
    assert!(read_ptb64_records(&ptb64[..31], 4, 64).is_err());
    assert!(read_ptb64_records_all(&ptb64[..31], 4).is_err());
    assert!(read_ptb64_records_all(&ptb64, 0).is_err());
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

    let mut writer = MeasureRecordBatchWriter::new(5, RecordFormat::ZeroOne);
    batch
        .final_write_unwritten_results_to(&mut writer, &[false; 5])
        .unwrap();
    let output = writer.write_end().expect("encode batch");
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
