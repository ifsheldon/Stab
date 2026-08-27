#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "sampling facade tests use direct fixture parsing and exhaustive error diagnostics"
)]

use super::*;
use crate::RecordFormat;

fn sampler(input: &str) -> CompiledSampler {
    let circuit = Circuit::from_stim_str(input).expect("parse circuit");
    CompiledSampler::compile(&circuit).expect("compile sampler")
}

#[test]
fn compiled_sampler_equality_preserves_executable_plan_semantics() {
    let first = sampler("H 0\nM 0\n");
    let same = sampler("H 0\nM 0\n");
    let different = sampler("H 0\nM 0 0\n");

    assert_eq!(first, same);
    assert_ne!(first, different);
}

#[test]
fn writes_stim_text_sample_formats() {
    let sampler = sampler("X 2 3 5\nM 0 1 2 3 4 5\n");

    assert_eq!(sampler.sample_bytes(1, RecordFormat::ZeroOne), b"001101\n");
    assert_eq!(sampler.sample_bytes(1, RecordFormat::B8), &[0x2c]);
    assert_eq!(
        sampler.sample_bytes(1, RecordFormat::R8),
        &[0x02, 0x00, 0x01, 0x00]
    );
    assert_eq!(sampler.sample_bytes(1, RecordFormat::Hits), b"2,3,5\n");
    assert_eq!(
        sampler.sample_bytes(1, RecordFormat::Dets),
        b"shot M2 M3 M5\n"
    );
    assert_eq!(
        sampler.sample_bytes(2, RecordFormat::Hits),
        b"2,3,5\n2,3,5\n"
    );
}

#[test]
fn seeded_sample_bytes_match_seeded_record_samples() {
    let sampler = sampler("H 0\nM 0\nM 0\nMPAD 0 1\n");
    let records = sampler.sample_zero_one_with_seed(32, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(32, RecordFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, RecordFormat::ZeroOne)
            .expect("encode 01 records")
    );
    assert_eq!(
        sampler.sample_bytes_with_seed(32, RecordFormat::B8, Some(5)),
        crate::result_formats::write_records(&records, RecordFormat::B8)
            .expect("encode b8 records")
    );
}

#[test]
fn fallible_materialization_and_encoding_preserve_seeded_outputs() {
    let deterministic = sampler("X 1\nM 0 1\n");
    let sampler = sampler(
        "H 0\n\
         M 0\n\
         CX rec[-1] 1\n\
         M 1\n\
         MPAD 0 1 0 1\n",
    );
    let records = sampler
        .try_sample_zero_one_with_seed(65, Some(17))
        .expect("fallible seeded materialization");
    assert_eq!(
        records,
        sampler.sample_zero_one_with_seed(65, Some(17)),
        "legacy materialization must delegate without changing seeded records"
    );

    for format in [
        RecordFormat::ZeroOne,
        RecordFormat::B8,
        RecordFormat::R8,
        RecordFormat::Hits,
        RecordFormat::Dets,
    ] {
        let expected =
            crate::result_formats::write_records(&records, format).expect("encode records");
        let fallible = sampler
            .try_sample_bytes_with_seed(65, format, Some(17))
            .expect("fallible seeded encoding");
        assert_eq!(fallible, expected, "{format:?}");
        assert_eq!(
            sampler.sample_bytes_with_seed(65, format, Some(17)),
            expected,
            "legacy {format:?} encoding must delegate without changing bytes"
        );
    }
    assert_eq!(
        deterministic
            .try_sample_zero_one_bytes(3)
            .expect("fallible 01 convenience adapter"),
        deterministic.sample_zero_one_bytes(3)
    );
}

#[test]
fn fallible_materializers_reject_usize_max_before_execution() {
    let sampler = sampler("M 0\n");

    let records_error = sampler
        .try_sample_zero_one_with_seed(usize::MAX, Some(1))
        .expect_err("materialized record request must reject");
    assert_zero_progress_storage_error(records_error, "record storage overflowed");

    let bytes_error = sampler
        .try_sample_bytes_with_seed(usize::MAX, RecordFormat::ZeroOne, Some(1))
        .expect_err("encoded output request must reject");
    assert_zero_progress_storage_error(bytes_error, "encoded output length overflowed");
}

fn assert_zero_progress_storage_error(
    error: RunError<std::convert::Infallible>,
    expected_message: &str,
) {
    match error {
        RunError::Engine {
            source: SamplingExecutionError::SessionStorageAllocation { message },
            progress,
        } => {
            assert!(
                message.contains(expected_message),
                "unexpected preflight diagnostic: {message}"
            );
            assert_eq!(progress.committed_shots(), ShotCount::new(0));
            assert_eq!(progress.attempted_batch_shots(), ShotCount::new(0));
        }
        other => panic!("unexpected fallible materialization error: {other:?}"),
    }
}

#[test]
fn streaming_samples_match_seeded_record_samples() {
    let sampler = sampler("H 0\nM 0\nCX rec[-1] 1\nM 1\n");
    let expected = sampler.sample_zero_one_with_seed(32, Some(5));
    let mut streamed = Vec::new();

    let result =
        sampler.for_each_sample_with_seed_and_reference_mode(32, Some(5), false, |record| {
            streamed.push(record.to_vec());
            Ok::<(), std::convert::Infallible>(())
        });

    match result {
        Ok(()) => {}
        Err(error) => match error {},
    }
    assert_eq!(streamed, expected);
}

#[test]
fn byte_sampling_measure_reset_uses_physical_result_for_reset() {
    let inverted_sampler = sampler("MR !0\nM 0\n");
    assert_eq!(
        inverted_sampler.sample_bytes(1, RecordFormat::ZeroOne),
        b"10\n"
    );
    assert_eq!(inverted_sampler.sample_bytes(1, RecordFormat::B8), &[0x01]);

    let noisy_sampler = sampler("MR(1) 0\nM 0\n");
    assert_eq!(
        noisy_sampler.sample_bytes_with_seed(1, RecordFormat::ZeroOne, Some(5)),
        b"10\n"
    );
    assert_eq!(
        noisy_sampler.sample_bytes_with_seed(1, RecordFormat::B8, Some(5)),
        &[0x01]
    );
}

#[test]
fn packed_sample_bytes_match_seeded_record_samples_for_surface_like_ops() {
    let sampler = sampler(
        "
        R 0 1 2 3
        H 0 2
        DEPOLARIZE1(0.001) 0 2
        CX 0 1 2 3
        DEPOLARIZE2(0.001) 0 1 2 3
        MR 0 2
        REPEAT 2 {
            H 0 2
            CX 0 1 2 3
            DEPOLARIZE2(0.001) 0 1 2 3
            H 0 2
            MR 0 2
        }
        M 1 3
        ",
    );
    let records = sampler.sample_zero_one_with_seed(64, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(64, RecordFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, RecordFormat::ZeroOne)
            .expect("encode 01 records")
    );
    assert_eq!(
        sampler.sample_bytes_with_seed(64, RecordFormat::B8, Some(5)),
        crate::result_formats::write_records(&records, RecordFormat::B8)
            .expect("encode b8 records")
    );
}

#[test]
fn direct_noisy_z_measurement_bytes_match_seeded_record_samples() {
    let sampler = sampler("X_ERROR(0.25) 0\nM 0\n");
    let records = sampler.sample_zero_one_with_seed(128, Some(5));

    assert_eq!(
        sampler.sample_bytes_with_seed(128, RecordFormat::ZeroOne, Some(5)),
        crate::result_formats::write_records(&records, RecordFormat::ZeroOne)
            .expect("encode 01 records")
    );
}

#[test]
fn writes_r8_samples_with_long_false_runs() {
    let compiled = sampler("X 1\nM 0 0 0 0 0 0 0 0 0 1\n");

    assert_eq!(compiled.sample_bytes(1, RecordFormat::R8), &[0x09, 0x00]);

    let long_zero_sampler = sampler(&format!("MPAD {}\n", "0 ".repeat(260)));
    assert_eq!(
        long_zero_sampler.sample_bytes(1, RecordFormat::R8),
        &[0xff, 0x05]
    );
}

#[test]
fn writes_ptb64_samples_in_measurement_major_shot_groups() {
    let sampler = sampler("X 1\nM 0 1\n");

    assert_eq!(
        sampler
            .sample_ptb64_bytes_with_seed(64, Some(5))
            .expect("sample ptb64"),
        [
            0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
        ]
    );
}

#[test]
fn rejects_ptb64_shot_counts_that_are_not_multiple_of_64() {
    let sampler = sampler("M 0\n");

    assert_eq!(
        sampler.sample_ptb64_bytes_with_seed(63, Some(5)),
        Err(CircuitError::invalid_sampler_compilation(
            "shots must be a multiple of 64 to use ptb64 format"
        ))
    );
}

#[test]
fn writes_b8_samples_with_per_shot_padding() {
    let sampler = sampler("X 0 8\nM 0 1 2 3 4 5 6 7 8\n");

    assert_eq!(
        sampler.sample_bytes(2, RecordFormat::B8),
        &[0x01, 0x01, 0x01, 0x01]
    );
}
