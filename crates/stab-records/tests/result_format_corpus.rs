#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "corpus tests use explicit panic messages to identify the exact fixture and reader that violated the checked oracle"
)]

use std::io::Read;

use stab_compat_corpus::{Acceptance, CheckedCase, CheckedCorpus, ResultFormat};
use stab_records::{DetsLayout, RecordFormat, RecordFraming, RecordStreamReader};

const fn sample_format(format: ResultFormat) -> RecordFormat {
    match format {
        ResultFormat::ZeroOne => RecordFormat::ZeroOne,
        ResultFormat::B8 => RecordFormat::B8,
        ResultFormat::R8 => RecordFormat::R8,
        ResultFormat::Hits => RecordFormat::Hits,
        ResultFormat::Dets => RecordFormat::Dets,
        ResultFormat::Ptb64 => RecordFormat::Ptb64,
    }
}

#[test]
fn checked_accepted_corpus_matches_every_applicable_record_reader()
-> Result<(), Box<dyn std::error::Error>> {
    check_corpus(Acceptance::Accepted)
}

#[test]
fn checked_rejected_corpus_matches_every_applicable_record_reader()
-> Result<(), Box<dyn std::error::Error>> {
    check_corpus(Acceptance::Rejected)
}

fn check_corpus(selected: Acceptance) -> Result<(), Box<dyn std::error::Error>> {
    let corpus = CheckedCorpus::parse(include_bytes!("../../../oracle/result-format-corpus.json"))?;

    for case in corpus
        .cases()
        .iter()
        .filter(|case| case.acceptance() == selected)
    {
        let expected = case.canonical_records();
        assert_eq!(
            expected.is_some(),
            case.acceptance() == Acceptance::Accepted,
            "{}",
            case.id()
        );

        if let Some(width) = case.measurement_only_width() {
            check_width_readers(
                case,
                case.input(),
                width,
                case.measurement_only_canonical_records(),
            );
        }
        if case.format() == ResultFormat::Dets {
            check_typed_dets_readers(case, case.input(), expected)?;
        }
    }
    Ok(())
}

fn check_width_readers(
    case: &CheckedCase,
    input: &[u8],
    width: usize,
    expected: Option<&[Vec<bool>]>,
) {
    let format = sample_format(case.format());

    for framing in [RecordFraming::Stream, RecordFraming::Records] {
        for batch in [1, 64] {
            let reader = RecordStreamReader::measurements(input, format, width, usize::MAX)
                .with_framing(framing);
            assert_records(
                case.id(),
                &format!("packed stream {framing:?} batch {batch}"),
                stream_all(reader, batch),
                expected,
            );
        }
    }
}

fn check_typed_dets_readers(
    case: &CheckedCase,
    input: &[u8],
    expected: Option<&[Vec<bool>]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let corpus_layout = case.layout();
    let layout = DetsLayout::try_new(
        corpus_layout.measurements(),
        corpus_layout.detectors(),
        corpus_layout.observables(),
    )?;
    for batch in [1, 64] {
        assert_records(
            case.id(),
            &format!("typed packed stream batch {batch}"),
            stream_all(RecordStreamReader::dets(input, layout, usize::MAX), batch),
            expected,
        );
    }
    Ok(())
}

/// Drains the packed streaming reader into dense records, or its first failure as text.
fn stream_all<R: Read>(
    mut reader: RecordStreamReader<R>,
    batch: usize,
) -> Result<Vec<Vec<bool>>, String> {
    let mut records = Vec::new();
    loop {
        match reader.read_batch(batch) {
            Ok(Some(view)) => {
                for shot in 0..view.shot_count() {
                    let row = view.shot(shot).map_err(|error| error.to_string())?;
                    records.push(
                        (0..row.len())
                            .map(|bit| row.get(bit) == Some(true))
                            .collect(),
                    );
                }
            }
            Ok(None) => return Ok(records),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn assert_records<E: std::fmt::Display>(
    case_id: &str,
    reader: &str,
    actual: Result<Vec<Vec<bool>>, E>,
    expected: Option<&[Vec<bool>]>,
) {
    match (actual, expected) {
        (Ok(actual), Some(expected)) => {
            assert_eq!(actual, expected, "{case_id} through {reader}");
        }
        (Err(_), None) => {}
        (Ok(actual), None) => {
            panic!("{case_id} through {reader} unexpectedly accepted {actual:?}");
        }
        (Err(error), Some(_)) => {
            panic!("{case_id} through {reader} unexpectedly rejected: {error}");
        }
    }
}
