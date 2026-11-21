#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "result-format unit tests use direct fixture assertions for compact diagnostics"
)]

use stab_bits::BitVec;

use super::reference::{read_records, write_records};
use super::*;
use crate::{
    PackedShotBatch, RecordBatchPart, RecordEncoder, RecordPart, RecordStreamReadError,
    RecordStreamReader,
};

/// Encodes whole measurement records through the product encoder.
fn encode(records: &[Vec<bool>], format: RecordFormat, width: usize) -> Vec<u8> {
    let batch = PackedShotBatch::from_records(records, width).expect("packed records");
    let mut encoder = RecordEncoder::try_new(format, width).expect("record encoder");
    let mut out = Vec::new();
    encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                batch.view(),
            )],
            &mut out,
        )
        .expect("encode records");
    assert_eq!(encoder.pending_group_records(), 0, "{format:?} group");
    out
}

/// Decodes a whole input through the product stream reader.
fn decode(
    input: &[u8],
    format: RecordFormat,
    width: usize,
) -> Result<Vec<Vec<bool>>, RecordStreamReadError> {
    let mut reader = RecordStreamReader::measurements(input, format, width, usize::MAX);
    let mut records = Vec::new();
    while let Some(view) = reader.read_batch(64)? {
        for shot in 0..view.shot_count() {
            let row = view.shot(shot)?;
            records.push((0..row.len()).map(|bit| row.get(bit).unwrap()).collect());
        }
    }
    Ok(records)
}

/// One record's bits from the bytes Stim's writer tests feed it, least significant bit first.
fn bits_of(bytes: &[u8], tail: bool) -> Vec<bool> {
    let mut bits = bytes
        .iter()
        .flat_map(|byte| (0..8).map(move |bit| byte & (1 << bit) != 0))
        .collect::<Vec<_>>();
    bits.push(tail);
    bits
}

fn block(bits: &[bool]) -> BitVec {
    BitVec::from_bits(bits.iter().copied())
}

/// Ported from Stim's `MeasureRecordWriter` contract tests
/// (`vendor/stim/src/stim/io/measure_record_writer.test.cc`): a namespace change is a no-op on
/// every non-DETS format, so bit positions keep counting across record parts (the HITS regression
/// once restarted them at zero), while DETS switches the namespace and restarts its position.
#[test]
fn record_parts_follow_stims_writer_namespace_contract() {
    let first = bits_of(&[0xF8], false);
    let second = bits_of(&[0xF8], true);
    for (format, expected) in [
        (RecordFormat::ZeroOne, b"000111110000111111\n".to_vec()),
        (RecordFormat::B8, vec![0xF8, 0xF0, 0x03]),
        (
            RecordFormat::Hits,
            b"3,4,5,6,7,12,13,14,15,16,17\n".to_vec(),
        ),
        (RecordFormat::R8, vec![3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]),
    ] {
        let (first, second) = (block(&first), block(&second));
        let mut encoder = RecordEncoder::try_new(format, 18).expect("encoder");
        let mut out = Vec::new();
        encoder
            .write_record(
                &[
                    RecordPart::new(DetsResultType::Detector, first.as_bitslice()),
                    RecordPart::new(DetsResultType::Observable, second.as_bitslice()),
                ],
                &mut out,
            )
            .expect("record");
        assert_eq!(out, expected, "{format:?}");
    }

    let detectors = block(&[bits_of(&[0xF8], false), bits_of(&[0xF8], false)].concat()[..17]);
    let observables = block(&[false, true]);
    let mut encoder = RecordEncoder::try_new(RecordFormat::Dets, 19).expect("encoder");
    let mut out = Vec::new();
    encoder
        .write_record(
            &[
                RecordPart::new(DetsResultType::Detector, detectors.as_bitslice()),
                RecordPart::new(DetsResultType::Observable, observables.as_bitslice()),
            ],
            &mut out,
        )
        .expect("record");
    assert_eq!(out, b"shot D3 D4 D5 D6 D7 D12 D13 D14 D15 D16 L1\n");
}

/// Empty DETS records keep their prefix, and sparse `r8` records chain 255-runs exactly at the
/// boundary, through the product encoder.
#[test]
fn record_encoder_handles_empty_dets_records_and_long_r8_gaps() {
    assert_eq!(
        encode(&[vec![], vec![], vec![]], RecordFormat::Dets, 0),
        b"shot\nshot\nshot\n"
    );
    let mut record = vec![false; 8 * 64 + 1 + 32];
    record[8 * 64] = true;
    assert_eq!(
        encode(&[record], RecordFormat::R8, 8 * 64 + 1 + 32),
        [255, 255, 2, 32]
    );
}

#[test]
fn per_record_encoders_reject_partial_ptb64_groups() {
    let records = vec![vec![true]; 63];
    let batch = PackedShotBatch::from_records(&records, 1).expect("records");
    let mut encoder = RecordEncoder::try_new(RecordFormat::Ptb64, 1).expect("encoder");
    let mut out = Vec::new();
    encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                batch.view(),
            )],
            &mut out,
        )
        .expect("partial group is retained");
    assert!(out.is_empty());
    assert_eq!(encoder.pending_group_records(), 63);
    let error = validate_ptb64_shot_count(63).expect_err("ptb64 needs whole groups");
    assert!(error.message().contains("multiple of 64"));
}

/// The product encoder and stream reader keep every format's exact byte contract, and the
/// whole-buffer reference codec agrees with both on the same fixtures.
#[test]
fn codecs_and_strict_grammars_contract() {
    let records = vec![
        vec![true, false, true, false, true, false, true, false, true],
        vec![false, true, false, true, false, true, false, true, false],
    ];
    for (format, expected) in [
        (RecordFormat::ZeroOne, b"101010101\n010101010\n".as_slice()),
        (RecordFormat::B8, &[0x55, 0x01, 0xAA, 0x00]),
        (RecordFormat::R8, &[0, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1]),
        (RecordFormat::Hits, b"0,2,4,6,8\n1,3,5,7\n".as_slice()),
        (
            RecordFormat::Dets,
            b"shot M0 M2 M4 M6 M8\nshot M1 M3 M5 M7\n".as_slice(),
        ),
    ] {
        let encoded = encode(&records, format, 9);
        assert_eq!(encoded, expected, "{format:?} byte contract");
        assert_eq!(write_records(&records, format).unwrap(), expected);
        assert_eq!(decode(&encoded, format, 9).unwrap(), records, "{format:?}");
        assert_eq!(read_records(&encoded, format, 9).unwrap(), records);
    }

    let ptb64_records = (0usize..64)
        .map(|shot| vec![shot.is_multiple_of(2), shot < 5, shot == 63, false])
        .collect::<Vec<_>>();
    let ptb64 = encode(&ptb64_records, RecordFormat::Ptb64, 4);
    assert_eq!(
        ptb64,
        [
            0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x1F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    assert_eq!(
        write_records(&ptb64_records, RecordFormat::Ptb64).unwrap(),
        ptb64
    );
    assert_eq!(
        decode(&ptb64, RecordFormat::Ptb64, 4).unwrap(),
        ptb64_records
    );
    assert_eq!(
        read_records(&ptb64, RecordFormat::Ptb64, 4).unwrap(),
        ptb64_records
    );

    for (format, input, width, expected) in [
        (
            RecordFormat::ZeroOne,
            b"01\r\n10\r\n".as_slice(),
            2,
            vec![vec![false, true], vec![true, false]],
        ),
        (
            RecordFormat::Hits,
            b"1,1\r\n2\r\n".as_slice(),
            3,
            vec![vec![false, false, false], vec![false, false, true]],
        ),
        (
            RecordFormat::Dets,
            b"   shot M1\r\nshot\r\n".as_slice(),
            2,
            vec![vec![false, true], vec![false, false]],
        ),
        // Duplicate DETS tokens set dense bits instead of toggling them.
        (
            RecordFormat::Dets,
            b"shot M0 M0\n".as_slice(),
            1,
            vec![vec![true]],
        ),
    ] {
        assert_eq!(decode(input, format, width).unwrap(), expected, "{input:?}");
        assert_eq!(read_records(input, format, width).unwrap(), expected);
    }

    for (format, input, width) in [
        (RecordFormat::ZeroOne, b"10".as_slice(), 2),
        (RecordFormat::ZeroOne, b"0x\n".as_slice(), 2),
        (RecordFormat::ZeroOne, b"0\n".as_slice(), 2),
        (RecordFormat::B8, &[0x01], 9),
        (RecordFormat::B8, &[], 0),
        (RecordFormat::R8, &[3], 2),
        (RecordFormat::Hits, b"1,,2\n".as_slice(), 3),
        (RecordFormat::Hits, b"1,\n".as_slice(), 3),
        (RecordFormat::Hits, b",1\n".as_slice(), 3),
        (RecordFormat::Hits, b"1,2".as_slice(), 3),
        (RecordFormat::Dets, b"shot  M0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot M0 \n".as_slice(), 1),
        (RecordFormat::Dets, b"shot\tM0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot D0\n".as_slice(), 1),
        (RecordFormat::Dets, b"shot L0\n".as_slice(), 1),
        (RecordFormat::Ptb64, &ptb64[..31], 4),
        (RecordFormat::Ptb64, &ptb64, 0),
    ] {
        assert!(
            decode(input, format, width).is_err(),
            "{format:?} accepted malformed fixture {input:?}"
        );
        assert!(read_records(input, format, width).is_err(), "{input:?}");
    }
    assert!(write_records(&ptb64_records[..63], RecordFormat::Ptb64).is_err());
}
