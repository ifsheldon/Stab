//! `stab sample` behavior not covered by the pinned-Stim manifest rows.

use std::ffi::OsString;

use stab_records::{
    BitIndex, BitPlaneBatch, MeasurementBatchView, MeasurementSink, RecordEncoder, RecordFormat,
    ShotIndex,
};
use tempfile::tempdir;

use super::support::{Scratch, run};
use crate::{CliSampleSink, SampleOutFormatArg, run_from};

#[test]
fn warmed_cli_sample_encoding_has_no_record_count_dependent_allocation() {
    let mut batch = BitPlaneBatch::zeros(256, 2_048).expect("construct wide batch");
    for bit_index in 0..batch.bits_per_shot() {
        for shot_index in 0..batch.shot_count() {
            if (shot_index + bit_index) % 64 != 0 {
                batch
                    .set(ShotIndex(shot_index), BitIndex(bit_index), true)
                    .expect("populate plane");
            }
        }
    }

    for format in [
        SampleOutFormatArg::ZeroOne,
        SampleOutFormatArg::B8,
        SampleOutFormatArg::R8,
        SampleOutFormatArg::Hits,
        SampleOutFormatArg::Dets,
        SampleOutFormatArg::Ptb64,
    ] {
        let mut output = std::io::sink();
        let mut sink = CliSampleSink {
            visible_measurements: None,
            filtered_record: Vec::new(),
            encoder: RecordEncoder::try_new(format.record_format(), 2_048)
                .expect("construct sample encoder"),
            output: &mut output,
        };
        let view = MeasurementBatchView::from_bit_planes(batch.view());
        sink.write_batch(view).expect("warm sample encoder");

        let one = allocation_counter::measure(|| {
            sink.write_batch(view).expect("encode one warmed batch");
        });
        let many = allocation_counter::measure(|| {
            for _ in 0..8 {
                sink.write_batch(view).expect("encode warmed batch");
            }
        });

        assert_eq!(
            one.count_total,
            0,
            "warmed {} allocated for one batch: {one:?}",
            format.record_format().as_str()
        );
        assert_eq!(
            many.count_total,
            0,
            "warmed {} allocated across repeated batches: {many:?}",
            format.record_format().as_str()
        );
    }
}

#[test]
fn warmed_filtered_cli_sample_encoding_reuses_projection_storage() {
    let batch = BitPlaneBatch::zeros(256, 2_048).expect("construct wide batch");
    let visible = (0..2_048).step_by(2).collect::<Vec<_>>();
    let mut output = std::io::sink();
    let mut sink = CliSampleSink {
        visible_measurements: Some(&visible),
        filtered_record: Vec::new(),
        encoder: RecordEncoder::try_new(RecordFormat::B8, visible.len())
            .expect("construct sample encoder"),
        output: &mut output,
    };
    let view = MeasurementBatchView::from_bit_planes(batch.view());
    sink.write_batch(view).expect("warm filtered encoder");

    let repeated = allocation_counter::measure(|| {
        for _ in 0..8 {
            sink.write_batch(view).expect("encode filtered batch");
        }
    });

    assert_eq!(
        repeated.count_total, 0,
        "warmed filtered encoding allocated across repeated batches: {repeated:?}"
    );
}

#[test]
fn zero_shot_sample_ignores_distinct_paths_without_creating_or_truncating_output() {
    let directory = tempdir().expect("temporary directory");
    let missing_input = directory.path().join("missing-input.stim");
    let absent_output = directory.path().join("absent-output.01");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            OsString::from("stab"),
            OsString::from("sample"),
            OsString::from("--shots=0"),
            OsString::from("--in"),
            missing_input.into_os_string(),
            OsString::from("--out"),
            absent_output.clone().into_os_string(),
        ],
        b"\xff".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert!(!absent_output.exists());

    let input = directory.path().join("invalid-input.stim");
    let output = directory.path().join("existing-output.01");
    std::fs::write(&input, b"\xff").expect("write invalid input");
    std::fs::write(&output, b"keep-me\n").expect("write output sentinel");
    let mut stderr = Vec::new();
    let status = run_from(
        [
            OsString::from("stab"),
            OsString::from("sample"),
            OsString::from("--shots=0"),
            OsString::from("--in"),
            input.into_os_string(),
            OsString::from("--out"),
            output.clone().into_os_string(),
        ],
        std::io::empty(),
        std::io::sink(),
        &mut stderr,
    );

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(
        std::fs::read(output).expect("read output sentinel"),
        b"keep-me\n"
    );
}

#[test]
fn zero_shot_sample_ignores_invalid_stdin() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=0"],
        b"\xff".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
}

#[test]
fn sample_skip_reference_sample_reports_raw_heralded_records() {
    // `--shots=1` and `--shots=2` of this circuit are pinned-Stim manifest rows; the raw
    // frame record with the reference removed is Stab's own contract.
    run(
        &["stab", "sample", "--shots=1", "--skip_reference_sample"],
        include_bytes!("../../../../oracle/fixtures/inputs/sample_heralded.stim"),
    )
    .assert_ok(b"011001001\n");
    run(
        &["stab", "sample", "--skip_reference_sample"],
        b"H 0\nS 0\nS 0\nH 0\nM 0\n",
    )
    .assert_ok(b"0\n");
}

#[test]
fn sample_writes_whole_ptb64_groups() {
    let group = [0; 8].into_iter().chain([0xff; 8]).collect::<Vec<u8>>();
    for (shots, groups) in [("0", 0), ("64", 1), ("128", 2)] {
        run(
            &["stab", "sample", "--shots", shots, "--out_format=ptb64"],
            b"X 1\nM 0 1\n",
        )
        .assert_ok(group.repeat(groups));
    }
}

#[test]
fn sample_seed_makes_noisy_output_reproducible() {
    let input = include_bytes!("../../../../oracle/fixtures/inputs/sample_noisy.stim");
    let first = run(&["stab", "sample", "--shots=1000", "--seed=5"], input);
    let second = run(&["stab", "sample", "--shots=1000", "--seed=5"], input);
    assert_eq!(first.status, 0);
    assert_eq!(first.stderr_text(), "");
    assert_eq!(first, second);
}

#[test]
fn sample_rejects_values_past_stim_i64_cli_bound() {
    for flag in ["--seed", "--shots"] {
        run(&["stab", "sample", flag, "9223372036854775808"], b"M 0\n")
            .assert_err(&["greater than Stim's i64 maximum"]);
    }
}

#[test]
fn sample_loop_folding_flag_preserves_reference_results() {
    let circuit = b"REPEAT 512 {\n    H 0\n    M 0\n    R 0\n}\n";
    // The frame sampler computes and removes the deterministic reference only in
    // `--skip_reference_sample` mode, so this combination exercises loop selection.
    let folded = run(
        &["stab", "sample", "--skip_reference_sample", "--seed=5"],
        circuit,
    );
    let iterated = run(
        &[
            "stab",
            "sample",
            "--skip_reference_sample",
            "--skip_loop_folding",
            "--seed=5",
        ],
        circuit,
    );

    assert_eq!(folded.status, 0);
    assert_eq!(folded, iterated);
    assert_eq!(folded.stdout.len(), 513);
    assert_eq!(folded.stdout.last(), Some(&b'\n'));
    assert!(folded.stderr.is_empty());
}

#[test]
fn sample_reads_and_writes_paths() {
    let scratch = Scratch::new();
    let input = scratch.file("input.stim", "X 0\nMR 0\nMR 0\n");
    let output = scratch.path("output.01");
    run(
        &[
            "stab",
            "sample",
            "--shots=1",
            "--in",
            &input,
            "--out",
            &output,
        ],
        b"",
    )
    .assert_ok(b"");
    assert_eq!(scratch.read("output.01"), b"10\n");
}
