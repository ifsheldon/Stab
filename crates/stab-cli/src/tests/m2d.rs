//! `stab m2d` conversion, sweep, and format behavior beyond the pinned-Stim manifest rows.

use stab_engine::{detection_record_width, measurement_record_count};
use stab_model::Circuit;
use stab_records::{RecordFormat, read_records, write_records};

use super::support::{CliRun, Scratch, ptb64_words, run};

const SWEEP_CX: &str = "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n";

/// Writes `circuit` into `scratch` and runs `stab m2d <args> --circuit <circuit>`.
fn m2d(scratch: &Scratch, circuit: &str, args: &[&str], stdin: &[u8]) -> CliRun {
    let circuit_path = scratch.file("circuit.stim", circuit);
    let mut argv = vec!["stab", "m2d"];
    argv.extend_from_slice(args);
    argv.extend(["--circuit", circuit_path.as_str()]);
    run(&argv, stdin)
}

#[test]
fn m2d_preserves_a_valid_prefix_before_a_later_malformed_record() {
    let result = run(
        &[
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
        b"1\nx\n",
    );
    assert_eq!(result.status, 1);
    assert_eq!(result.stdout, b"shot D0\n");
    result.assert_stderr_contains(&["01 record contains non-bit byte"]);
}

#[test]
fn m2d_accepts_append_observables_and_skip_reference() {
    let scratch = Scratch::new();
    let circuit = "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n";
    let input = b"00\n01\n10\n11\n";
    for (extra, expected) in [
        (
            &["--append_observables"][..],
            "shot D0\nshot D0 D1 L2\nshot\nshot D1 L2\n",
        ),
        (&[], "shot D0\nshot D0 D1\nshot\nshot D1\n"),
        (
            &["--skip_reference_sample"],
            "shot\nshot D1\nshot D0\nshot D0 D1\n",
        ),
    ] {
        let mut args = vec!["--in_format=01", "--out_format=dets"];
        args.extend_from_slice(extra);
        m2d(&scratch, circuit, &args, input).assert_ok(expected);
    }
    m2d(
        &scratch,
        circuit,
        &[
            "--in_format=b8",
            "--out_format=01",
            "--append_observables",
            "--skip_reference_sample",
        ],
        &[0b0000_0000, 0b0000_0010, 0b0000_0001, 0b0000_0011],
    )
    .assert_ok(b"00000\n01001\n10000\n11001\n");
}

#[test]
fn m2d_ran_without_feedback_matches_feedback_inlined_oracle_case() {
    let round = "CX 0 2 1 2\nM 2\nCX rec[-1] 2\n";
    let circuit = format!(
        "{round}DETECTOR rec[-1]\nTICK\n{round}DETECTOR rec[-1] rec[-2]\nTICK\n\
         {round}DETECTOR rec[-1] rec[-2]\nTICK\nM 0 1\nDETECTOR rec[-1] rec[-2] rec[-3]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n"
    );
    m2d(
        &Scratch::new(),
        &circuit,
        &[
            "--in_format=01",
            "--out_format=dets",
            "--append_observables",
            "--ran_without_feedback",
        ],
        b"00000\n11100\n01100\n00100\n00010\n00001\n",
    )
    .assert_ok(b"shot\nshot D0 D1\nshot D1 D2\nshot D2 D3\nshot D3\nshot D3 L0\n");
}

#[test]
fn m2d_ran_without_feedback_accepts_xcz_ycz_feedback() {
    let scratch = Scratch::new();
    let circuit = |first: &str, second: &str| {
        format!("R 0 1 2\nM 0\n{first}\n{second}\nM 1 2\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n")
    };
    let args = [
        "--in_format=01",
        "--out_format=dets",
        "--ran_without_feedback",
    ];
    let input = b"000\n100\n111\n011\n";
    let xcz = m2d(
        &scratch,
        &circuit("XCZ 1 rec[-1]", "YCZ 2 rec[-1]"),
        &args,
        input,
    );
    let cxy = m2d(
        &scratch,
        &circuit("CX rec[-1] 1", "CY rec[-1] 2"),
        &args,
        input,
    );
    assert_eq!(xcz.status, 0);
    assert_eq!(xcz.stderr_text(), "");
    assert_eq!(xcz, cxy);
}

#[test]
fn m2d_writes_sparse_observable_side_output_widths() {
    let scratch = Scratch::new();
    for (observable_index, expected_obs) in [(0, "0\n"), (10, "00000000000\n")] {
        let obs = scratch.path("obs.01");
        m2d(
            &scratch,
            &format!(
                "M 0\nREPEAT 1024 {{\n    DETECTOR rec[-1]\n}}\n\
                 OBSERVABLE_INCLUDE({observable_index}) rec[-1]\n"
            ),
            &["--in_format=01", "--obs_out", &obs],
            b"0\n",
        )
        .assert_ok(format!("{}\n", "0".repeat(1024)));
        assert_eq!(scratch.read("obs.01"), expected_obs.as_bytes());
    }
}

#[test]
fn m2d_round_trips_sparse_text_formats() {
    let scratch = Scratch::new();
    let circuit = "M 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let obs_r8 = scratch.path("obs.r8");
    m2d(
        &scratch,
        circuit,
        &[
            "--in_format=hits",
            "--out_format=r8",
            "--obs_out",
            &obs_r8,
            "--obs_out_format=r8",
        ],
        b"\n0\n1\n0,1\n",
    )
    .assert_ok([2, 0, 1, 1, 0, 0, 0, 0]);
    assert_eq!(scratch.read("obs.r8"), [1, 1, 0, 0, 0, 0]);

    let obs_hits = scratch.path("obs.hits");
    m2d(
        &scratch,
        circuit,
        &[
            "--in_format=r8",
            "--out_format=hits",
            "--obs_out",
            &obs_hits,
            "--obs_out_format=hits",
        ],
        &[2, 0, 1, 1, 0, 0, 0, 0],
    )
    .assert_ok(b"\n0\n1\n0,1\n");
    assert_eq!(scratch.read("obs.hits"), b"\n\n0\n0\n");
}

#[test]
fn m2d_ignores_pauli_target_observable_annotations() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.01");
    m2d(
        &scratch,
        "QUBIT_COORDS(0, 0) 0\nQUBIT_COORDS(1, 0) 1\nQUBIT_COORDS(0, 1) 2\nQUBIT_COORDS(1, 1) 3\n\
         OBSERVABLE_INCLUDE(0) X0 X1\nOBSERVABLE_INCLUDE(1) Z0 Z2\n\
         MPP X0*X1*X2*X3 Z0*Z1 Z2*Z3\nDEPOLARIZE1(0.001) 0 1 2 3\nMPP X0*X1*X2*X3 Z0*Z1 Z2*Z3\n\
         DETECTOR rec[-1] rec[-4]\nDETECTOR rec[-2] rec[-5]\nDETECTOR rec[-3] rec[-6]\n\
         OBSERVABLE_INCLUDE(0) X0 X1\nOBSERVABLE_INCLUDE(1) Z0 Z2\n",
        &["--in_format=01", "--obs_out", &obs],
        b"000000\n100100\n000110\n",
    )
    .assert_ok(b"000\n000\n011\n");
    assert_eq!(scratch.read("obs.01"), b"00\n00\n00\n");
}

#[test]
fn m2d_rejects_oversized_text_records() {
    let scratch = Scratch::new();
    let mut oversized = vec![b'0'; 1_048_577];
    oversized.push(b'\n');
    m2d(
        &scratch,
        SWEEP_CX,
        &["--in_format=01", "--memory-budget=2MiB"],
        &oversized,
    )
    .assert_err(&[
        "m2d measurement input has more than 1048576 bytes, exceeding the memory budget",
        "--memory-budget",
    ]);

    let sweep = scratch.file("oversized_sweep.01", &oversized);
    m2d(
        &scratch,
        SWEEP_CX,
        &[
            "--in_format=01",
            "--sweep",
            &sweep,
            "--sweep_format=01",
            "--memory-budget=2MiB",
        ],
        b"0\n",
    )
    .assert_err(&["m2d sweep input has more than 1048576 bytes, exceeding the memory budget"]);
}

#[test]
fn m2d_converts_the_same_generated_measurements_in_text_and_bitpacked_formats() {
    let scratch = Scratch::new();
    for (code, task, rounds) in [
        ("repetition_code", "memory", "2"),
        ("surface_code", "rotated_memory_z", "3"),
        ("surface_code", "unrotated_memory_z", "3"),
        ("color_code", "memory_xyz", "2"),
    ] {
        let generated = run(
            &[
                "stab",
                "gen",
                "--code",
                code,
                "--task",
                task,
                "--distance",
                "3",
                "--rounds",
                rounds,
                "--before_measure_flip_probability",
                "0.125",
            ],
            b"",
        );
        assert_eq!(generated.status, 0, "{task}: {}", generated.stderr_text());
        let circuit = generated.stdout_text();
        let parsed = Circuit::from_stim_bytes(circuit.as_bytes()).expect("parse generated circuit");
        let measurement_width = measurement_record_count(&parsed).expect("measurement width");
        let detection_width = detection_record_width(&parsed).expect("detection width");

        let sampled = run(
            &[
                "stab",
                "sample",
                "--shots=64",
                "--seed=5",
                "--out_format=01",
            ],
            circuit.as_bytes(),
        );
        assert_eq!(sampled.status, 0, "{task}: {}", sampled.stderr_text());
        let measurements = read_records(&sampled.stdout, RecordFormat::ZeroOne, measurement_width)
            .expect("decode sampled measurements");
        let b8_measurements =
            write_records(&measurements, RecordFormat::B8).expect("encode sampled measurements");

        let convert = |format: &str, input: &[u8]| {
            let result = m2d(
                &scratch,
                circuit,
                &[
                    "--in_format",
                    format,
                    "--out_format",
                    format,
                    "--append_observables",
                ],
                input,
            );
            assert_eq!(
                result.status,
                0,
                "{task} {format}: {}",
                result.stderr_text()
            );
            assert!(result.stderr.is_empty(), "{task} {format} wrote stderr");
            result.stdout
        };
        let zero_one = convert("01", &sampled.stdout);
        let b8 = convert("b8", &b8_measurements);
        assert_eq!(
            read_records(&zero_one, RecordFormat::ZeroOne, detection_width)
                .expect("decode 01 detections"),
            read_records(&b8, RecordFormat::B8, detection_width).expect("decode b8 detections"),
            "{task} m2d format equivalence"
        );
    }
}

#[test]
fn m2d_reads_ptb64_records_and_writes_supported_formats() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.b8");
    let alternating = 0xAAAA_AAAA_AAAA_AAAA_u64;
    m2d(
        &scratch,
        "M 0 1\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[
            "--in_format=ptb64",
            "--out_format=01",
            "--obs_out_format=b8",
            "--obs_out",
            &obs,
        ],
        &ptb64_words(&[u64::MAX, alternating]),
    )
    .assert_ok("1\n".repeat(64));
    let expected_obs: Vec<_> = (0..64)
        .map(|shot_index| u8::from(alternating & (1u64 << shot_index) != 0))
        .collect();
    assert_eq!(scratch.read("obs.b8"), expected_obs);
}

#[test]
fn m2d_rejects_ptb64_detector_output_like_stim() {
    m2d(
        &Scratch::new(),
        "M 0\nDETECTOR rec[-1]\n",
        &["--in_format=01", "--out_format=ptb64"],
        b"1\n",
    )
    .assert_err(&["format ptb64 is not supported for detection data"]);
}

#[test]
fn m2d_rejects_ptb64_observable_output_like_stim() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.ptb64");
    m2d(
        &scratch,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[
            "--in_format=01",
            "--out_format=01",
            "--obs_out_format=ptb64",
            "--obs_out",
            &obs,
        ],
        b"1\n",
    )
    .assert_err(&["format ptb64 is not supported for detection data"]);
    assert!(!scratch.exists("obs.ptb64"));
}

#[test]
fn m2d_rejects_record_shapes_that_do_not_match_the_circuit() {
    let scratch = Scratch::new();
    for input in [&[][..], &ptb64_words(&[0])] {
        m2d(
            &scratch,
            "TICK\n",
            &["--in_format=ptb64", "--out_format=01"],
            input,
        )
        .assert_err(&["ptb64 input cannot infer a shot count for zero-width records"]);
    }
    m2d(
        &scratch,
        "M 0 1\nDETECTOR rec[-1]\n",
        &["--in_format=01"],
        b"0\n",
    )
    .assert_err(&["expected 2 bits"]);
}

#[test]
fn m2d_dets_input_accepts_measurement_tokens_only() {
    let scratch = Scratch::new();
    let circuit = "M 0\nDETECTOR rec[-1]\n";
    m2d(
        &scratch,
        circuit,
        &[
            "--in_format=dets",
            "--out_format=dets",
            "--skip_reference_sample",
        ],
        b"shot M0\nshot\n",
    )
    .assert_ok(b"shot D0\nshot\n");
    m2d(
        &scratch,
        circuit,
        &["--in_format=dets", "--out_format=dets"],
        b"shot D0\n",
    )
    .assert_err(&["DETS token D0 exceeds namespace width 0"]);
}

#[test]
fn m2d_ran_without_feedback_preserves_sweep_controls_and_drops_classical_noops() {
    let scratch = Scratch::new();
    let sweep = scratch.file("sweep.01", "0\n1\n");
    m2d(
        &scratch,
        "CX sweep[0] 0\nM 0\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1]\n",
        &[
            "--in_format=01",
            "--out_format=dets",
            "--ran_without_feedback",
            "--sweep",
            &sweep,
        ],
        b"00\n10\n",
    )
    .assert_ok(b"shot\nshot\n");
    m2d(
        &scratch,
        "M 0\nCZ rec[-1] sweep[1]\nMX 1\nDETECTOR rec[-1]\n",
        &[
            "--in_format=01",
            "--out_format=dets",
            "--ran_without_feedback",
        ],
        b"10\n",
    )
    .assert_ok(b"shot\n");
}

#[test]
fn m2d_uses_all_false_sweep_bits_when_sweep_input_is_omitted() {
    m2d(
        &Scratch::new(),
        SWEEP_CX,
        &["--in_format=01", "--out_format=dets"],
        b"0\n1\n",
    )
    .assert_ok(b"shot\nshot D0\n");
}

#[test]
fn m2d_streams_sweep_conditioned_records_and_observables() {
    let scratch = Scratch::new();
    let sweep = scratch.file("sweep.01", "0\n1\n0\n1\n");
    m2d(
        &scratch,
        SWEEP_CX,
        &[
            "--in_format=01",
            "--out_format=dets",
            "--sweep",
            &sweep,
            "--sweep_format=01",
        ],
        b"0\n0\n1\n1\n",
    )
    .assert_ok(b"shot\nshot D0\nshot D0\nshot\n");

    let sweep_b8 = scratch.file("sweep.b8", [0, 1, 0, 1]);
    m2d(
        &scratch,
        "CX sweep[0] 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        &[
            "--in_format=b8",
            "--out_format=b8",
            "--sweep",
            &sweep_b8,
            "--sweep_format=b8",
        ],
        &[0, 0, 1, 1],
    )
    .assert_ok([0, 1, 1, 0]);

    let obs = scratch.path("obs.b8");
    m2d(
        &scratch,
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[
            "--in_format=01",
            "--out_format=01",
            "--sweep",
            &sweep,
            "--obs_out",
            &obs,
            "--obs_out_format=b8",
        ],
        b"0\n0\n1\n1\n",
    )
    .assert_ok(b"0\n1\n1\n0\n");
    assert_eq!(scratch.read("obs.b8"), [0, 1, 1, 0]);
}

#[test]
fn m2d_accepts_sweep_records_in_every_result_format() {
    let scratch = Scratch::new();
    let alternating_64: String = (0..64)
        .map(|shot| if shot % 2 == 0 { "0\n" } else { "1\n" })
        .collect();
    let four_shots = (b"0\n0\n1\n1\n".to_vec(), b"0\n1\n1\n0\n".to_vec());
    let cases = [
        ("01", b"0\n1\n0\n1\n".to_vec(), four_shots.clone()),
        ("b8", vec![0, 1, 0, 1], four_shots.clone()),
        ("r8", vec![1, 0, 0, 1, 0, 0], four_shots.clone()),
        ("hits", b"\n0\n\n0\n".to_vec(), four_shots.clone()),
        (
            "dets",
            b"\nshot\n\r\n   shot M0\n\nshot\n   \nshot M0\n".to_vec(),
            four_shots,
        ),
        (
            "ptb64",
            ptb64_words(&[0xAAAA_AAAA_AAAA_AAAA]),
            ("0\n".repeat(64).into_bytes(), alternating_64.into_bytes()),
        ),
    ];

    for (format, sweep_bytes, (input, expected)) in cases {
        let sweep = scratch.file(&format!("sweep.{format}"), sweep_bytes);
        m2d(
            &scratch,
            SWEEP_CX,
            &[
                "--in_format=01",
                "--out_format=01",
                "--sweep",
                &sweep,
                "--sweep_format",
                format,
            ],
            &input,
        )
        .assert_ok(expected);
    }
}

#[test]
fn m2d_rejects_unterminated_text_records() {
    let scratch = Scratch::new();
    m2d(
        &scratch,
        SWEEP_CX,
        &["--in_format=01", "--out_format=01"],
        b"0",
    )
    .assert_err(&["m2d measurement input 01 record must end with a newline"]);

    for format in ["01", "hits"] {
        let sweep = scratch.file(&format!("unterminated.{format}"), b"0");
        m2d(
            &scratch,
            SWEEP_CX,
            &[
                "--in_format=01",
                "--out_format=01",
                "--sweep",
                &sweep,
                "--sweep_format",
                format,
            ],
            b"0\n",
        )
        .assert_err(&[&format!(
            "m2d sweep input {format} record must end with a newline"
        )]);
    }
}

#[test]
fn m2d_rejects_sweep_record_count_mismatches() {
    let scratch = Scratch::new();
    for (sweep_records, input, expected) in [
        (
            "0\n",
            &b"0\n0\n"[..],
            "m2d measurement input has more records than sweep input",
        ),
        (
            "0\n0\n",
            b"0\n",
            "m2d sweep input has more records than measurement input",
        ),
    ] {
        let sweep = scratch.file("sweep.01", sweep_records);
        let result = m2d(
            &scratch,
            SWEEP_CX,
            &["--in_format=01", "--out_format=01", "--sweep", &sweep],
            input,
        );
        assert_eq!(result.status, 1);
        assert_eq!(result.stdout, b"0\n", "the matched prefix is still written");
        result.assert_stderr_contains(&[expected]);
    }
}

#[test]
fn m2d_rejects_invalid_sampler_sweep_target_order() {
    let scratch = Scratch::new();
    let sweep = scratch.file("sweep.01", "1\n");
    for gate in ["CX", "CY"] {
        for skip_reference in [&[][..], &["--skip_reference_sample"]] {
            let mut args = vec![
                "--in_format=01",
                "--out_format=dets",
                "--sweep",
                &sweep,
                "--sweep_format=01",
            ];
            args.extend_from_slice(skip_reference);
            m2d(
                &scratch,
                &format!("{gate} 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n"),
                &args,
                b"0\n",
            )
            .assert_err(&["target shape"]);
        }
    }
}

#[test]
fn m2d_rejects_missing_and_malformed_sweep_input() {
    let scratch = Scratch::new();
    let missing = scratch.path("missing_sweep.01");
    let malformed = scratch.file("malformed_sweep.01", "00\n");
    for (sweep, expected) in [
        (&missing, "failed to read"),
        (&malformed, "01 data did not end with a newline"),
    ] {
        m2d(
            &scratch,
            SWEEP_CX,
            &["--in_format=01", "--out_format=01", "--sweep", sweep],
            b"0\n",
        )
        .assert_err(&[expected]);
    }
}

#[test]
fn m2d_zero_width_b8_records_follow_stim() {
    let scratch = Scratch::new();
    let empty_sweep = scratch.file("empty_sweep.b8", []);
    let nonempty_sweep = scratch.file("nonempty_sweep.b8", [0]);
    let zero_width = |in_format: &str, sweep: &str, input: &[u8]| {
        m2d(
            &scratch,
            "",
            &[
                in_format,
                "--out_format=01",
                "--sweep",
                sweep,
                "--sweep_format=b8",
            ],
            input,
        )
    };

    zero_width("--in_format=01", &empty_sweep, b"").assert_ok(b"");
    zero_width("--in_format=01", &empty_sweep, b"\n\n").assert_ok(b"\n\n");
    zero_width("--in_format=b8", &empty_sweep, b"")
        .assert_err(&["m2d measurement input b8 input cannot represent zero-width records"]);
    zero_width("--in_format=01", &nonempty_sweep, b"")
        .assert_err(&["m2d sweep input b8 zero-width input must be empty"]);
}

#[test]
fn m2d_reads_input_path_and_writes_output_paths() {
    let scratch = Scratch::new();
    let measurements = scratch.file("measurements.01", "1\n0\n");
    let output = scratch.path("detectors.dets");
    let obs = scratch.path("observables.01");
    m2d(
        &scratch,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[
            "--in_format=01",
            "--out_format=dets",
            "--in",
            &measurements,
            "--out",
            &output,
            "--obs_out",
            &obs,
            "--obs_out_format=01",
        ],
        b"ignored stdin",
    )
    .assert_ok(b"");
    assert_eq!(scratch.read("detectors.dets"), b"shot D0\nshot\n");
    assert_eq!(scratch.read("observables.01"), b"1\n0\n");
}
