use std::ffi::OsString;

use super::{FailingWriter, run_from};
use tempfile::tempdir;

struct CliRun {
    status: i32,
    stdout: Vec<u8>,
    stderr: String,
}

fn run_m2d(args: Vec<OsString>, input: &[u8]) -> CliRun {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, input, &mut stdout, &mut stderr);
    CliRun {
        status,
        stdout,
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    }
}

fn m2d_args(extra: &[&str], circuit_path: std::ffi::OsString) -> Vec<OsString> {
    let mut args = vec![OsString::from("stab"), OsString::from("m2d")];
    args.extend(extra.iter().copied().map(OsString::from));
    args.push(OsString::from("--circuit"));
    args.push(circuit_path);
    args
}

#[test]
fn pf7_m2d_cli_accepts_append_observables_and_skip_reference() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    std::fs::write(
        &circuit_path,
        "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    )
    .expect("write circuit");
    let input = b"00\n01\n10\n11\n";

    let appended = run_m2d(
        m2d_args(
            &[
                "--in_format=01",
                "--out_format=dets",
                "--append_observables",
            ],
            circuit_path.as_os_str().to_os_string(),
        ),
        input.as_slice(),
    );
    assert_eq!(appended.status, 0);
    assert_eq!(
        String::from_utf8(appended.stdout).expect("stdout is UTF-8"),
        "shot D0\nshot D0 D1 L2\nshot\nshot D1 L2\n"
    );
    assert_eq!(appended.stderr, "");

    let detectors_only = run_m2d(
        m2d_args(
            &["--in_format=01", "--out_format=dets"],
            circuit_path.as_os_str().to_os_string(),
        ),
        input.as_slice(),
    );
    assert_eq!(detectors_only.status, 0);
    assert_eq!(
        String::from_utf8(detectors_only.stdout).expect("stdout is UTF-8"),
        "shot D0\nshot D0 D1\nshot\nshot D1\n"
    );
    assert_eq!(detectors_only.stderr, "");

    let skip_reference = run_m2d(
        m2d_args(
            &[
                "--in_format=01",
                "--out_format=dets",
                "--skip_reference_sample",
            ],
            circuit_path.into_os_string(),
        ),
        input.as_slice(),
    );
    assert_eq!(skip_reference.status, 0);
    assert_eq!(
        String::from_utf8(skip_reference.stdout).expect("stdout is UTF-8"),
        "shot\nshot D1\nshot D0\nshot D0 D1\n"
    );
    assert_eq!(skip_reference.stderr, "");
}

#[test]
fn pf7_m2d_cli_writes_sparse_observable_side_output_widths() {
    let dir = tempdir().expect("temp dir");
    for (observable_index, expected_obs) in [(0, "0\n"), (10, "00000000000\n")] {
        let circuit_path = dir
            .path()
            .join(format!("observable_{observable_index}.stim"));
        let obs_path = dir.path().join(format!("observable_{observable_index}.01"));
        std::fs::write(
            &circuit_path,
            format!(
                "M 0\nREPEAT 1024 {{\n    DETECTOR rec[-1]\n}}\nOBSERVABLE_INCLUDE({observable_index}) rec[-1]\n"
            ),
        )
        .expect("write circuit");

        let run = run_m2d(
            m2d_args(
                &[
                    "--in_format=01",
                    "--obs_out",
                    obs_path.to_str().expect("utf-8 path"),
                ],
                circuit_path.into_os_string(),
            ),
            b"0\n",
        );

        assert_eq!(run.status, 0);
        assert_eq!(
            String::from_utf8(run.stdout).expect("stdout is UTF-8"),
            format!("{}\n", "0".repeat(1024))
        );
        assert_eq!(run.stderr, "");
        assert_eq!(
            std::fs::read_to_string(obs_path).expect("read obs output"),
            expected_obs
        );
    }
}

#[test]
fn pf7_m2d_cli_ignores_pauli_target_observable_annotations() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    let obs_path = dir.path().join("obs.01");
    std::fs::write(
        &circuit_path,
        "\
QUBIT_COORDS(0, 0) 0
QUBIT_COORDS(1, 0) 1
QUBIT_COORDS(0, 1) 2
QUBIT_COORDS(1, 1) 3
OBSERVABLE_INCLUDE(0) X0 X1
OBSERVABLE_INCLUDE(1) Z0 Z2
MPP X0*X1*X2*X3 Z0*Z1 Z2*Z3
DEPOLARIZE1(0.001) 0 1 2 3
MPP X0*X1*X2*X3 Z0*Z1 Z2*Z3
DETECTOR rec[-1] rec[-4]
DETECTOR rec[-2] rec[-5]
DETECTOR rec[-3] rec[-6]
OBSERVABLE_INCLUDE(0) X0 X1
OBSERVABLE_INCLUDE(1) Z0 Z2
",
    )
    .expect("write circuit");

    let run = run_m2d(
        m2d_args(
            &[
                "--in_format=01",
                "--obs_out",
                obs_path.to_str().expect("utf-8 path"),
            ],
            circuit_path.into_os_string(),
        ),
        b"000000\n100100\n000110\n",
    );

    assert_eq!(run.status, 0);
    assert_eq!(
        String::from_utf8(run.stdout).expect("stdout is UTF-8"),
        "000\n000\n011\n"
    );
    assert_eq!(run.stderr, "");
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("read obs output"),
        "00\n00\n00\n"
    );
}

#[test]
fn pf7_m2d_cli_rejects_selected_format_width_and_writer_failures() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0 1\nDETECTOR rec[-1]\n").expect("write circuit");

    let ptb64_output = run_m2d(
        m2d_args(
            &["--in_format=01", "--out_format=ptb64"],
            circuit_path.as_os_str().to_os_string(),
        ),
        b"00\n",
    );
    assert_eq!(ptb64_output.status, 1);
    assert_eq!(ptb64_output.stdout, b"");
    assert!(
        ptb64_output
            .stderr
            .contains("format ptb64 is not supported for detection data"),
        "{}",
        ptb64_output.stderr
    );

    let width_mismatch = run_m2d(
        m2d_args(&["--in_format=01"], circuit_path.as_os_str().to_os_string()),
        b"0\n",
    );
    assert_eq!(width_mismatch.status, 1);
    assert_eq!(width_mismatch.stdout, b"");
    assert!(width_mismatch.stderr.contains("expected 2 bits"));

    let bad_dets = run_m2d(
        m2d_args(
            &["--in_format=dets", "--out_format=dets"],
            circuit_path.as_os_str().to_os_string(),
        ),
        b"shot D0\n",
    );
    assert_eq!(bad_dets.status, 1);
    assert_eq!(bad_dets.stdout, b"");
    assert!(
        bad_dets
            .stderr
            .contains("measurement dets input cannot contain D tokens")
    );

    let mut stdout = FailingWriter;
    let mut stderr = Vec::new();
    let status = run_from(
        m2d_args(
            &["--in_format=ptb64", "--out_format=01"],
            circuit_path.into_os_string(),
        ),
        vec![0; 125_008].as_slice(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert!(
        String::from_utf8(stderr)
            .expect("stderr is UTF-8")
            .contains("failed to write output: intentional write stop")
    );
}
