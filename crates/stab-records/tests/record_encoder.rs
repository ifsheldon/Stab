#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::panic_in_result_fn,
    reason = "encoder properties report the exact layout and bytes that violated a contract"
)]

//! Differential properties for the Records-owned [`RecordEncoder`].
//!
//! Every entry point (whole batches, split batches, single records with dirty tail words, and
//! 64-shot bit planes) must produce exactly the bytes of an independent per-bit reference encoder
//! for random layouts, including zero-width parts, widths that are not multiples of 8 or 64,
//! empty batches, all-zero and all-one records, and PTB64 groups.

use proptest::prelude::*;
use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_bits::BitSlice;
use stab_records::{
    BitPlane64Batch, DetsResultType, FormatErrorCode, PackedShotBatch, RecordBatchPart,
    RecordEncoder, RecordFormat, RecordPart, RecordWriteError,
};

const FORMATS: [RecordFormat; 6] = [
    RecordFormat::ZeroOne,
    RecordFormat::B8,
    RecordFormat::R8,
    RecordFormat::Hits,
    RecordFormat::Dets,
    RecordFormat::Ptb64,
];

const NAMESPACES: [DetsResultType; 3] = [
    DetsResultType::Measurement,
    DetsResultType::Detector,
    DetsResultType::Observable,
];

/// One column group: its namespace and every shot's bits.
#[derive(Clone, Debug)]
struct Group {
    namespace: DetsResultType,
    width: usize,
    shots: Vec<Vec<bool>>,
}

/// Independent per-bit reference: one record is the concatenation of every group's shot bits.
fn reference(format: RecordFormat, groups: &[Group], shot_count: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let width: usize = groups.iter().map(|group| group.width).sum();
    if format == RecordFormat::Ptb64 {
        for group_start in (0..shot_count).step_by(64) {
            for bit in 0..width {
                let mut word = 0_u64;
                for lane in 0..64 {
                    if concatenated(groups, group_start + lane)[bit] {
                        word |= 1 << lane;
                    }
                }
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
        return out;
    }
    for shot in 0..shot_count {
        let bits = concatenated(groups, shot);
        match format {
            RecordFormat::ZeroOne => {
                out.extend(bits.iter().map(|bit| if *bit { b'1' } else { b'0' }));
                out.push(b'\n');
            }
            RecordFormat::B8 => {
                for chunk in bits.chunks(8) {
                    let mut byte = 0_u8;
                    for (index, bit) in chunk.iter().enumerate() {
                        if *bit {
                            byte |= 1 << index;
                        }
                    }
                    out.push(byte);
                }
            }
            RecordFormat::R8 => {
                let mut run = 0_u8;
                for bit in &bits {
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
            RecordFormat::Hits => {
                let hits = (0..bits.len())
                    .filter(|index| bits[*index])
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>();
                out.extend_from_slice(hits.join(",").as_bytes());
                out.push(b'\n');
            }
            _ => {
                out.extend_from_slice(b"shot");
                for group in groups {
                    let prefix = char::from(group.namespace.prefix());
                    for (index, bit) in group.shots[shot].iter().enumerate() {
                        if *bit {
                            out.extend_from_slice(format!(" {prefix}{index}").as_bytes());
                        }
                    }
                }
                out.push(b'\n');
            }
        }
    }
    out
}

fn concatenated(groups: &[Group], shot: usize) -> Vec<bool> {
    groups
        .iter()
        .flat_map(|group| group.shots[shot].iter().copied())
        .collect()
}

fn packed(group: &Group, shots: std::ops::Range<usize>) -> PackedShotBatch {
    PackedShotBatch::from_records(&group.shots[shots], group.width).expect("packed group")
}

fn encode_batches(
    format: RecordFormat,
    groups: &[Group],
    shot_count: usize,
    split: usize,
) -> Result<Vec<u8>, RecordWriteError> {
    let width = groups.iter().map(|group| group.width).sum();
    let mut encoder = RecordEncoder::try_new(format, width)?;
    let mut out = Vec::new();
    for range in [0..split, split..shot_count] {
        let batches = groups
            .iter()
            .map(|group| packed(group, range.clone()))
            .collect::<Vec<_>>();
        let parts = groups
            .iter()
            .zip(&batches)
            .map(|(group, batch)| RecordBatchPart::new(group.namespace, batch.view()))
            .collect::<Vec<_>>();
        encoder.write_batch(&parts, &mut out)?;
    }
    assert_eq!(encoder.pending_group_records(), 0);
    Ok(out)
}

/// Encodes record by record from raw words whose bits beyond each part's width are all set.
fn encode_dirty_records(
    format: RecordFormat,
    groups: &[Group],
    shot_count: usize,
) -> Result<Vec<u8>, RecordWriteError> {
    let width = groups.iter().map(|group| group.width).sum();
    let mut encoder = RecordEncoder::try_new(format, width)?;
    let mut out = Vec::new();
    for shot in 0..shot_count {
        let words = groups
            .iter()
            .map(|group| {
                let mut words = vec![0_u64; group.width.div_ceil(64)];
                for (index, bit) in group.shots[shot].iter().enumerate() {
                    if *bit {
                        words[index / 64] |= 1 << (index % 64);
                    }
                }
                if let Some(last) = words.last_mut()
                    && group.width % 64 != 0
                {
                    *last |= u64::MAX << (group.width % 64);
                }
                words
            })
            .collect::<Vec<_>>();
        let parts = groups
            .iter()
            .zip(&words)
            .map(|(group, words)| {
                RecordPart::new(
                    group.namespace,
                    BitSlice::new(words, group.width).expect("record words"),
                )
            })
            .collect::<Vec<_>>();
        encoder.write_record(&parts, &mut out)?;
    }
    Ok(out)
}

fn encode_bit_planes(
    format: RecordFormat,
    group: &Group,
    shot_count: usize,
) -> Result<Vec<u8>, RecordWriteError> {
    let mut encoder = RecordEncoder::try_new(format, group.width)?;
    let mut out = Vec::new();
    for start in (0..shot_count).step_by(64) {
        let end = (start + 64).min(shot_count);
        let planes =
            BitPlane64Batch::from_shot_major(packed(group, start..end).view()).expect("bit planes");
        encoder.write_bit_planes(planes.view(), group.namespace, &mut out)?;
    }
    Ok(out)
}

fn group(width: usize, shot_count: usize) -> impl Strategy<Value = Group> {
    (
        prop::sample::select(NAMESPACES.to_vec()),
        prop_oneof![Just(0_u32), Just(1_u32), 2_u32..=255, Just(256_u32)],
    )
        .prop_flat_map(move |(namespace, density)| {
            prop::collection::vec(
                prop::collection::vec((0_u32..256).prop_map(move |roll| roll < density), width),
                shot_count,
            )
            .prop_map(move |shots| Group {
                namespace,
                width,
                shots,
            })
        })
}

fn case() -> impl Strategy<Value = (RecordFormat, Vec<Group>, usize, usize)> {
    let width = prop_oneof![
        3 => 0_usize..=200,
        2 => prop::sample::select(vec![0_usize, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 255, 256, 600]),
    ];
    (
        prop::sample::select(FORMATS.to_vec()),
        prop::collection::vec(width, 1..=3),
        0_usize..=130,
    )
        .prop_flat_map(|(format, widths, shots)| {
            let shot_count = if format == RecordFormat::Ptb64 {
                (shots / 64) * 64
            } else {
                shots
            };
            let groups = widths
                .into_iter()
                .map(|width| group(width, shot_count))
                .collect::<Vec<_>>();
            (Just(format), groups, Just(shot_count), 0..=shot_count)
        })
        .prop_map(|(format, groups, shot_count, split)| {
            let split = if format == RecordFormat::Ptb64 {
                split / 64 * 64
            } else {
                split
            };
            (format, groups, shot_count, split)
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

proptest! {
    #![proptest_config(config(0xe6e6_0001, 512))]

    #[test]
    fn every_entry_point_matches_the_per_bit_reference(
        (format, groups, shot_count, split) in case(),
    ) {
        let widths = groups.iter().map(|group| group.width).collect::<Vec<_>>();
        let context = format!("format={} widths={widths:?} shots={shot_count} split={split}", format.as_str());
        let expected = reference(format, &groups, shot_count);

        let batched = encode_batches(format, &groups, shot_count, split).expect("batch encode");
        prop_assert_eq!(&batched, &expected, "batch: {}", context);

        let records = encode_dirty_records(format, &groups, shot_count).expect("record encode");
        prop_assert_eq!(&records, &expected, "record: {}", context);

        if let [group] = groups.as_slice() {
            let planes = encode_bit_planes(format, group, shot_count).expect("bit-plane encode");
            prop_assert_eq!(&planes, &expected, "bit planes: {}", context);
        }
    }
}

#[test]
fn ptb64_holds_partial_groups_until_the_64th_record() {
    let records = (0..64)
        .map(|shot| vec![shot % 3 == 0, true, false])
        .collect::<Vec<_>>();
    let group = Group {
        namespace: DetsResultType::Measurement,
        width: 3,
        shots: records,
    };
    let mut encoder = RecordEncoder::try_new(RecordFormat::Ptb64, 3).expect("encoder");
    let mut out = Vec::new();
    let first = packed(&group, 0..63);
    encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                first.view(),
            )],
            &mut out,
        )
        .expect("partial group");
    assert!(out.is_empty());
    assert_eq!(encoder.pending_group_records(), 63);

    let last = packed(&group, 63..64);
    encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                last.view(),
            )],
            &mut out,
        )
        .expect("complete group");
    assert_eq!(encoder.pending_group_records(), 0);
    assert_eq!(out, reference(RecordFormat::Ptb64, &[group], 64));
}

#[test]
fn mismatched_layouts_are_rejected_before_any_byte_is_written() {
    let mut encoder = RecordEncoder::try_new(RecordFormat::Hits, 4).expect("encoder");
    let mut out = Vec::new();
    let narrow = PackedShotBatch::zeros(2, 3).expect("narrow batch");
    let error = encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Detector,
                narrow.view(),
            )],
            &mut out,
        )
        .expect_err("width mismatch");
    assert!(matches!(
        error,
        RecordWriteError::Format(ref error) if error.code() == FormatErrorCode::InvalidRecordWidth
    ));

    let detectors = PackedShotBatch::zeros(2, 2).expect("detectors");
    let observables = PackedShotBatch::zeros(3, 2).expect("observables");
    assert!(
        encoder
            .write_batch(
                &[
                    RecordBatchPart::new(DetsResultType::Detector, detectors.view()),
                    RecordBatchPart::new(DetsResultType::Observable, observables.view()),
                ],
                &mut out,
            )
            .is_err()
    );
    assert!(encoder.write_batch(&[], &mut out).is_err());
    assert!(out.is_empty());
}

/// Fails every write after accepting `budget` bytes.
struct FailAfter {
    budget: usize,
}

impl std::io::Write for FailAfter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.budget {
            return Err(std::io::Error::other("injected write failure"));
        }
        self.budget -= buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn destination_failures_surface_as_io_errors() {
    let records = PackedShotBatch::from_records(&vec![vec![true; 100]; 4096], 100).expect("batch");
    let mut encoder = RecordEncoder::try_new(RecordFormat::ZeroOne, 100).expect("encoder");
    let error = encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                records.view(),
            )],
            &mut FailAfter { budget: 1000 },
        )
        .expect_err("injected failure");
    assert!(
        matches!(error, RecordWriteError::Io(ref error) if error.to_string() == "injected write failure")
    );
}
