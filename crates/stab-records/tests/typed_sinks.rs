#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "contract tests use compact deterministic fixtures"
)]

use stab_records::{
    DemSampleBatchView, DemSampleCodecSink, DemSampleSink, DetectionBatchView, DetectionCodecSink,
    DetectionSink, DetectorWidth, FormatErrorCode, MeasureRecordWriter, MeasurementBatchView,
    MeasurementCodecSink, MeasurementSink, MeasurementWidth, ObservableWidth, PackedShotBatch,
    RecordFormat, RecordResult, SampleFormat, SampledErrorWidth, write_ptb64_records_checked,
    write_records,
};

#[test]
fn measurement_codec_sinks_match_every_legacy_encoding() -> RecordResult<()> {
    let records = patterned_records(64, 17, 3);
    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let batch = PackedShotBatch::from_records(&records, 17)?;
        let mut sink = MeasurementCodecSink::try_new(format, MeasurementWidth::new(17))?;
        sink.write_batch(MeasurementBatchView::new(batch.view()))?;
        assert_eq!(
            sink.into_bytes()?,
            write_records(&records, sample_format(format))
        );
    }

    let first = PackedShotBatch::from_records(&records[..10], 17)?;
    let second = PackedShotBatch::from_records(&records[10..], 17)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::Ptb64, MeasurementWidth::new(17))?;
    sink.write_batch(MeasurementBatchView::new(first.view()))?;
    sink.write_batch(MeasurementBatchView::new(second.view()))?;
    assert_eq!(sink.into_bytes()?, write_ptb64_records_checked(&records)?);
    Ok(())
}

#[test]
fn detection_codec_keeps_dets_namespaces_until_encoding() -> RecordResult<()> {
    let detector_records = patterned_records(64, 5, 7);
    let observable_records = patterned_records(64, 3, 11);
    let expected_merged = detector_records
        .iter()
        .zip(&observable_records)
        .map(|(detectors, observables)| {
            detectors
                .iter()
                .chain(observables)
                .copied()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Ptb64,
    ] {
        let detectors = PackedShotBatch::from_records(&detector_records, 5)?;
        let observables = PackedShotBatch::from_records(&observable_records, 3)?;
        let batch = DetectionBatchView::try_new(detectors.view(), observables.view())?;
        let mut sink =
            DetectionCodecSink::try_new(format, DetectorWidth::new(5), ObservableWidth::new(3))?;
        sink.write_batch(batch)?;
        let expected = if format == RecordFormat::Ptb64 {
            write_ptb64_records_checked(&expected_merged)?
        } else {
            write_records(&expected_merged, sample_format(format))
        };
        assert_eq!(sink.into_bytes()?, expected);
    }

    let detectors = PackedShotBatch::from_records(&detector_records[..2], 5)?;
    let observables = PackedShotBatch::from_records(&observable_records[..2], 3)?;
    let batch = DetectionBatchView::try_new(detectors.view(), observables.view())?;
    let mut sink = DetectionCodecSink::try_new(
        RecordFormat::Dets,
        DetectorWidth::new(5),
        ObservableWidth::new(3),
    )?;
    sink.write_batch(batch)?;

    let mut expected = MeasureRecordWriter::new(SampleFormat::Dets);
    for (detectors, observables) in detector_records[..2].iter().zip(&observable_records[..2]) {
        expected.begin_dets_result_type(stab_records::DetsResultType::Detector);
        expected.write_bits(detectors);
        expected.begin_dets_result_type(stab_records::DetsResultType::Observable);
        expected.write_bits(observables);
        expected.write_end();
    }
    assert_eq!(sink.into_bytes()?, expected.into_bytes());
    Ok(())
}

#[test]
fn dem_codec_routes_sampled_errors_to_their_own_stream() -> RecordResult<()> {
    let detectors = PackedShotBatch::from_records(&[vec![true, false], vec![false, true]], 2)?;
    let observables = PackedShotBatch::from_records(&[vec![true], vec![false]], 1)?;
    let errors =
        PackedShotBatch::from_records(&[vec![false, true, true], vec![true, false, false]], 3)?;
    let detection = DetectionBatchView::try_new(detectors.view(), observables.view())?;
    let batch = DemSampleBatchView::try_new(detection, Some(errors.view()))?;

    let mut sink = DemSampleCodecSink::try_new(
        RecordFormat::Dets,
        DetectorWidth::new(2),
        ObservableWidth::new(1),
        Some((RecordFormat::Hits, SampledErrorWidth::new(3))),
    )?;
    sink.write_batch(batch)?;
    let output = sink.into_records()?;

    assert_eq!(output.detection_records(), b"shot D0 L0\nshot D1\n");
    assert_eq!(output.sampled_error_records(), Some(&b"1,2\n0\n"[..]));
    let (detections, errors) = output.into_parts();
    assert_eq!(detections, b"shot D0 L0\nshot D1\n");
    assert_eq!(errors, Some(b"1,2\n0\n".to_vec()));
    Ok(())
}

#[test]
fn typed_sinks_reject_layout_mismatches_before_writing() -> RecordResult<()> {
    let wrong = PackedShotBatch::from_records(&[vec![true, false]], 2)?;
    let right = PackedShotBatch::from_records(&[vec![true, false, true]], 3)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::ZeroOne, MeasurementWidth::new(3))?;

    let error = sink
        .write_batch(MeasurementBatchView::new(wrong.view()))
        .expect_err("mismatched semantic width");
    assert_eq!(error.code(), FormatErrorCode::InvalidRecordWidth);

    sink.write_batch(MeasurementBatchView::new(right.view()))?;
    assert_eq!(sink.into_bytes()?, b"101\n");

    let detectors = PackedShotBatch::from_records(&[vec![true]], 1)?;
    let observables = PackedShotBatch::from_records(&[vec![false]], 1)?;
    let detection = DetectionBatchView::try_new(detectors.view(), observables.view())?;
    let without_errors = DemSampleBatchView::try_new(detection, None)?;
    let mut dem_sink = DemSampleCodecSink::try_new(
        RecordFormat::Dets,
        DetectorWidth::new(1),
        ObservableWidth::new(1),
        Some((RecordFormat::Hits, SampledErrorWidth::new(2))),
    )?;
    assert!(
        dem_sink
            .write_batch(without_errors)
            .expect_err("missing required sampled-error plane")
            .message()
            .contains("omitted sampled errors")
    );
    Ok(())
}

#[test]
fn ptb64_finish_is_bounded_recoverable_and_final() -> RecordResult<()> {
    let records = patterned_records(64, 9, 13);
    let first = PackedShotBatch::from_records(&records[..63], 9)?;
    let last = PackedShotBatch::from_records(&records[63..], 9)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::Ptb64, MeasurementWidth::new(9))?;
    sink.write_batch(MeasurementBatchView::new(first.view()))?;

    assert!(
        sink.finish()
            .expect_err("partial PTB64 group")
            .message()
            .contains("63 trailing records")
    );
    sink.write_batch(MeasurementBatchView::new(last.view()))?;
    sink.finish()?;

    let extra = PackedShotBatch::from_records(&[vec![false; 9]], 9)?;
    assert!(
        sink.write_batch(MeasurementBatchView::new(extra.view()))
            .expect_err("writes after finalization")
            .message()
            .contains("after sink finalization")
    );
    assert_eq!(sink.into_bytes()?, write_ptb64_records_checked(&records)?);

    let zero_width = vec![Vec::new(); 64];
    let empty = PackedShotBatch::from_records(&zero_width, 0)?;
    let mut empty_sink =
        MeasurementCodecSink::try_new(RecordFormat::Ptb64, MeasurementWidth::new(0))?;
    empty_sink.write_batch(MeasurementBatchView::new(empty.view()))?;
    assert_eq!(empty_sink.into_bytes()?, Vec::<u8>::new());
    Ok(())
}

fn patterned_records(shots: usize, width: usize, stride: usize) -> Vec<Vec<bool>> {
    (0..shots)
        .map(|shot| {
            (0..width)
                .map(|bit| (shot * stride + bit * 5 + 1).is_multiple_of(7))
                .collect()
        })
        .collect()
}

fn sample_format(format: RecordFormat) -> SampleFormat {
    match format {
        RecordFormat::ZeroOne => SampleFormat::ZeroOne,
        RecordFormat::B8 => SampleFormat::B8,
        RecordFormat::R8 => SampleFormat::R8,
        RecordFormat::Hits => SampleFormat::Hits,
        RecordFormat::Dets => SampleFormat::Dets,
        RecordFormat::Ptb64 => panic!("PTB64 has a dedicated grouped writer"),
        _ => panic!("test helper does not support future record formats"),
    }
}
