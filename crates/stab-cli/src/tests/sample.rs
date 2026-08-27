use std::ffi::OsString;

use stab_records::{BitPlane64Batch, MeasurementBatchView, MeasurementSink};
use tempfile::tempdir;

use super::run_from;
use crate::{CliSampleSink, SampleOutFormatArg};

#[test]
fn warmed_cli_sample_encoding_has_no_record_count_dependent_allocation() {
    let mut batch = BitPlane64Batch::zeros(64, 2_048).expect("construct wide batch");
    for bit_index in 0..batch.bits_per_shot() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bit_index modulo 64 is always representable as u32"
        )]
        let rotation = (bit_index % 64) as u32;
        batch
            .copy_plane_from_word(bit_index, u64::MAX.rotate_left(rotation))
            .expect("populate plane");
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
            format,
            visible_measurements: None,
            filtered_record: None,
            writer: format.stream_writer().expect("construct sample writer"),
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
    let batch = BitPlane64Batch::zeros(64, 2_048).expect("construct wide batch");
    let visible = (0..2_048).step_by(2).collect::<Vec<_>>();
    let mut output = std::io::sink();
    let format = SampleOutFormatArg::B8;
    let mut sink = CliSampleSink {
        format,
        visible_measurements: Some(&visible),
        filtered_record: Some(Vec::with_capacity(visible.len())),
        writer: format.stream_writer().expect("construct sample writer"),
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

struct FailingWriter {
    kind: std::io::ErrorKind,
}

impl std::io::Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(self.kind))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(self.kind))
    }
}

#[test]
fn broken_output_pipe_exits_141_with_empty_stderr_like_stim() {
    // Pinned Stim dies silently via SIGPIPE when its stdout pipe closes
    // (decision D2); a broken-pipe-rooted failure must report nothing and
    // exit with the same shell-visible 141 status.
    for argv in [
        vec!["stab", "sample", "--shots", "4"],
        vec!["stab", "detect", "--shots", "4"],
        vec![
            "stab",
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
        ],
    ] {
        let mut stderr = Vec::new();
        let code = run_from(
            argv.iter().map(OsString::from),
            "X 0\nM 0\nDETECTOR rec[-1]\n".as_bytes(),
            FailingWriter {
                kind: std::io::ErrorKind::BrokenPipe,
            },
            &mut stderr,
        );
        assert_eq!(code, 141, "{argv:?}");
        assert!(stderr.is_empty(), "{argv:?}: {stderr:?}");
    }
}

#[test]
fn genuine_output_write_failures_still_report_a_diagnostic() {
    let mut stderr = Vec::new();
    let code = run_from(
        ["stab", "sample", "--shots", "4"]
            .iter()
            .map(OsString::from),
        "X 0\nM 0\n".as_bytes(),
        FailingWriter {
            kind: std::io::ErrorKind::StorageFull,
        },
        &mut stderr,
    );
    assert_eq!(code, 1);
    assert!(
        !stderr.is_empty(),
        "non-pipe output failures must keep their diagnostics"
    );
}
