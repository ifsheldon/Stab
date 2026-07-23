#![allow(
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "corpus tests use explicit panic messages to identify the exact fixture and reader that violated the checked oracle"
)]

use serde::Deserialize;
use stab_core::{
    BitSlice, CircuitResult, DetsLayout, DetsResultType, DetsToken, SampleFormat,
    result_formats::{SparseShot, read_dets_records, read_measurement_records, read_records},
    result_streaming::{
        for_each_dets_packed_record, for_each_dets_record, for_each_dets_sparse_shot,
        for_each_dets_token_record, for_each_packed_record, for_each_record,
        for_each_sparse_record,
    },
};

#[derive(Debug, Deserialize)]
struct Corpus {
    schema_version: u32,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    id: String,
    format: CorpusFormat,
    layout: Layout,
    input_hex: String,
    acceptance: Acceptance,
    canonical_01_hex: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum CorpusFormat {
    #[serde(rename = "01")]
    ZeroOne,
    #[serde(rename = "hits")]
    Hits,
    #[serde(rename = "dets")]
    Dets,
}

impl CorpusFormat {
    const fn sample_format(self) -> SampleFormat {
        match self {
            Self::ZeroOne => SampleFormat::ZeroOne,
            Self::Hits => SampleFormat::Hits,
            Self::Dets => SampleFormat::Dets,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct Layout {
    measurements: usize,
    detectors: usize,
    observables: usize,
}

impl Layout {
    fn dets(self) -> CircuitResult<DetsLayout> {
        DetsLayout::try_new(self.measurements, self.detectors, self.observables)
    }

    fn width(self) -> usize {
        self.measurements + self.detectors + self.observables
    }

    const fn is_measurement_only(self) -> bool {
        self.detectors == 0 && self.observables == 0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Acceptance {
    Accepted,
    Rejected,
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
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../../oracle/result-format-corpus.json"))?;
    assert_eq!(corpus.schema_version, 1);

    for case in corpus
        .cases
        .into_iter()
        .filter(|case| case.acceptance == selected)
    {
        let input = hex::decode(&case.input_hex)?;
        let expected = case
            .canonical_01_hex
            .as_deref()
            .map(hex::decode)
            .transpose()?
            .map(|canonical| read_records(&canonical, SampleFormat::ZeroOne, case.layout.width()))
            .transpose()?;
        assert_eq!(
            expected.is_some(),
            case.acceptance == Acceptance::Accepted,
            "{}",
            case.id
        );

        if case.format != CorpusFormat::Dets || case.layout.is_measurement_only() {
            check_width_readers(&case, &input, expected.as_deref());
        }
        if case.format == CorpusFormat::Dets {
            check_typed_dets_readers(&case, &input, expected.as_deref())?;
        }
    }
    Ok(())
}

fn check_width_readers(case: &CorpusCase, input: &[u8], expected: Option<&[Vec<bool>]>) {
    let format = case.format.sample_format();
    let width = case.layout.width();
    assert_records(
        &case.id,
        "materialized",
        read_records(input, format, width),
        expected,
    );
    assert_records(
        &case.id,
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
    assert_records(&case.id, "dense visitor", dense_result, expected);

    let mut packed = Vec::new();
    let packed_result = for_each_packed_record(input, format, width, |record| {
        packed.push(bits(record));
        Ok(())
    })
    .map(|()| packed);
    assert_records(&case.id, "packed visitor", packed_result, expected);

    let mut sparse = Vec::new();
    let sparse_result = for_each_sparse_record(input, format, width, |record| {
        sparse.push(record.to_vec());
        Ok(())
    })
    .map(|()| sparse_to_dense(&sparse, width, case.format));
    assert_records(&case.id, "sparse visitor", sparse_result, expected);
}

fn check_typed_dets_readers(
    case: &CorpusCase,
    input: &[u8],
    expected: Option<&[Vec<bool>]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = case.layout.dets()?;
    assert_records(
        &case.id,
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
    assert_records(&case.id, "typed dense visitor", dense_result, expected);

    let mut packed = Vec::new();
    let packed_result = for_each_dets_packed_record(input, layout, |record| {
        packed.push(bits(record));
        Ok(())
    })
    .map(|()| packed);
    assert_records(&case.id, "typed packed visitor", packed_result, expected);

    let mut token_records = Vec::new();
    let token_result = for_each_dets_token_record(input, layout, |record| {
        token_records.push(record.to_vec());
        Ok(())
    });
    let token_accepted = token_result.is_ok();
    let token_dense_result =
        token_result.map(|()| typed_tokens_to_dense(&token_records, case.layout));
    assert_records(
        &case.id,
        "typed token visitor",
        token_dense_result,
        expected,
    );

    let mut sparse_shots = Vec::new();
    let sparse_shot_result = for_each_dets_sparse_shot(input, layout, |shot| {
        sparse_shots.push(shot.clone());
        Ok(())
    });
    match (case.acceptance, sparse_shot_result, token_accepted) {
        (Acceptance::Accepted, Ok(()), true) => {
            assert_eq!(
                sparse_shots,
                typed_tokens_to_sparse_shots(&token_records, case.layout),
                "{} SparseShot visitor",
                case.id
            );
        }
        (Acceptance::Rejected, Err(_), false) => {}
        (expected_acceptance, sparse_result, tokens_result) => {
            panic!(
                "{} SparseShot/token acceptance mismatch: expected {expected_acceptance:?}, sparse={sparse_result:?}, tokens={tokens_result:?}",
                case.id
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
    format: CorpusFormat,
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
                if format == CorpusFormat::Hits {
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
            let mut dense = vec![false; layout.width()];
            for token in record {
                let offset = match token.result_type() {
                    DetsResultType::Measurement => 0,
                    DetsResultType::Detector => layout.measurements,
                    DetsResultType::Observable => layout.measurements + layout.detectors,
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
            let mut observables = vec![false; layout.observables];
            for token in record {
                match token.result_type() {
                    DetsResultType::Measurement => {
                        let Ok(index) = u64::try_from(token.index()) else {
                            panic!("measurement DETS index did not fit u64");
                        };
                        hits.push(index);
                    }
                    DetsResultType::Detector => {
                        let Some(index) = layout.measurements.checked_add(token.index()) else {
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
