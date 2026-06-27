use super::run_from;
use tempfile::tempdir;

fn ptb64_words(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn detect_basic_matches_m9_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "detect", "--shots", "3"],
        include_bytes!("../../../../oracle/fixtures/inputs/detect_basic.stim").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m9_detect_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn detect_dets_output_includes_observables_by_default_like_stim() {
    let input = "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";

    let mut default_stdout = Vec::new();
    let mut default_stderr = Vec::new();
    let default_status = run_from(
        ["stab", "detect", "--out_format=dets"],
        input.as_bytes(),
        &mut default_stdout,
        &mut default_stderr,
    );

    assert_eq!(default_status, 0);
    assert_eq!(String::from_utf8(default_stdout).unwrap(), "shot L0 D0\n");
    assert_eq!(String::from_utf8(default_stderr).unwrap(), "");

    let mut append_stdout = Vec::new();
    let mut append_stderr = Vec::new();
    let append_status = run_from(
        [
            "stab",
            "detect",
            "--out_format=dets",
            "--append_observables",
        ],
        input.as_bytes(),
        &mut append_stdout,
        &mut append_stderr,
    );

    assert_eq!(append_status, 0);
    assert_eq!(String::from_utf8(append_stdout).unwrap(), "shot D0 L0\n");
    assert_eq!(String::from_utf8(append_stderr).unwrap(), "");
}

#[test]
fn detect_zero_shots_returns_without_parsing_input() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "detect", "--shots", "0"],
        b"not a stim circuit".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(stdout, b"");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn detect_supports_deprecated_prepend_observables_alias() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "detect", "--prepend_observables"],
        "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "11\n");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("[DEPRECATION] Avoid using `--prepend_observables`")
    );
}

#[test]
fn detect_rejects_conflicting_observable_routes() {
    let temp_dir = tempdir().expect("temp dir");
    let obs_path = temp_dir.path().join("obs.01");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--out_format=dets",
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
        ],
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("cannot combine --prepend_observables")
    );
}

#[test]
fn detect_appends_observables_and_writes_bit_packed_output() {
    let input = "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";

    let mut text_stdout = Vec::new();
    let mut text_stderr = Vec::new();
    let text_status = run_from(
        ["stab", "detect", "--append_observables"],
        input.as_bytes(),
        &mut text_stdout,
        &mut text_stderr,
    );

    assert_eq!(text_status, 0);
    assert_eq!(String::from_utf8(text_stdout).unwrap(), "11\n");
    assert_eq!(String::from_utf8(text_stderr).unwrap(), "");

    let mut b8_stdout = Vec::new();
    let mut b8_stderr = Vec::new();
    let b8_status = run_from(
        [
            "stab",
            "detect",
            "--append_observables",
            "--out_format",
            "b8",
        ],
        input.as_bytes(),
        &mut b8_stdout,
        &mut b8_stderr,
    );

    assert_eq!(b8_status, 0);
    assert_eq!(b8_stdout, [0b0000_0011]);
    assert_eq!(String::from_utf8(b8_stderr).unwrap(), "");
}

#[test]
fn detect_writes_ptb64_detector_and_observable_outputs() {
    let temp_dir = tempdir().expect("temp dir");
    let obs_path = temp_dir.path().join("obs.ptb64");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--shots=64",
            "--out_format=ptb64",
            "--obs_out_format=ptb64",
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
        ],
        "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(stdout, ptb64_words(&[u64::MAX]));
    assert_eq!(
        std::fs::read(obs_path).expect("read obs"),
        ptb64_words(&[u64::MAX])
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn detect_rejects_ptb64_shots_that_are_not_multiple_of_64() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "detect", "--shots=63", "--out_format=ptb64"],
        "M 0\nDETECTOR rec[-1]\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("shots must be a multiple of 64 to use ptb64 format")
    );
}

#[test]
fn detect_supports_pauli_target_observable_flips() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--append_observables",
            "--shots",
            "128",
            "--seed",
            "5",
        ],
        b"RZ 0\nOBSERVABLE_INCLUDE(0) X0\nOBSERVABLE_INCLUDE(1) Y0\nOBSERVABLE_INCLUDE(2) Z0\n"
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    let text = String::from_utf8(stdout).unwrap();
    let mut x_or_y_hits = 0;
    for line in text.lines() {
        assert_eq!(line.len(), 3);
        let bytes = line.as_bytes();
        assert_eq!(bytes.first(), bytes.get(1));
        assert_eq!(bytes.get(2), Some(&b'0'));
        x_or_y_hits += usize::from(bytes.first() == Some(&b'1'));
    }
    assert!((32..96).contains(&x_or_y_hits));
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn detect_supports_product_measurements_with_pauli_observable_flips() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--append_observables",
            "--shots",
            "128",
            "--seed",
            "5",
        ],
        b"RX 0 1\nMXX 0 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    let text = String::from_utf8(stdout).unwrap();
    let mut observable_hits = 0;
    for line in text.lines() {
        let bytes = line.as_bytes();
        assert_eq!(bytes.first(), Some(&b'0'));
        observable_hits += usize::from(bytes.get(1) == Some(&b'1'));
    }
    assert!((32..96).contains(&observable_hits));
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn detect_can_write_observables_to_a_separate_file() {
    let temp_dir = tempdir().expect("temp dir");
    let obs_path = temp_dir.path().join("obs.b8");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--out_format=01",
            "--obs_out_format=b8",
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
        ],
        "X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "1\n");
    assert_eq!(std::fs::read(obs_path).expect("read obs"), [0b0000_0001]);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_basic_matches_m9_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=dets",
            concat!(
                "--circuit=",
                env!("CARGO_MANIFEST_DIR"),
                "/../../oracle/fixtures/inputs/m2d_basic.stim"
            ),
        ],
        include_bytes!("../../../../oracle/fixtures/inputs/m2d_basic_measurements.01").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m9_m2d_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn legacy_m2d_flag_matches_m9_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "--m2d",
            "--in_format=01",
            "--out_format=dets",
            concat!(
                "--circuit=",
                env!("CARGO_MANIFEST_DIR"),
                "/../../oracle/fixtures/inputs/m2d_basic.stim"
            ),
        ],
        include_bytes!("../../../../oracle/fixtures/inputs/m2d_basic_measurements.01").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m9_m2d_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_supports_reference_skip_observables_and_b8_input() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(
        &circuit_path,
        "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    )
    .expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=b8",
            "--out_format=01",
            "--append_observables",
            "--skip_reference_sample",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        [0b0000_0000, 0b0000_0010, 0b0000_0001, 0b0000_0011].as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "00000\n01001\n10000\n11001\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_ignores_pauli_target_observables_like_stim_conversion() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(
        &circuit_path,
        "M 0\nOBSERVABLE_INCLUDE(0) X0\nOBSERVABLE_INCLUDE(1) rec[-1]\n",
    )
    .expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--append_observables",
            "--skip_reference_sample",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "01\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_reads_ptb64_records_and_writes_supported_formats() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let obs_path = temp_dir.path().join("obs.b8");
    std::fs::write(
        &circuit_path,
        "M 0 1\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");

    let alternating = 0xAAAAAAAA_AAAAAAAAu64;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=ptb64",
            "--out_format=01",
            "--obs_out_format=b8",
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        ptb64_words(&[u64::MAX, alternating]).as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "1\n".repeat(64));
    let expected_obs: Vec<_> = (0..64)
        .map(|shot_index| u8::from(alternating & (1u64 << shot_index) != 0))
        .collect();
    assert_eq!(std::fs::read(obs_path).expect("read obs"), expected_obs);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn m2d_rejects_ptb64_detector_output_like_stim() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=ptb64",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("format ptb64 is not supported for detection data")
    );
}

#[test]
fn m2d_rejects_ptb64_observable_output_like_stim() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    let obs_path = temp_dir.path().join("obs.ptb64");
    std::fs::write(
        &circuit_path,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--out_format=01",
            "--obs_out_format=ptb64",
            "--obs_out",
            obs_path.to_str().expect("utf-8 path"),
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(!obs_path.exists());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("format ptb64 is not supported for detection data")
    );
}

#[test]
fn m2d_rejects_zero_width_ptb64_input() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "TICK\n").expect("write circuit");
    let nonempty_ptb64 = ptb64_words(&[0]);

    for input in [&[][..], nonempty_ptb64.as_slice()] {
        let mut stdout = Vec::new();
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
            input,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 1);
        assert_eq!(stdout, b"");
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("ptb64 input cannot infer a shot count for zero-width records")
        );
    }
}

#[test]
fn m2d_rejects_excessive_ptb64_decoded_shots_before_expansion() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut stdout = Vec::new();
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
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("m2d ptb64 input decodes to 1000064 records but the limit is 1000000")
    );
}

#[test]
fn m2d_dets_input_accepts_measurement_tokens_only() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut ok_stdout = Vec::new();
    let mut ok_stderr = Vec::new();
    let ok_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=dets",
            "--out_format=dets",
            "--skip_reference_sample",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"shot M0\nshot\n".as_slice(),
        &mut ok_stdout,
        &mut ok_stderr,
    );
    assert_eq!(ok_status, 0);
    assert_eq!(String::from_utf8(ok_stdout).unwrap(), "shot D0\nshot\n");
    assert_eq!(String::from_utf8(ok_stderr).unwrap(), "");

    let mut bad_stdout = Vec::new();
    let mut bad_stderr = Vec::new();
    let bad_status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=dets",
            "--out_format=dets",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        b"shot D0\n".as_slice(),
        &mut bad_stdout,
        &mut bad_stderr,
    );
    assert_eq!(bad_status, 1);
    assert_eq!(bad_stdout, b"");
    assert!(
        String::from_utf8(bad_stderr)
            .unwrap()
            .contains("measurement dets input cannot contain D tokens")
    );
}

#[test]
fn m2d_rejects_measurement_width_mismatches() {
    let temp_dir = tempdir().expect("temp dir");
    let circuit_path = temp_dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0 1\nDETECTOR rec[-1]\n").expect("write circuit");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "m2d",
            "--in_format=01",
            "--circuit",
            circuit_path.to_str().expect("utf-8 path"),
        ],
        "0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("expected 2 bits")
    );
}
