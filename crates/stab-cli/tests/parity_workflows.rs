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

/// Runs a command that must not read standard input, which is closed instead of piped.
///
/// A child that exits before reading would make a piped write fail with `BrokenPipe`, so these
/// commands get no pipe at all; any accidental stdin read sees end of input and changes output.
fn run_without_stdin<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_stab"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run stab")
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

/// One record block spelled in every result format, recorded from pinned Stim v1.16.0.
///
/// The text and byte fields hold the block itself; `ptb64` holds the little-endian bit planes of
/// one complete 64-shot group: the block followed by all-zero shots, or the block repeated when
/// it is a single uniform shot. Uniform blocks are recorded from Stim command outputs; ptb64
/// planes of non-uniform blocks are recorded from Stim's ptb64 writer through
/// `sample_dem --replay_err_in ... --err_out_format=ptb64`, because `stim convert` cannot write
/// ptb64.
struct Records {
    zero_one: &'static str,
    b8: &'static [u8],
    r8: &'static [u8],
    hits: &'static str,
    dets: &'static str,
    ptb64: &'static [u64],
}

impl Records {
    /// Returns the block repeated `repeat` times, or the complete ptb64 group.
    fn encoded(&self, format: TestFormat, repeat: usize) -> Vec<u8> {
        match format {
            TestFormat::ZeroOne => self.zero_one.as_bytes().repeat(repeat),
            TestFormat::B8 => self.b8.repeat(repeat),
            TestFormat::R8 => self.r8.repeat(repeat),
            TestFormat::Hits => self.hits.as_bytes().repeat(repeat),
            TestFormat::Dets => self.dets.as_bytes().repeat(repeat),
            TestFormat::Ptb64 => self
                .ptb64
                .iter()
                .flat_map(|plane| plane.to_le_bytes())
                .collect(),
        }
    }
}

/// Pads a `01` block with all-zero records up to one 64-shot ptb64 group.
fn padded_to_ptb64_group(block: &str, padding_record: &str) -> Vec<u8> {
    let padding = 64 - block.lines().count();
    format!("{block}{}", padding_record.repeat(padding)).into_bytes()
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::write(path, bytes).expect("write fixture file");
}

/// Four 10-bit shots whose set bits are asymmetric within and across bytes, so reversed bit,
/// byte, shot, or plane order changes every encoding.
const CONVERT_BLOCK: Records = Records {
    zero_one: "1000000001\n0110100000\n0000000000\n1111111111\n",
    b8: &[0x01, 0x02, 0x16, 0x00, 0x00, 0x00, 0xff, 0x03],
    r8: &[
        0x00, 0x08, 0x00, 0x01, 0x00, 0x01, 0x05, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ],
    hits: "0,9\n1,2,4\n\n0,1,2,3,4,5,6,7,8,9\n",
    dets: "shot M0 M9\nshot M1 M2 M4\nshot\nshot M0 M1 M2 M3 M4 M5 M6 M7 M8 M9\n",
    ptb64: &[0x9, 0xa, 0xa, 0x8, 0xa, 0x8, 0x8, 0x8, 0x8, 0x9],
};

#[test]
fn convert_real_process_covers_layout_and_format_routes() {
    let block_01 = CONVERT_BLOCK.zero_one;
    let group_01 = padded_to_ptb64_group(block_01, "0000000000\n");
    let decoded_01 = |format: TestFormat| match format {
        TestFormat::Ptb64 => group_01.clone(),
        _ => block_01.as_bytes().to_vec(),
    };

    for format in FORMATS {
        let output = run(
            [
                "convert",
                "--in_format",
                format.as_str(),
                "--out_format=01",
                "--num_measurements=10",
            ],
            &CONVERT_BLOCK.encoded(format, 1),
        );
        assert_ok(&output, &format!("convert {format} input"));
        assert_eq!(output.stdout, decoded_01(format), "convert {format} input");
    }

    for format in FORMATS {
        let output = run(
            [
                "convert",
                "--in_format=01",
                "--out_format",
                format.as_str(),
                "--num_measurements=10",
            ],
            &decoded_01(format),
        );
        assert_ok(&output, &format!("convert {format} output"));
        assert_eq!(
            output.stdout,
            CONVERT_BLOCK.encoded(format, 1),
            "convert {format} output"
        );
    }

    // Expected outputs below were recorded from pinned Stim v1.16.0.
    let raw = run(
        [
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
        ],
        b"01\n10\n",
    );
    assert_ok(&raw, "convert raw-width route");
    assert_eq!(raw.stdout, [0x02, 0x01]);

    let dir = tempdir().expect("tempdir");
    let input = dir.path().join("typed.01");
    let output = dir.path().join("detectors.b8");
    let observables = dir.path().join("observables.dets");
    let circuit = dir.path().join("layout.stim");
    write(&input, b"10\n01\n11\n00\n");
    write(
        &circuit,
        b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let typed = run_without_stdin([
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
    ]);
    assert_ok(&typed, "convert circuit layout and side output");
    assert!(typed.stdout.is_empty());
    assert_eq!(
        fs::read(output).expect("primary output"),
        [0x01, 0x00, 0x01, 0x00]
    );
    assert_eq!(
        fs::read(observables).expect("observable output"),
        b"shot\nshot L0\nshot L0\nshot\n"
    );

    let dem = dir.path().join("layout.dem");
    write(&dem, b"error(0) D0 L0\n");
    let dem_layout = run(
        [
            OsString::from("convert"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=dets"),
            OsString::from("--dem"),
            dem.into_os_string(),
        ],
        b"10\n01\n",
    );
    assert_ok(&dem_layout, "convert DEM layout");
    assert_eq!(dem_layout.stdout, b"shot D0\nshot L0\n");

    let explicit_layout = run(
        [
            "convert",
            "--in_format=01",
            "--out_format=dets",
            "--num_measurements=1",
            "--num_detectors=1",
            "--num_observables=1",
        ],
        b"101\n011\n",
    );
    assert_ok(&explicit_layout, "convert explicit typed layout");
    assert_eq!(explicit_layout.stdout, b"shot M0 L0\nshot D0 L0\n");
}

#[test]
fn sample_real_process_covers_options_and_all_output_formats() {
    const SHOTS: usize = 64;
    // `X 1` makes the noiseless reference sample nonzero and `X_ERROR(1)` flips qubit 0 in every
    // shot, so the reference-sample mode and the noise frame both change the recorded bits.
    // Expected records were recorded from pinned Stim v1.16.0.
    const WITH_REFERENCE: Records = Records {
        zero_one: "1101\n",
        b8: &[0x0b],
        r8: &[0x00, 0x00, 0x01, 0x00],
        hits: "0,1,3\n",
        dets: "shot M0 M1 M3\n",
        ptb64: &[u64::MAX, u64::MAX, 0, u64::MAX],
    };
    const SKIPPED_REFERENCE: Records = Records {
        zero_one: "1000\n",
        b8: &[0x01],
        r8: &[0x00, 0x03],
        hits: "0\n",
        dets: "shot M0\n",
        ptb64: &[u64::MAX, 0, 0, 0],
    };
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(&circuit, b"X 1\nREPEAT 2 {\nX_ERROR(1) 0\nM 0 1\n}\n");

    for (skip_reference, expected) in [(false, &WITH_REFERENCE), (true, &SKIPPED_REFERENCE)] {
        for format in FORMATS {
            let output_path = dir.path().join(format!("samples.{format}"));
            let mut args = vec![
                OsString::from("sample"),
                OsString::from("--shots=64"),
                OsString::from("--seed=5"),
                OsString::from("--skip_loop_folding"),
                OsString::from("--out_format"),
                OsString::from(format.as_str()),
                OsString::from("--in"),
                circuit.clone().into_os_string(),
                OsString::from("--out"),
                output_path.clone().into_os_string(),
            ];
            if skip_reference {
                args.push(OsString::from("--skip_reference_sample"));
            }
            let label = format!("sample {format} skip_reference={skip_reference}");
            let output = run_without_stdin(args);
            assert_ok(&output, &label);
            assert!(output.stdout.is_empty());
            assert_eq!(
                fs::read(output_path).expect("sample output"),
                expected.encoded(format, SHOTS),
                "{label}"
            );
        }
    }
}

#[test]
fn detect_real_process_covers_options_and_output_routes() {
    const SHOTS: usize = 64;
    // Detectors D0 and D2 and observable L1 fire in every shot while D1 and L0 stay quiet, so
    // detector, observable, and bit order are all visible. Expected records were recorded from
    // pinned Stim v1.16.0.
    const APPENDED: Records = Records {
        zero_one: "10101\n",
        b8: &[0x15],
        r8: &[0x00, 0x01, 0x01, 0x00],
        hits: "0,2,4\n",
        dets: "shot D0 D2 L1\n",
        ptb64: &[u64::MAX, 0, u64::MAX, 0, u64::MAX],
    };
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(
        &circuit,
        b"X_ERROR(1) 0 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1]\n",
    );

    for format in FORMATS {
        let output_path = dir.path().join(format!("detections.{format}"));
        let output = run_without_stdin([
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
        ]);
        assert_ok(&output, &format!("detect {format}"));
        assert_eq!(
            fs::read(output_path).expect("detection output"),
            APPENDED.encoded(format, SHOTS),
            "detect {format}"
        );
    }

    // Without --append_observables, Stim's dets output prepends the observables.
    let prepended = run_without_stdin([
        OsString::from("detect"),
        OsString::from("--shots=2"),
        OsString::from("--out_format=dets"),
        OsString::from("--in"),
        circuit.clone().into_os_string(),
    ]);
    assert_ok(&prepended, "detect dets default observable placement");
    assert_eq!(prepended.stdout, b"shot L1 D0 D2\nshot L1 D0 D2\n");

    let observables = dir.path().join("observables.ptb64");
    let side_output = run_without_stdin([
        OsString::from("detect"),
        OsString::from("--shots=64"),
        OsString::from("--out_format=01"),
        OsString::from("--in"),
        circuit.into_os_string(),
        OsString::from("--obs_out"),
        observables.clone().into_os_string(),
        OsString::from("--obs_out_format=ptb64"),
    ]);
    assert_ok(&side_output, "detect observable side output");
    assert_eq!(side_output.stdout, b"101\n".repeat(SHOTS));
    assert_eq!(
        fs::read(observables).expect("observable output"),
        [0_u64, u64::MAX]
            .iter()
            .flat_map(|plane| plane.to_le_bytes())
            .collect::<Vec<_>>()
    );
}

/// Recorded from pinned Stim v1.16.0 for the argument list in the test below.
const GENERATED_REPETITION_CODE: &str = "\
# Generated repetition_code circuit.\n\
# task: memory\n\
# rounds: 2\n\
# distance: 3\n\
# before_round_data_depolarization: 0.04\n\
# before_measure_flip_probability: 0.03\n\
# after_reset_flip_probability: 0.02\n\
# after_clifford_depolarization: 0.01\n\
# layout:\n\
# L0 Z1 d2 Z3 d4\n\
# Legend:\n\
#     d# = data qubit\n\
#     L# = data qubit with logical observable crossing\n\
#     Z# = measurement qubit\n\
R 0 1 2 3 4\n\
X_ERROR(0.02) 0 1 2 3 4\n\
TICK\n\
DEPOLARIZE1(0.04) 0 2 4\n\
CX 0 1 2 3\n\
DEPOLARIZE2(0.01) 0 1 2 3\n\
TICK\n\
CX 2 1 4 3\n\
DEPOLARIZE2(0.01) 2 1 4 3\n\
TICK\n\
X_ERROR(0.03) 1 3\n\
MR 1 3\n\
X_ERROR(0.02) 1 3\n\
DETECTOR(1, 0) rec[-2]\n\
DETECTOR(3, 0) rec[-1]\n\
TICK\n\
DEPOLARIZE1(0.04) 0 2 4\n\
CX 0 1 2 3\n\
DEPOLARIZE2(0.01) 0 1 2 3\n\
TICK\n\
CX 2 1 4 3\n\
DEPOLARIZE2(0.01) 2 1 4 3\n\
TICK\n\
X_ERROR(0.03) 1 3\n\
MR 1 3\n\
X_ERROR(0.02) 1 3\n\
SHIFT_COORDS(0, 1)\n\
DETECTOR(1, 0) rec[-2] rec[-4]\n\
DETECTOR(3, 0) rec[-1] rec[-3]\n\
X_ERROR(0.03) 0 2 4\n\
M 0 2 4\n\
DETECTOR(1, 1) rec[-2] rec[-3] rec[-5]\n\
DETECTOR(3, 1) rec[-1] rec[-2] rec[-4]\n\
OBSERVABLE_INCLUDE(0) rec[-1]\n\
";

#[test]
fn gen_real_process_routes_geometry_noise_and_paths() {
    let dir = tempdir().expect("tempdir");
    let ignored_input = dir.path().join("ignored.stim");
    let output_path = dir.path().join("generated.stim");
    write(&ignored_input, b"ignored by Stim's generator contract\n");
    let output = run_without_stdin([
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
    ]);
    assert_ok(&output, "gen complete option route");
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read_to_string(output_path).expect("generated circuit"),
        GENERATED_REPETITION_CODE
    );
}

/// The reference sample is nonzero (`X 0`), `CX sweep[0] 0` flips the second measurement when
/// the sweep bit is set, and `CX rec[-1] 1` is classical feedback, so `--sweep`,
/// `--skip_reference_sample`, and `--ran_without_feedback` each change the detection events.
const M2D_SWEEP_CIRCUIT: &[u8] = b"X 0\nM 0\nCX sweep[0] 0\nCX rec[-1] 1\nM 0 1\nDETECTOR rec[-2] rec[-3]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1]\n";

/// Measurement and sweep blocks with distinct per-shot patterns, recorded from pinned Stim
/// v1.16.0 like [`CONVERT_BLOCK`].
const M2D_MEASUREMENTS: Records = Records {
    zero_one: "100\n110\n011\n111\n",
    b8: &[0x01, 0x03, 0x06, 0x07],
    r8: &[
        0x00, 0x02, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ],
    hits: "0\n0,1\n1,2\n0,1,2\n",
    dets: "shot M0\nshot M0 M1\nshot M1 M2\nshot M0 M1 M2\n",
    ptb64: &[0xb, 0xe, 0xc],
};
const M2D_SWEEPS: Records = Records {
    zero_one: "0\n1\n0\n1\n",
    b8: &[0x00, 0x01, 0x00, 0x01],
    r8: &[0x01, 0x00, 0x00, 0x01, 0x00, 0x00],
    hits: "\n0\n\n0\n",
    dets: "shot\nshot M0\nshot\nshot M0\n",
    ptb64: &[0xa],
};

#[test]
fn m2d_real_process_covers_measurement_sweep_and_output_routes() {
    let dir = tempdir().expect("tempdir");
    let circuit = dir.path().join("input.stim");
    write(&circuit, M2D_SWEEP_CIRCUIT);

    // Recorded from pinned Stim v1.16.0 with --append_observables, --skip_reference_sample, and
    // --ran_without_feedback. Without the sweep data the second and fourth shots would read
    // 0111 and 0010.
    const APPENDED_BLOCK: &str = "1101\n1101\n1111\n1000\n";
    for format in FORMATS {
        let measurements = dir.path().join(format!("measurements.{format}"));
        let sweep = dir.path().join(format!("sweep.{format}"));
        write(&measurements, M2D_MEASUREMENTS.encoded(format, 1));
        write(&sweep, M2D_SWEEPS.encoded(format, 1));
        let output = run_without_stdin([
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
        ]);
        assert_ok(&output, &format!("m2d {format} inputs"));
        let expected = match format {
            TestFormat::Ptb64 => padded_to_ptb64_group(APPENDED_BLOCK, "0000\n"),
            _ => APPENDED_BLOCK.as_bytes().to_vec(),
        };
        assert_eq!(output.stdout, expected, "m2d {format} inputs");
    }

    // Detector and observable side outputs with the reference sample and feedback applied,
    // recorded from pinned Stim v1.16.0 (01: detectors 11 11 10 10, observables 11 11 00 10).
    let separate_outputs: [(TestFormat, &[u8], &[u8]); 5] = [
        (
            TestFormat::ZeroOne,
            b"11\n11\n10\n10\n",
            b"11\n11\n00\n10\n",
        ),
        (
            TestFormat::B8,
            &[0x03, 0x03, 0x01, 0x01],
            &[0x03, 0x03, 0x00, 0x01],
        ),
        (
            TestFormat::R8,
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01],
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01],
        ),
        (TestFormat::Hits, b"0,1\n0,1\n0\n0\n", b"0,1\n0,1\n\n0\n"),
        (
            TestFormat::Dets,
            b"shot D0 D1\nshot D0 D1\nshot D0\nshot D0\n",
            b"shot L0 L1\nshot L0 L1\nshot\nshot L0\n",
        ),
    ];
    let measurements = dir.path().join("measurements.01");
    let sweep = dir.path().join("sweep.01");
    write(&measurements, M2D_MEASUREMENTS.zero_one);
    write(&sweep, M2D_SWEEPS.zero_one);
    for (format, expected_detectors, expected_observables) in separate_outputs {
        let output_path = dir.path().join(format!("detections.{format}"));
        let observables = dir.path().join(format!("observables.{format}"));
        let output = run_without_stdin([
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
        ]);
        assert_ok(&output, &format!("m2d {format} outputs"));
        assert_eq!(
            fs::read(output_path).expect("detection output"),
            expected_detectors,
            "m2d {format} detectors"
        );
        assert_eq!(
            fs::read(observables).expect("observable output"),
            expected_observables,
            "m2d {format} observables"
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
        let output = run_without_stdin([
            OsString::from("detect"),
            OsString::from("--shots=64"),
            OsString::from("--seed=17"),
            OsString::from("--append_observables"),
            OsString::from("--out_format=ptb64"),
            OsString::from("--in"),
            circuit.as_os_str().to_owned(),
        ]);
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
    let output = run_without_stdin([
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
    ]);
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
    // Errors 0 and 2 have probability one and error 1 never fires, so sampled shots are
    // deterministic and asymmetric. Expected records were recorded from pinned Stim v1.16.0.
    const DETECTORS: Records = Records {
        zero_one: "101\n",
        b8: &[0x05],
        r8: &[0x00, 0x01, 0x00],
        hits: "0,2\n",
        dets: "shot D0 D2\n",
        ptb64: &[u64::MAX, 0, u64::MAX],
    };
    const OBSERVABLES: Records = Records {
        zero_one: "01\n",
        b8: &[0x02],
        r8: &[0x01, 0x00],
        hits: "1\n",
        dets: "shot L1\n",
        ptb64: &[0, u64::MAX],
    };
    const ERRORS: Records = Records {
        zero_one: "101\n",
        b8: &[0x05],
        r8: &[0x00, 0x01, 0x00],
        hits: "0,2\n",
        dets: "shot M0 M2\n",
        ptb64: &[u64::MAX, 0, u64::MAX],
    };
    // Four distinct replayed error patterns followed by one surplus record (`110`). Stim replays
    // exactly the first --shots records, so the surplus must be ignored; ptb64 carries a surplus
    // all-ones group after the first 64 shots instead.
    const REPLAY: Records = Records {
        zero_one: "100\n010\n001\n111\n110\n",
        b8: &[0x01, 0x02, 0x04, 0x07, 0x03],
        r8: &[
            0x00, 0x02, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ],
        hits: "0\n1\n2\n0,1,2\n0,1\n",
        dets: "shot M0\nshot M1\nshot M2\nshot M0 M1 M2\nshot M0 M1\n",
        ptb64: &[0x9, 0xa, 0xc, u64::MAX, u64::MAX, u64::MAX],
    };
    // Replay overrides sampling: error 2 fires only where the replay data says so.
    const REPLAYED_DETECTORS: &str = "100\n010\n001\n111\n";
    const REPLAYED_OBSERVABLES: &str = "00\n10\n01\n11\n";

    let dir = tempdir().expect("tempdir");
    let dem = dir.path().join("input.dem");
    write(&dem, b"error(1) D0\nerror(0) D1 L0\nerror(1) D2 L1\n");

    for format in FORMATS {
        let primary = dir.path().join(format!("primary.{format}"));
        let observables = dir.path().join(format!("observables.{format}"));
        let errors = dir.path().join(format!("errors.{format}"));
        let output = run_without_stdin([
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
        ]);
        assert_ok(&output, &format!("sample_dem {format} outputs"));
        assert_eq!(
            fs::read(primary).expect("primary output"),
            DETECTORS.encoded(format, SHOTS),
            "sample_dem {format} detectors"
        );
        assert_eq!(
            fs::read(observables).expect("observable output"),
            OBSERVABLES.encoded(format, SHOTS),
            "sample_dem {format} observables"
        );
        assert_eq!(
            fs::read(errors).expect("error output"),
            ERRORS.encoded(format, SHOTS),
            "sample_dem {format} errors"
        );

        let replay = dir.path().join(format!("replay.{format}"));
        let replayed_observables = dir.path().join(format!("replayed-observables.{format}"));
        write(&replay, REPLAY.encoded(format, 1));
        let (shots, expected_detectors, expected_observables) = match format {
            TestFormat::Ptb64 => (
                "--shots=64",
                padded_to_ptb64_group(REPLAYED_DETECTORS, "000\n"),
                padded_to_ptb64_group(REPLAYED_OBSERVABLES, "00\n"),
            ),
            _ => (
                "--shots=4",
                REPLAYED_DETECTORS.as_bytes().to_vec(),
                REPLAYED_OBSERVABLES.as_bytes().to_vec(),
            ),
        };
        let replayed = run_without_stdin([
            OsString::from("sample_dem"),
            OsString::from(shots),
            OsString::from("--out_format=01"),
            OsString::from("--obs_out_format=01"),
            OsString::from("--replay_err_in_format"),
            OsString::from(format.as_str()),
            OsString::from("--in"),
            dem.clone().into_os_string(),
            OsString::from("--replay_err_in"),
            replay.into_os_string(),
            OsString::from("--obs_out"),
            replayed_observables.clone().into_os_string(),
        ]);
        assert_ok(&replayed, &format!("sample_dem {format} replay"));
        assert_eq!(
            replayed.stdout, expected_detectors,
            "sample_dem {format} replay detectors"
        );
        assert_eq!(
            fs::read(replayed_observables).expect("replayed observable output"),
            expected_observables,
            "sample_dem {format} replay observables"
        );
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
        let output = run_without_stdin(*args);
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
        let output = run_without_stdin(args);
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
