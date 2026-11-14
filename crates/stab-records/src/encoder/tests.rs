#![allow(
    clippy::expect_used,
    reason = "encoder unit tests use direct fixture assertions"
)]

use std::io::{self, Write};

use super::{DRAIN_THRESHOLD_BYTES, RecordBatchPart, RecordEncoder, RecordPart, SEGMENT_BITS};
use crate::{BitPlaneBatch, DetsResultType, PackedShotBatch, RecordFormat};

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
            let records = (0_usize..256)
                .map(|shot| {
                    (0_usize..width)
                        .map(|bit| (shot * 17 + bit * 31).is_multiple_of(43))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let packed = PackedShotBatch::from_records(&records, width).expect("pack records");
            let planes = BitPlaneBatch::from_shot_major(packed.view()).expect("transpose");
            let parts = [RecordBatchPart::new(
                DetsResultType::Measurement,
                planes.view(),
            )];
            let mut encoder = RecordEncoder::try_new(format, width).expect("encoder");
            let mut sink = io::sink();
            encoder
                .write_batch(&parts, &mut sink)
                .expect("warm encoder storage");
            let allocations = allocation_counter::measure(|| {
                for _ in 0..8 {
                    encoder
                        .write_batch(&parts, &mut sink)
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

/// Batch encoding splits a record part wider than `SEGMENT_BITS` into word-aligned segments and
/// drains between them; the bytes must equal the unsplit single-record encoding in every
/// format, including DETS indices that continue across segments and B8 parts that start
/// mid-byte, while one wide record never holds its whole encoding.
#[test]
fn wide_parts_encode_in_segments_exactly_like_whole_records() {
    // Wide enough that the `01` encoding alone exceeds the pending bound asserted below.
    let wide = 80 * SEGMENT_BITS + 77;
    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        for (leading, density) in [(0_usize, 3_usize), (5, 997)] {
            let width = leading + wide;
            let bits = (0..width)
                .map(|bit| (bit * 7 + bit / 64).is_multiple_of(density))
                .collect::<Vec<_>>();
            let (head_bits, body_bits) = bits.split_at(leading);
            let head = PackedShotBatch::from_records(&[head_bits.to_vec()], leading)
                .expect("pack leading part");
            let body =
                PackedShotBatch::from_records(&[body_bits.to_vec()], wide).expect("pack wide part");
            let parts = [
                RecordBatchPart::new(DetsResultType::Observable, head.view()),
                RecordBatchPart::new(DetsResultType::Detector, body.view()),
            ];
            let mut segmented = Vec::new();
            RecordEncoder::try_new(format, width)
                .expect("encoder")
                .write_batch(&parts, &mut segmented)
                .expect("encode batch");
            let mut whole = Vec::new();
            RecordEncoder::try_new(format, width)
                .expect("encoder")
                .write_record(
                    &[
                        RecordPart::new(DetsResultType::Observable, head.shot(0).expect("head")),
                        RecordPart::new(DetsResultType::Detector, body.shot(0).expect("body")),
                    ],
                    &mut whole,
                )
                .expect("encode record");
            assert_eq!(
                segmented, whole,
                "{format:?} leading {leading} density {density}"
            );
            let mut counted = CountingWriter::default();
            let mut encoder = RecordEncoder::try_new(format, width).expect("encoder");
            encoder
                .write_batch(&parts, &mut counted)
                .expect("encode batch");
            assert!(
                encoder.pending.capacity() <= 4 * super::SEGMENT_DRAIN_BYTES,
                "{format:?}: a wide record kept {} bytes pending",
                encoder.pending.capacity()
            );
        }
    }
}
