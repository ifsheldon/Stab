#![allow(
    clippy::expect_used,
    reason = "encoder unit tests use direct fixture assertions"
)]

use std::io::{self, Write};

use super::{DRAIN_THRESHOLD_BYTES, RecordBatchPart, RecordEncoder};
use crate::{BitPlane64Batch, DetsResultType, PackedShotBatch, RecordFormat};

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
    writes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes += buffer.len();
        self.writes += 1;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn large_batches_drain_in_bounded_chunks_instead_of_per_record() {
    let records = (0..20_000)
        .map(|shot| (0..37).map(|bit| (shot + bit) % 5 == 0).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let batch = PackedShotBatch::from_records(&records, 37).expect("batch");
    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let mut encoder = RecordEncoder::try_new(format, 37).expect("encoder");
        let mut out = CountingWriter::default();
        encoder
            .write_batch(
                &[RecordBatchPart::new(
                    DetsResultType::Measurement,
                    batch.view(),
                )],
                &mut out,
            )
            .expect("encode");
        assert!(out.bytes > 0);
        assert!(
            out.writes <= out.bytes / DRAIN_THRESHOLD_BYTES + 1,
            "{format:?}: {} writes for {} bytes",
            out.writes,
            out.bytes
        );
        assert!(encoder.pending.capacity() <= 2 * DRAIN_THRESHOLD_BYTES);
    }
}

#[test]
fn repeated_bit_plane_batches_reuse_encoder_storage() {
    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
        RecordFormat::Ptb64,
    ] {
        for width in [1, 9, 64, 257, 1_009] {
            let records = (0_usize..64)
                .map(|shot| {
                    (0_usize..width)
                        .map(|bit| (shot * 17 + bit * 31).is_multiple_of(43))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let packed = PackedShotBatch::from_records(&records, width).expect("pack records");
            let planes = BitPlane64Batch::from_shot_major(packed.view()).expect("transpose");
            let mut encoder = RecordEncoder::try_new(format, width).expect("encoder");
            let mut sink = io::sink();
            encoder
                .write_bit_planes(planes.view(), DetsResultType::Measurement, &mut sink)
                .expect("warm encoder storage");
            let allocations = allocation_counter::measure(|| {
                for _ in 0..8 {
                    encoder
                        .write_bit_planes(planes.view(), DetsResultType::Measurement, &mut sink)
                        .expect("reuse encoder storage");
                }
            });
            assert_eq!(
                allocations.count_total, 0,
                "{format:?} width {width}: {allocations:?}"
            );
        }
    }
}
