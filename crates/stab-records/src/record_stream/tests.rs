#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "streaming reader tests use direct fixture assertions for compact diagnostics"
)]

use super::*;
use crate::{read_records, write_records};

const TEST_TEXT_LIMIT: usize = 1024 * 1024;

/// Feeds bytes a few at a time and counts `read` calls.
struct Trickle<'a> {
    bytes: &'a [u8],
    step: usize,
    reads: usize,
}

impl<'a> Trickle<'a> {
    fn new(bytes: &'a [u8], step: usize) -> Self {
        Self {
            bytes,
            step,
            reads: 0,
        }
    }
}

impl Read for Trickle<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        let step = self.step.min(self.bytes.len()).min(buffer.len());
        buffer[..step].copy_from_slice(&self.bytes[..step]);
        self.bytes = &self.bytes[step..];
        Ok(step)
    }
}

fn drain<R: Read>(
    reader: &mut RecordStreamReader<R>,
    batch: usize,
) -> (Vec<Vec<bool>>, Option<RecordStreamReadError>) {
    let mut records = Vec::new();
    loop {
        match reader.read_batch(batch) {
            Ok(Some(view)) => {
                for shot in 0..view.shot_count() {
                    let row = view.shot(shot).expect("shot in view");
                    records.push((0..row.len()).map(|bit| row.get(bit).unwrap()).collect());
                }
            }
            Ok(None) => return (records, None),
            Err(error) => return (records, Some(error)),
        }
    }
}

fn measurements(input: &[u8], format: RecordFormat, width: usize, step: usize) -> Vec<Vec<bool>> {
    let mut reader =
        RecordStreamReader::measurements(Trickle::new(input, step), format, width, TEST_TEXT_LIMIT);
    let (records, error) = drain(&mut reader, 3);
    assert!(error.is_none(), "{format:?} step {step}: {error:?}");
    records
}

fn format_error(error: Option<RecordStreamReadError>) -> FormatError {
    match error {
        Some(RecordStreamReadError::Format(error)) => error,
        other => panic!("expected a format error, got {other:?}"),
    }
}

#[test]
fn packed_stream_preserves_typed_dets_namespaces_and_duplicate_semantics() {
    // Every sample format streams the same records across transport splits and batch sizes,
    // including widths that are not multiples of 8 or 64.
    for width in [0_usize, 1, 9, 64, 70] {
        let records = (0..128_usize)
            .map(|shot| {
                (0..width)
                    .map(|bit| (shot * 7 + bit * 11) % 13 == 0)
                    .collect()
            })
            .collect::<Vec<Vec<bool>>>();
        for format in [
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Dets,
            RecordFormat::Ptb64,
        ] {
            if width == 0 && matches!(format, RecordFormat::B8 | RecordFormat::Ptb64) {
                continue;
            }
            let input = write_records(&records, format).expect("encode records");
            for step in [1, 7, input.len().max(1)] {
                assert_eq!(
                    measurements(&input, format, width, step),
                    records,
                    "{format:?} width {width} step {step}"
                );
            }
        }
    }

    // Typed DETS layouts resolve each namespace to its own column range; repeated tokens set.
    let layout = DetsLayout::try_new(2, 2, 2).expect("layout");
    let input = b"shot M0 M0 D0 D0 L0 L0 L1\nshot M1 D1 L0\n\n";
    let mut reader = RecordStreamReader::dets(Trickle::new(input, 2), layout, TEST_TEXT_LIMIT);
    let (records, error) = drain(&mut reader, 64);
    assert!(error.is_none());
    assert_eq!(
        records,
        vec![
            vec![true, false, true, false, true, true],
            vec![false, true, false, true, true, false],
        ]
    );

    // HITS duplicates toggle by default and set under the replay policy.
    let input = b"1,1,2\n0,0\n";
    assert_eq!(
        measurements(input, RecordFormat::Hits, 3, 1),
        vec![vec![false, false, true], vec![false, false, false]]
    );
    let mut reader =
        RecordStreamReader::measurements(&input[..], RecordFormat::Hits, 3, TEST_TEXT_LIMIT)
            .with_hits_duplicates(HitsDuplicates::Set);
    assert_eq!(
        drain(&mut reader, 64).0,
        vec![vec![false, true, true], vec![true, false, false]]
    );
}

#[test]
fn stream_framing_errors_are_byte_identical_to_whole_buffer_errors() {
    let cases: &[(RecordFormat, &[u8], usize)] = &[
        (RecordFormat::ZeroOne, b"101\n1", 3),
        (RecordFormat::ZeroOne, b"10\n", 3),
        (RecordFormat::ZeroOne, b"101\n1x1\n", 3),
        (RecordFormat::ZeroOne, b"101\r", 3),
        (RecordFormat::B8, &[0xAB], 9),
        (RecordFormat::B8, &[0xAB, 0x01, 0x02], 9),
        (RecordFormat::B8, &[], 0),
        (RecordFormat::R8, &[4], 3),
        (RecordFormat::R8, &[255], 300),
        (RecordFormat::R8, &[1, 1, 200], 3),
        (RecordFormat::Hits, b"100,1\n", 3),
        (RecordFormat::Hits, b"18446744073709551616\n", 3),
        (RecordFormat::Hits, b"0\n3", 3),
        (RecordFormat::Dets, b"D2\n", 3),
        (RecordFormat::Dets, b"shot X2\n", 3),
        (RecordFormat::Dets, b"shot M0\nshot M9\n", 3),
        (RecordFormat::Ptb64, &[0; 23], 3),
        (RecordFormat::Ptb64, &[], 0),
    ];
    for (format, input, width) in cases {
        let expected = read_records(input, *format, *width).expect_err("whole-buffer rejection");
        for step in [1, 2, input.len().max(1)] {
            let mut reader = RecordStreamReader::measurements(
                Trickle::new(input, step),
                *format,
                *width,
                TEST_TEXT_LIMIT,
            );
            let (_, error) = drain(&mut reader, 2);
            assert_eq!(
                format_error(error),
                expected,
                "{format:?} {input:?} step {step}"
            );
        }
    }
}

#[test]
fn records_before_a_failure_are_delivered_before_it() {
    let input = b"101\n010\nbad\n";
    let mut reader = RecordStreamReader::measurements(
        Trickle::new(input, 3),
        RecordFormat::ZeroOne,
        3,
        TEST_TEXT_LIMIT,
    );
    let batch = reader
        .read_batch(64)
        .expect("leading records")
        .expect("batch");
    assert_eq!(batch.shot_count(), 2);
    let error = format_error(reader.read_batch(64).err());
    assert_eq!(error.span().expect("span").byte_start(), 8);

    // A complete PTB64 group streams before a trailing partial group is rejected.
    let records = vec![vec![true, false, true]; 64];
    let mut input = write_records(&records, RecordFormat::Ptb64).expect("ptb64");
    input.push(0);
    let mut reader =
        RecordStreamReader::measurements(Trickle::new(&input, 5), RecordFormat::Ptb64, 3, 0);
    let (streamed, error) = drain(&mut reader, 10);
    assert_eq!(streamed, records);
    assert_eq!(
        format_error(error).code(),
        FormatErrorCode::InvalidPackedLength
    );
}

#[test]
fn record_framing_reports_line_offsets_terminators_and_limits() {
    let framed = |input: &'static [u8], format: RecordFormat, width: usize| {
        let mut reader = RecordStreamReader::measurements(input, format, width, 8)
            .with_framing(RecordFraming::Records);
        drain(&mut reader, 64)
    };

    // Grammar failures carry the line start and a line-relative span.
    let (records, error) = framed(b"101\n1x1\n", RecordFormat::ZeroOne, 3);
    assert_eq!(records.len(), 1);
    match error {
        Some(RecordStreamReadError::Record {
            byte_offset,
            source,
        }) => {
            assert_eq!(byte_offset, 4);
            assert_eq!(source.span().expect("span").byte_start(), 1);
        }
        other => panic!("expected a line diagnostic, got {other:?}"),
    }

    // Unterminated 01 and HITS lines are reported with their grammar diagnostic; DETS may end
    // at end of input.
    for (input, format) in [
        (&b"101\n101"[..], RecordFormat::ZeroOne),
        (&b"1\n9"[..], RecordFormat::Hits),
    ] {
        let (_, error) = framed(input, format, 3);
        assert!(
            matches!(
                error,
                Some(RecordStreamReadError::UnterminatedRecord {
                    byte_offset: 2..=4,
                    line_len: 1..=3,
                    ..
                })
            ),
            "{format:?}: {error:?}"
        );
    }
    assert_eq!(framed(b"shot M1", RecordFormat::Dets, 3).0.len(), 1);

    // Lines and, under the budget, runs of blank DETS lines are bounded by the record limit.
    assert!(matches!(
        framed(b"010101010\n", RecordFormat::ZeroOne, 9).1,
        Some(RecordStreamReadError::RecordTooLarge { limit: 8 })
    ));
    let blank = b"\n\n\n\n\n\n\n\n\nshot M0\n";
    assert_eq!(framed(blank, RecordFormat::Dets, 3).0.len(), 1);
    let mut reader = RecordStreamReader::measurements(&blank[..], RecordFormat::Dets, 3, 8)
        .with_framing(RecordFraming::Records)
        .with_blank_dets_line_budget();
    assert!(matches!(
        drain(&mut reader, 64).1,
        Some(RecordStreamReadError::RecordTooLarge { limit: 8 })
    ));

    // Packed formats report truncation and zero widths structurally.
    let (records, error) = framed(&[1, 2, 3], RecordFormat::B8, 9);
    assert_eq!(records.len(), 1);
    assert!(matches!(
        error,
        Some(RecordStreamReadError::TruncatedRecord {
            bytes_read: 1,
            record_bytes: 2
        })
    ));
    for format in [RecordFormat::B8, RecordFormat::Ptb64] {
        assert!(matches!(
            framed(b"", format, 0).1,
            Some(RecordStreamReadError::ZeroWidthRecords)
        ));
    }
}

#[test]
fn empty_zero_width_input_supplies_records_only_when_empty() {
    let mut reader = RecordStreamReader::measurements(&b""[..], RecordFormat::B8, 0, 0)
        .with_framing(RecordFraming::Records)
        .with_empty_zero_width_input();
    assert_eq!(
        reader
            .read_batch(5)
            .expect("empty records")
            .expect("batch")
            .shot_count(),
        5
    );
    assert!(!reader.has_more_records().expect("no further records"));

    let mut reader = RecordStreamReader::measurements(&[0_u8][..], RecordFormat::B8, 0, 0)
        .with_framing(RecordFraming::Records)
        .with_empty_zero_width_input();
    assert!(matches!(
        reader.has_more_records(),
        Err(RecordStreamReadError::NonEmptyZeroWidthInput)
    ));
}

#[test]
fn buffered_reads_issue_a_bounded_number_of_transport_reads() {
    const RECORDS: usize = 100_000;
    let records = vec![vec![true, false, true, true, false, false, true, false, true]; RECORDS];
    for format in [RecordFormat::B8, RecordFormat::ZeroOne, RecordFormat::Hits] {
        let input = write_records(&records, format).expect("encode");
        let mut transport = Trickle::new(&input, usize::MAX);
        let mut reader =
            RecordStreamReader::measurements(&mut transport, format, 9, TEST_TEXT_LIMIT);
        let mut decoded = 0_usize;
        while let Some(batch) = reader.read_batch(64).expect("decode") {
            decoded += batch.shot_count();
        }
        assert_eq!(decoded, RECORDS);
        let reads = transport.reads;
        // One read per 32 KiB or more of input, plus the end-of-input read.
        let bound = input.len() / (32 * 1024) + 2;
        assert!(
            reads <= bound,
            "{format:?} issued {reads} reads for {} bytes; bound {bound}",
            input.len()
        );
    }
}

#[test]
fn skipping_and_probing_records_consume_exactly_what_they_report() {
    let input = b"1\n0\n1\n";
    let mut reader = RecordStreamReader::measurements(&input[..], RecordFormat::ZeroOne, 1, 8);
    assert_eq!(reader.skip_records(2).expect("skip"), 2);
    assert!(reader.has_more_records().expect("third record"));
    assert!(!reader.has_more_records().expect("end"));
    assert_eq!(reader.skip_records(5).expect("nothing left"), 0);
    assert!(reader.read_batch(0).is_err());
}

#[test]
fn transport_failures_surface_as_io_errors() {
    struct FailingTransport;
    impl Read for FailingTransport {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("intentional transport stop"))
        }
    }
    let mut reader =
        RecordStreamReader::measurements(FailingTransport, RecordFormat::ZeroOne, 1, 8);
    assert!(matches!(
        reader.read_batch(1),
        Err(RecordStreamReadError::Io(_))
    ));
}
