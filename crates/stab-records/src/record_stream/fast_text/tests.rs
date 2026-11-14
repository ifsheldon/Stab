#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "differential reader tests use direct fixture assertions for compact diagnostics"
)]

//! Differential tests: the single-pass HITS and DETS decoders, and the packed stream reader as a
//! whole, must reproduce the full-grammar path and the whole-buffer `Vec<bool>` readers exactly,
//! for valid and hostile input alike.

use std::io::Read;

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};

use super::super::{HitsDuplicates, RecordFraming, RecordStreamReader};
use crate::{DetsLayout, RecordFormat, read_dets_records, read_records, write_records};

const WIDTHS: [usize; 6] = [0, 1, 63, 64, 65, 1000];

/// Bytes hostile mutations draw from: every byte the text grammars give meaning to, plus junk.
const ALPHABET: &[u8] = b"0123456789,\n\r \tMDLshotx\x00\xff";

/// Feeds at most `step` bytes per read.
struct Trickle<'a> {
    bytes: &'a [u8],
    step: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let step = self.step.min(self.bytes.len()).min(buffer.len());
        buffer[..step].copy_from_slice(&self.bytes[..step]);
        self.bytes = &self.bytes[step..];
        Ok(step)
    }
}

#[derive(Clone, Copy, Debug)]
struct ReaderSetup {
    format: RecordFormat,
    layout: DetsLayout,
    framing: RecordFraming,
    duplicates: HitsDuplicates,
    blank_budget: bool,
    limit: usize,
    step: usize,
    batch: usize,
}

type Outcome = (Vec<Vec<bool>>, Option<String>);

fn read_all(input: &[u8], setup: ReaderSetup, grammar_only: bool) -> Outcome {
    let source = Trickle {
        bytes: input,
        step: setup.step,
    };
    let mut reader = match setup.format {
        RecordFormat::Dets => RecordStreamReader::dets(source, setup.layout, setup.limit),
        format => {
            RecordStreamReader::measurements(source, format, setup.layout.total_bits(), setup.limit)
        }
    }
    .with_framing(setup.framing)
    .with_hits_duplicates(setup.duplicates);
    if setup.blank_budget {
        reader = reader.with_blank_dets_line_budget();
    }
    if grammar_only {
        reader = reader.with_grammar_only();
    }
    let mut records = Vec::new();
    loop {
        match reader.read_batch(setup.batch) {
            Ok(Some(view)) => {
                for shot in 0..view.shot_count() {
                    let row = view.shot(shot).expect("shot in view");
                    records.push((0..row.len()).map(|bit| row.get(bit).unwrap()).collect());
                }
            }
            Ok(None) => return (records, None),
            Err(error) => return (records, Some(format!("{error:?}"))),
        }
    }
}

/// Checks the packed reader against its grammar-only reference and, for the default stream
/// configuration, against the whole-buffer `Vec<bool>` reader.
fn assert_matches_references(input: &[u8], setup: ReaderSetup) -> Result<(), TestCaseError> {
    let fast = read_all(input, setup, false);
    let reference = read_all(input, setup, true);
    prop_assert_eq!(&fast, &reference, "{:?} on {:?}", setup, input);
    let whole_buffer_comparable = setup.framing == RecordFraming::Stream
        && setup.duplicates == HitsDuplicates::Toggle
        && setup.limit > input.len();
    if whole_buffer_comparable {
        let whole = match setup.format {
            RecordFormat::Dets => read_dets_records(input, setup.layout),
            format => read_records(input, format, setup.layout.total_bits()),
        };
        match whole {
            Ok(records) => prop_assert_eq!(&fast, &(records, None), "{:?}", setup),
            // Line framing can narrow a diagnostic span relative to the whole buffer (a DETS
            // prefix span stops at the line end), so only the rejection itself is compared; the
            // grammar-only reader above pins the exact diagnostic.
            Err(error) => prop_assert!(
                fast.1.is_some(),
                "{:?} accepted input the whole-buffer reader rejects with {:?}",
                setup,
                error
            ),
        }
    }
    Ok(())
}

/// Separator lines the DETS generator interleaves: blank, CRLF-blank, and whitespace-only lines.
const SEPARATORS: [&[u8]; 4] = [b"", b"\n", b"\r\n", b" \t\n"];

fn hits_text(records: &[Vec<usize>], crlf: &[bool]) -> Vec<u8> {
    let mut text = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let line = record
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        text.extend_from_slice(line.as_bytes());
        text.extend_from_slice(if crlf[index % crlf.len()] {
            b"\r\n"
        } else {
            b"\n"
        });
    }
    text
}

fn dets_text(records: &[Vec<usize>], layout: DetsLayout, crlf: &[bool]) -> Vec<u8> {
    let mut text = Vec::new();
    for (index, record) in records.iter().enumerate() {
        text.extend_from_slice(SEPARATORS[(index + crlf.len()) % SEPARATORS.len()]);
        text.extend_from_slice(b"shot");
        for bit in record {
            let (prefix, local) = if *bit < layout.measurements() {
                ('M', *bit)
            } else if *bit < layout.measurements() + layout.detectors() {
                ('D', bit - layout.measurements())
            } else {
                ('L', bit - layout.measurements() - layout.detectors())
            };
            text.extend_from_slice(format!(" {prefix}{local}").as_bytes());
        }
        text.extend_from_slice(if crlf[index % crlf.len()] {
            b"\r\n"
        } else {
            b"\n"
        });
    }
    text
}

/// Applies `(position, byte, kind)` edits: 0 replaces, 1 inserts, 2 deletes.
fn mutate(mut input: Vec<u8>, edits: &[(usize, usize, u8)], truncate: Option<usize>) -> Vec<u8> {
    for (position, byte, kind) in edits {
        let byte = ALPHABET[byte % ALPHABET.len()];
        let position = position % (input.len() + 1);
        match kind % 3 {
            0 if position < input.len() => input[position] = byte,
            1 => input.insert(position, byte),
            2 if position < input.len() => {
                input.remove(position);
            }
            _ => {}
        }
    }
    if let Some(len) = truncate {
        input.truncate(len % (input.len() + 1));
    }
    input
}

fn record_strategy() -> impl Strategy<Value = Vec<Vec<u16>>> {
    prop::collection::vec(prop::collection::vec(any::<u16>(), 0..12), 0..7)
}

fn edit_strategy() -> impl Strategy<Value = Vec<(usize, usize, u8)>> {
    prop::collection::vec((any::<usize>(), any::<usize>(), any::<u8>()), 0..3)
}

fn indexes(raw: &[Vec<u16>], width: usize) -> Vec<Vec<usize>> {
    raw.iter()
        .map(|record| {
            record
                .iter()
                .filter(|_| width > 0)
                .map(|bit| usize::from(*bit) % width.max(1))
                .collect()
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 768,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x05ab_7e47_fa57),
        ..ProptestConfig::default()
    })]

    #[test]
    fn sparse_text_readers_match_the_grammar_and_bool_references(
        width_choice in 0..WIDTHS.len(),
        raw in record_strategy(),
        crlf in prop::collection::vec(any::<bool>(), 1..4),
        edits in edit_strategy(),
        truncate in prop::option::of(any::<usize>()),
        dets in any::<bool>(),
        namespaces in (any::<u16>(), any::<u16>()),
        framing_records in any::<bool>(),
        set_duplicates in any::<bool>(),
        blank_budget in any::<bool>(),
        limit_choice in 0..3_usize,
        step in prop::sample::select(vec![1_usize, 2, 3, 7, 64, usize::MAX]),
        batch in 1..5_usize,
    ) {
        let width = WIDTHS[width_choice];
        let detectors = usize::from(namespaces.0) % (width + 1);
        let observables = usize::from(namespaces.1) % (width - detectors + 1);
        let layout = if dets {
            DetsLayout::try_new(width - detectors - observables, detectors, observables).unwrap()
        } else {
            DetsLayout::measurement_only(width)
        };
        let records = indexes(&raw, width);
        let text = if dets {
            dets_text(&records, layout, &crlf)
        } else {
            hits_text(&records, &crlf)
        };
        let input = mutate(text, &edits, truncate);
        let setup = ReaderSetup {
            format: if dets { RecordFormat::Dets } else { RecordFormat::Hits },
            layout,
            framing: if framing_records { RecordFraming::Records } else { RecordFraming::Stream },
            duplicates: if set_duplicates { HitsDuplicates::Set } else { HitsDuplicates::Toggle },
            blank_budget,
            limit: [6, 40, 1 << 20][limit_choice],
            step,
            batch,
        };
        assert_matches_references(&input, setup)?;
    }

    #[test]
    fn packed_readers_match_the_bool_reference_for_every_format(
        width_choice in 0..WIDTHS.len(),
        raw in record_strategy(),
        format in prop::sample::select(vec![
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Dets,
            RecordFormat::Ptb64,
        ]),
        edits in edit_strategy(),
        truncate in prop::option::of(any::<usize>()),
        step in prop::sample::select(vec![1_usize, 5, 64, usize::MAX]),
    ) {
        let width = WIDTHS[width_choice];
        if width == 0 && matches!(format, RecordFormat::B8 | RecordFormat::Ptb64) {
            return Ok(());
        }
        let mut records = indexes(&raw, width)
            .iter()
            .map(|hits| {
                let mut record = vec![false; width];
                for bit in hits {
                    record[*bit] = true;
                }
                record
            })
            .collect::<Vec<_>>();
        if format == RecordFormat::Ptb64 {
            records.resize(64, vec![true; width]);
        }
        let encoded = write_records(&records, format).unwrap();
        let setup = ReaderSetup {
            format,
            layout: DetsLayout::measurement_only(width),
            framing: RecordFraming::Stream,
            duplicates: HitsDuplicates::Toggle,
            blank_budget: false,
            limit: 1 << 20,
            step,
            batch: 3,
        };
        prop_assert_eq!(read_all(&encoded, setup, false), (records, None));
        let hostile = mutate(encoded, &edits, truncate);
        assert_matches_references(&hostile, setup)?;
    }
}

#[test]
fn wide_lines_spanning_many_refills_decode_once_and_identically() {
    let width = 200_003;
    let records = (0..5_usize)
        .map(|shot| (0..width).filter(|bit| (bit * 7 + shot) % 5 == 0).collect())
        .collect::<Vec<Vec<usize>>>();
    let layout = DetsLayout::try_new(100_000, 100_000, 3).unwrap();
    for (format, text) in [
        (RecordFormat::Hits, hits_text(&records, &[false, true])),
        (
            RecordFormat::Dets,
            dets_text(&records, layout, &[true, false]),
        ),
    ] {
        for step in [997, 64 * 1024, usize::MAX] {
            for framing in [RecordFraming::Stream, RecordFraming::Records] {
                let setup = ReaderSetup {
                    format,
                    layout: if format == RecordFormat::Dets {
                        layout
                    } else {
                        DetsLayout::measurement_only(width)
                    },
                    framing,
                    duplicates: HitsDuplicates::Toggle,
                    blank_budget: true,
                    limit: 4 << 20,
                    step,
                    batch: 2,
                };
                let fast = read_all(&text, setup, false);
                assert_eq!(fast, read_all(&text, setup, true), "{setup:?}");
                assert_eq!(fast.0.len(), records.len());
                assert!(fast.1.is_none());
                for (decoded, hits) in fast.0.iter().zip(&records) {
                    assert_eq!(decoded.iter().filter(|bit| **bit).count(), hits.len());
                    assert!(hits.iter().all(|bit| decoded[*bit]));
                }
            }
        }
    }
}

#[test]
fn hostile_digit_runs_spanning_refills_are_judged_by_the_grammar() {
    // Multi-megabyte digit runs (with and without leading zeros) arrive in small reads; the
    // single-pass decoder must decline them in linear time and leave the diagnostic to the
    // grammar, while a 20-digit leading-zero index still decodes.
    let mut zeros = vec![b'0'; 30];
    zeros.extend_from_slice(b"7\n");
    for (format, line, records) in [
        (
            RecordFormat::Hits,
            [b"1\n".as_slice(), &vec![b'9'; 3 << 20], b"\n"].concat(),
            1,
        ),
        (
            RecordFormat::Hits,
            [b"1\n".as_slice(), &vec![b'0'; 3 << 20], b"5\n"].concat(),
            2,
        ),
        (RecordFormat::Hits, [b"1\n".as_slice(), &zeros].concat(), 2),
        (
            RecordFormat::Dets,
            [b"shot M1\nshot M".as_slice(), &vec![b'0'; 3 << 20], b"\n"].concat(),
            2,
        ),
    ] {
        for framing in [RecordFraming::Stream, RecordFraming::Records] {
            let setup = ReaderSetup {
                format,
                layout: DetsLayout::measurement_only(64),
                framing,
                duplicates: HitsDuplicates::Toggle,
                blank_budget: false,
                limit: 8 << 20,
                step: 4096,
                batch: 4,
            };
            let fast = read_all(&line, setup, false);
            assert_eq!(fast, read_all(&line, setup, true), "{setup:?}");
            assert_eq!(fast.0.len(), records, "{setup:?}");
        }
    }
}
