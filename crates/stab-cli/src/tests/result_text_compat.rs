use std::ffi::OsString;

use stab_compat_corpus::{Acceptance, CheckedCase, CheckedCorpus, ResultFormat};
use tempfile::tempdir;

use crate::run_from;

const MALFORMED_TEXT_RECORDS: &[(&str, &[u8])] = &[
    ("01", b"101"),
    ("hits", b"1,,2\n"),
    ("hits", b"1,\n"),
    ("hits", b",1\n"),
    ("hits", b"1,2"),
    ("dets", b"shotM0\n"),
    ("dets", b"shot  M0\n"),
    ("dets", b"shot M0 \n"),
    ("dets", b"shot\tM0\n"),
];

fn assert_cli_rejects(args: impl IntoIterator<Item = OsString>, input: &[u8], context: &str) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, input, &mut stdout, &mut stderr);
    assert_eq!(status, 1, "{context}");
    assert_eq!(stdout, Vec::<u8>::new(), "{context}");
    assert!(!stderr.is_empty(), "{context}");
}

#[test]
fn malformed_text_records_are_rejected_by_convert_m2d_and_replay() {
    let dir = tempdir().expect("temp dir");
    let circuit_path = dir.path().join("input.stim");
    std::fs::write(&circuit_path, "M 0 1 2\nDETECTOR rec[-1]\n").expect("write circuit");
    let circuit_arg = circuit_path.as_os_str().to_owned();
    let dem = b"error(1) D0\nerror(1) D1\nerror(1) D2\n";

    for (case_index, (format, input)) in MALFORMED_TEXT_RECORDS.iter().enumerate() {
        assert_cli_rejects(
            [
                OsString::from("stab"),
                OsString::from("convert"),
                OsString::from("--in_format"),
                OsString::from(format),
                OsString::from("--out_format"),
                OsString::from("01"),
                OsString::from("--num_measurements"),
                OsString::from("3"),
            ],
            input,
            &format!("convert case {case_index}: {input:?}"),
        );

        assert_cli_rejects(
            [
                OsString::from("stab"),
                OsString::from("m2d"),
                OsString::from("--in_format"),
                OsString::from(format),
                OsString::from("--out_format"),
                OsString::from("01"),
                OsString::from("--skip_reference_sample"),
                OsString::from("--circuit"),
                circuit_arg.clone(),
            ],
            input,
            &format!("m2d case {case_index}: {input:?}"),
        );

        let replay_path = dir.path().join(format!("replay-{case_index}.{format}"));
        std::fs::write(&replay_path, input).expect("write replay input");
        assert_cli_rejects(
            [
                OsString::from("stab"),
                OsString::from("sample_dem"),
                OsString::from("--replay_err_in"),
                replay_path.into_os_string(),
                OsString::from("--replay_err_in_format"),
                OsString::from(format),
                OsString::from("--shots"),
                OsString::from("1"),
            ],
            dem,
            &format!("sample_dem replay case {case_index}: {input:?}"),
        );
    }
}

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

        if case.format() == ResultFormat::Dets && !layout.is_measurement_only() {
            continue;
        }
        let circuit_path = dir.path().join(format!("{}.stim", case.id()));
        let width = layout.total_bits().expect("validated layout");
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
