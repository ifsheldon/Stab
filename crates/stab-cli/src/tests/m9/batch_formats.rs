use std::ffi::OsString;

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
fn detect_streams_primary_and_side_output_across_batch_boundaries() {
    const SHOTS: &str = "4096";
    let circuit = b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_slice();
    let dir = tempdir().expect("temp dir");
    let obs_path = dir.path().join("observables.b8");
    let args = vec![
        OsString::from("stab"),
        OsString::from("detect"),
        OsString::from("--shots"),
        OsString::from(SHOTS),
        OsString::from("--out_format=01"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format=b8"),
    ];
    let mut stdout = CountingWriter::default();
    let mut stderr = Vec::new();
    let status = run_from(args, circuit, &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    assert!(stdout.bytes > 0);
    assert!(
        stdout.nonempty_writes > 1,
        "primary output stayed in one batch"
    );
    assert!(
        std::fs::metadata(obs_path)
            .expect("observable output metadata")
            .len()
            > 0
    );
}

#[test]
fn m2d_streams_primary_and_side_output_across_batch_boundaries() {
    const SHOTS: usize = 4096;
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(
        &circuit_path,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");

    let input = b"0\n".repeat(SHOTS);
    let obs_path = temp_dir.path().join("observables.b8");
    let args = vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--out_format=01"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format=b8"),
        OsString::from("--circuit"),
        circuit_path.into_os_string(),
    ];
    let mut stdout = CountingWriter::default();
    let mut stderr = Vec::new();
    let status = run_from(args, input.as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    assert!(stdout.bytes > 0);
    assert!(
        stdout.nonempty_writes > 1,
        "primary output stayed in one batch"
    );
    assert!(
        std::fs::metadata(obs_path)
            .expect("observable output metadata")
            .len()
            > 0
    );
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
