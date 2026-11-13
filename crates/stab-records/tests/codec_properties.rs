#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "codec properties report the exact format, width, and bytes that violated a contract"
)]

//! Generated properties for every result-record codec.
//!
//! Hostile bytes at arbitrary widths must decode to records or a typed `FormatError` through
//! every reader, and all readers (materialized, dense, packed, sparse, chunked streaming, and the
//! raw b8 frame path) must agree on acceptance and content. Written records must match
//! independent reference encoders and decode back unchanged.

use std::io::Read;

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_bits::BitSlice;
use stab_records::{
    BitPlane64Batch, DetsLayout, DetsResultType, MeasureRecordWriter, PackedShotBatch,
    RecordFormat, RecordResult, RecordStreamError, RecordStreamReader, for_each_dets_packed_record,
    for_each_dets_record, for_each_dets_sparse_shot, for_each_dets_token_record,
    for_each_packed_record, for_each_ptb64_record, for_each_record, for_each_sparse_record,
    read_dets_records, read_measurement_records, read_ptb64_records, read_ptb64_records_all,
    read_records, try_for_each_record, write_bit_plane_64_batch, write_records,
};

const FORMATS: [RecordFormat; 6] = [
    RecordFormat::ZeroOne,
    RecordFormat::B8,
    RecordFormat::R8,
    RecordFormat::Hits,
    RecordFormat::Dets,
    RecordFormat::Ptb64,
];

/// Delivers two short reads at salt-chosen boundaries, then the remainder, so streaming decoders
/// must resume frames split across transport reads. Long trickles are avoided because each refill
/// grows the stream buffer by a fixed chunk.
struct Chunked<'a> {
    data: &'a [u8],
    limits: [usize; 2],
    reads: usize,
}

impl<'a> Chunked<'a> {
    fn new(data: &'a [u8], salt: usize) -> Self {
        Self {
            data,
            limits: [1 + salt % 11, 1 + salt % 29],
            reads: 0,
        }
    }
}

impl Read for Chunked<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let limit = self.limits.get(self.reads).copied().unwrap_or(usize::MAX);
        self.reads += 1;
        let count = limit.min(buf.len()).min(self.data.len());
        let (head, tail) = self.data.split_at(count);
        buf.get_mut(..count)
            .expect("count is bounded by the buffer length")
            .copy_from_slice(head);
        self.data = tail;
        Ok(count)
    }
}

fn width() -> BoxedStrategy<usize> {
    prop_oneof![
        6 => 0_usize..=300,
        3 => prop::sample::select(vec![1_usize, 7, 8, 9, 63, 64, 65, 127, 128, 254, 255, 256, 257, 510, 511, 512]),
        1 => 300_usize..=5000,
    ]
    .boxed()
}

fn hostile_bytes() -> BoxedStrategy<Vec<u8>> {
    let text = prop::sample::select(b"01\n\r \t,MDLshot0123456789-+x\x00\xff".to_vec());
    let binary = prop_oneof![
        3 => prop::sample::select(vec![0x00_u8, 0x01, 0x02, 0x07, 0x08, 0x80, 0xfe, 0xff]),
        1 => any::<u8>(),
    ];
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..200),
        prop::collection::vec(text, 0..200),
        prop::collection::vec(binary, 0..200),
    ]
    .boxed()
}

/// Inputs whose length lands on or near a whole number of ptb64 groups for the chosen width.
fn ptb64_shaped() -> impl Strategy<Value = (usize, Vec<u8>)> {
    (1_usize..=40, 0_usize..=2, -9_i32..=9).prop_flat_map(|(width, groups, delta)| {
        let exact = width * 8 * groups;
        let len =
            usize::try_from(i64::try_from(exact).expect("small") + i64::from(delta)).unwrap_or(0);
        (Just(width), prop::collection::vec(any::<u8>(), len))
    })
}

fn config(seed: u64, cases: u32) -> Config {
    Config {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    }
}

fn bits(record: BitSlice<'_>) -> Vec<bool> {
    (0..record.len())
        .map(|index| record.get(index).expect("index below record length"))
        .collect()
}

fn sparse_to_dense(indices: &[u64], width: usize, toggle: bool) -> Option<Vec<bool>> {
    let mut dense = vec![false; width];
    for index in indices {
        let bit = dense.get_mut(usize::try_from(*index).ok()?)?;
        *bit = if toggle { !*bit } else { true };
    }
    Some(dense)
}

fn stream_all(mut reader: RecordStreamReader<Chunked<'_>>) -> Result<Vec<Vec<bool>>, String> {
    let mut records = Vec::new();
    loop {
        match reader.next_record() {
            Ok(Some(record)) => records.push(record.to_vec()),
            Ok(None) => return Ok(records),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn assert_same(
    reader: &str,
    actual: Result<Vec<Vec<bool>>, String>,
    expected: &RecordResult<Vec<Vec<bool>>>,
    context: &str,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(&actual, expected, "{reader}: {context}"),
        (Err(_), Err(_)) => {}
        (actual, expected) => panic!(
            "{reader} disagreed with read_records on acceptance: {context}\n{reader}: {actual:?}\nread_records: {expected:?}"
        ),
    }
}

/// Checks every generic measurement-shaped reader against `read_records`.
fn check_generic_readers(format: RecordFormat, width: usize, input: &[u8]) {
    let context = format!(
        "format={} width={width} input_len={}",
        format.as_str(),
        input.len()
    );
    let expected = read_records(input, format, width);
    if let Ok(records) = &expected {
        assert!(
            records.iter().all(|record| record.len() == width),
            "decoded record width differs: {context}"
        );
    }

    assert_same(
        "read_measurement_records",
        read_measurement_records(input, format, width).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut dense = Vec::new();
    let result = for_each_record(input, format, width, |record| {
        dense.push(record.to_vec());
        Ok(())
    });
    assert_same(
        "for_each_record",
        result.map(|()| dense).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut packed = Vec::new();
    let result = for_each_packed_record(input, format, width, |record| {
        packed.push(bits(record));
        Ok(())
    });
    assert_same(
        "for_each_packed_record",
        result.map(|()| packed).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut sparse = Vec::new();
    let result = for_each_sparse_record(input, format, width, |record| {
        sparse.push(
            sparse_to_dense(record, width, format == RecordFormat::Hits)
                .expect("sparse index exceeded the declared width"),
        );
        Ok(())
    });
    assert_same(
        "for_each_sparse_record",
        result.map(|()| sparse).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let streamed = stream_all(RecordStreamReader::measurements(
        Chunked::new(input, input.len() * 31 + width),
        format,
        width,
        usize::MAX,
    ));
    assert_same("RecordStreamReader", streamed, &expected, &context);

    if format == RecordFormat::B8 {
        let mut reader = RecordStreamReader::measurements(
            Chunked::new(input, input.len() * 7 + width + 5),
            format,
            width,
            usize::MAX,
        );
        let mut frames = Vec::new();
        let framed = loop {
            match reader.next_b8_packed_record() {
                Ok(Some(frame)) => frames.push(
                    (0..width)
                        .map(|bit| {
                            frame
                                .get(bit / 8)
                                .is_some_and(|byte| byte >> (bit % 8) & 1 == 1)
                        })
                        .collect(),
                ),
                Ok(None) => break Ok(frames),
                Err(error) => break Err(error.to_string()),
            }
        };
        assert_same("next_b8_packed_record", framed, &expected, &context);
    }

    // A visitor error cancels decoding immediately after the first delivered record.
    let mut visited = 0_usize;
    let cancelled = try_for_each_record(input, format, width, |_| {
        visited += 1;
        Err("stop")
    });
    match (&expected, cancelled) {
        (Ok(records), Ok(())) => assert!(records.is_empty(), "visitor error ignored: {context}"),
        (Ok(records), Err(RecordStreamError::Visitor(_))) => {
            assert!(!records.is_empty(), "{context}");
            assert_eq!(
                visited, 1,
                "decoding continued after cancellation: {context}"
            );
        }
        (_, Err(RecordStreamError::Format(_)) | Ok(())) | (Err(_), Err(_)) => {
            assert!(
                visited <= 1,
                "decoding continued after cancellation: {context}"
            );
        }
    }
}

fn check_dets_layout_readers(layout: DetsLayout, input: &[u8]) {
    let context = format!("layout={layout:?} input_len={}", input.len());
    let expected = read_dets_records(input, layout);

    let mut dense = Vec::new();
    let result = for_each_dets_record(input, layout, |record| {
        dense.push(record.to_vec());
        Ok(())
    });
    assert_same(
        "for_each_dets_record",
        result.map(|()| dense).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut packed = Vec::new();
    let result = for_each_dets_packed_record(input, layout, |record| {
        packed.push(bits(record));
        Ok(())
    });
    assert_same(
        "for_each_dets_packed_record",
        result.map(|()| packed).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut tokens = Vec::new();
    let result = for_each_dets_token_record(input, layout, |record| {
        let mut dense = vec![false; layout.total_bits()];
        for token in record {
            let offset = match token.result_type() {
                DetsResultType::Measurement => 0,
                DetsResultType::Detector => layout.measurements(),
                DetsResultType::Observable => layout.measurements() + layout.detectors(),
            };
            *dense
                .get_mut(offset + token.index())
                .expect("token index exceeded its namespace") = true;
        }
        tokens.push(dense);
        Ok(())
    });
    assert_same(
        "for_each_dets_token_record",
        result.map(|()| tokens).map_err(|error| error.to_string()),
        &expected,
        &context,
    );

    let mut shots = 0_usize;
    let sparse = for_each_dets_sparse_shot(input, layout, |_| {
        shots += 1;
        Ok(())
    });
    match (&expected, sparse) {
        (Ok(records), Ok(())) => assert_eq!(records.len(), shots, "{context}"),
        (Err(_), Err(_)) => {}
        (expected, sparse) => {
            panic!("SparseShot acceptance differs: {context}\n{expected:?}\n{sparse:?}")
        }
    }

    let streamed = stream_all(RecordStreamReader::dets(
        Chunked::new(input, input.len() * 13 + layout.total_bits()),
        layout,
        usize::MAX,
    ));
    assert_same("RecordStreamReader::dets", streamed, &expected, &context);
}

proptest! {
    #![proptest_config(config(0xc0de_c0de_0001, 2048))]

    #[test]
    fn hostile_bytes_agree_across_every_generic_reader(
        format in prop::sample::select(FORMATS.to_vec()),
        width in width(),
        input in hostile_bytes(),
    ) {
        check_generic_readers(format, width, &input);
    }

    #[test]
    fn hostile_dets_bytes_agree_across_every_typed_reader(
        measurements in 0_usize..40,
        detectors in 0_usize..40,
        observables in 0_usize..40,
        input in hostile_bytes(),
    ) {
        let layout = DetsLayout::try_new(measurements, detectors, observables).expect("layout");
        check_dets_layout_readers(layout, &input);
    }

    #[test]
    fn hostile_ptb64_groups_agree_across_shot_bounded_readers(
        (width, input) in ptb64_shaped(),
        max_shots in prop_oneof![0_usize..=200, Just(64), Just(128), Just(192)],
    ) {
        check_generic_readers(RecordFormat::Ptb64, width, &input);
        let context = format!("width={width} max_shots={max_shots} input_len={}", input.len());
        let bounded = read_ptb64_records(&input, width, max_shots);
        let mut visited = Vec::new();
        let streamed = for_each_ptb64_record(&input, width, max_shots, |record| {
            visited.push(record.to_vec());
            Ok(())
        });
        assert_same(
            "for_each_ptb64_record",
            streamed.map(|()| visited).map_err(|error| error.to_string()),
            &bounded,
            &context,
        );
        if let Ok(records) = &bounded {
            prop_assert_eq!(records.len(), max_shots, "{}", context);
            let whole_groups = input
                .get(..input.len() - input.len() % (8 * width))
                .expect("prefix within input");
            let all = read_ptb64_records_all(whole_groups, width).expect("whole groups decode");
            prop_assert_eq!(all.get(..max_shots), Some(records.as_slice()), "{}", context);
        }
    }
}

fn reference_b8(records: &[Vec<bool>]) -> Vec<u8> {
    let mut out = Vec::new();
    for record in records {
        let mut bytes = vec![0_u8; record.len().div_ceil(8)];
        for (index, bit) in record.iter().enumerate() {
            if *bit {
                *bytes.get_mut(index / 8).expect("byte in record") |= 1 << (index % 8);
            }
        }
        out.extend(bytes);
    }
    out
}

/// Stim r8: each set bit emits the count of zeros before it; a run of 255 zeros emits `0xFF`
/// and continues; the record ends with the count of trailing zeros.
fn reference_r8(records: &[Vec<bool>]) -> Vec<u8> {
    let mut out = Vec::new();
    for record in records {
        let mut run = 0_u8;
        for bit in record {
            if *bit {
                out.push(run);
                run = 0;
            } else {
                run += 1;
                if run == u8::MAX {
                    out.push(u8::MAX);
                    run = 0;
                }
            }
        }
        out.push(run);
    }
    out
}

fn reference_ptb64(records: &[Vec<bool>], width: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for group in records.chunks(64) {
        for bit in 0..width {
            let mut word = 0_u64;
            for (shot, record) in group.iter().enumerate() {
                if record.get(bit).copied().unwrap_or(false) {
                    word |= 1 << shot;
                }
            }
            out.extend_from_slice(&word.to_le_bytes());
        }
    }
    out
}

fn reference_text(records: &[Vec<bool>], format: RecordFormat) -> Vec<u8> {
    let mut out = String::new();
    for record in records {
        let hits = record
            .iter()
            .enumerate()
            .filter(|(_, bit)| **bit)
            .map(|(index, _)| index);
        match format {
            RecordFormat::ZeroOne => {
                out.extend(record.iter().map(|bit| if *bit { '1' } else { '0' }));
            }
            RecordFormat::Hits => {
                out.push_str(
                    &hits
                        .map(|index| index.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            _ => {
                out.push_str("shot");
                for index in hits {
                    out.push_str(&format!(" M{index}"));
                }
            }
        }
        out.push('\n');
    }
    out.into_bytes()
}

/// Records with a chosen density, so sparse widths produce long r8 `0xFF` chains and dense ones
/// produce runs that end exactly at the record width.
fn records(width: usize, count: usize) -> impl Strategy<Value = Vec<Vec<bool>>> {
    prop_oneof![Just(0_u32), Just(1_u32), 2_u32..=255, Just(256_u32)].prop_flat_map(
        move |density| {
            prop::collection::vec(
                prop::collection::vec((0_u32..256).prop_map(move |roll| roll < density), width),
                count,
            )
        },
    )
}

fn round_trip_case() -> impl Strategy<Value = (RecordFormat, usize, Vec<Vec<bool>>)> {
    (
        prop::sample::select(FORMATS.to_vec()),
        prop_oneof![
            4 => 0_usize..=300,
            2 => prop::sample::select(vec![1_usize, 8, 254, 255, 256, 509, 510, 511]),
        ],
        0_usize..5,
    )
        .prop_flat_map(|(format, width, count)| {
            let count = if format == RecordFormat::Ptb64 {
                count.min(3) * 64
            } else {
                count
            };
            (Just(format), Just(width), records(width, count))
        })
}

proptest! {
    #![proptest_config(config(0xc0de_c0de_0002, 512))]

    #[test]
    fn written_records_match_reference_encoders_and_decode_back(
        (format, width, records) in round_trip_case(),
    ) {
        let context = format!("format={} width={width} records={}", format.as_str(), records.len());
        let encoded = write_records(&records, format).expect("records encode");
        let reference = match format {
            RecordFormat::B8 => reference_b8(&records),
            RecordFormat::R8 => reference_r8(&records),
            RecordFormat::Ptb64 => reference_ptb64(&records, width),
            _ => reference_text(&records, format),
        };
        prop_assert_eq!(&encoded, &reference, "{}", context);

        let batch_encoded = if format == RecordFormat::Ptb64 {
            let mut out = Vec::new();
            for group in records.chunks(64) {
                let packed = PackedShotBatch::from_records(group, width).expect("packed group");
                let planes = BitPlane64Batch::from_shot_major(packed.view()).expect("bit planes");
                out.extend(write_bit_plane_64_batch(planes.view()).expect("bit-plane encode"));
            }
            out
        } else {
            let packed = PackedShotBatch::from_records(&records, width).expect("packed batch");
            let mut writer = MeasureRecordWriter::try_new(format).expect("per-record writer");
            writer.write_packed_batch(packed.view()).expect("packed encode");
            writer.into_bytes()
        };
        prop_assert_eq!(&batch_encoded, &encoded, "packed writer differs: {}", context);

        // Zero-width b8 and ptb64 records carry no bytes, so their count is not recoverable.
        if width == 0 && matches!(format, RecordFormat::B8 | RecordFormat::Ptb64) {
            prop_assert!(read_records(&encoded, format, width).is_err(), "{}", context);
            return Ok(());
        }
        prop_assert_eq!(
            &read_records(&encoded, format, width).expect("decode"),
            &records,
            "{}",
            context
        );
        prop_assert_eq!(
            stream_all(RecordStreamReader::measurements(
                Chunked::new(&encoded, encoded.len() + width),
                format,
                width,
                usize::MAX,
            ))
            .expect("stream decode"),
            records,
            "{}",
            context
        );
    }
}
