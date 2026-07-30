use std::hint::black_box;

use sha2::{Digest as _, Sha256};
use stab_core::{
    SampleFormat,
    advanced::records::{
        for_each_packed_record, for_each_ptb64_record_all, for_each_sparse_record,
        write_ptb64_records_checked, write_records,
    },
};

use super::super::{
    TINY_DIRECT_COMPARE_REPETITIONS, measure_stab, measure_stab_batched,
    semantic_preflight::require_exact, stab_runner_error,
};
use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

const MEASURE_READER_BITS: usize = 10_000;
const MEASURE_READER_PTB64_SHA256: &str =
    "0dbbbd64cce63c604aa405ba83ca00aba650905cca17b4dcbfd615980ac89ad0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MeasureReaderMode {
    Packed,
    Sparse,
}

pub(super) fn run_measure_reader_format_row(
    row: &BenchmarkRow,
    format: SampleFormat,
    cases: &[(&'static str, MeasureReaderMode, usize)],
) -> Result<Vec<Measurement>, BenchError> {
    cases
        .iter()
        .map(|(name, mode, denominator)| {
            let source_record = measure_reader_record(*denominator);
            let input = write_records(std::slice::from_ref(&source_record), format);
            validate_measure_reader_input_digest(&row.id, &input, format, *denominator)?;
            validate_measure_reader_preflight(&row.id, &input, format, *mode, &source_record)?;
            measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
                let mut set_bits = 0usize;
                match mode {
                    MeasureReaderMode::Packed => {
                        for_each_packed_record(&input, format, MEASURE_READER_BITS, |record| {
                            set_bits += record.popcount();
                            Ok(())
                        })
                    }
                    MeasureReaderMode::Sparse => {
                        for_each_sparse_record(&input, format, MEASURE_READER_BITS, |hits| {
                            set_bits += hits.len();
                            Ok(())
                        })
                    }
                }
                .map_err(|error| stab_runner_error(&row.id, error))?;
                black_box(set_bits);
                Ok(())
            })
        })
        .collect()
}

pub(super) fn run_measure_reader_ptb64_row(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    let source_record = measure_reader_record(10);
    let ptb64_records = (0..64).map(|_| source_record.clone()).collect::<Vec<_>>();
    let ptb64_input = write_ptb64_records_checked(&ptb64_records)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    validate_frozen_input_digest(
        &row.id,
        "PTB64 measurement reader input",
        &ptb64_input,
        MEASURE_READER_PTB64_SHA256,
    )?;
    validate_ptb64_reader_preflight(&row.id, &ptb64_input, &source_record)?;
    Ok(vec![measure_stab(
        "stab_measure_reader_ptb64_64x10k_contract",
        || {
            let mut set_bits = 0usize;
            for_each_ptb64_record_all(&ptb64_input, MEASURE_READER_BITS, |record| {
                set_bits += record.iter().filter(|bit| **bit).count();
                Ok(())
            })
            .map_err(|error| stab_runner_error(&row.id, error))?;
            black_box(set_bits);
            Ok(())
        },
    )?])
}

pub(super) fn validate_measure_reader_input_digest(
    row_id: &str,
    input: &[u8],
    format: SampleFormat,
    denominator: usize,
) -> Result<(), BenchError> {
    let expected = match (format, denominator) {
        (SampleFormat::ZeroOne, 10) => {
            "76ddc795bc51947da415b2fc5bac6e7d30b948bcd813309db6075b2c00b0db40"
        }
        (SampleFormat::B8, 10) => {
            "3c6008cec14343e54fba03d665043ef59e68370e12b446e442afdb233e88f4fa"
        }
        (SampleFormat::R8, 10) => {
            "cf256c7b6499e41f098f267e35f115abcc1b57fdc2de41f4cbc7d4d3041afefe"
        }
        (SampleFormat::R8, 100) => {
            "43e16015e8e60c97063ab47c30684f69ef3716bc38a572d0ad603b3885200a52"
        }
        (SampleFormat::Hits, 10) => {
            "739e9a3344b85f790ae633b3ada8a5e9d466bafbcbb5a3e999ca956c21d390cf"
        }
        (SampleFormat::Hits, 100) => {
            "93afafc60cae0af47244df4b36dbfc4d0866611eb1307b1cd8dc6a744c087e87"
        }
        (SampleFormat::Dets, 10) => {
            "2a6d998dbb26149505728e84aa669ba56b404e2d033120c9f74a880615e5dfc5"
        }
        (SampleFormat::Dets, 100) => {
            "f11213f1e37e3e16307249b385ef2fedf8c0ac14da64a7bbe930113df93880f0"
        }
        _ => {
            return Err(stab_runner_error(
                row_id,
                format!(
                    "measurement reader has no frozen input digest for {format:?} density 1/{denominator}"
                ),
            ));
        }
    };
    validate_frozen_input_digest(row_id, "measurement reader input", input, expected)
}

fn validate_frozen_input_digest(
    row_id: &str,
    contract: &str,
    input: &[u8],
    expected: &str,
) -> Result<(), BenchError> {
    let actual = hex::encode(Sha256::digest(input));
    require_exact(row_id, contract, actual.as_str(), expected)
}

pub(super) fn validate_measure_reader_preflight(
    row_id: &str,
    input: &[u8],
    format: SampleFormat,
    mode: MeasureReaderMode,
    expected: &[bool],
) -> Result<(), BenchError> {
    match mode {
        MeasureReaderMode::Packed => {
            let mut record_count = 0_usize;
            let mut exact = true;
            for_each_packed_record(input, format, MEASURE_READER_BITS, |record| {
                record_count += 1;
                exact &= record.len() == expected.len()
                    && expected
                        .iter()
                        .enumerate()
                        .all(|(index, bit)| record.get(index) == Some(*bit));
                Ok(())
            })
            .map_err(|error| stab_runner_error(row_id, error))?;
            require_exact(
                row_id,
                "packed measurement reader",
                &(record_count, exact),
                &(1, true),
            )
        }
        MeasureReaderMode::Sparse => {
            let expected_hits = expected
                .iter()
                .enumerate()
                .filter_map(|(index, bit)| bit.then_some(index as u64))
                .collect::<Vec<_>>();
            let mut records = Vec::new();
            for_each_sparse_record(input, format, MEASURE_READER_BITS, |hits| {
                records.push(hits.to_vec());
                Ok(())
            })
            .map_err(|error| stab_runner_error(row_id, error))?;
            require_exact(
                row_id,
                "sparse measurement reader",
                &records,
                &vec![expected_hits],
            )
        }
    }
}

pub(super) fn validate_ptb64_reader_preflight(
    row_id: &str,
    input: &[u8],
    expected: &[bool],
) -> Result<(), BenchError> {
    let mut record_count = 0_usize;
    let mut exact = true;
    for_each_ptb64_record_all(input, MEASURE_READER_BITS, |record| {
        record_count += 1;
        exact &= record.len() == expected.len()
            && record
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual == expected);
        Ok(())
    })
    .map_err(|error| stab_runner_error(row_id, error))?;
    require_exact(
        row_id,
        "PTB64 measurement reader",
        &(record_count, exact),
        &(64, true),
    )
}

pub(super) fn measure_reader_record(denominator: usize) -> Vec<bool> {
    (0..MEASURE_READER_BITS)
        .map(|index| (index * 17 + 3) % denominator == 0)
        .collect()
}

pub(super) fn measure_reader_denominator_from_name(name: &str) -> Option<usize> {
    name.rsplit_once("_per")
        .and_then(|(_, denominator)| denominator.parse::<usize>().ok())
}
