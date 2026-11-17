#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "resource-contract fixtures use direct setup assertions"
)]

use std::assert_matches;
use std::hint::black_box;

use stab_records::{
    BitPlaneBatch, DetsLayout, DetsResultType, PackedShotBatch, RecordBatchPart, RecordEncoder,
    RecordFormat, RecordFraming, RecordResult, RecordStreamReadError, RecordStreamReader,
    RecordStreamStorage, write_ptb64_records_checked, write_records,
};

/// The packed reader's initial transport buffer.
const BUFFER_BYTES: u64 = 64 * 1024;
const BATCH_RECORDS: usize = 64;

/// Drains a packed reader in 64-record batches, touching every batch.
fn drain(mut reader: RecordStreamReader<&[u8]>) -> Result<(), RecordStreamReadError> {
    while let Some(batch) = reader.read_batch(BATCH_RECORDS)? {
        black_box(batch.shot_count());
    }
    Ok(())
}

fn measurements(
    input: &[u8],
    format: RecordFormat,
    width: usize,
) -> Result<(), RecordStreamReadError> {
    drain(RecordStreamReader::measurements(
        input,
        format,
        width,
        usize::MAX,
    ))
}

#[test]
fn packed_streaming_allocations_follow_width_not_record_count() -> RecordResult<()> {
    const WIDTH: usize = 193;
    const RECORDS: usize = 257;
    let record = (0usize..WIDTH)
        .map(|bit| (bit * 17).is_multiple_of(43))
        .collect::<Vec<_>>();

    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let one = write_records(std::slice::from_ref(&record), format)?;
        let many = one.repeat(RECORDS);
        assert_width_bounded(
            &format!("{format:?}"),
            WIDTH,
            || measurements(&one, format, WIDTH),
            || measurements(&many, format, WIDTH),
        );
    }

    let mut duplicate_hits = b"0".to_vec();
    for _ in 1..16_384 {
        duplicate_hits.extend_from_slice(b",0");
    }
    duplicate_hits.push(b'\n');
    assert_width_bounded(
        "HITS duplicate-heavy",
        1,
        || measurements(b"0\n", RecordFormat::Hits, 1),
        || measurements(&duplicate_hits, RecordFormat::Hits, 1),
    );

    let layout = DetsLayout::try_new(128, 64, 32)?;
    let one_dets_record = b"shot M0 M127 D0 D63 L0 L31\n".to_vec();
    let many_dets_records = one_dets_record.repeat(RECORDS);
    let dets = |input: &[u8]| drain(RecordStreamReader::dets(input, layout, usize::MAX));
    assert_width_bounded(
        "DETS",
        layout.total_bits(),
        || dets(&one_dets_record),
        || dets(&many_dets_records),
    );
    let duplicate_dets = duplicate_dets_record(16_384);
    assert_width_bounded(
        "DETS duplicate-heavy",
        layout.total_bits(),
        || dets(b"shot D0\n"),
        || dets(&duplicate_dets),
    );

    let ptb64_records = (0usize..256)
        .map(|shot| {
            (0usize..WIDTH)
                .map(|bit| (shot * 17 + bit * 31).is_multiple_of(97))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ptb64 = write_ptb64_records_checked(&ptb64_records)?;
    let one_ptb64_group = ptb64
        .get(..WIDTH * 8)
        .expect("four PTB64 groups contain a complete first group");
    assert_width_bounded(
        "PTB64",
        WIDTH,
        || measurements(one_ptb64_group, RecordFormat::Ptb64, WIDTH),
        || measurements(&ptb64, RecordFormat::Ptb64, WIDTH),
    );
    Ok(())
}

#[test]
fn declared_widths_beyond_the_input_fail_typed_without_width_sized_allocation() {
    // 2^40 bits is 128 GiB per record; none of these inputs proves that such a record exists.
    const HUGE: usize = 1 << 40;
    let cases: &[(RecordFormat, &[u8])] = &[
        (RecordFormat::B8, &[0xff; 10]),
        (RecordFormat::R8, &[0x00, 0x05]),
        (RecordFormat::Ptb64, &[0x01; 100]),
        (RecordFormat::ZeroOne, b"0101\n"),
        (RecordFormat::Hits, b"1,x\n"),
        (RecordFormat::Dets, b"shot Q1\n"),
    ];
    for (format, input) in cases {
        for framing in [RecordFraming::Stream, RecordFraming::Records] {
            let reader = || {
                RecordStreamReader::measurements(*input, *format, HUGE, usize::MAX)
                    .with_framing(framing)
            };
            let measured = allocation_counter::measure(|| {
                assert!(
                    drain(reader()).is_err(),
                    "{format:?} {framing:?} accepted a record its input cannot hold"
                );
            });
            assert!(
                measured.bytes_max <= BUFFER_BYTES + 4_096,
                "{format:?} {framing:?} allocated for the declared width: {measured:?}"
            );
        }
    }

    // A one-byte HITS record proves a full-width record exists; storage for it is reserved
    // fallibly, so an unsatisfiable width is a typed error rather than an abort.
    let error = measurements(b"\n", RecordFormat::Hits, usize::MAX / 2)
        .expect_err("unsatisfiable record storage");
    assert_matches!(error, RecordStreamReadError::Format(_));
}

/// Hands out at most `step` bytes per read, so a frame spans many refills.
struct Chunked<'a> {
    bytes: &'a [u8],
    step: usize,
}

impl std::io::Read for Chunked<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let step = self.step.min(self.bytes.len()).min(buffer.len());
        let (head, tail) = self.bytes.split_at(step);
        buffer
            .get_mut(..step)
            .expect("step fits the buffer")
            .copy_from_slice(head);
        self.bytes = tail;
        Ok(step)
    }
}

/// The admitted storage bounds of the packed reader and the encoder cover what a read-and-encode
/// loop really allocates, for worst-case frames (every `r8` bit set) arriving in partial reads
/// and for widths on both sides of the encoder's segment size.
#[test]
fn reader_and_encoder_storage_bounds_cover_peak_allocation() {
    for width in [1_usize, 70, 65_537] {
        let records = (0..64_usize)
            .map(|shot| {
                (0..width)
                    .map(|bit| shot % 2 == 0 || bit % 3 == 0)
                    .collect()
            })
            .collect::<Vec<Vec<bool>>>();
        let batch_records = (1_usize << 20)
            .checked_div(width.div_ceil(8))
            .unwrap_or(1)
            .clamp(1, 64);
        for in_format in [
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Ptb64,
            RecordFormat::ZeroOne,
            RecordFormat::Hits,
            RecordFormat::Dets,
        ] {
            let input = write_records(&records, in_format).expect("encode input");
            for out_format in [
                RecordFormat::ZeroOne,
                RecordFormat::B8,
                RecordFormat::R8,
                RecordFormat::Hits,
                RecordFormat::Dets,
                RecordFormat::Ptb64,
            ] {
                // Text readers bound their transport buffer through the per-record cap, which
                // here admits the longest line exactly.
                let text_cap = input
                    .split_inclusive(|byte| *byte == b'\n')
                    .map(<[u8]>::len)
                    .max()
                    .unwrap_or_default();
                let convert = || {
                    let mut reader = RecordStreamReader::measurements(
                        Chunked {
                            bytes: &input,
                            step: 4_093,
                        },
                        in_format,
                        width,
                        text_cap,
                    );
                    let mut encoder = RecordEncoder::try_new(out_format, width).expect("encoder");
                    let mut sink = std::io::sink();
                    while let Some(batch) = reader.read_batch(batch_records).expect("read") {
                        let part = RecordBatchPart::new(DetsResultType::Measurement, batch);
                        encoder.write_batch(&[part], &mut sink).expect("encode");
                    }
                };
                let measured = allocation_counter::measure(convert);
                let reader = RecordStreamStorage::new(in_format, width, batch_records);
                let text_buffer = if reader.transport_bytes() == 0 {
                    RecordStreamStorage::text_transport_bytes(text_cap)
                } else {
                    0
                };
                let bound = reader.bytes()
                    + text_buffer
                    + RecordEncoder::shot_major_scratch_bytes(out_format, width);
                assert!(
                    u128::from(measured.bytes_max) <= bound,
                    "{in_format:?} to {out_format:?} at width {width} peaked at {} bytes above \
                     the {bound}-byte bound: {measured:?}",
                    measured.bytes_max
                );
            }
        }
    }
    assert_eq!(PackedShotBatch::storage_bytes(3, 65), 48);
}

/// The CLI admits an output encoder's shot-major scratch for its known record count and its
/// bit-plane scratch before sampling into it, so encoding detection-shaped bit-plane batches
/// (detectors then observables, in parts of differing widths) must peak within those two
/// bounds whatever the format, including runs whose whole output is below the drain threshold.
#[test]
fn bit_plane_encoding_stays_within_admitted_encoder_scratch() {
    const SHOTS: usize = 256;
    for (detectors, observables) in [(1, 1), (9, 3), (257, 64), (5_003, 2), (70_001, 1)] {
        let records = |width: usize, salt: usize| {
            let rows = (0..SHOTS)
                .map(|shot| {
                    (0..width)
                        .map(|bit| (shot * 17 + bit * 31 + salt).is_multiple_of(43))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let packed = PackedShotBatch::from_records(&rows, width).expect("pack records");
            BitPlaneBatch::from_shot_major(packed.view()).expect("transpose")
        };
        let detector_planes = records(detectors, 0);
        let observable_planes = records(observables, 7);
        let parts = [
            RecordBatchPart::new(DetsResultType::Detector, detector_planes.view()),
            RecordBatchPart::new(DetsResultType::Observable, observable_planes.view()),
        ];
        let width = detectors + observables;
        for format in [
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Dets,
            RecordFormat::Ptb64,
        ] {
            let measured = allocation_counter::measure(|| {
                let mut encoder = RecordEncoder::try_new(format, width).expect("encoder");
                let mut sink = std::io::sink();
                for _ in 0..2 {
                    encoder.write_batch(&parts, &mut sink).expect("encode");
                }
            });
            let records = 2 * SHOTS as u64;
            let bound = RecordEncoder::shot_major_scratch_bytes_for_records(format, width, records)
                + RecordEncoder::bit_plane_scratch_bytes(format, &[detectors, observables]);
            assert!(
                bound
                    <= RecordEncoder::shot_major_scratch_bytes(format, width)
                        + RecordEncoder::bit_plane_scratch_bytes(format, &[detectors, observables])
            );
            assert!(
                u128::from(measured.bytes_max) <= bound,
                "{format:?} at {detectors}+{observables} bits peaked at {} bytes above the \
                 {bound}-byte bound: {measured:?}",
                measured.bytes_max
            );
        }
    }
}

/// `r8` records decode as their bytes arrive, so a record frame many times the transport buffer
/// costs one fixed buffer, the first record's row, and the batch rather than the frame.
#[test]
fn wide_r8_records_decode_through_a_fixed_transport_buffer() {
    const WIDTH: usize = 2_000_000;
    // Every bit set (a 2 MB frame), then no bit set (about 7.8 KB of 255 runs).
    let records = vec![vec![true; WIDTH], vec![false; WIDTH]];
    let input = write_records(&records, RecordFormat::R8).expect("encode r8");
    assert!(input.len() > 2_000_000);
    for step in [65_536, 4_093] {
        let read = || {
            let mut reader = RecordStreamReader::measurements(
                Chunked {
                    bytes: &input,
                    step,
                },
                RecordFormat::R8,
                WIDTH,
                0,
            );
            let mut set_bits = Vec::new();
            while let Some(batch) = reader.read_batch(1).expect("read r8") {
                let row = batch.shot(0).expect("one record");
                set_bits.push((0..WIDTH).filter(|bit| row.get(*bit) == Some(true)).count());
            }
            set_bits
        };
        let mut set_bits = Vec::new();
        let measured = allocation_counter::measure(|| set_bits = read());
        assert_eq!(set_bits, vec![WIDTH, 0]);
        let bound = RecordStreamStorage::new(RecordFormat::R8, WIDTH, 1).bytes();
        // The vector of counts the closure returns.
        let slack = 1_024;
        assert!(
            u128::from(measured.bytes_max) <= bound + slack,
            "step {step} peaked at {} bytes above the {bound}-byte bound",
            measured.bytes_max
        );
        assert!(bound < input.len() as u128 / 3);
    }
}

/// A text reader's transport buffer stays within [`RecordStreamStorage::text_transport_bytes`]
/// of its record cap for caps on both sides of the initial buffer and between its doublings,
/// for the longest line the cap accepts and for one byte more, whatever the read sizes.
#[test]
fn text_transport_buffer_stays_within_its_record_cap_bound() {
    const KIB: usize = 1024;
    // Diagnostic strings of a refused line.
    const DIAGNOSTIC_SLACK: u128 = 1_024;
    for cap in [
        1_000,
        64 * KIB - 1,
        64 * KIB,
        64 * KIB + 1,
        96 * KIB + 7,
        128 * KIB + 1,
    ] {
        for format in [RecordFormat::ZeroOne, RecordFormat::Hits] {
            for (line_len, accepted) in [(cap, true), (cap + 1, false)] {
                let (line, width) = match format {
                    RecordFormat::ZeroOne => {
                        let mut line = b"01".repeat(line_len / 2 + 1);
                        line.truncate(line_len - 1);
                        line.push(b'\n');
                        (line, line_len - 1)
                    }
                    // `0,0,...,0\n` toggles one bit; an odd length ends in `\r\n`.
                    _ => {
                        let mut line = b"0,".repeat(line_len / 2);
                        line.pop();
                        line.extend_from_slice(if line_len % 2 == 0 { b"\n" } else { b"\r\n" });
                        (line, 1)
                    }
                };
                assert_eq!(line.len(), line_len);
                for step in [1, 4_093, usize::MAX] {
                    let read = || {
                        let mut reader = RecordStreamReader::measurements(
                            Chunked { bytes: &line, step },
                            format,
                            width,
                            cap,
                        );
                        let mut records = 0;
                        let result = loop {
                            match reader.read_batch(BATCH_RECORDS) {
                                Ok(Some(batch)) => records += batch.shot_count(),
                                Ok(None) => break Ok(records),
                                Err(error) => break Err(error),
                            }
                        };
                        black_box(result)
                    };
                    let mut outcome = None;
                    let measured = allocation_counter::measure(|| outcome = Some(read()));
                    let outcome = outcome.expect("measured read ran");
                    assert_eq!(
                        outcome.is_ok_and(|records| records == 1),
                        accepted,
                        "{format:?} cap {cap} line {line_len} step {step}"
                    );
                    let bound = RecordStreamStorage::text_transport_bytes(cap)
                        + PackedShotBatch::storage_bytes(BATCH_RECORDS, width)
                        + DIAGNOSTIC_SLACK;
                    assert!(
                        u128::from(measured.bytes_max) <= bound,
                        "{format:?} cap {cap} line {line_len} step {step} peaked at {} bytes \
                         above the {bound}-byte bound",
                        measured.bytes_max
                    );
                }
            }
        }
    }
    // The cap a budget admits keeps the buffer within that budget.
    for budget in [0_u128, 1, 2, 3, 64 * 1024, 64 * 1024 + 1, 1 << 30] {
        let cap = RecordStreamStorage::max_text_record_bytes(budget);
        assert!(budget == 0 || RecordStreamStorage::text_transport_bytes(cap) <= budget);
        assert!(RecordStreamStorage::text_transport_bytes(cap + 1) > budget);
    }
}

fn duplicate_dets_record(repetitions: usize) -> Vec<u8> {
    let mut record = b"shot".to_vec();
    for index in 0..repetitions {
        record.extend_from_slice(match index % 3 {
            0 => b" M0",
            1 => b" D0",
            _ => b" L0",
        });
    }
    record.push(b'\n');
    record
}

/// One-record and many-record reads must allocate identically and stay within one transport
/// buffer plus width-derived batch and PTB64 group storage.
fn assert_width_bounded(
    reader: &str,
    width: usize,
    mut read_one: impl FnMut() -> Result<(), RecordStreamReadError>,
    mut read_many: impl FnMut() -> Result<(), RecordStreamReadError>,
) {
    read_one().expect("warm one-record streaming reader");
    read_many().expect("warm many-record streaming reader");
    let one = allocation_counter::measure(|| read_one().expect("measure one-record reader"));
    let many = allocation_counter::measure(|| read_many().expect("measure many-record reader"));
    let row_bytes = u64::try_from(width.div_ceil(64) * 8).expect("test width fits u64");
    let width_bound = BUFFER_BYTES
        .saturating_add(row_bytes.saturating_mul(2 * 64))
        .saturating_add(u64::try_from(width * 8).expect("test width fits u64"))
        .saturating_add(4_096);
    assert_eq!(
        many.count_total, one.count_total,
        "{reader} allocation count grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "{reader} allocated bytes grew with record count: one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_current, 0,
        "{reader} retained scratch after returning: {many:?}"
    );
    assert!(
        many.bytes_max <= width_bound,
        "{reader} exceeded width-derived scratch bound {width_bound}: {many:?}"
    );
}
