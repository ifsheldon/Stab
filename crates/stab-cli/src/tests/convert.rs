use tempfile::tempdir;

use crate::run_from;

fn run_cli(args: &[&str], input: &[u8]) -> (i32, Vec<u8>, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);
    (
        status,
        stdout,
        String::from_utf8(stderr).expect("stderr is utf-8"),
    )
}

fn run_ok(args: &[&str], input: &[u8]) -> Vec<u8> {
    let (status, stdout, stderr) = run_cli(args, input);
    assert_eq!(status, 0, "stderr: {stderr}");
    assert_eq!(stderr, "");
    stdout
}

fn write_fixture(name: &str, text: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join(name);
    std::fs::write(&path, text).expect("write fixture");
    let path = path.to_str().expect("utf-8 path").to_string();
    (dir, path)
}

fn convert_between_formats(
    input: &[u8],
    in_format: &str,
    out_format: &str,
    extra: &[&str],
) -> Vec<u8> {
    let mut args = vec![
        "stab",
        "convert",
        "--in_format",
        in_format,
        "--out_format",
        out_format,
    ];
    args.extend_from_slice(extra);
    run_ok(&args, input)
}

fn alternating_records_64(width: usize) -> String {
    let mut records = String::new();
    for shot in 0..64 {
        for bit in 0..width {
            records.push(if (shot + bit) % 2 == 0 { '1' } else { '0' });
        }
        records.push('\n');
    }
    records
}

#[test]
fn convert_measurements_with_circuit_types_m_to_dets() {
    let (_dir, circuit_path) = write_fixture(
        "layout.stim",
        "M 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    );
    let zero_one = b"00\n01\n10\n11\n";
    let b8 = convert_between_formats(zero_one, "01", "b8", &["--bits_per_shot", "2"]);
    let hits = b"\n1\n0\n0,1\n".to_vec();
    let r8 = convert_between_formats(zero_one, "01", "r8", &["--bits_per_shot", "2"]);
    let expected = "shot\nshot M1\nshot M0\nshot M0 M1\n";

    for (format, input) in [
        ("01", zero_one.as_slice()),
        ("b8", b8.as_slice()),
        ("hits", hits.as_slice()),
        ("r8", r8.as_slice()),
    ] {
        let output = run_ok(
            &[
                "stab",
                "convert",
                "--in_format",
                format,
                "--out_format",
                "dets",
                "--circuit",
                &circuit_path,
                "--types",
                "M",
            ],
            input,
        );
        assert_eq!(String::from_utf8(output).unwrap(), expected, "{format}");
    }
}

#[test]
fn convert_detection_observable_records_with_circuit_and_obs_out() {
    let (_dir, circuit_path) = write_fixture(
        "layout.stim",
        "M 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    );
    let input = b"10101\n01010\n";
    let output = run_ok(
        &[
            "stab",
            "convert",
            "--out_format",
            "dets",
            "--circuit",
            &circuit_path,
            "--types",
            "DL",
        ],
        input,
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "shot D0 L0 L2\nshot D1 L1\n"
    );

    let dir = tempdir().expect("temp dir");
    let obs_path = dir.path().join("obs.dets");
    let obs_path = obs_path.to_str().expect("utf-8 path").to_string();
    let primary = run_ok(
        &[
            "stab",
            "convert",
            "--out_format",
            "dets",
            "--circuit",
            &circuit_path,
            "--types",
            "DL",
            "--obs_out",
            &obs_path,
            "--obs_out_format",
            "dets",
        ],
        input,
    );
    assert_eq!(String::from_utf8(primary).unwrap(), "shot D0\nshot D1\n");
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("read obs output"),
        "shot L0 L2\nshot L1\n"
    );
}

#[test]
fn convert_round_trips_detection_observable_records_from_counts_and_dem() {
    let zero_one = b"10101\n01010\n00000\n11111\n";
    let layout = ["--num_detectors", "2", "--num_observables", "3"];
    for format in ["01", "b8", "r8", "hits", "dets"] {
        let encoded = convert_between_formats(zero_one, "01", format, &layout);
        let decoded = convert_between_formats(&encoded, format, "01", &layout);
        assert_eq!(decoded, zero_one, "{format}");
    }

    let (_dir, dem_path) = write_fixture(
        "layout.dem",
        "detector D0\ndetector D1\nlogical_observable L2\n",
    );
    let decoded = run_ok(
        &[
            "stab",
            "convert",
            "--in_format",
            "dets",
            "--out_format",
            "01",
            "--dem",
            &dem_path,
        ],
        b"shot D1 L2\n",
    );
    assert_eq!(String::from_utf8(decoded).unwrap(), "01001\n");
}

#[test]
fn convert_ptb64_round_trips_and_interops_with_other_formats() {
    let records = alternating_records_64(5);
    let layout = ["--num_detectors", "2", "--num_observables", "3"];
    let ptb64 = convert_between_formats(records.as_bytes(), "01", "ptb64", &layout);
    let decoded = convert_between_formats(&ptb64, "ptb64", "01", &layout);
    assert_eq!(String::from_utf8(decoded).unwrap(), records);

    for format in ["b8", "hits", "dets"] {
        let direct = convert_between_formats(records.as_bytes(), "01", format, &layout);
        let from_ptb64 = convert_between_formats(&ptb64, "ptb64", format, &layout);
        assert_eq!(from_ptb64, direct, "{format}");
    }
}

#[test]
fn convert_ptb64_observable_side_output_round_trips() {
    let records = alternating_records_64(2);
    let dir = tempdir().expect("temp dir");
    let obs_path = dir.path().join("obs.ptb64");
    let obs_path = obs_path.to_str().expect("utf-8 path").to_string();
    let primary = run_ok(
        &[
            "stab",
            "convert",
            "--out_format",
            "01",
            "--num_detectors",
            "1",
            "--num_observables",
            "1",
            "--obs_out",
            &obs_path,
            "--obs_out_format",
            "ptb64",
        ],
        records.as_bytes(),
    );

    let expected_primary = records
        .lines()
        .map(|line| {
            let first = line.chars().next().expect("nonempty line");
            format!("{first}\n")
        })
        .collect::<String>();
    assert_eq!(String::from_utf8(primary).unwrap(), expected_primary);

    let side = std::fs::read(&obs_path).expect("read side output");
    let decoded = run_ok(
        &[
            "stab",
            "convert",
            "--in_format",
            "ptb64",
            "--out_format",
            "01",
            "--num_observables",
            "1",
        ],
        &side,
    );
    let expected_side = records
        .lines()
        .map(|line| {
            let second = line.chars().nth(1).expect("second bit");
            format!("{second}\n")
        })
        .collect::<String>();
    assert_eq!(String::from_utf8(decoded).unwrap(), expected_side);
}

#[test]
fn convert_measurement_records_and_raw_bits_round_trip() {
    let measurements = b"101\n010\n111\n000\n";
    for format in ["01", "b8", "r8", "hits", "dets"] {
        let encoded =
            convert_between_formats(measurements, "01", format, &["--num_measurements", "3"]);
        let decoded = convert_between_formats(&encoded, format, "01", &["--num_measurements", "3"]);
        assert_eq!(decoded, measurements, "{format}");
    }

    let b8 = convert_between_formats(b"10\n01\n", "01", "b8", &["--bits_per_shot", "2"]);
    let decoded = convert_between_formats(&b8, "b8", "01", &["--bits_per_shot", "2"]);
    assert_eq!(decoded, b"10\n01\n");
}

#[test]
fn convert_b8_to_b8_preserves_byte_aligned_records_exactly() {
    let input = vec![
        0x00, 0xff, 0x5a, 0xa5, 0x13, 0x37, 0xc0, 0xde, 0x10, 0x32, 0x54, 0x76,
    ];
    let output = convert_between_formats(&input, "b8", "b8", &["--bits_per_shot", "24"]);
    assert_eq!(output, input);
}

#[test]
fn convert_b8_to_b8_keeps_non_byte_aligned_padding_canonical() {
    let output = convert_between_formats(&[0xff], "b8", "b8", &["--bits_per_shot", "3"]);
    assert_eq!(output, vec![0x07]);
}

#[test]
fn convert_rejects_incomplete_ptb64_output_group() {
    let (status, stdout, stderr) = run_cli(
        &[
            "stab",
            "convert",
            "--out_format",
            "ptb64",
            "--bits_per_shot",
            "2",
        ],
        b"10\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, Vec::<u8>::new());
    assert!(stderr.contains("groups of 64"));
}

#[test]
fn convert_rejects_layout_and_format_failures() {
    let (_dir, circuit_path) = write_fixture("layout.stim", "M 0\n");
    let failure_cases: Vec<(Vec<&str>, &[u8], &str)> = vec![
        (
            vec!["stab", "convert", "--circuit", &circuit_path],
            b"0\n",
            "--circuit requires --types",
        ),
        (
            vec![
                "stab",
                "convert",
                "--circuit",
                &circuit_path,
                "--types",
                "X",
            ],
            b"0\n",
            "unknown result type",
        ),
        (
            vec![
                "stab",
                "convert",
                "--circuit",
                &circuit_path,
                "--types",
                "MM",
            ],
            b"0\n",
            "duplicate result type",
        ),
        (
            vec!["stab", "convert", "--out_format", "b8"],
            b"10\n",
            "not enough information",
        ),
        (
            vec![
                "stab",
                "convert",
                "--out_format",
                "dets",
                "--bits_per_shot",
                "2",
            ],
            b"10\n",
            "to write to dets",
        ),
        (
            vec!["stab", "convert", "--num_measurements", "3"],
            b"10\n",
            "expected 3 bits",
        ),
        (
            vec![
                "stab",
                "convert",
                "--in_format",
                "b8",
                "--bits_per_shot",
                "9",
            ],
            &[0],
            "not a multiple",
        ),
        (
            vec![
                "stab",
                "convert",
                "--in_format",
                "dets",
                "--num_detectors",
                "2",
            ],
            b"shot L0\n",
            "not included",
        ),
    ];

    for (args, input, expected) in failure_cases {
        let (status, stdout, stderr) = run_cli(&args, input);
        assert_eq!(status, 1, "{args:?}");
        assert_eq!(stdout, Vec::<u8>::new(), "{args:?}");
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
    }
}

#[test]
fn convert_reports_missing_input_output_and_layout_paths() {
    let missing = "/tmp/stab-this-path-should-not-exist/fixture";
    for (args, input, expect_empty_stdout) in [
        (
            vec!["stab", "convert", "--in", missing, "--bits_per_shot", "1"],
            b"1\n".as_slice(),
            true,
        ),
        (
            vec!["stab", "convert", "--circuit", missing, "--types", "M"],
            b"1\n".as_slice(),
            true,
        ),
        (
            vec!["stab", "convert", "--dem", missing],
            b"1\n".as_slice(),
            true,
        ),
        (
            vec!["stab", "convert", "--out", missing, "--bits_per_shot", "1"],
            b"1\n".as_slice(),
            true,
        ),
        (
            vec![
                "stab",
                "convert",
                "--obs_out",
                missing,
                "--num_measurements",
                "1",
                "--num_observables",
                "1",
            ],
            b"11\n".as_slice(),
            false,
        ),
    ] {
        let (status, stdout, stderr) = run_cli(&args, input);
        assert_eq!(status, 1, "{args:?}");
        if expect_empty_stdout {
            assert_eq!(stdout, Vec::<u8>::new(), "{args:?}");
        }
        assert!(!stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn legacy_convert_alias_dispatches_and_conflicts_with_other_legacy_modes() {
    let output = run_ok(
        &[
            "stab",
            "--convert",
            "--in_format",
            "01",
            "--out_format",
            "b8",
            "--bits_per_shot",
            "2",
        ],
        b"10\n01\n",
    );
    assert_eq!(output, vec![1, 2]);

    let (status, stdout, stderr) = run_cli(&["stab", "--convert", "--sample"], b"10\n");
    assert_eq!(status, 1);
    assert_eq!(stdout, Vec::<u8>::new());
    assert!(stderr.contains("--sample"));
}
