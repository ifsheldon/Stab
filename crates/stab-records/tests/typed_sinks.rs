#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "contract tests use compact deterministic fixtures"
)]

//! The typed sinks route each semantic column group to the record encoder: their bytes equal
//! those of one encoder fed the same records as namespace-tagged parts, whose own encoding is
//! checked against an independent reference in `record_encoder.rs`.

use stab_records::{
    BitPlaneBatch, DemSampleBatchView, DemSampleCodecSink, DemSampleSink, DetectionBatchView,
    DetectionCodecSink, DetectionSink, DetectorWidth, DetsResultType, FormatErrorCode,
    MeasurementBatchView, MeasurementCodecSink, MeasurementSink, MeasurementWidth, ObservableWidth,
    PackedShotBatch, RecordBatchPart, RecordEncoder, RecordFormat, RecordResult, SampledErrorWidth,
};

/// Encodes complete records whose columns are split into namespace-tagged groups.
fn encode_parts(
    format: RecordFormat,
    parts: &[(DetsResultType, &[Vec<bool>])],
) -> RecordResult<Vec<u8>> {
    let batches = parts
        .iter()
        .map(|(_, records)| {
            let width = records.first().map_or(0, Vec::len);
            PackedShotBatch::from_records(records, width)
        })
        .collect::<RecordResult<Vec<_>>>()?;
    let parts = parts
        .iter()
        .zip(&batches)
        .map(|((namespace, _), batch)| RecordBatchPart::new(*namespace, batch.view()))
        .collect::<Vec<_>>();
    let width = batches.iter().map(PackedShotBatch::bits_per_shot).sum();
    let mut encoder = RecordEncoder::try_new(format, width)?;
    let mut out = Vec::new();
    encoder
        .write_batch(&parts, &mut out)
        .expect("in-memory record encoding");
    assert_eq!(encoder.pending_group_records(), 0, "complete PTB64 groups");
    Ok(out)
}

fn encode(records: &[Vec<bool>], format: RecordFormat) -> RecordResult<Vec<u8>> {
    encode_parts(format, &[(DetsResultType::Measurement, records)])
}

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
        let expected = encode(&records, format)?;
        assert_eq!(sink.into_bytes()?, expected);

        let planes = BitPlaneBatch::from_shot_major(batch.view())?;
        let mut plane_sink = MeasurementCodecSink::try_new(format, MeasurementWidth::new(17))?;
        plane_sink.write_batch(MeasurementBatchView::from_bit_planes(planes.view()))?;
        assert_eq!(plane_sink.into_bytes()?, expected);
    }

    let first = PackedShotBatch::from_records(&records[..10], 17)?;
    let second = PackedShotBatch::from_records(&records[10..], 17)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::Ptb64, MeasurementWidth::new(17))?;
    sink.write_batch(MeasurementBatchView::new(first.view()))?;
    sink.write_batch(MeasurementBatchView::new(second.view()))?;
    assert_eq!(sink.into_bytes()?, encode(&records, RecordFormat::Ptb64)?);

    let planes =
        BitPlaneBatch::from_shot_major(PackedShotBatch::from_records(&records, 17)?.view())?;
    let mut plane_sink =
        MeasurementCodecSink::try_new(RecordFormat::Ptb64, MeasurementWidth::new(17))?;
    plane_sink.write_batch(MeasurementBatchView::from_bit_planes(planes.view()))?;
    assert_eq!(
        plane_sink.into_bytes()?,
        encode(&records, RecordFormat::Ptb64)?
    );
    Ok(())
}

#[test]
fn measurement_codec_reserves_known_record_counts_without_changing_bytes() -> RecordResult<()> {
    let records = patterned_records(65, 1, 3);
    let batch = PackedShotBatch::from_records(&records, 1)?;
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::ZeroOne, MeasurementWidth::new(1))?;
    sink.reserve_records(records.len())?;
    sink.write_batch(MeasurementBatchView::new(batch.view()))?;
    sink.finish()?;
    assert!(sink.reserve_records(1).is_err());
    assert_eq!(sink.into_bytes()?, encode(&records, RecordFormat::ZeroOne)?);
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
        let expected = encode(&expected_merged, format)?;
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

    let expected = encode_parts(
        RecordFormat::Dets,
        &[
            (DetsResultType::Detector, &detector_records[..2]),
            (DetsResultType::Observable, &observable_records[..2]),
        ],
    )?;
    assert_eq!(sink.into_bytes()?, expected);
    Ok(())
}

#[test]
fn dem_codec_routes_sampled_errors_to_their_own_stream() -> RecordResult<()> {
    let detectors = PackedShotBatch::from_records(&[vec![true, false], vec![false, true]], 2)?;
    let observables = PackedShotBatch::from_records(&[vec![true], vec![false]], 1)?;
    let errors =
        PackedShotBatch::from_records(&[vec![false, true, true], vec![true, false, false]], 3)?;
    let detection = DetectionBatchView::try_new(detectors.view(), observables.view())?;
    let batch = DemSampleBatchView::try_new(detection, Some(errors.view().into()))?;

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
    assert_eq!(sink.into_bytes()?, encode(&records, RecordFormat::Ptb64)?);

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
