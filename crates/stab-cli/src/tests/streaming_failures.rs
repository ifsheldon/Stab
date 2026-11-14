//! Streaming output contracts: large outputs stream batch by batch, writer failures surface with
//! exact progress, and a broken pipe exits like Stim's SIGPIPE death.

use std::io::ErrorKind;

use super::support::{
    CountingWriter, FailAfterOneWrite, FailingWriter, FlushFailWriter, KindFailWriter, Scratch,
};
use crate::run_from;

#[test]
fn huge_outputs_stream_until_the_writer_fails() {
    let scratch = Scratch::new();
    let circuit = scratch.file("circuit.stim", "M 0\nDETECTOR rec[-1]\n");
    let ptb64_input = vec![0; 125_008];
    for (args, stdin, expected) in [
        (
            &["stab", "sample", "--shots=1000000000"][..],
            &b"M 0\n"[..],
            "sampling sink write-batch failed after 0 committed shots while attempting 256 shots: \
             intentional write stop",
        ),
        (
            &["stab", "detect", "--shots", "64000001"],
            b"M 0\nDETECTOR rec[-1]\n",
            "failed to write output: intentional write stop",
        ),
        (
            &["stab", "sample_dem", "--shots", "64000001"],
            b"",
            "failed to write output: intentional write stop",
        ),
        (
            &[
                "stab",
                "m2d",
                "--in_format=ptb64",
                "--out_format=01",
                "--circuit",
                &circuit,
            ],
            &ptb64_input,
            "failed to write output: intentional write stop",
        ),
    ] {
        let mut stderr = Vec::new();
        let status = run_from(args.iter().copied(), stdin, FailingWriter, &mut stderr);
        assert_eq!(status, 1, "{args:?}");
        let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
    }
}

#[test]
fn sample_reports_second_batch_writer_failure_with_exact_progress() {
    let mut stdout = FailAfterOneWrite::default();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=512"],
        b"M 0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout.writes, 2);
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    for expected in [
        "sampling sink write-batch failed",
        "after 256 committed shots",
        "while attempting 256 shots",
        "intentional second-batch stop",
    ] {
        assert!(stderr.contains(expected), "{stderr}");
    }
}

#[test]
fn sample_reports_flush_failure_as_sink_finalization() {
    let mut stdout = FlushFailWriter::default();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=65"],
        b"M 0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout.bytes, 130);
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    for expected in [
        "sampling sink finish failed",
        "after 65 committed shots",
        "while attempting 0 shots",
        "intentional finish failure",
    ] {
        assert!(stderr.contains(expected), "{stderr}");
    }
}

#[test]
fn primary_and_side_outputs_stream_across_batch_boundaries() {
    let scratch = Scratch::new();
    let circuit = scratch.file(
        "circuit.stim",
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let observables = scratch.path("observables.b8");
    let errors = scratch.path("errors.b8");
    let measurements = b"0\n".repeat(4096);
    for (args, stdin, side_outputs) in [
        (
            &[
                "stab",
                "detect",
                "--shots=4096",
                "--out_format=01",
                "--obs_out",
                &observables,
                "--obs_out_format=b8",
            ][..],
            &b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"[..],
            &["observables.b8"][..],
        ),
        (
            &[
                "stab",
                "m2d",
                "--in_format=01",
                "--out_format=01",
                "--obs_out",
                &observables,
                "--obs_out_format=b8",
                "--circuit",
                &circuit,
            ],
            &measurements,
            &["observables.b8"],
        ),
        (
            &[
                "stab",
                "sample_dem",
                "--shots=4096",
                "--out_format=01",
                "--obs_out",
                &observables,
                "--obs_out_format=b8",
                "--err_out",
                &errors,
                "--err_out_format=b8",
            ],
            b"error(1) D0 L0\n",
            &["observables.b8", "errors.b8"],
        ),
    ] {
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(args.iter().copied(), stdin, &mut stdout, &mut stderr);

        assert_eq!(status, 0, "{args:?}: {}", String::from_utf8_lossy(&stderr));
        assert!(stderr.is_empty(), "{args:?}");
        assert!(stdout.bytes > 0, "{args:?}");
        assert!(
            stdout.nonempty_writes > 1,
            "{args:?}: primary output stayed in one batch"
        );
        for side in side_outputs {
            assert!(!scratch.read(side).is_empty(), "{args:?}: empty {side}");
        }
    }
}

#[test]
fn broken_output_pipe_exits_141_with_empty_stderr_like_stim() {
    // Pinned Stim dies silently via SIGPIPE when its stdout pipe closes (decision D2); a
    // broken-pipe-rooted failure must report nothing and exit with the same shell-visible 141.
    for argv in [
        &["stab", "sample", "--shots", "4"][..],
        &["stab", "detect", "--shots", "4"],
        &[
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
            argv.iter().copied(),
            b"X 0\nM 0\nDETECTOR rec[-1]\n".as_slice(),
            KindFailWriter(ErrorKind::BrokenPipe),
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
        ["stab", "sample", "--shots", "4"],
        b"X 0\nM 0\n".as_slice(),
        KindFailWriter(ErrorKind::StorageFull),
        &mut stderr,
    );
    assert_eq!(code, 1);
    assert!(
        !stderr.is_empty(),
        "non-pipe output failures must keep their diagnostics"
    );
}

/// The binary wraps standard output in [`crate::io_plan::buffered_output`]; these runs use the
/// same wrapper, so a failure while spilling the buffer mid-stream and a failure only at the
/// final flush must both exit non-zero with the existing diagnostic.
#[test]
fn buffered_stdout_reports_mid_stream_and_flush_only_failures() {
    let mut inner = FailAfterOneWrite::default();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=200000"],
        b"M 0\n".as_slice(),
        crate::io_plan::buffered_output(&mut inner),
        &mut stderr,
    );
    assert_eq!(status, 1);
    // The first spill succeeds and the second fails; dropping the buffer may retry once more.
    assert!(inner.writes >= 2);
    let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
    assert!(
        stderr.contains("sampling sink write-batch failed"),
        "{stderr}"
    );
    assert!(stderr.contains("intentional second-batch stop"), "{stderr}");

    for args in [
        &["stab", "sample", "--shots=64"][..],
        &["stab", "detect", "--shots=64"],
        &["stab", "sample_dem", "--shots=64"],
        &[
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=hits",
            "--bits_per_shot=3",
        ],
    ] {
        let mut inner = FlushFailWriter::default();
        let mut stderr = Vec::new();
        let stdin: &[u8] = match args.get(1).copied().unwrap_or_default() {
            "convert" => b"101\n",
            "sample_dem" => b"error(0.5) D0\n",
            _ => b"M 0\nDETECTOR rec[-1]\n",
        };
        let status = run_from(
            args.iter().copied(),
            stdin,
            crate::io_plan::buffered_output(&mut inner),
            &mut stderr,
        );
        assert_eq!(status, 1, "{args:?}");
        assert!(
            inner.bytes > 0,
            "{args:?}: buffered bytes never reached the writer"
        );
        let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
        assert!(
            stderr.contains("intentional finish failure"),
            "{args:?}: {stderr}"
        );
    }
}

/// Many small records reach buffered standard output in buffer-sized writes, not one per record
/// or one per 64-shot batch.
#[test]
fn buffered_stdout_writes_many_records_in_bounded_chunks() {
    let circuit = "X_ERROR(0.3) 0 1 2 3 4 5 6 7 8 9 10\nM 0 1 2 3 4 5 6 7 8 9 10\n";
    for format in ["ptb64", "01", "hits", "dets", "r8", "b8"] {
        let mut inner = CountingWriter::default();
        let mut stderr = Vec::new();
        let format_arg = format!("--out_format={format}");
        let status = run_from(
            ["stab", "sample", "--shots=8192", "--seed=5", &format_arg],
            circuit.as_bytes(),
            crate::io_plan::buffered_output(&mut inner),
            &mut stderr,
        );
        assert_eq!(status, 0, "{format}: {}", String::from_utf8_lossy(&stderr));
        let half_buffers = inner.bytes / (crate::io_plan::OUTPUT_BUFFER_BYTES / 2);
        assert!(
            inner.nonempty_writes <= half_buffers + 1,
            "{format}: {} writes for {} bytes",
            inner.nonempty_writes,
            inner.bytes
        );
    }
}
