use super::run_from;
use tempfile::tempdir;

#[test]
fn gen_repetition_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
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
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m7_gen_repetition_code.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn gen_surface_rotated_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "2",
            "--rounds",
            "1",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m7_gen_surface_rotated_z.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn gen_surface_unrotated_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "unrotated_memory_z",
            "--distance",
            "2",
            "--rounds",
            "1",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m7_gen_surface_unrotated_z.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn gen_color_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "color_code",
            "--task",
            "memory_xyz",
            "--distance",
            "3",
            "--rounds",
            "2",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m7_gen_color_code.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn convert_01_to_dets_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=dets",
            "--num_detectors=2",
        ],
        include_bytes!("../../../oracle/fixtures/inputs/convert_measurements.01").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m7_convert_01_to_dets.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn convert_stim_from_stdin_to_canonical_output() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "convert", "--in_format=stim", "--out_format=stim"],
        "# comment\nH 0\nH 1\nM 0\nM 1\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "H 0 1\nM 0 1\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn convert_stim_reads_and_writes_paths() {
    let temp_dir = tempdir().expect("temp dir");
    let input_path = temp_dir.path().join("input.stim");
    let output_path = temp_dir.path().join("output.stim");
    std::fs::write(&input_path, "CX 0 1\nCX 2 3\n").expect("write input");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "convert",
            "--in_format=stim",
            "--out_format=stim",
            "--in",
            input_path.to_str().expect("utf-8 path"),
            "--out",
            output_path.to_str().expect("utf-8 path"),
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    assert_eq!(
        std::fs::read_to_string(output_path).expect("read output"),
        "CX 0 1 2 3\n"
    );
}

#[test]
fn cli_rejects_unknown_arguments_like_arg_parse_test() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
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
            "--unknown",
            "5",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 2);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(String::from_utf8(stderr).unwrap().contains("--unknown"));
}

#[test]
fn cli_requires_mandatory_arguments_like_arg_parse_test() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 2);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(String::from_utf8(stderr).unwrap().contains("--code"));
}

#[test]
fn cli_rejects_invalid_integer_arguments_like_arg_parse_test() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "not-an-int",
            "--rounds",
            "2",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 2);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(String::from_utf8(stderr).unwrap().contains("invalid value"));
}

#[test]
fn cli_rejects_invalid_enum_arguments_like_arg_parse_test() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "unknown_code",
            "--task",
            "memory",
            "--distance",
            "3",
            "--rounds",
            "2",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 2);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(String::from_utf8(stderr).unwrap().contains("unknown_code"));
}

#[test]
fn cli_rejects_out_of_range_probability_arguments_like_arg_parse_test() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
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
            "--before_measure_flip_probability",
            "1.1",
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(String::from_utf8(stderr).unwrap().contains("probability"));
}

#[test]
fn smoke_sampler_outputs_zero_measurements_for_each_shot() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots", "2"],
        "M 0 1\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "00\n00\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn smoke_sampler_ignores_comments_and_ticks() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=1"],
        "# comment\nTICK\nMZ 2 # after\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "0\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn smoke_sampler_rejects_non_smoke_instructions() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample"],
        "H 0\nM 0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("only supports M and MZ")
    );
}

#[test]
fn smoke_sampler_is_hidden_from_public_help() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(["stab", "--help"], "".as_bytes(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert!(!String::from_utf8(stdout).unwrap().contains("sample"));
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}
