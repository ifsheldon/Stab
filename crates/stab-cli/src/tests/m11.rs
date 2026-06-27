use super::run_from;

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
