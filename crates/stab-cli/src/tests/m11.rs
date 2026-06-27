use super::run_from;
use std::ffi::OsString;
use tempfile::tempdir;

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
