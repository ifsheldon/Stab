use super::run_from;
use tempfile::tempdir;

#[test]
fn m2d_ran_without_feedback_preserves_sweep_controls() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let sweep_path = temp_dir.path().join("sweep.01");
    std::fs::write(
        &circuit_path,
        "CX sweep[0] 0\nM 0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(&sweep_path, "0\n1\n").expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=dets",
            "--ran_without_feedback",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"00\n10\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "shot\nshot\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_uses_all_false_sweep_bits_when_sweep_input_is_omitted() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=dets",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "shot\nshot D0\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_streams_sweep_conditioned_text_records() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let sweep_path = temp_dir.path().join("sweep.01");
    std::fs::write(&circuit_path, "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");
    std::fs::write(&sweep_path, "0\n1\n0\n1\n").expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=dets",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=01",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n0\n1\n1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "shot\nshot D0\nshot D0\nshot\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_streams_sweep_conditioned_b8_records() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let sweep_path = temp_dir.path().join("sweep.b8");
    std::fs::write(
        &circuit_path,
        "CX sweep[0] 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(
        &sweep_path,
        [0b0000_0000, 0b0000_0001, 0b0000_0000, 0b0000_0001],
    )
    .expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=b8",
            "--out_format=b8",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        [0b0000_0000, 0b0000_0000, 0b0000_0001, 0b0000_0001].as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(stdout, [0b0000_0000, 0b0000_0001, 0b0000_0001, 0b0000_0000]);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_writes_sweep_conditioned_observables_to_side_output() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let sweep_path = temp_dir.path().join("sweep.01");
    let obs_path = temp_dir.path().join("obs.b8");
    std::fs::write(
        &circuit_path,
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(&sweep_path, "0\n1\n0\n1\n").expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
            "--obs_out_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n0\n1\n1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "0\n1\n1\n0\n");
    assert_eq!(std::fs::read(obs_path).expect("read obs"), [0, 1, 1, 0]);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_rejects_sweep_record_count_mismatches() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let short_sweep_path = temp_dir.path().join("short_sweep.01");
    let long_sweep_path = temp_dir.path().join("long_sweep.01");
    std::fs::write(&circuit_path, "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");
    std::fs::write(&short_sweep_path, "0\n").expect("write short sweep");
    std::fs::write(&long_sweep_path, "0\n0\n").expect("write long sweep");

    let mut short_stdout = Vec::new();
    let mut short_stderr = Vec::new();
    let short_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            short_sweep_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n0\n".as_slice(),
        &mut short_stdout,
        &mut short_stderr,
    );
    assert_eq!(short_status, 1);
    assert_eq!(short_stdout, b"0\n");
    assert!(
        String::from_utf8(short_stderr)
            .unwrap()
            .contains("m2d measurement input has more records than sweep input")
    );

    let mut long_stdout = Vec::new();
    let mut long_stderr = Vec::new();
    let long_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            long_sweep_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n".as_slice(),
        &mut long_stdout,
        &mut long_stderr,
    );
    assert_eq!(long_status, 1);
    assert_eq!(long_stdout, b"0\n");
    assert!(
        String::from_utf8(long_stderr)
            .unwrap()
            .contains("m2d sweep input has more records than measurement input")
    );
}

#[test]
fn m2d_rejects_missing_and_malformed_sweep_input() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let malformed_sweep_path = temp_dir.path().join("malformed_sweep.01");
    let missing_sweep_path = temp_dir.path().join("missing_sweep.01");
    std::fs::write(&circuit_path, "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");
    std::fs::write(&malformed_sweep_path, "00\n").expect("write malformed sweep");

    let mut missing_stdout = Vec::new();
    let mut missing_stderr = Vec::new();
    let missing_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            missing_sweep_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n".as_slice(),
        &mut missing_stdout,
        &mut missing_stderr,
    );
    assert_eq!(missing_status, 1);
    assert_eq!(missing_stdout, b"");
    assert!(
        String::from_utf8(missing_stderr)
            .unwrap()
            .contains("failed to read")
    );

    let mut malformed_stdout = Vec::new();
    let mut malformed_stderr = Vec::new();
    let malformed_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            malformed_sweep_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"0\n".as_slice(),
        &mut malformed_stdout,
        &mut malformed_stderr,
    );
    assert_eq!(malformed_status, 1);
    assert_eq!(malformed_stdout, b"");
    assert!(
        String::from_utf8(malformed_stderr)
            .unwrap()
            .contains("01 record expected 1 bits, got 2")
    );
}

#[test]
fn m2d_accepts_empty_b8_sweep_for_zero_sweep_width_text_inputs() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("empty.stim");
    let sweep_path = temp_dir.path().join("empty_sweep.b8");
    std::fs::write(&circuit_path, "").expect("write circuit");
    std::fs::write(&sweep_path, []).expect("write sweep");

    let mut empty_stdout = Vec::new();
    let mut empty_stderr = Vec::new();
    let empty_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"".as_slice(),
        &mut empty_stdout,
        &mut empty_stderr,
    );
    assert_eq!(empty_status, 0);
    assert_eq!(empty_stdout, b"");
    assert_eq!(String::from_utf8(empty_stderr).unwrap(), "");

    let mut some_stdout = Vec::new();
    let mut some_stderr = Vec::new();
    let some_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"\n\n".as_slice(),
        &mut some_stdout,
        &mut some_stderr,
    );
    assert_eq!(some_status, 0);
    assert_eq!(some_stdout, b"\n\n");
    assert_eq!(String::from_utf8(some_stderr).unwrap(), "");
}

#[test]
fn m2d_rejects_zero_width_b8_measurement_input_like_stim() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("empty.stim");
    let sweep_path = temp_dir.path().join("empty_sweep.b8");
    std::fs::write(&circuit_path, "").expect("write circuit");
    std::fs::write(&sweep_path, []).expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=b8",
            "--out_format=01",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"".as_slice(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("m2d measurement input b8 input cannot represent zero-width records")
    );
}

#[test]
fn m2d_rejects_nonempty_b8_sweep_for_zero_sweep_width() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("empty.stim");
    let sweep_path = temp_dir.path().join("nonempty_sweep.b8");
    std::fs::write(&circuit_path, "").expect("write circuit");
    std::fs::write(&sweep_path, [0]).expect("write sweep");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            sweep_path.to_str().expect("utf-8 path"),
            "--sweep_format=b8",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"".as_slice(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("m2d sweep input b8 zero-width input must be empty")
    );
}
