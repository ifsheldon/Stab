#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "codec properties report the exact format, width, and bytes that violated a contract, and the reference decoder indexes inputs whose bounds it has just checked"
)]

//! Generated properties for every result-record codec.
//!
//! Hostile bytes at arbitrary widths must decode to records or a typed error through every
//! reader. The materialized readers and the packed streaming reader, under both framings and at
//! arbitrary batch sizes and transport splits, must agree with a simple per-bit reference decoder
//! on acceptance and content, and the streaming reader must report the same diagnostic code as
//! the materialized reader. Written records must match independent reference encoders and decode
//! back unchanged.

use std::io::Read;

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_records::{
    BitPlane64Batch, DetsLayout, DetsResultType, FormatErrorCode, PackedShotBatch, RecordBatchPart,
    RecordEncoder, RecordFormat, RecordFraming, RecordStreamReadError, RecordStreamReader,
    read_dets_records, read_measurement_records, read_ptb64_records, read_ptb64_records_all,
    read_records, write_bit_plane_64_batch, write_records,
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
/// must resume frames split across transport reads.
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

/// Drains a packed reader in batches of `batch` records, returning every record as dense bits,
/// or the first failure.
fn stream_all<R: Read>(
    mut reader: RecordStreamReader<R>,
    batch: usize,
) -> Result<Vec<Vec<bool>>, RecordStreamReadError> {
    let mut records = Vec::new();
    while let Some(view) = reader.read_batch(batch)? {
        for shot in 0..view.shot_count() {
            let row = view.shot(shot)?;
            records.push(
                (0..row.len())
                    .map(|bit| row.get(bit) == Some(true))
                    .collect(),
            );
        }
    }
    Ok(records)
}

fn error_code(error: &RecordStreamReadError) -> Option<FormatErrorCode> {
    match error {
        RecordStreamReadError::Format(error)
        | RecordStreamReadError::Record { source: error, .. }
        | RecordStreamReadError::UnterminatedRecord { source: error, .. } => Some(error.code()),
        _ => None,
    }
}

/// A deliberately simple per-bit decoder for one whole input, written from the format grammar
/// rather than from the product decoders. `None` means the input is malformed.
fn reference_decode(input: &[u8], format: RecordFormat, width: usize) -> Option<Vec<Vec<bool>>> {
    match format {
        RecordFormat::ZeroOne => reference_zero_one(input, width),
        RecordFormat::B8 => {
            let bytes = width.div_ceil(8);
            if bytes == 0 || !input.len().is_multiple_of(bytes) {
                return None;
            }
            Some(
                input
                    .chunks(bytes)
                    .map(|chunk| {
                        (0..width)
                            .map(|bit| chunk[bit / 8] >> (bit % 8) & 1 == 1)
                            .collect()
                    })
                    .collect(),
            )
        }
        RecordFormat::R8 => reference_r8_decode(input, width),
        RecordFormat::Hits => reference_hits(input, width),
        RecordFormat::Dets => reference_dets(input, width),
        RecordFormat::Ptb64 => {
            if width == 0 || !input.len().is_multiple_of(8 * width) {
                return None;
            }
            let mut records = Vec::new();
            for group in input.chunks(8 * width) {
                for shot in 0..64 {
                    records.push(
                        (0..width)
                            .map(|bit| group[bit * 8 + shot / 8] >> (shot % 8) & 1 == 1)
                            .collect(),
                    );
                }
            }
            Some(records)
        }
        other => panic!("no reference decoder for {other:?}"),
    }
}

/// Consumes a record terminator: `\n`, or `\r\n`.
fn reference_newline(input: &[u8], offset: &mut usize) -> Option<()> {
    match input.get(*offset)? {
        b'\n' => *offset += 1,
        b'\r' if input.get(*offset + 1) == Some(&b'\n') => *offset += 2,
        _ => return None,
    }
    Some(())
}

fn reference_zero_one(input: &[u8], width: usize) -> Option<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0;
    while offset < input.len() {
        let mut record = Vec::new();
        for _ in 0..width {
            match input.get(offset)? {
                b'0' => record.push(false),
                b'1' => record.push(true),
                _ => return None,
            }
            offset += 1;
        }
        reference_newline(input, &mut offset)?;
        records.push(record);
    }
    Some(records)
}

fn reference_r8_decode(input: &[u8], width: usize) -> Option<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0;
    while offset < input.len() {
        let mut record = vec![false; width];
        let mut position = 0;
        loop {
            let run = usize::from(*input.get(offset)?);
            offset += 1;
            position += run;
            if position > width {
                return None;
            }
            if run == 255 {
                continue;
            }
            if position == width {
                break;
            }
            record[position] = true;
            position += 1;
        }
        records.push(record);
    }
    Some(records)
}

fn reference_unsigned(input: &[u8], offset: &mut usize) -> Option<u64> {
    let start = *offset;
    let mut value = 0_u64;
    while let Some(digit @ b'0'..=b'9') = input.get(*offset) {
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))?;
        *offset += 1;
    }
    (*offset > start).then_some(value)
}

fn reference_hits(input: &[u8], width: usize) -> Option<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0;
    while offset < input.len() {
        let mut record = vec![false; width];
        if reference_newline(input, &mut offset).is_none() {
            loop {
                let index = usize::try_from(reference_unsigned(input, &mut offset)?).ok()?;
                let bit = record.get_mut(index)?;
                *bit = !*bit;
                match input.get(offset)? {
                    b',' => offset += 1,
                    b'\n' => {
                        offset += 1;
                        break;
                    }
                    b'\r' => {
                        offset += 1;
                        match input.get(offset)? {
                            b'\n' => {
                                offset += 1;
                                break;
                            }
                            b',' => offset += 1,
                            _ => return None,
                        }
                    }
                    _ => return None,
                }
            }
        } else if input.get(offset - 1) != Some(&b'\n') {
            return None;
        }
        records.push(record);
    }
    Some(records)
}

fn reference_dets(input: &[u8], width: usize) -> Option<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0;
    loop {
        while matches!(input.get(offset), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            offset += 1;
        }
        if offset == input.len() {
            return Some(records);
        }
        if input.get(offset..offset + 4)? != b"shot" {
            return None;
        }
        offset += 4;
        let mut record = vec![false; width];
        loop {
            let mut next = input.get(offset).copied();
            if next == Some(b'\r') {
                offset += 1;
                next = input.get(offset).copied();
            }
            match next {
                None => {
                    records.push(record);
                    return Some(records);
                }
                Some(b'\n') => {
                    offset += 1;
                    break;
                }
                Some(b' ') if input.get(offset + 1) == Some(&b'M') => {
                    offset += 2;
                    let index = usize::try_from(reference_unsigned(input, &mut offset)?).ok()?;
                    *record.get_mut(index)? = true;
                }
                _ => return None,
            }
        }
        records.push(record);
    }
}

/// Checks the materialized readers and the packed streaming reader against the reference.
fn check_generic_readers(format: RecordFormat, width: usize, input: &[u8]) {
    let context = format!(
        "format={} width={width} input_len={}",
        format.as_str(),
        input.len()
    );
    let reference = reference_decode(input, format, width);
    let expected = read_records(input, format, width);
    match (&reference, &expected) {
        (Some(reference), Ok(expected)) => assert_eq!(reference, expected, "{context}"),
        (None, Err(_)) => {}
        _ => panic!(
            "read_records disagreed with the reference decoder: {context}\n{expected:?}\n{reference:?}"
        ),
    }
    assert_eq!(
        read_measurement_records(input, format, width).ok(),
        expected.as_ref().ok().cloned(),
        "read_measurement_records: {context}"
    );

    for framing in [RecordFraming::Stream, RecordFraming::Records] {
        let batch = 1 + (input.len() + width) % 70;
        let reader = RecordStreamReader::measurements(
            Chunked::new(input, input.len() * 31 + width),
            format,
            width,
            usize::MAX,
        )
        .with_framing(framing);
        let streamed = stream_all(reader, batch);
        match (&streamed, &expected) {
            (Ok(streamed), Ok(expected)) => {
                assert_eq!(streamed, expected, "{framing:?} batch={batch}: {context}");
            }
            (Err(error), Err(expected)) if framing == RecordFraming::Stream => assert_eq!(
                error_code(error),
                Some(expected.code()),
                "streamed diagnostic differs: {context}\n{error:?}\n{expected:?}"
            ),
            (Err(_), Err(_)) => {}
            (streamed, expected) => panic!(
                "{framing:?} streaming disagreed on acceptance: {context}\n{streamed:?}\n{expected:?}"
            ),
        }
    }
}

fn check_dets_layout_readers(layout: DetsLayout, input: &[u8]) {
    let context = format!("layout={layout:?} input_len={}", input.len());
    let expected = read_dets_records(input, layout);
    let reader = RecordStreamReader::dets(
        Chunked::new(input, input.len() * 13 + layout.total_bits()),
        layout,
        usize::MAX,
    );
    match (stream_all(reader, 1 + input.len() % 5), &expected) {
        (Ok(streamed), Ok(expected)) => assert_eq!(&streamed, expected, "{context}"),
        (Err(error), Err(expected)) => {
            assert_eq!(error_code(&error), Some(expected.code()), "{context}");
        }
        (streamed, expected) => {
            panic!("typed DETS streaming disagreed: {context}\n{streamed:?}\n{expected:?}")
        }
    }
}

proptest! {
    #![proptest_config(config(0xc0de_c0de_0001, 2048))]

    #[test]
    fn hostile_bytes_agree_with_the_reference_decoder_across_every_reader(
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
            let mut encoder = RecordEncoder::try_new(format, width).expect("record encoder");
            let mut out = Vec::new();
            encoder
                .write_batch(
                    &[RecordBatchPart::new(DetsResultType::Measurement, packed.view())],
                    &mut out,
                )
                .expect("packed encode");
            out
        };
        prop_assert_eq!(&batch_encoded, &encoded, "record encoder differs: {}", context);

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
            reference_decode(&encoded, format, width).expect("reference decode"),
            records.clone(),
            "{}",
            context
        );
        prop_assert_eq!(
            stream_all(
                RecordStreamReader::measurements(
                    Chunked::new(&encoded, encoded.len() + width),
                    format,
                    width,
                    usize::MAX,
                ),
                1 + records.len() % 67,
            )
            .expect("stream decode"),
            records,
            "{}",
            context
        );
    }
}
