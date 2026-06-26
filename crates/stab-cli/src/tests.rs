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
fn legacy_gen_repetition_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "--gen=repetition_code",
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
fn legacy_gen_space_separated_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "--gen",
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
fn gen_surface_rotated_x_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
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
        include_str!("../../../oracle/fixtures/expected/m7_gen_surface_rotated_x.stdout")
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
fn gen_surface_unrotated_x_code_matches_m7_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "unrotated_memory_x",
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
        include_str!("../../../oracle/fixtures/expected/m7_gen_surface_unrotated_x.stdout")
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
fn sample_basic_matches_m8_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots", "2"],
        include_bytes!("../../../oracle/fixtures/inputs/sample_basic.stim").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m8_sample_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn legacy_sample_flag_matches_m8_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "--sample=2"],
        include_bytes!("../../../oracle/fixtures/inputs/sample_basic.stim").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m8_sample_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn legacy_sample_space_separated_flag_matches_m8_oracle_golden() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "--sample", "2"],
        include_bytes!("../../../oracle/fixtures/inputs/sample_basic.stim").as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m8_sample_basic.stdout")
    );
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_supports_deterministic_pauli_frame_measurements() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=1"],
        "X 0\nM 0\nM !1\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "11\n");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}

#[test]
fn sample_writes_sparse_text_formats_like_stim() {
    let input = include_bytes!("../../../oracle/fixtures/inputs/sample_sparse_text_formats.stim")
        .as_slice();

    let mut dets_stdout = Vec::new();
    let mut dets_stderr = Vec::new();
    let dets_status = run_from(
        ["stab", "sample", "--shots=1", "--out_format=dets"],
        input,
        &mut dets_stdout,
        &mut dets_stderr,
    );

    assert_eq!(dets_status, 0);
    assert_eq!(
        String::from_utf8(dets_stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m8_sample_dets.stdout")
    );
    assert_eq!(String::from_utf8(dets_stderr).unwrap(), "");

    let mut hits_stdout = Vec::new();
    let mut hits_stderr = Vec::new();
    let hits_status = run_from(
        ["stab", "sample", "--shots=1", "--out_format=hits"],
        input,
        &mut hits_stdout,
        &mut hits_stderr,
    );

    assert_eq!(hits_status, 0);
    assert_eq!(
        String::from_utf8(hits_stdout).unwrap(),
        include_str!("../../../oracle/fixtures/expected/m8_sample_hits.stdout")
    );
    assert_eq!(String::from_utf8(hits_stderr).unwrap(), "");
}

#[test]
fn sample_rejects_binary_formats_until_m8_result_writers_land() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--out_format=b8"],
        "M 0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("unsupported sample output format")
    );
}

#[test]
fn sample_seed_makes_noisy_x_error_reproducible() {
    let input = include_bytes!("../../../oracle/fixtures/inputs/sample_noisy.stim").as_slice();

    let mut first_stdout = Vec::new();
    let mut first_stderr = Vec::new();
    let first_status = run_from(
        ["stab", "sample", "--shots=1000", "--seed=5"],
        input,
        &mut first_stdout,
        &mut first_stderr,
    );

    let mut second_stdout = Vec::new();
    let mut second_stderr = Vec::new();
    let second_status = run_from(
        ["stab", "sample", "--shots=1000", "--seed=5"],
        input,
        &mut second_stdout,
        &mut second_stderr,
    );

    assert_eq!(first_status, 0);
    assert_eq!(second_status, 0);
    assert_eq!(first_stdout, second_stdout);
    assert_eq!(String::from_utf8(first_stderr).unwrap(), "");
    assert_eq!(String::from_utf8(second_stderr).unwrap(), "");

    let stdout = String::from_utf8(first_stdout).unwrap();
    let hits = stdout.lines().filter(|line| *line == "1").count();
    assert!(
        (175..=325).contains(&hits),
        "expected roughly 250 noisy hits, got {hits}"
    );
}

#[test]
fn sample_reads_and_writes_paths() {
    let temp_dir = tempdir().expect("temp dir");
    let input_path = temp_dir.path().join("input.stim");
    let output_path = temp_dir.path().join("output.01");
    std::fs::write(&input_path, "X 0\nMR 0\nMR 0\n").expect("write input");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "sample",
            "--shots=1",
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
        "10\n"
    );
}

#[test]
fn sample_rejects_unsupported_tableau_semantics() {
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
            .contains("deterministic M8 sampler subset does not support H")
    );
}

#[test]
fn sample_is_visible_in_public_help() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(["stab", "--help"], "".as_bytes(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert!(String::from_utf8(stdout).unwrap().contains("sample"));
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
}
