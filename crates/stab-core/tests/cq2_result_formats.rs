#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "qualification tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    SampleFormat,
    result_formats::{
        MeasureRecord, MeasureRecordBatch, MeasureRecordBatchWriter, MeasureRecordWriter,
        read_records,
    },
    result_streaming::{for_each_packed_record, for_each_record, for_each_sparse_record},
};

#[test]
fn cq_result_sparse_duplicate_tokens_toggle_dense_records() {
    let expected = vec![false, false, false, false];

    assert_eq!(
        read_records(b"1,1\n", SampleFormat::Hits, 4).unwrap(),
        vec![expected.clone()]
    );
    assert_eq!(
        read_records(b"shot M1 M1\n", SampleFormat::Dets, 4).unwrap(),
        vec![expected.clone()]
    );

    let mut dense = Vec::new();
    for_each_record(b"1,1\n", SampleFormat::Hits, 4, |record| {
        dense.push(record.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(dense, vec![expected.clone()]);

    let mut packed: Vec<Vec<bool>> = Vec::new();
    for_each_packed_record(b"shot D1 D1\n", SampleFormat::Dets, 4, |record| {
        packed.push(
            (0..record.len())
                .map(|index| record.get(index).unwrap())
                .collect(),
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(packed, vec![expected]);
}

#[test]
fn cq_result_sparse_visitors_preserve_token_order_and_duplicates() {
    let mut records = Vec::new();
    for_each_sparse_record(b"3,1,1\n", SampleFormat::Hits, 4, |hits| {
        records.push(hits.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(records, vec![vec![3, 1, 1]]);

    records.clear();
    for_each_sparse_record(b"shot D3 D1 D1\n", SampleFormat::Dets, 4, |hits| {
        records.push(hits.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(records, vec![vec![3, 1, 1]]);
}

#[test]
fn cq_result_measure_records_enforce_configured_lookback_limit() {
    let mut record = MeasureRecord::new(2);
    record.record_result(false);
    record.record_result(true);
    record.record_result(false);
    assert_eq!(record.lookback(1), Some(false));
    assert_eq!(record.lookback(2), Some(true));
    assert_eq!(record.lookback(3), None);

    let mut batch = MeasureRecordBatch::new(2, 2);
    batch.record_result(vec![false, true]).unwrap();
    batch.record_result(vec![true, false]).unwrap();
    batch.record_result(vec![false, false]).unwrap();
    assert_eq!(batch.lookback(1), Some([false, false].as_slice()));
    assert_eq!(batch.lookback(2), Some([true, false].as_slice()));
    assert_eq!(batch.lookback(3), None);
}

#[test]
fn cq_result_batch_intermediate_flushes_complete_chunks_once() {
    let mut batch = MeasureRecordBatch::new(3, 5);
    for _ in 0..300 {
        batch.record_result(vec![false; 3]).unwrap();
    }
    let mut reference = vec![false; 300];
    reference[0] = true;
    reference[255] = true;
    reference[256] = true;

    let mut writer = MeasureRecordBatchWriter::new(3, SampleFormat::ZeroOne);
    batch
        .intermediate_write_unwritten_results_to(&mut writer, &reference)
        .unwrap();
    assert_eq!(batch.unwritten(), 44);
    assert!(batch.stored() <= 49);

    let first_output = writer.write_end();
    for shot in first_output.split(|byte| *byte == b'\n').take(3) {
        assert_eq!(shot.len(), 256);
        assert_eq!(shot[0], b'1');
        assert_eq!(shot[255], b'1');
        assert!(shot[1..255].iter().all(|byte| *byte == b'0'));
    }

    batch
        .intermediate_write_unwritten_results_to(&mut writer, &reference)
        .unwrap();
    assert_eq!(writer.write_end(), first_output);

    batch
        .final_write_unwritten_results_to(&mut writer, &reference)
        .unwrap();
    assert_eq!(batch.unwritten(), 0);
    let final_output = writer.write_end();
    for shot in final_output.split(|byte| *byte == b'\n').take(3) {
        assert_eq!(shot.len(), 300);
        assert_eq!(shot[256], b'1');
        assert!(shot[257..].iter().all(|byte| *byte == b'0'));
    }
}

#[test]
fn cq_result_batch_reference_sample_is_measurement_indexed_and_zero_padded() {
    let mut batch = MeasureRecordBatch::new(2, 2);
    batch.record_result(vec![false, false]).unwrap();
    batch.record_result(vec![false, false]).unwrap();

    let mut writer = MeasureRecordBatchWriter::new(2, SampleFormat::ZeroOne);
    batch
        .final_write_unwritten_results_to(&mut writer, &[true])
        .unwrap();
    assert_eq!(writer.write_end(), b"10\n10\n");
}

#[test]
fn cq_result_measure_record_basic_usage_and_value_contract_match_stim() {
    let mut record = MeasureRecord::new(20);
    for index in 0..102 {
        record.record_result(index % 2 == 0);
    }
    assert_eq!(record.storage_len(), 102);
    assert_eq!(record.lookback(1), Some(false));
    assert_eq!(record.lookback(2), Some(true));
    assert_eq!(record, record.clone());
    assert!(format!("{record:?}").contains("MeasureRecord"));

    let mut writer = MeasureRecordWriter::new(SampleFormat::ZeroOne);
    record.write_unwritten_results_to(&mut writer).unwrap();
    assert_eq!(
        writer.into_bytes(),
        (0..102)
            .map(|index| if index % 2 == 0 { b'1' } else { b'0' })
            .collect::<Vec<_>>()
    );
    assert!(record.storage_len() <= 20);
}

#[test]
fn cq_result_batch_basic_usage_and_compaction_match_stim() {
    let first = vec![true, false, true, false, true];
    let second = vec![false, true, false, true, false];
    let mut batch = MeasureRecordBatch::new(5, 20);
    for measurement in 0..102 {
        batch
            .record_result(if measurement % 2 == 0 {
                first.clone()
            } else {
                second.clone()
            })
            .unwrap();
    }

    let mut writer = MeasureRecordBatchWriter::new(5, SampleFormat::ZeroOne);
    batch
        .intermediate_write_unwritten_results_to(&mut writer, &[])
        .unwrap();
    assert_eq!(batch.unwritten(), 102);
    assert!(writer.write_end().iter().all(|byte| *byte == b'\n'));

    for measurement in 102..1102 {
        batch
            .record_result(if measurement % 2 == 0 {
                first.clone()
            } else {
                second.clone()
            })
            .unwrap();
    }
    batch
        .intermediate_write_unwritten_results_to(&mut writer, &[])
        .unwrap();
    assert!(batch.unwritten() < 100);
    assert!(batch.stored() < 100);
    batch
        .final_write_unwritten_results_to(&mut writer, &[])
        .unwrap();
    assert_eq!(batch.unwritten(), 0);
    assert!(batch.stored() < 100);

    let output = writer.write_end();
    for (shot, line) in output.split(|byte| *byte == b'\n').take(5).enumerate() {
        assert_eq!(line.len(), 1102);
        for (measurement, byte) in line.iter().enumerate() {
            assert_eq!(*byte, b'0' + u8::from((shot + measurement + 1) % 2 == 1));
        }
    }
}

#[test]
fn cq_result_batch_zero_edit_and_value_contract_match_stim() {
    let mut batch = MeasureRecordBatch::new(5, 2);
    batch.record_zero_result_to_edit()[2] = true;
    batch.record_zero_result_to_edit()[3] = true;
    assert_eq!(
        batch.lookback(2),
        Some([false, false, true, false, false].as_slice())
    );
    assert_eq!(
        batch.lookback(1),
        Some([false, false, false, true, false].as_slice())
    );
    assert_eq!(batch, batch.clone());
    assert!(format!("{batch:?}").contains("MeasureRecordBatch"));
    assert!(batch.record_result(vec![false; 4]).is_err());
}
