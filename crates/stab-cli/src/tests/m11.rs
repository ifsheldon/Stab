use super::run_from;
use stab_core::result_formats::write_ptb64_records;
use std::ffi::OsString;
use tempfile::tempdir;

fn ptb64_words(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn sample_dem_deterministic_matches_m11_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample_dem", "--shots", "3"],
        include_bytes!("../../../../oracle/fixtures/inputs/sample_dem_deterministic.dem")
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../../oracle/fixtures/expected/m11_sample_dem_deterministic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_noisy_seeded_output_matches_m11_statistical_plan() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample_dem", "--shots", "1000", "--seed", "5"],
        include_bytes!("../../../../oracle/fixtures/inputs/sample_dem_noisy.dem").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    let stdout = String::from_utf8(stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1000);
    let hits = lines.iter().filter(|line| **line == "1").count();
    assert!(
        (180..=320).contains(&hits),
        "expected noisy DEM hits near p=0.25, got {hits}"
    );
    assert!(lines.iter().all(|line| *line == "0" || *line == "1"));
}

#[test]
fn sample_dem_writes_observables_to_obs_out_like_upstream() {
    let dir = tempdir().expect("tempdir");
    let obs_path = dir.path().join("obs.01");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--out_format"),
        OsString::from("01"),
        OsString::from("--obs_out_format"),
        OsString::from("01"),
        OsString::from("--shots"),
        OsString::from("5"),
        OsString::from("--seed"),
        OsString::from("0"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(0) D0\nerror(1) D1 L2\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "01\n01\n01\n01\n01\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("obs output"),
        "001\n001\n001\n001\n001\n"
    );
}

#[test]
fn sample_dem_dets_output_keeps_observables_separate_like_upstream() {
    let dir = tempdir().expect("tempdir");
    let obs_path = dir.path().join("obs.dets");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--out_format"),
        OsString::from("dets"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format"),
        OsString::from("dets"),
        OsString::from("--shots"),
        OsString::from("2"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(1) D0 L0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "shot D0\nshot D0\n");
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("obs output"),
        "shot L0\nshot L0\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_rejects_conflicting_observable_routes() {
    for args in [
        vec![
            OsString::from("stab"),
            OsString::from("sample_dem"),
            OsString::from("--append_observables"),
            OsString::from("--prepend_observables"),
        ],
        vec![
            OsString::from("stab"),
            OsString::from("sample_dem"),
            OsString::from("--append_observables"),
            OsString::from("--obs_out"),
            OsString::from("obs.01"),
        ],
        vec![
            OsString::from("stab"),
            OsString::from("sample_dem"),
            OsString::from("--prepend_observables"),
            OsString::from("--obs_out"),
            OsString::from("obs.01"),
        ],
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            args,
            b"error(1) D0 L0\n".as_slice(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 1);
        assert_eq!(String::from_utf8(stdout).unwrap(), "");
        assert!(
            String::from_utf8(stderr).unwrap().contains(
                "cannot combine --prepend_observables, --append_observables, or --obs_out"
            )
        );
    }
}

#[test]
fn sample_dem_writes_error_records_to_err_out_like_upstream() {
    let dir = tempdir().expect("tempdir");
    let err_path = dir.path().join("errors.01");
    let obs_path = dir.path().join("obs.01");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--err_out"),
        err_path.clone().into_os_string(),
        OsString::from("--err_out_format"),
        OsString::from("01"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format"),
        OsString::from("01"),
        OsString::from("--shots"),
        OsString::from("2"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "11\n11\n");
    assert_eq!(
        std::fs::read_to_string(err_path).expect("error output"),
        "101\n101\n"
    );
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("obs output"),
        "1\n1\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_writes_ptb64_detector_observable_and_error_streams() {
    let dir = tempdir().expect("tempdir");
    let obs_path = dir.path().join("obs.ptb64");
    let err_path = dir.path().join("errors.ptb64");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--out_format"),
        OsString::from("ptb64"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format"),
        OsString::from("ptb64"),
        OsString::from("--err_out"),
        err_path.clone().into_os_string(),
        OsString::from("--err_out_format"),
        OsString::from("ptb64"),
        OsString::from("--shots"),
        OsString::from("64"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(stdout, ptb64_words(&[u64::MAX, u64::MAX]));
    assert_eq!(
        std::fs::read(obs_path).expect("obs output"),
        ptb64_words(&[u64::MAX])
    );
    assert_eq!(
        std::fs::read(err_path).expect("error output"),
        ptb64_words(&[u64::MAX, 0, u64::MAX])
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_replays_error_records_into_detector_and_observable_streams() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.dets");
    let err_copy_path = dir.path().join("errors.hits");
    let obs_path = dir.path().join("obs.01");
    std::fs::write(&replay_path, "shot M0\nshot M1\nshot M0 M1\nshot M0\n")
        .expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.clone().into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("dets"),
        OsString::from("--err_out"),
        err_copy_path.clone().into_os_string(),
        OsString::from("--err_out_format"),
        OsString::from("hits"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format"),
        OsString::from("01"),
        OsString::from("--shots"),
        OsString::from("3"),
        OsString::from("--seed"),
        OsString::from("0"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(0.25) D0 L0\nerror(0.25) D1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "10\n01\n11\n");
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("obs output"),
        "1\n0\n1\n"
    );
    assert_eq!(
        std::fs::read_to_string(err_copy_path).expect("error copy output"),
        "0\n1\n0,1\n"
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_replays_ptb64_error_records() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.ptb64");
    let obs_path = dir.path().join("obs.01");
    let mut replay_input = write_ptb64_records(&vec![vec![true, false]; 64]);
    replay_input.push(0xA5);
    std::fs::write(&replay_path, replay_input).expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("ptb64"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--shots"),
        OsString::from("64"),
        OsString::from("--seed"),
        OsString::from("0"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(0.25) D0 L0\nerror(0.25) D1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "10\n".repeat(64));
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("obs output"),
        "1\n".repeat(64)
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_round_trips_r8_detector_observable_error_and_replay_streams() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.r8");
    let obs_path = dir.path().join("obs.r8");
    let err_path = dir.path().join("errors-copy.r8");
    std::fs::write(&replay_path, [0, 1, 1, 0]).expect("write r8 replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--out_format"),
        OsString::from("r8"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("r8"),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format"),
        OsString::from("r8"),
        OsString::from("--err_out"),
        err_path.clone().into_os_string(),
        OsString::from("--err_out_format"),
        OsString::from("r8"),
        OsString::from("--shots"),
        OsString::from("2"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(0.25) D0 L0\nerror(0.25) D1 L1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(stdout, [0, 1, 1, 0]);
    assert_eq!(
        std::fs::read(obs_path).expect("r8 observable output"),
        [0, 1, 1, 0]
    );
    assert_eq!(
        std::fs::read(err_path).expect("r8 error copy output"),
        [0, 1, 1, 0]
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_replays_stim_compatible_crlf_text_records() {
    for (format, replay_input, dem_input, expected_stdout) in [
        ("01", b"1\r\n".as_slice(), "error(0.25) D0\n", "1\n"),
        (
            "hits",
            b"0\r\n".as_slice(),
            "error(0.25) D0\nerror(0.25) D1\n",
            "10\n",
        ),
        (
            "dets",
            b"shot M1\r\n\r\n\n   shot M0\r\n\n".as_slice(),
            "error(0.25) D0\nerror(0.25) D1\n",
            "01\n10\n",
        ),
    ] {
        let dir = tempdir().expect("tempdir");
        let replay_path = dir.path().join(format!("errors.{format}"));
        std::fs::write(&replay_path, replay_input).expect("write CRLF replay input");
        let args = vec![
            OsString::from("stab"),
            OsString::from("sample_dem"),
            OsString::from("--replay_err_in"),
            replay_path.into_os_string(),
            OsString::from("--replay_err_in_format"),
            OsString::from(format),
            OsString::from("--shots"),
            OsString::from(expected_stdout.lines().count().to_string()),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(args, dem_input.as_bytes(), &mut stdout, &mut stderr);

        assert_eq!(status, 0);
        assert_eq!(String::from_utf8(stdout).unwrap(), expected_stdout);
        assert_eq!(String::from_utf8(stderr).unwrap(), "");
    }
}

#[test]
fn sample_dem_rejects_excessive_blank_dets_replay_prefix() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.dets");
    let mut replay_input = vec![b'\n'; 1_048_577];
    replay_input.extend_from_slice(b"shot M0\n");
    std::fs::write(&replay_path, replay_input).expect("write over-budget blank replay prefix");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("dets"),
        OsString::from("--shots"),
        OsString::from("1"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"error(1) D0\n".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("sample_dem replay text record is too large; limit is 1048576 bytes")
    );
}

#[test]
fn sample_dem_rejects_truncated_ptb64_replay_input() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.ptb64");
    std::fs::write(&replay_path, ptb64_words(&[u64::MAX])).expect("write truncated replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("ptb64"),
        OsString::from("--shots"),
        OsString::from("64"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(0.25) D0 L0\nerror(0.25) D1\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("ptb64 input expected at least 16 bytes")
    );
}

#[test]
fn sample_dem_rejects_ptb64_shots_that_are_not_multiple_of_64() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample_dem", "--out_format=ptb64", "--shots=63"],
        b"error(1) D0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("shots must be a multiple of 64 to use ptb64 format")
    );
}

#[test]
fn sample_dem_rejects_replay_record_count_mismatch() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.01");
    std::fs::write(&replay_path, "1\n").expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--shots"),
        OsString::from("2"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"error(1) D0\n".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("replay error input has 1 records but --shots requested 2")
    );
}

#[test]
fn sample_dem_replay_ignores_malformed_extra_text_records_after_requested_shots() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.01");
    std::fs::write(&replay_path, "1\nnot-a-record\n").expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--shots"),
        OsString::from("1"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"error(1) D0\n".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "1\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_replay_ignores_partial_extra_b8_records_after_requested_shots() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("errors.b8");
    std::fs::write(&replay_path, [0b0000_0001, 0, 0b0000_0001]).expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format"),
        OsString::from("b8"),
        OsString::from("--shots"),
        OsString::from("1"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"error(1) D0\nerror(1) D1\nerror(1) D2\nerror(1) D3\nerror(1) D4\nerror(1) D5\nerror(1) D6\nerror(1) D7\nerror(1) D8\n"
            .as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "100000000\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_dem_rejects_excessive_buffered_output_before_sampling() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample_dem", "--shots", "64000001"],
        b"".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("DEM sampler would require 64000001 buffered units")
    );
}

#[test]
fn sample_dem_rejects_oversized_input_file_before_reading() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("oversized.dem");
    let file = std::fs::File::create(&input_path).expect("create oversized DEM input");
    file.set_len(64 * 1024 * 1024 + 1)
        .expect("mark oversized DEM input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--in"),
        input_path.into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("sample_dem input is too large; limit is 67108864 bytes")
    );
}

#[test]
fn sample_dem_zero_shots_validates_declared_input_paths_like_stim() {
    let dir = tempdir().expect("tempdir");
    for (flag, missing_path) in [
        ("--in", dir.path().join("missing.dem")),
        ("--replay_err_in", dir.path().join("missing.01")),
    ] {
        let args = vec![
            OsString::from("stab"),
            OsString::from("sample_dem"),
            OsString::from("--shots"),
            OsString::from("0"),
            OsString::from(flag),
            missing_path.clone().into_os_string(),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(args, b"".as_slice(), &mut stdout, &mut stderr);

        assert_eq!(status, 1);
        assert_eq!(String::from_utf8(stdout).unwrap(), "");
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("failed to read"), "{stderr}");
        assert!(
            stderr.contains(missing_path.file_name().unwrap().to_str().unwrap()),
            "{stderr}"
        );
    }
}

#[test]
fn sample_dem_rejects_excessive_replay_buffers_before_reading_replay_path() {
    let dir = tempdir().expect("tempdir");
    let missing_replay_path = dir.path().join("missing.01");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        missing_replay_path.into_os_string(),
        OsString::from("--shots"),
        OsString::from("64000001"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("DEM sampler would require 64000001 buffered units"));
    assert!(!stderr.contains("missing.01"));
}
