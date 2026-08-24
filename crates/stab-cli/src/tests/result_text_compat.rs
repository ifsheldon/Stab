use std::ffi::OsString;

use stab_compat_corpus::{Acceptance, CheckedCase, CheckedCorpus};
use tempfile::tempdir;

use crate::run_from;

#[test]
fn checked_corpus_matches_convert_and_applicable_streaming_cli_paths() {
    let corpus = CheckedCorpus::parse(include_bytes!(
        "../../../../oracle/result-format-corpus.json"
    ))
    .expect("parse result-format corpus");
    let dir = tempdir().expect("temp dir");

    for case in corpus.cases() {
        let layout = case.layout();
        let convert_args = vec![
            OsString::from("stab"),
            OsString::from("convert"),
            OsString::from("--in_format"),
            OsString::from(case.format().name()),
            OsString::from("--out_format"),
            OsString::from("01"),
            OsString::from("--num_measurements"),
            OsString::from(layout.measurements().to_string()),
            OsString::from("--num_detectors"),
            OsString::from(layout.detectors().to_string()),
            OsString::from("--num_observables"),
            OsString::from(layout.observables().to_string()),
        ];
        let (status, stdout, stderr) = run_cli_owned(convert_args, case.input());
        assert_acceptance(case, "convert", status, &stderr);
        if let Some(expected) = case.canonical_01() {
            assert_eq!(stdout, expected, "{} convert output", case.id());
        }

        let Some(width) = case.measurement_only_width() else {
            continue;
        };
        let circuit_path = dir.path().join(format!("{}.stim", case.id()));
        std::fs::write(&circuit_path, measurement_circuit(width)).expect("write corpus circuit");
        let (status, _, stderr) = run_cli_owned(
            vec![
                OsString::from("stab"),
                OsString::from("m2d"),
                OsString::from("--in_format"),
                OsString::from(case.format().name()),
                OsString::from("--out_format"),
                OsString::from("01"),
                OsString::from("--skip_reference_sample"),
                OsString::from("--circuit"),
                circuit_path.into_os_string(),
            ],
            case.input(),
        );
        assert_acceptance(case, "m2d", status, &stderr);

        let replay_path = dir.path().join(format!("{}.replay", case.id()));
        std::fs::write(&replay_path, case.input()).expect("write corpus replay");
        let (status, _, stderr) = run_cli_owned(
            vec![
                OsString::from("stab"),
                OsString::from("sample_dem"),
                OsString::from("--replay_err_in"),
                replay_path.into_os_string(),
                OsString::from("--replay_err_in_format"),
                OsString::from(case.format().name()),
                OsString::from("--shots"),
                OsString::from(case.replay_shots().to_string()),
            ],
            measurement_dem(width).as_bytes(),
        );
        assert_acceptance(case, "sample_dem replay", status, &stderr);
    }
}

fn run_cli_owned(args: Vec<OsString>, input: &[u8]) -> (i32, Vec<u8>, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, input, &mut stdout, &mut stderr);
    (
        status,
        stdout,
        String::from_utf8(stderr).expect("stderr is UTF-8"),
    )
}

fn assert_acceptance(case: &CheckedCase, path: &str, status: i32, stderr: &str) {
    match case.acceptance() {
        Acceptance::Accepted => {
            assert_eq!(status, 0, "{} through {path}: {stderr}", case.id());
            assert_eq!(stderr, "", "{} through {path}", case.id());
        }
        Acceptance::Rejected => {
            assert_ne!(status, 0, "{} through {path}", case.id());
            assert!(!stderr.is_empty(), "{} through {path}", case.id());
        }
    }
}

fn measurement_circuit(width: usize) -> String {
    let mut circuit = String::from("M");
    for qubit in 0..width {
        circuit.push(' ');
        circuit.push_str(&qubit.to_string());
    }
    circuit.push_str("\nDETECTOR rec[-1]\n");
    circuit
}

fn measurement_dem(width: usize) -> String {
    let mut dem = String::new();
    for index in 0..width {
        dem.push_str(&format!("error(1) D{index}\n"));
    }
    dem
}
