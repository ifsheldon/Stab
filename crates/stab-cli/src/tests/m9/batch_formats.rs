use std::ffi::OsString;

use stab_core::{
    SampleFormat,
    result_formats::{write_ptb64_records_checked, write_records},
};
use tempfile::tempdir;

use super::{FailingWriter, run_from};

#[derive(Debug, Default)]
struct CountingWriter {
    bytes: usize,
    nonempty_writes: usize,
}

impl std::io::Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes += buffer.len();
        self.nonempty_writes += usize::from(!buffer.is_empty());
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn detect_routes_4096_shots_through_every_supported_batch_format() {
    const SHOTS: &str = "4096";
    let circuit = b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_slice();
    for format in ["01", "b8", "r8", "hits", "dets", "ptb64"] {
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(
            ["stab", "detect", "--shots", SHOTS, "--out_format", format],
            circuit,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 0, "primary format={format}");
        assert!(stderr.is_empty(), "primary format={format}");
        assert!(stdout.bytes > 0, "primary format={format}");
        assert!(
            stdout.nonempty_writes > 1,
            "primary format={format} did not cross a batch boundary"
        );
    }

    for format in ["01", "b8", "r8", "hits", "dets", "ptb64"] {
        let dir = tempdir().expect("temp dir");
        let obs_path = dir.path().join(format!("observables.{format}"));
        let args = vec![
            OsString::from("stab"),
            OsString::from("detect"),
            OsString::from("--shots"),
            OsString::from(SHOTS),
            OsString::from("--out_format=01"),
            OsString::from("--obs_out"),
            obs_path.clone().into_os_string(),
            OsString::from("--obs_out_format"),
            OsString::from(format),
        ];
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(args, circuit, &mut stdout, &mut stderr);

        assert_eq!(status, 0, "observable format={format}");
        assert!(stderr.is_empty(), "observable format={format}");
        assert!(stdout.nonempty_writes > 1, "observable format={format}");
        assert!(
            std::fs::metadata(obs_path)
                .expect("observable output metadata")
                .len()
                > 0,
            "observable format={format}"
        );
    }
}

#[test]
fn m2d_routes_4096_records_through_every_supported_batch_format() {
    const SHOTS: usize = 4096;
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(
        &circuit_path,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");

    for (format_name, format) in [
        ("01", TestRecordFormat::Sample(SampleFormat::ZeroOne)),
        ("b8", TestRecordFormat::Sample(SampleFormat::B8)),
        ("r8", TestRecordFormat::Sample(SampleFormat::R8)),
        ("hits", TestRecordFormat::Sample(SampleFormat::Hits)),
        ("dets", TestRecordFormat::Sample(SampleFormat::Dets)),
        ("ptb64", TestRecordFormat::Ptb64),
    ] {
        let input = encoded_single_bit_records(format, SHOTS);
        let args = vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format"),
            OsString::from(format_name),
            OsString::from("--out_format=01"),
            OsString::from("--circuit"),
            circuit_path.clone().into_os_string(),
        ];
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(args, input.as_slice(), &mut stdout, &mut stderr);

        assert_eq!(status, 0, "input format={format_name}");
        assert!(stderr.is_empty(), "input format={format_name}");
        assert!(stdout.bytes > 0, "input format={format_name}");
        assert!(
            stdout.nonempty_writes > 1,
            "input format={format_name} did not cross a batch boundary"
        );
    }

    let input = encoded_single_bit_records(TestRecordFormat::Sample(SampleFormat::ZeroOne), SHOTS);
    for format in ["01", "b8", "r8", "hits", "dets"] {
        let args = vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format"),
            OsString::from(format),
            OsString::from("--circuit"),
            circuit_path.clone().into_os_string(),
        ];
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(args, input.as_slice(), &mut stdout, &mut stderr);

        assert_eq!(status, 0, "output format={format}");
        assert!(stderr.is_empty(), "output format={format}");
        assert!(stdout.bytes > 0, "output format={format}");
        assert!(
            stdout.nonempty_writes > 1,
            "output format={format} did not cross a batch boundary"
        );
    }

    for format in ["01", "b8", "r8", "hits", "dets"] {
        let obs_path = temp_dir.path().join(format!("observables.{format}"));
        let args = vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--obs_out"),
            obs_path.clone().into_os_string(),
            OsString::from("--obs_out_format"),
            OsString::from(format),
            OsString::from("--circuit"),
            circuit_path.clone().into_os_string(),
        ];
        let mut stdout = CountingWriter::default();
        let mut stderr = Vec::new();
        let status = run_from(args, input.as_slice(), &mut stdout, &mut stderr);

        assert_eq!(status, 0, "observable format={format}");
        assert!(stderr.is_empty(), "observable format={format}");
        assert!(stdout.nonempty_writes > 1, "observable format={format}");
        assert!(
            std::fs::metadata(obs_path)
                .expect("observable output metadata")
                .len()
                > 0,
            "observable format={format}"
        );
    }
}

#[test]
fn m2d_streams_large_ptb64_input_until_writer_failure() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut stdout = FailingWriter;
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=ptb64",
            "--out_format=01",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        vec![0; 125_008].as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("failed to write output: intentional write stop")
    );
}

#[derive(Clone, Copy)]
enum TestRecordFormat {
    Sample(SampleFormat),
    Ptb64,
}

fn encoded_single_bit_records(format: TestRecordFormat, shots: usize) -> Vec<u8> {
    let records = vec![vec![false]; shots];
    match format {
        TestRecordFormat::Ptb64 => {
            write_ptb64_records_checked(&records).expect("encode complete 64-shot PTB64 groups")
        }
        TestRecordFormat::Sample(format) => write_records(&records, format),
    }
}
