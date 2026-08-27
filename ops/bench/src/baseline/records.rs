use std::hint::black_box;

use stab_records::{
    BitPlane64Batch, DetectorWidth, DetsLayout, MeasurementBatchView, MeasurementCodecSink,
    MeasurementSink, MeasurementWidth, ObservableWidth, PackedShotBatch, RecordFormat,
    for_each_dets_packed_record, write_bit_plane_64_batch, write_ptb64_records_checked,
    write_records,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab, stab_runner_error};

const SHOTS: usize = 64;
const BITS_PER_SHOT: usize = 10_000;
const DETS_RECORDS: usize = 4_096;
const DETECTOR_WIDTH: usize = 64;
const OBSERVABLE_WIDTH: usize = 8;

const WRITE_B8: &str = "stab_records_write_packed_batch_b8_64x10000";
const WRITE_PTB64: &str = "stab_records_write_bit_plane_ptb64_64x10000";
const TO_BIT_PLANE: &str = "stab_records_shot_major_to_bit_plane_64x10000";
const TO_SHOT_MAJOR: &str = "stab_records_bit_plane_to_shot_major_10000x64";
const READ_DETS: &str = "stab_records_read_dets_packed_dl72_4096";

pub(super) fn run_record_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m8-record-writer-contract" => run_writer_row(row).map(Some),
        "m8-record-batch-transpose-contract" => run_transpose_row(row).map(Some),
        "m8-record-dets-layout-contract" => run_dets_layout_row(row).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m8-record-writer-contract", WRITE_B8 | WRITE_PTB64)
        | ("m8-record-batch-transpose-contract", TO_BIT_PLANE | TO_SHOT_MAJOR) => {
            Some(((SHOTS * BITS_PER_SHOT) as f64, "bits/s"))
        }
        ("m8-record-dets-layout-contract", READ_DETS) => Some((DETS_RECORDS as f64, "records/s")),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m8-record-writer-contract" => Some(
            "contract-only: Stab measures direct stable typed b8 and PTB64 component writers; pinned Stim has writer correctness tests but no equivalent public writer perf filter",
        ),
        "m8-record-batch-transpose-contract" => Some(
            "contract-only: Stab measures direct stable shot-major and bit-plane conversion; pinned Stim exposes the layout through internal SIMD batch machinery without an equivalent public component benchmark",
        ),
        "m8-record-dets-layout-contract" => Some(
            "contract-only: Stab measures layout-aware D/L DETS parsing over deterministic records; pinned Stim reader perf filters use a different untyped workload shape",
        ),
        _ => None,
    }
}

fn run_writer_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let batch = patterned_batch(row)?;
    let planes = BitPlane64Batch::from_shot_major(batch.view())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    validate_writer_outputs(row, &batch, &planes)?;
    Ok(vec![
        measure_stab(WRITE_B8, || {
            let mut sink = MeasurementCodecSink::try_new(
                RecordFormat::B8,
                MeasurementWidth::new(BITS_PER_SHOT),
            )
            .map_err(|error| stab_runner_error(&row.id, error))?;
            sink.write_batch(MeasurementBatchView::new(batch.view()))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let bytes = sink
                .into_bytes()
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes);
            Ok(())
        })?,
        measure_stab(WRITE_PTB64, || {
            let bytes = write_bit_plane_64_batch(planes.view())
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(bytes);
            Ok(())
        })?,
    ])
}

fn run_transpose_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let batch = patterned_batch(row)?;
    let planes = BitPlane64Batch::from_shot_major(batch.view())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let round_trip = PackedShotBatch::from_bit_planes(planes.view())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if round_trip != batch {
        return Err(stab_runner_error(
            &row.id,
            "shot-major and bit-plane preflight did not round trip",
        ));
    }
    Ok(vec![
        measure_stab(TO_BIT_PLANE, || {
            let output = BitPlane64Batch::from_shot_major(batch.view())
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(output);
            Ok(())
        })?,
        measure_stab(TO_SHOT_MAJOR, || {
            let output = PackedShotBatch::from_bit_planes(planes.view())
                .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(output);
            Ok(())
        })?,
    ])
}

fn run_dets_layout_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let input = dets_input();
    let layout = DetsLayout::from_widths(
        MeasurementWidth::new(0),
        DetectorWidth::new(DETECTOR_WIDTH),
        ObservableWidth::new(OBSERVABLE_WIDTH),
    )
    .map_err(|error| stab_runner_error(&row.id, error))?;
    let (record_count, set_bits) = dets_counts(&input, layout, row)?;
    if record_count != DETS_RECORDS || set_bits != DETS_RECORDS * 9 {
        return Err(stab_runner_error(
            &row.id,
            format!("typed DETS preflight decoded {record_count} records and {set_bits} set bits"),
        ));
    }
    Ok(vec![measure_stab(READ_DETS, || {
        let (_, set_bits) = dets_counts(&input, layout, row)?;
        black_box(set_bits);
        Ok(())
    })?])
}

fn validate_writer_outputs(
    row: &BenchmarkRow,
    batch: &PackedShotBatch,
    planes: &BitPlane64Batch,
) -> Result<(), BenchError> {
    let records = batch
        .to_records()
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let mut sink =
        MeasurementCodecSink::try_new(RecordFormat::B8, MeasurementWidth::new(BITS_PER_SHOT))
            .map_err(|error| stab_runner_error(&row.id, error))?;
    sink.write_batch(MeasurementBatchView::new(batch.view()))
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let actual_b8 = sink
        .into_bytes()
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let expected_b8 = write_records(&records, RecordFormat::B8)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    if actual_b8 != expected_b8 {
        return Err(stab_runner_error(
            &row.id,
            "typed B8 writer preflight disagreed with the established adapter",
        ));
    }

    let actual_ptb64 = write_bit_plane_64_batch(planes.view())
        .map_err(|error| stab_runner_error(&row.id, error))?;
    let expected_ptb64 =
        write_ptb64_records_checked(&records).map_err(|error| stab_runner_error(&row.id, error))?;
    if actual_ptb64 != expected_ptb64 {
        return Err(stab_runner_error(
            &row.id,
            "typed PTB64 writer preflight disagreed with the established adapter",
        ));
    }
    Ok(())
}

fn dets_counts(
    input: &[u8],
    layout: DetsLayout,
    row: &BenchmarkRow,
) -> Result<(usize, usize), BenchError> {
    let mut record_count = 0_usize;
    let mut set_bits = 0_usize;
    for_each_dets_packed_record(input, layout, |record| {
        record_count = record_count.saturating_add(1);
        set_bits = set_bits.saturating_add(record.popcount());
        Ok(())
    })
    .map_err(|error| stab_runner_error(&row.id, error))?;
    Ok((record_count, set_bits))
}

fn patterned_batch(row: &BenchmarkRow) -> Result<PackedShotBatch, BenchError> {
    let mut batch = PackedShotBatch::zeros(SHOTS, BITS_PER_SHOT)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    for shot in 0..SHOTS {
        for bit in 0..BITS_PER_SHOT {
            let value = (shot * 17 + bit * 31 + 7).is_multiple_of(97);
            batch
                .set(shot, bit, value)
                .map_err(|error| stab_runner_error(&row.id, error))?;
        }
    }
    Ok(batch)
}

fn dets_input() -> Vec<u8> {
    let mut input = Vec::with_capacity(DETS_RECORDS * 48);
    for shot in 0..DETS_RECORDS {
        input.extend_from_slice(b"shot");
        for offset in 0..8 {
            input.extend_from_slice(b" D");
            input.extend_from_slice(
                ((shot * 13 + offset * 17) % DETECTOR_WIDTH)
                    .to_string()
                    .as_bytes(),
            );
        }
        input.extend_from_slice(b" L");
        input.extend_from_slice((shot % OBSERVABLE_WIDTH).to_string().as_bytes());
        input.push(b'\n');
    }
    input
}

#[cfg(test)]
mod tests {
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{BenchmarkRow, Milestone, Runner, ThresholdClass};

    use super::{compare_note, measurement_work, run_record_compare_row};

    #[test]
    fn record_rows_have_direct_component_runners_and_work_units() {
        for (id, names) in [
            (
                "m8-record-writer-contract",
                &[
                    "stab_records_write_packed_batch_b8_64x10000",
                    "stab_records_write_bit_plane_ptb64_64x10000",
                ][..],
            ),
            (
                "m8-record-batch-transpose-contract",
                &[
                    "stab_records_shot_major_to_bit_plane_64x10000",
                    "stab_records_bit_plane_to_shot_major_10000x64",
                ][..],
            ),
            (
                "m8-record-dets-layout-contract",
                &["stab_records_read_dets_packed_dl72_4096"][..],
            ),
        ] {
            let row = BenchmarkRow {
                id: id.to_string(),
                milestone: Milestone::M8,
                threshold_class: ThresholdClass::ReportOnly,
                runner: Runner::ContractOnly,
                upstream_source: "src/stim/io/measure_record_reader.test.cc".to_string(),
                stim_perf_filter: String::new(),
                argv: String::new(),
                stdin_path: String::new(),
                phase: "throughput".to_string(),
                measurement: "result-records".to_string(),
                description: "test row".to_string(),
                comparability: ComparabilityClass::ContractOnly,
            };
            let measurements = run_record_compare_row(&row)
                .expect("record row")
                .expect("record runner");
            assert_eq!(
                measurements
                    .iter()
                    .map(|measurement| measurement.name.as_str())
                    .collect::<Vec<_>>(),
                names
            );
            assert!(compare_note(id).is_some());
            for name in names {
                assert!(measurement_work(id, name).is_some());
            }
        }
        assert_eq!(
            measurement_work(
                "m8-record-dets-layout-contract",
                "stab_records_read_dets_packed_dl72_4096"
            ),
            Some((4_096.0, "records/s"))
        );
    }
}
