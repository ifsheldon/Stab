#![allow(
    clippy::expect_used,
    reason = "real-process parity fixtures use direct setup and assertions for compact diagnostics"
)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::tempdir;

#[derive(Clone, Copy)]
enum TestFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}

impl TestFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ZeroOne => "01",
            Self::B8 => "b8",
            Self::R8 => "r8",
            Self::Hits => "hits",
            Self::Dets => "dets",
            Self::Ptb64 => "ptb64",
        }
    }
}

impl std::fmt::Display for TestFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

const FORMATS: [TestFormat; 6] = [
    TestFormat::ZeroOne,
    TestFormat::B8,
    TestFormat::R8,
    TestFormat::Hits,
    TestFormat::Dets,
    TestFormat::Ptb64,
];
const SINGLE_RECORD_OUTPUT_FORMATS: [TestFormat; 5] = [
    TestFormat::ZeroOne,
    TestFormat::B8,
    TestFormat::R8,
    TestFormat::Hits,
    TestFormat::Dets,
];

fn run<I, S>(args: I, input: &[u8]) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(env!("CARGO_BIN_EXE_stab"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stab");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write fixture input");
    child.wait_with_output().expect("wait for stab")
}

fn assert_ok(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label}: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{label}: unexpected stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn zero_records(format: TestFormat, width: usize, shots: usize) -> Vec<u8> {
    match format {
        TestFormat::ZeroOne => format!("{}\n", "0".repeat(width))
            .repeat(shots)
            .into_bytes(),
        TestFormat::B8 => vec![0; width.div_ceil(8) * shots],
        TestFormat::R8 => {
            let run_length = u8::try_from(width).expect("test helper uses one r8 run");
            vec![run_length; shots]
        }
        TestFormat::Hits => vec![b'\n'; shots],
        TestFormat::Dets => b"shot\n".repeat(shots),
        TestFormat::Ptb64 => {
            assert_eq!(shots % 64, 0, "ptb64 requires complete groups");
            vec![0; shots / 64 * width * size_of::<u64>()]
        }
    }
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::write(path, bytes).expect("write fixture file");
}

#[test]
fn convert_real_process_covers_layout_and_format_routes() {
    const SHOTS: usize = 64;
    const WIDTH: usize = 2;
    let expected_01 = zero_records(TestFormat::ZeroOne, WIDTH, SHOTS);

    for format in FORMATS {
        let output = run(
            [
                "convert",
                "--in_format",
                format.as_str(),
                "--out_format=01",
                "--num_measurements=2",
            ],
            &zero_records(format, WIDTH, SHOTS),
        );
        assert_ok(&output, &format!("convert {format} input"));
        assert_eq!(output.stdout, expected_01, "convert {format} input");
    }

    for format in FORMATS {
        let output = run(
            [
                "convert",
                "--in_format=01",
                "--out_format",
                format.as_str(),
                "--num_measurements=2",
            ],
            &expected_01,
        );
        assert_ok(&output, &format!("convert {format} output"));
        assert_eq!(
            output.stdout,
            zero_records(format, WIDTH, SHOTS),
            "convert {format} output"
        );
    }

    let raw = run(
        [
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
        ],
        b"00\n",
    );
    assert_ok(&raw, "convert raw-width route");
    assert_eq!(raw.stdout, [0]);

    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("typed.01");
    let output = dir.path().join("detectors.b8");
    let observables = dir.path().join("observables.dets");
    let circuit = dir.path().join("layout.stim");
    write(&input, &expected_01);
    write(
        &circuit,
        b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let typed = run(
        [
            OsString::from("convert"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=b8"),
            OsString::from("--types=DL"),
            OsString::from("--in"),
            input.clone().into_os_string(),
            OsString::from("--out"),
            output.clone().into_os_string(),
            OsString::from("--circuit"),
            circuit.into_os_string(),
            OsString::from("--obs_out"),
            observables.clone().into_os_string(),
            OsString::from("--obs_out_format=dets"),
        ],
        b"unused stdin",
    );
    assert_ok(&typed, "convert circuit layout and side output");
    assert!(typed.stdout.is_empty());
    assert_eq!(fs::read(output).expect("primary output"), vec![0; SHOTS]);
    assert_eq!(
        fs::read(observables).expect("observable output"),
        zero_records(TestFormat::Dets, 1, SHOTS)
    );

    let dem = dir.path().join("layout.dem");
    write(&dem, b"error(0) D0 L0\n");
    let dem_layout = run(
        [
            OsString::from("convert"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--dem"),
            dem.into_os_string(),
        ],
        b"00\n",
    );
    assert_ok(&dem_layout, "convert DEM layout");
    assert_eq!(dem_layout.stdout, b"00\n");

    let explicit_layout = run(
        [
            "convert",
            "--in_format=01",
            "--out_format=01",
            "--num_measurements=1",
            "--num_detectors=1",
            "--num_observables=1",
        ],
        b"000\n",
    );
    assert_ok(&explicit_layout, "convert explicit typed layout");
    assert_eq!(explicit_layout.stdout, b"000\n");
}

#[test]
fn sample_real_process_covers_options_and_all_output_formats() {
    const SHOTS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(&circuit, b"REPEAT 2 {\nM 0 1\n}\n");

    for format in FORMATS {
        let output_path = dir.path().join(format!("samples.{format}"));
        let output = run(
            [
                OsString::from("sample"),
                OsString::from("--shots=64"),
                OsString::from("--seed=5"),
                OsString::from("--skip_loop_folding"),
                OsString::from("--skip_reference_sample"),
                OsString::from("--out_format"),
                OsString::from(format.as_str()),
                OsString::from("--in"),
                circuit.clone().into_os_string(),
                OsString::from("--out"),
                output_path.clone().into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("sample {format}"));
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(output_path).expect("sample output"),
            zero_records(format, 4, SHOTS),
            "sample {format}"
        );
    }
}

#[test]
fn detect_real_process_covers_options_and_output_routes() {
    const SHOTS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(
        &circuit,
        b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );

    for format in FORMATS {
        let output_path = dir.path().join(format!("detections.{format}"));
        let output = run(
            [
                OsString::from("detect"),
                OsString::from("--shots=64"),
                OsString::from("--seed=5"),
                OsString::from("--append_observables"),
                OsString::from("--out_format"),
                OsString::from(format.as_str()),
                OsString::from("--in"),
                circuit.clone().into_os_string(),
                OsString::from("--out"),
                output_path.clone().into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("detect {format}"));
        assert_eq!(
            fs::read(output_path).expect("detection output"),
            zero_records(format, 2, SHOTS),
            "detect {format}"
        );
    }

    let observables = dir.path().join("observables.ptb64");
    let side_output = run(
        [
            OsString::from("detect"),
            OsString::from("--shots=64"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            circuit.into_os_string(),
            OsString::from("--obs_out"),
            observables.clone().into_os_string(),
            OsString::from("--obs_out_format=ptb64"),
        ],
        b"unused stdin",
    );
    assert_ok(&side_output, "detect observable side output");
    assert_eq!(
        side_output.stdout,
        zero_records(TestFormat::ZeroOne, 1, SHOTS)
    );
    assert_eq!(
        fs::read(observables).expect("observable output"),
        zero_records(TestFormat::Ptb64, 1, SHOTS)
    );
}

#[test]
fn gen_real_process_routes_geometry_noise_and_paths() {
    let dir = tempdir().expect("tempdir");
    let ignored_input = dir.path().join("ignored.stim");
    let output_path = dir.path().join("generated.stim");
    write(&ignored_input, b"ignored by Stim's generator contract\n");
    let output = run(
        [
            OsString::from("gen"),
            OsString::from("--code=repetition_code"),
            OsString::from("--task=memory"),
            OsString::from("--distance=3"),
            OsString::from("--rounds=2"),
            OsString::from("--after_clifford_depolarization=0.01"),
            OsString::from("--after_reset_flip_probability=0.02"),
            OsString::from("--before_measure_flip_probability=0.03"),
            OsString::from("--before_round_data_depolarization=0.04"),
            OsString::from("--in"),
            ignored_input.into_os_string(),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
        ],
        b"unused stdin",
    );
    assert_ok(&output, "gen complete option route");
    assert!(output.stdout.is_empty());
    let generated = fs::read_to_string(output_path).expect("generated circuit");
    for expected in [
        "# before_round_data_depolarization: 0.04",
        "X_ERROR(0.02)",
        "DEPOLARIZE2(0.01)",
        "X_ERROR(0.03)",
        "OBSERVABLE_INCLUDE(0)",
    ] {
        assert!(generated.contains(expected), "missing {expected}");
    }
}

#[test]
fn m2d_real_process_covers_measurement_sweep_and_output_routes() {
    const SHOTS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(
        &circuit,
        b"M 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );

    for format in FORMATS {
        let measurements = dir.path().join(format!("measurements.{format}"));
        let sweep = dir.path().join(format!("sweep.{format}"));
        write(&measurements, zero_records(format, 2, SHOTS));
        write(&sweep, zero_records(format, 1, SHOTS));
        let output = run(
            [
                OsString::from("m2d"),
                OsString::from("--in_format"),
                OsString::from(format.as_str()),
                OsString::from("--sweep_format"),
                OsString::from(format.as_str()),
                OsString::from("--out_format=01"),
                OsString::from("--append_observables"),
                OsString::from("--skip_reference_sample"),
                OsString::from("--ran_without_feedback"),
                OsString::from("--circuit"),
                circuit.clone().into_os_string(),
                OsString::from("--in"),
                measurements.into_os_string(),
                OsString::from("--sweep"),
                sweep.into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("m2d {format} inputs"));
        assert_eq!(output.stdout, zero_records(TestFormat::ZeroOne, 2, SHOTS));
    }

    let measurements = dir.path().join("measurements.01");
    let sweep = dir.path().join("sweep.01");
    write(&measurements, zero_records(TestFormat::ZeroOne, 2, SHOTS));
    write(&sweep, zero_records(TestFormat::ZeroOne, 1, SHOTS));
    for format in SINGLE_RECORD_OUTPUT_FORMATS {
        let output_path = dir.path().join(format!("detections.{format}"));
        let observables = dir.path().join(format!("observables.{format}"));
        let output = run(
            [
                OsString::from("m2d"),
                OsString::from("--in_format=01"),
                OsString::from("--sweep_format=01"),
                OsString::from("--out_format"),
                OsString::from(format.as_str()),
                OsString::from("--obs_out_format"),
                OsString::from(format.as_str()),
                OsString::from("--circuit"),
                circuit.clone().into_os_string(),
                OsString::from("--in"),
                measurements.clone().into_os_string(),
                OsString::from("--sweep"),
                sweep.clone().into_os_string(),
                OsString::from("--out"),
                output_path.clone().into_os_string(),
                OsString::from("--obs_out"),
                observables.clone().into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("m2d {format} outputs"));
        assert_eq!(
            fs::read(output_path).expect("detection output"),
            zero_records(format, 1, SHOTS)
        );
        assert_eq!(
            fs::read(observables).expect("observable output"),
            zero_records(format, 1, SHOTS)
        );
    }
}

#[test]
fn m2d_real_process_routes_sweep_pauli_observable_corrections() {
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("pauli-sweep.stim");
    let sweeps = dir.path().join("sweeps.01");
    write(
        &circuit,
        b"R 0 1 2 3\nCZ sweep[0] 0\nOBSERVABLE_INCLUDE(0) X0\nCX sweep[1] 1\nOBSERVABLE_INCLUDE(1) Z1\nCX sweep[2] 2\nOBSERVABLE_INCLUDE(2) Y2\nREPEAT 3 {\n    CX sweep[3] 3\n}\nOBSERVABLE_INCLUDE(3) Z3\n",
    );
    write(&sweeps, b"0000\n1111\n");

    for skip_reference in [false, true] {
        let mut args = vec![
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--append_observables"),
            OsString::from("--sweep_format=01"),
            OsString::from("--circuit"),
            circuit.clone().into_os_string(),
            OsString::from("--sweep"),
            sweeps.clone().into_os_string(),
        ];
        if skip_reference {
            args.push(OsString::from("--skip_reference_sample"));
        }
        let output = run(args, b"\n\n");
        assert_ok(&output, "m2d appended Pauli-observable correction");
        assert_eq!(output.stdout, b"0000\n1111\n");
    }

    let observables = dir.path().join("observables.01");
    let output = run(
        [
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--obs_out_format=01"),
            OsString::from("--skip_reference_sample"),
            OsString::from("--sweep_format=01"),
            OsString::from("--circuit"),
            circuit.into_os_string(),
            OsString::from("--sweep"),
            sweeps.into_os_string(),
            OsString::from("--obs_out"),
            observables.clone().into_os_string(),
        ],
        b"\n\n",
    );
    assert_ok(&output, "m2d separate Pauli-observable correction");
    assert_eq!(output.stdout, b"\n\n");
    assert_eq!(
        fs::read(observables).expect("observable side output"),
        b"0000\n1111\n"
    );
}

#[test]
fn folded_detection_workflows_match_unrolled_outputs_including_ptb64() {
    const SHOTS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let folded = dir.path().join("folded.stim");
    let unrolled = dir.path().join("unrolled.stim");
    write(
        &folded,
        b"M 0\nREPEAT 3 {\nM 0\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\nREPEAT 2 {\nM 0\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1] rec[-2]\n}\n}\n",
    );
    let mut unrolled_text = String::from("M 0\n");
    for _ in 0..3 {
        unrolled_text.push_str("M 0\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n");
        for _ in 0..2 {
            unrolled_text
                .push_str("M 0\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1] rec[-2]\n");
        }
    }
    write(&unrolled, unrolled_text);
    let measurements = b"0101101001\n".repeat(SHOTS);

    let run_m2d = |label: &str, circuit: &Path| {
        let observables = dir.path().join(format!("{label}-observables.b8"));
        let output = run(
            [
                OsString::from("m2d"),
                OsString::from("--in_format=01"),
                OsString::from("--out_format=b8"),
                OsString::from("--skip_reference_sample"),
                OsString::from("--circuit"),
                circuit.as_os_str().to_owned(),
                OsString::from("--obs_out"),
                observables.clone().into_os_string(),
                OsString::from("--obs_out_format=b8"),
            ],
            &measurements,
        );
        assert_ok(&output, &format!("{label} m2d"));
        (
            output.stdout,
            fs::read(observables).expect("observable output"),
        )
    };
    let folded_m2d = run_m2d("folded", &folded);
    let unrolled_m2d = run_m2d("unrolled", &unrolled);
    assert_eq!(folded_m2d, unrolled_m2d);
    assert_eq!(folded_m2d.0.len(), 9_usize.div_ceil(8) * SHOTS);
    assert_eq!(folded_m2d.1.len(), 2_usize.div_ceil(8) * SHOTS);

    let run_detect = |label: &str, circuit: &Path| {
        let output = run(
            [
                OsString::from("detect"),
                OsString::from("--shots=64"),
                OsString::from("--seed=17"),
                OsString::from("--append_observables"),
                OsString::from("--out_format=ptb64"),
                OsString::from("--in"),
                circuit.as_os_str().to_owned(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("{label} detect"));
        output.stdout
    };
    let folded_detect = run_detect("folded", &folded);
    let unrolled_detect = run_detect("unrolled", &unrolled);
    assert_eq!(folded_detect, unrolled_detect);
    assert_eq!(folded_detect.len(), 11 * size_of::<u64>());
}

#[test]
fn analyze_errors_real_process_routes_all_nondeprecated_options() {
    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("input.stim");
    let output_path = dir.path().join("output.dem");
    write(
        &input,
        b"REPEAT 2 {\nX_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n}\n",
    );
    let output = run(
        [
            OsString::from("analyze_errors"),
            OsString::from("--decompose_errors"),
            OsString::from("--block_decompose_from_introducing_remnant_edges"),
            OsString::from("--ignore_decomposition_failures"),
            OsString::from("--fold_loops"),
            OsString::from("--allow_gauge_detectors"),
            OsString::from("--approximate_disjoint_errors=1"),
            OsString::from("--in"),
            input.into_os_string(),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
        ],
        b"unused stdin",
    );
    assert_ok(&output, "analyze_errors complete option route");
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(output_path).expect("DEM output"),
        "error(0.25) D0 D1\nerror(0.25) D1\n"
    );
}

#[test]
fn sample_dem_real_process_covers_primary_side_and_replay_routes() {
    const SHOTS: usize = 64;
    let dir = tempdir().expect("tempdir");
    let dem = dir.path().join("input.dem");
    write(&dem, b"error(0) D0 L0\n");

    for format in FORMATS {
        let primary = dir.path().join(format!("primary.{format}"));
        let observables = dir.path().join(format!("observables.{format}"));
        let errors = dir.path().join(format!("errors.{format}"));
        let output = run(
            [
                OsString::from("sample_dem"),
                OsString::from("--shots=64"),
                OsString::from("--seed=5"),
                OsString::from("--out_format"),
                OsString::from(format.as_str()),
                OsString::from("--obs_out_format"),
                OsString::from(format.as_str()),
                OsString::from("--err_out_format"),
                OsString::from(format.as_str()),
                OsString::from("--in"),
                dem.clone().into_os_string(),
                OsString::from("--out"),
                primary.clone().into_os_string(),
                OsString::from("--obs_out"),
                observables.clone().into_os_string(),
                OsString::from("--err_out"),
                errors.clone().into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&output, &format!("sample_dem {format} outputs"));
        let expected = zero_records(format, 1, SHOTS);
        assert_eq!(fs::read(primary).expect("primary output"), expected);
        assert_eq!(fs::read(observables).expect("observable output"), expected);
        assert_eq!(fs::read(errors).expect("error output"), expected);

        let replay = dir.path().join(format!("replay.{format}"));
        write(&replay, &expected);
        let replayed = run(
            [
                OsString::from("sample_dem"),
                OsString::from("--shots=64"),
                OsString::from("--out_format=01"),
                OsString::from("--replay_err_in_format"),
                OsString::from(format.as_str()),
                OsString::from("--in"),
                dem.clone().into_os_string(),
                OsString::from("--replay_err_in"),
                replay.into_os_string(),
            ],
            b"unused stdin",
        );
        assert_ok(&replayed, &format!("sample_dem {format} replay"));
        assert_eq!(replayed.stdout, zero_records(TestFormat::ZeroOne, 1, SHOTS));
    }
}

#[test]
fn json_diagnostics_real_process_keeps_machine_output_separate() {
    let output = run(
        ["--error-format=json", "sample", "--shots=1"],
        b"M not-a-target\n",
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let line = std::str::from_utf8(&output.stderr).expect("UTF-8 diagnostic");
    assert_eq!(line.lines().count(), 1);
    let value: serde_json::Value = serde_json::from_str(line).expect("JSON Lines diagnostic");
    assert_eq!(
        value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        value.get("severity").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        value.get("code").and_then(serde_json::Value::as_str),
        Some("invalid-target-syntax")
    );
    assert_eq!(
        value
            .get("context")
            .and_then(|context| context.get("dialect"))
            .and_then(serde_json::Value::as_str),
        Some("stim-circuit")
    );
}

#[test]
fn removed_compatibility_routes_are_absent_from_real_binary_and_help() {
    let removed: &[(&[&str], &str)] = &[
        (&["--gen=repetition_code"], "--gen"),
        (&["--convert"], "--convert"),
        (&["--sample=1"], "--sample"),
        (&["--detect=1"], "--detect"),
        (&["--m2d"], "--m2d"),
        (&["--analyze_errors"], "--analyze_errors"),
        (&["--detector_hypergraph"], "--detector_hypergraph"),
        (&["--help=sample"], "--help"),
        (&["sample", "--frame0"], "--frame0"),
        (
            &["detect", "--prepend_observables"],
            "--prepend_observables",
        ),
        (
            &["sample_dem", "--append_observables"],
            "--append_observables",
        ),
        (
            &["sample_dem", "--prepend_observables"],
            "--prepend_observables",
        ),
    ];
    for (args, removed_flag) in removed {
        let output = run(*args, b"M 0\n");
        assert!(!output.status.success(), "removed route accepted: {args:?}");
        assert!(
            output.stdout.is_empty(),
            "removed route wrote stdout: {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(removed_flag),
            "removed route error omitted {removed_flag}: {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for args in [
        &["help", "sample"][..],
        &["sample", "--help"][..],
        &["help", "detect"][..],
        &["detect", "--help"][..],
        &["help", "sample_dem"][..],
        &["sample_dem", "--help"][..],
    ] {
        let output = run(args, b"");
        assert_ok(&output, &format!("help surface {args:?}"));
        let help = String::from_utf8_lossy(&output.stdout);
        for removed_flag in ["--frame0", "--prepend_observables", "--append_observables"] {
            if args.contains(&"detect") && removed_flag == "--append_observables" {
                continue;
            }
            assert!(
                !help.contains(removed_flag),
                "help {args:?} advertises removed flag {removed_flag}: {help}"
            );
        }
    }
}
