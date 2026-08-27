use std::ffi::OsString;

use super::run_from;
use tempfile::tempdir;

#[test]
fn m2d_path_io_reads_input_path_and_writes_output_paths() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    let measurement_path = dir.path().join("measurements.01");
    let output_path = dir.path().join("detectors.dets");
    let obs_path = dir.path().join("observables.01");
    std::fs::write(
        &circuit_path,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(&measurement_path, "1\n0\n").expect("write measurements");

    let args = vec![
        OsString::from("stab"),
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--out_format=dets"),
        OsString::from("--in"),
        measurement_path.into_os_string(),
        OsString::from("--out"),
        output_path.clone().into_os_string(),
        OsString::from("--obs_out"),
        obs_path.clone().into_os_string(),
        OsString::from("--obs_out_format=01"),
        OsString::from("--circuit"),
        circuit_path.into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, b"ignored stdin".as_slice(), &mut stdout, &mut stderr);

    assert_eq!(status, 0);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    assert_eq!(
        std::fs::read_to_string(output_path).expect("read m2d detector output"),
        "shot D0\nshot\n"
    );
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("read m2d observable output"),
        "1\n0\n"
    );
}

#[test]
fn m2d_path_io_reports_missing_circuit_and_measurement_paths() {
    let dir = tempdir().expect("temp dir");
    let missing_circuit = dir.path().join("missing.stim");
    let output_path = dir.path().join("output.01");
    let mut circuit_stdout = Vec::new();
    let mut circuit_stderr = Vec::new();
    let circuit_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
            OsString::from("--circuit"),
            missing_circuit.clone().into_os_string(),
        ],
        b"0\n".as_slice(),
        &mut circuit_stdout,
        &mut circuit_stderr,
    );

    assert_eq!(circuit_status, 1);
    assert_eq!(String::from_utf8(circuit_stdout).unwrap(), "");
    let circuit_error = String::from_utf8(circuit_stderr).unwrap();
    assert!(circuit_error.contains("failed to read"), "{circuit_error}");
    assert!(circuit_error.contains("missing.stim"), "{circuit_error}");
    assert!(!output_path.exists());

    let circuit_path = dir.path().join("input.stim");
    let missing_measurements = dir.path().join("missing.01");
    let unwritable_output = dir.path().join("missing-dir").join("output.01");
    std::fs::write(&circuit_path, "M 0\nDETECTOR rec[-1]\n").expect("write circuit");
    let mut measurement_stdout = Vec::new();
    let mut measurement_stderr = Vec::new();
    let measurement_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            missing_measurements.clone().into_os_string(),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
            OsString::from("--circuit"),
            circuit_path.into_os_string(),
        ],
        b"".as_slice(),
        &mut measurement_stdout,
        &mut measurement_stderr,
    );

    assert_eq!(measurement_status, 1);
    assert_eq!(String::from_utf8(measurement_stdout).unwrap(), "");
    let measurement_error = String::from_utf8(measurement_stderr).unwrap();
    assert!(
        measurement_error.contains("failed to read"),
        "{measurement_error}"
    );
    assert!(
        measurement_error.contains("missing.01"),
        "{measurement_error}"
    );
    assert!(
        !measurement_error.contains("failed to write"),
        "{measurement_error}"
    );
    assert!(!unwritable_output.exists());
}

#[test]
fn m2d_path_io_opens_paths_before_converter_setup() {
    let dir = tempdir().expect("temp dir");
    let invalid_converter_circuit = dir.path().join("invalid-converter.stim");
    std::fs::write(&invalid_converter_circuit, "DETECTOR rec[-1]\n").expect("write circuit");

    let missing_measurements = dir.path().join("missing.01");
    let unwritable_output = dir.path().join("missing-output-dir").join("output.01");
    let mut missing_input_stdout = Vec::new();
    let mut missing_input_stderr = Vec::new();
    let missing_input_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            missing_measurements.clone().into_os_string(),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
            OsString::from("--circuit"),
            invalid_converter_circuit.clone().into_os_string(),
        ],
        b"".as_slice(),
        &mut missing_input_stdout,
        &mut missing_input_stderr,
    );

    assert_eq!(missing_input_status, 1);
    assert_eq!(String::from_utf8(missing_input_stdout).unwrap(), "");
    let missing_input_error = String::from_utf8(missing_input_stderr).unwrap();
    assert!(
        missing_input_error.contains("failed to read"),
        "{missing_input_error}"
    );
    assert!(
        missing_input_error.contains("missing.01"),
        "{missing_input_error}"
    );
    assert!(
        !missing_input_error.contains("rec[-1]"),
        "{missing_input_error}"
    );
    assert!(!unwritable_output.exists());

    let measurements = dir.path().join("measurements.01");
    let unwritable_output = dir
        .path()
        .join("still-missing-output-dir")
        .join("output.01");
    std::fs::write(&measurements, "").expect("write empty measurements");
    let mut unwritable_output_stdout = Vec::new();
    let mut unwritable_output_stderr = Vec::new();
    let unwritable_output_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            measurements.into_os_string(),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
            OsString::from("--circuit"),
            invalid_converter_circuit.into_os_string(),
        ],
        b"".as_slice(),
        &mut unwritable_output_stdout,
        &mut unwritable_output_stderr,
    );

    assert_eq!(unwritable_output_status, 1);
    assert_eq!(String::from_utf8(unwritable_output_stdout).unwrap(), "");
    let unwritable_output_error = String::from_utf8(unwritable_output_stderr).unwrap();
    assert!(
        unwritable_output_error.contains("failed to write"),
        "{unwritable_output_error}"
    );
    assert!(
        unwritable_output_error.contains("output.01"),
        "{unwritable_output_error}"
    );
    assert!(
        !unwritable_output_error.contains("rec[-1]"),
        "{unwritable_output_error}"
    );
    assert!(!unwritable_output.exists());
}

#[test]
fn m2d_path_io_opens_all_inputs_before_outputs_and_preflights_output_batch() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    let measurement_path = dir.path().join("measurements.01");
    let missing_sweep = dir.path().join("missing-sweep.01");
    let unwritable_output = dir.path().join("missing-output-dir").join("output.01");
    std::fs::write(
        &circuit_path,
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(&measurement_path, "0\n").expect("write measurements");
    let mut output_stdout = Vec::new();
    let mut output_stderr = Vec::new();
    let output_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            measurement_path.clone().into_os_string(),
            OsString::from("--out"),
            unwritable_output.clone().into_os_string(),
            OsString::from("--sweep"),
            missing_sweep.clone().into_os_string(),
            OsString::from("--circuit"),
            circuit_path.clone().into_os_string(),
        ],
        b"".as_slice(),
        &mut output_stdout,
        &mut output_stderr,
    );

    assert_eq!(output_status, 1);
    assert_eq!(String::from_utf8(output_stdout).unwrap(), "");
    let output_error = String::from_utf8(output_stderr).unwrap();
    assert!(output_error.contains("failed to read"), "{output_error}");
    assert!(output_error.contains("missing-sweep"), "{output_error}");
    assert!(!output_error.contains("output.01"), "{output_error}");

    let sweep_path = dir.path().join("sweep.01");
    let output_path = dir.path().join("output.01");
    let unwritable_obs = dir.path().join("missing-obs-dir").join("obs.01");
    std::fs::write(&sweep_path, "0\n").expect("write sweep");
    std::fs::write(&output_path, "keep\n").expect("seed primary output");
    let mut obs_stdout = Vec::new();
    let mut obs_stderr = Vec::new();
    let obs_status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            measurement_path.into_os_string(),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
            OsString::from("--sweep"),
            sweep_path.into_os_string(),
            OsString::from("--obs_out"),
            unwritable_obs.clone().into_os_string(),
            OsString::from("--obs_out_format=01"),
            OsString::from("--circuit"),
            circuit_path.into_os_string(),
        ],
        b"not-used".as_slice(),
        &mut obs_stdout,
        &mut obs_stderr,
    );

    assert_eq!(obs_status, 1);
    assert_eq!(String::from_utf8(obs_stdout).unwrap(), "");
    let obs_error = String::from_utf8(obs_stderr).unwrap();
    assert!(obs_error.contains("failed to write"), "{obs_error}");
    assert!(obs_error.contains("obs.01"), "{obs_error}");
    assert_eq!(
        std::fs::read_to_string(output_path).expect("read preserved m2d output"),
        "keep\n"
    );
}

#[test]
fn m2d_converter_admission_precedes_output_truncation() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("over-limit.stim");
    let measurement_path = dir.path().join("measurements.01");
    let output_path = dir.path().join("output.01");
    let obs_path = dir.path().join("observables.01");
    std::fs::write(&circuit_path, "REPEAT 1000001 {\nTICK\n}\n").expect("write circuit");
    std::fs::write(&measurement_path, "").expect("write measurements");
    std::fs::write(&output_path, "primary sentinel\n").expect("seed primary output");
    std::fs::write(&obs_path, "observable sentinel\n").expect("seed observable output");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--in"),
            measurement_path.into_os_string(),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
            OsString::from("--obs_out"),
            obs_path.clone().into_os_string(),
            OsString::from("--obs_out_format=01"),
            OsString::from("--circuit"),
            circuit_path.into_os_string(),
        ],
        b"not-used".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    let error = String::from_utf8(stderr).expect("UTF-8 diagnostic");
    assert!(
        error.contains("1000001 expanded instructions; current limit is 1000000"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_to_string(output_path).expect("read primary sentinel"),
        "primary sentinel\n"
    );
    assert_eq!(
        std::fs::read_to_string(obs_path).expect("read observable sentinel"),
        "observable sentinel\n"
    );
}

#[test]
fn m2d_skip_reference_still_validates_before_output_truncation() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("invalid-control.stim");
    let measurement_path = dir.path().join("measurements.01");
    let output_path = dir.path().join("output.dets");
    let obs_path = dir.path().join("observables.01");
    std::fs::write(&circuit_path, "CX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");
    std::fs::write(&measurement_path, "0\n").expect("write measurements");
    std::fs::write(&output_path, "primary sentinel\n").expect("seed primary output");
    std::fs::write(&obs_path, "observable sentinel\n").expect("seed observable output");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=dets"),
            OsString::from("--skip_reference_sample"),
            OsString::from("--in"),
            measurement_path.into_os_string(),
            OsString::from("--out"),
            output_path.clone().into_os_string(),
            OsString::from("--obs_out"),
            obs_path.clone().into_os_string(),
            OsString::from("--obs_out_format=01"),
            OsString::from("--circuit"),
            circuit_path.into_os_string(),
        ],
        b"not-used".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(
        String::from_utf8(stderr)
            .expect("UTF-8 diagnostic")
            .contains("CX target shape")
    );
    assert_eq!(
        std::fs::read_to_string(output_path).unwrap(),
        "primary sentinel\n"
    );
    assert_eq!(
        std::fs::read_to_string(obs_path).unwrap(),
        "observable sentinel\n"
    );
}
