#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "corpus tests use explicit panic messages to identify the exact fixture and reader that violated the checked oracle"
)]

use stab_compat_corpus::{Acceptance, CheckedCase, CheckedCorpus, Layout, ResultFormat};
use stab_core::{
    BitSlice, CircuitResult, DetsLayout, DetsResultType, DetsToken, SampleFormat,
    result_formats::{SparseShot, read_dets_records, read_measurement_records, read_records},
    result_streaming::{
        for_each_dets_packed_record, for_each_dets_record, for_each_dets_sparse_shot,
        for_each_dets_token_record, for_each_packed_record, for_each_record,
        for_each_sparse_record,
    },
};

const fn sample_format(format: ResultFormat) -> SampleFormat {
    match format {
        ResultFormat::ZeroOne => SampleFormat::ZeroOne,
        ResultFormat::Hits => SampleFormat::Hits,
        ResultFormat::Dets => SampleFormat::Dets,
    }
}

#[test]
fn checked_accepted_corpus_matches_every_applicable_core_reader()
-> Result<(), Box<dyn std::error::Error>> {
    check_corpus(Acceptance::Accepted)
}

#[test]
fn checked_rejected_corpus_matches_every_applicable_core_reader()
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

        if case.format() != ResultFormat::Dets || case.layout().is_measurement_only() {
            check_width_readers(case, case.input(), expected);
        }
        if case.format() == ResultFormat::Dets {
            check_typed_dets_readers(case, case.input(), expected)?;
        }
    }
    Ok(())
}

fn check_width_readers(case: &CheckedCase, input: &[u8], expected: Option<&[Vec<bool>]>) {
    let format = sample_format(case.format());
    let width = case.layout().total_bits().expect("validated layout");
    assert_records(
        case.id(),
        "materialized",
        read_records(input, format, width),
        expected,
    );
    assert_records(
        case.id(),
        "measurement-only",
        read_measurement_records(input, format, width),
        expected,
    );

    let mut dense = Vec::new();
    let dense_result = for_each_record(input, format, width, |record| {
        dense.push(record.to_vec());
        Ok(())
    })
    .map(|()| dense);
    assert_records(case.id(), "dense visitor", dense_result, expected);

    let mut packed = Vec::new();
    let packed_result = for_each_packed_record(input, format, width, |record| {
        packed.push(bits(record));
        Ok(())
    })
    .map(|()| packed);
    assert_records(case.id(), "packed visitor", packed_result, expected);

    let mut sparse = Vec::new();
    let sparse_result = for_each_sparse_record(input, format, width, |record| {
        sparse.push(record.to_vec());
        Ok(())
    })
    .map(|()| sparse_to_dense(&sparse, width, case.format()));
    assert_records(case.id(), "sparse visitor", sparse_result, expected);
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
    assert_records(
        case.id(),
        "typed materialized",
        read_dets_records(input, layout),
        expected,
    );

    let mut dense = Vec::new();
    let dense_result = for_each_dets_record(input, layout, |record| {
        dense.push(record.to_vec());
        Ok(())
    })
    .map(|()| dense);
    assert_records(case.id(), "typed dense visitor", dense_result, expected);

    let mut packed = Vec::new();
    let packed_result = for_each_dets_packed_record(input, layout, |record| {
        packed.push(bits(record));
        Ok(())
    })
    .map(|()| packed);
    assert_records(case.id(), "typed packed visitor", packed_result, expected);

    let mut token_records = Vec::new();
    let token_result = for_each_dets_token_record(input, layout, |record| {
        token_records.push(record.to_vec());
        Ok(())
    });
    let token_accepted = token_result.is_ok();
    let token_dense_result =
        token_result.map(|()| typed_tokens_to_dense(&token_records, corpus_layout));
    assert_records(
        case.id(),
        "typed token visitor",
        token_dense_result,
        expected,
    );

    let mut sparse_shots = Vec::new();
    let sparse_shot_result = for_each_dets_sparse_shot(input, layout, |shot| {
        sparse_shots.push(shot.clone());
        Ok(())
    });
    match (case.acceptance(), sparse_shot_result, token_accepted) {
        (Acceptance::Accepted, Ok(()), true) => {
            assert_eq!(
                sparse_shots,
                typed_tokens_to_sparse_shots(&token_records, corpus_layout),
                "{} SparseShot visitor",
                case.id()
            );
        }
        (Acceptance::Rejected, Err(_), false) => {}
        (expected_acceptance, sparse_result, tokens_result) => {
            panic!(
                "{} SparseShot/token acceptance mismatch: expected {expected_acceptance:?}, sparse={sparse_result:?}, tokens={tokens_result:?}",
                case.id()
            );
        }
    }
    Ok(())
}

fn assert_records(
    case_id: &str,
    reader: &str,
    actual: CircuitResult<Vec<Vec<bool>>>,
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

fn bits(record: BitSlice<'_>) -> Vec<bool> {
    (0..record.len())
        .map(|index| {
            let Some(bit) = record.get(index) else {
                panic!("packed record index {index} was out of range");
            };
            bit
        })
        .collect()
}

fn sparse_to_dense(
    sparse_records: &[Vec<u64>],
    width: usize,
    format: ResultFormat,
) -> Vec<Vec<bool>> {
    sparse_records
        .iter()
        .map(|record| {
            let mut dense = vec![false; width];
            for index in record {
                let Ok(index) = usize::try_from(*index) else {
                    panic!("sparse index {index} did not fit usize");
                };
                let Some(bit) = dense.get_mut(index) else {
                    panic!("sparse index {index} exceeded width {width}");
                };
                if format == ResultFormat::Hits {
                    *bit = !*bit;
                } else {
                    *bit = true;
                }
            }
            dense
        })
        .collect()
}

fn typed_tokens_to_dense(records: &[Vec<DetsToken>], layout: Layout) -> Vec<Vec<bool>> {
    records
        .iter()
        .map(|record| {
            let width = layout.total_bits().expect("validated layout");
            let mut dense = vec![false; width];
            for token in record {
                let offset = match token.result_type() {
                    DetsResultType::Measurement => 0,
                    DetsResultType::Detector => layout.measurements(),
                    DetsResultType::Observable => layout.measurements() + layout.detectors(),
                };
                let Some(index) = offset.checked_add(token.index()) else {
                    panic!("typed DETS offset overflowed");
                };
                let width = dense.len();
                let Some(bit) = dense.get_mut(index) else {
                    panic!("typed DETS index {index} exceeded width {width}");
                };
                *bit = true;
            }
            dense
        })
        .collect()
}

fn typed_tokens_to_sparse_shots(records: &[Vec<DetsToken>], layout: Layout) -> Vec<SparseShot> {
    records
        .iter()
        .map(|record| {
            let mut hits = Vec::new();
            let mut observables = vec![false; layout.observables()];
            for token in record {
                match token.result_type() {
                    DetsResultType::Measurement => {
                        let Ok(index) = u64::try_from(token.index()) else {
                            panic!("measurement DETS index did not fit u64");
                        };
                        hits.push(index);
                    }
                    DetsResultType::Detector => {
                        let Some(index) = layout.measurements().checked_add(token.index()) else {
                            panic!("detector DETS offset overflowed");
                        };
                        let Ok(index) = u64::try_from(index) else {
                            panic!("detector DETS index did not fit u64");
                        };
                        hits.push(index);
                    }
                    DetsResultType::Observable => {
                        let Some(bit) = observables.get_mut(token.index()) else {
                            panic!("observable DETS index exceeded mask");
                        };
                        *bit = !*bit;
                    }
                }
            }
            SparseShot::new(hits, observables)
        })
        .collect()
}
