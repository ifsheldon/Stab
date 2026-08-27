use super::*;
use std::ffi::OsString;

use super::support::PinnedStimProgram;
use crate::ensure_stim_binary;
use crate::process::run_process;

#[test]
#[ignore = "builds and executes the pinned Stim CLI"]
fn pinned_stim_convert_ptb64_routes_reproduce_single_record_writer_bug() {
    let root = RepoRoot::resolve(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root"),
    )
    .expect("repository root");
    let stim = ensure_stim_binary(&root, false).expect("pinned Stim binary");
    let input = b"0\n".repeat(64);
    let primary_args = [
        "convert",
        "--in_format=01",
        "--out_format=ptb64",
        "--bits_per_shot=1",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    let primary = run_process(&stim, &primary_args, &input, Some(&root.path))
        .expect("execute primary-output reproduction");
    assert!(!primary.success());
    assert!(
        String::from_utf8_lossy(&primary.stderr.bytes)
            .contains("SAMPLE_FORMAT_PTB64 incompatible with SingleMeasurementRecord")
    );

    let directory = tempfile::tempdir().expect("observable output directory");
    let observable_path = directory.path().join("observables.ptb64");
    let observable_args = vec![
        OsString::from("convert"),
        OsString::from("--in_format=01"),
        OsString::from("--out_format=01"),
        OsString::from("--num_observables=1"),
        OsString::from("--obs_out_format=ptb64"),
        OsString::from("--obs_out"),
        observable_path.into_os_string(),
    ];
    let observable = run_process(&stim, &observable_args, &input, Some(&root.path))
        .expect("execute observable-output reproduction");
    assert!(!observable.success());
    assert!(
        String::from_utf8_lossy(&observable.stderr.bytes)
            .contains("SAMPLE_FORMAT_PTB64 incompatible with SingleMeasurementRecord")
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_circuit_equality_ignores_repeat_tags() {
    run_pinned_stim_circuit_probe(
        "repeat-tag",
        br#"
#include "stim/circuit/circuit.h"

int main() {
    stim::Circuit left("REPEAT[left] 2 {\nX_ERROR(0.1) 0\n}\n");
    stim::Circuit right("REPEAT[right] 2 {\nX_ERROR(0.1) 0\n}\n");
    return left == right && left.approx_equals(right, 0) ? 0 : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_circuit_equality_retains_orphaned_repeat_storage() {
    run_pinned_stim_circuit_probe(
        "orphaned-repeat-storage",
        br#"
#include "stim/circuit/circuit.h"

int main() {
    stim::Circuit popped("REPEAT 2 {\nX_ERROR(0.1) 0\n}\n");
    stim::Circuit empty;
    popped.operations.clear();
    return popped != empty && !popped.approx_equals(empty, 0) ? 0 : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_count_qubits_includes_mpad_values() {
    run_pinned_stim_circuit_probe(
        "count-qubits-mpad",
        br#"
#include "stim/circuit/circuit.h"

int main() {
    stim::Circuit mixed("H 0\nMPAD 1\n");
    stim::Circuit pads("MPAD 0 1 0\n");
    return mixed.count_qubits() == 2 && pads.count_qubits() == 2 ? 0 : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_inverse_qec_misindexes_duplicate_reset_flows() {
    run_pinned_stim_circuit_probe(
        "inverse-qec-duplicate-reset-flow",
        br#"
#include <vector>

#include "stim/util_top/circuit_inverse_qec.h"

int main() {
    std::vector<stim::Flow<64>> flows{
        stim::Flow<64>::from_str("1 -> Z0"),
    };
    auto result = stim::circuit_inverse_qec<64>(stim::Circuit("R 0 0\n"), flows);
    return result.first == stim::Circuit("M 0 0\n") &&
                   result.second == std::vector<stim::Flow<64>>{
                       stim::Flow<64>::from_str("Z -> rec[-4] xor rec[-3]"),
                   }
               ? 0
               : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_inverse_qec_rewrites_negative_zero_records() {
    run_pinned_stim_circuit_probe(
        "inverse-qec-negative-zero",
        br#"
#include "stim/util_top/circuit_inverse_qec.h"

int main() {
    auto detector = stim::circuit_inverse_qec<64>(
        stim::Circuit("R 0\nM 0\nDETECTOR rec[-0]\n"), {});
    auto measure_reset = stim::circuit_inverse_qec<64>(
        stim::Circuit("R 0\nM 0\nMR 0\nDETECTOR rec[-0]\n"), {});
    return detector.first == stim::Circuit("M 0 0\n") &&
                   measure_reset.first == stim::Circuit("MR 0\nM 0 0\n")
               ? 0
               : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_inverse_qec_silently_drops_flow_observables() {
    run_pinned_stim_circuit_probe(
        "inverse-qec-dropped-flow-observable",
        br#"
#include <vector>

#include "stim/util_top/circuit_inverse_qec.h"

int main() {
    std::vector<stim::Flow<64>> flows{
        stim::Flow<64>::from_str("1 -> Z0 xor obs[0]"),
    };
    auto result = stim::circuit_inverse_qec<64>(stim::Circuit("R 0\n"), flows);
    return result.first == stim::Circuit("M 0\n") &&
                   result.second == std::vector<stim::Flow<64>>{
                       stim::Flow<64>::from_str("Z0 -> rec[-1]"),
                   }
               ? 0
               : 1;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_weighted_wcnf_emits_false_sentinels_for_sparse_zero_probability_targets() {
    run_pinned_stim_circuit_probe(
        "weighted-wcnf-zero-probability-sentinel",
        br#"
#include <string>

#include "stim/search/sat/wcnf.h"

int main() {
    auto actual = stim::likeliest_error_sat_problem(stim::DetectorErrorModel(
        "error(0) D9 L3\n"
        "error(0.1) D0 L0\n"
        "error(0.1) D0\n"), 10);
    return actual.find("18446744073709551615") == std::string::npos ? 1 : 0;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes a probe against the pinned Stim library"]
fn pinned_stim_reference_signs_cover_repeats_paulis_and_zero_sweeps() {
    run_pinned_stim_circuit_probe(
        "reference-signs",
        br#"
#include "stim/circuit/circuit.h"
#include "stim/simulators/measurements_to_detection_events.h"

stim::simd_bit_table<stim::MAX_BITWORD_WIDTH> signs(const stim::Circuit &circuit) {
    stim::simd_bit_table<stim::MAX_BITWORD_WIDTH> measurements(circuit.count_measurements(), 1);
    stim::simd_bit_table<stim::MAX_BITWORD_WIDTH> sweeps(0, 1);
    return stim::measurements_to_detection_events(
        measurements,
        sweeps,
        circuit,
        true,
        false);
}

int main() {
    auto documented = signs(stim::Circuit(
        "X 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n"
        "OBSERVABLE_INCLUDE(3) rec[-1] rec[-2]\n"));
    if (!documented[0][0] || documented[1][0] || documented[2][0] ||
        documented[3][0] || documented[4][0] || !documented[5][0]) {
        return 1;
    }

    auto folded = signs(stim::Circuit(
        "X 0\nREPEAT 2 {\nM 0\nDETECTOR rec[-1]\n"
        "OBSERVABLE_INCLUDE(2) X0 rec[-1]\nX 0\n}\n"));
    if (!folded[0][0] || folded[1][0] || folded[2][0] || folded[3][0] ||
        !folded[4][0]) {
        return 2;
    }

    auto cancellation = signs(stim::Circuit(
        "X 0\nX_ERROR(1) 0\nM(1) 0\nDETECTOR rec[-1] rec[-1]\n"
        "OBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"));
    if (cancellation[0][0] || cancellation[1][0]) {
        return 3;
    }

    auto sweep = signs(stim::Circuit(
        "X 0\nCX sweep[2] 0\nM 0\nDETECTOR rec[-1]\n"));
    if (!sweep[0][0]) {
        return 4;
    }

    auto xcz = signs(stim::Circuit(
        "RX 0\nXCZ 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n"));
    auto ycz = signs(stim::Circuit(
        "RX 0\nYCZ 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n"));
    return !xcz[0][0] && !ycz[0][0] ? 0 : 5;
}
"#,
    );
}

#[test]
#[ignore = "builds and executes the pinned Stim CLI"]
fn pinned_stim_sweep_controls_cross_folded_conversion_boundaries() {
    let root = RepoRoot::resolve(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root"),
    )
    .expect("repository root");
    let stim = ensure_stim_binary(&root, false).expect("pinned Stim binary");
    let directory = tempfile::tempdir().expect("probe directory");
    let circuit = directory.path().join("folded-sweep.stim");
    let sweeps = directory.path().join("sweeps.01");
    std::fs::write(
        &circuit,
        "REPEAT 3 {\nR 0 1 4 5\nRX 2 3\nCX sweep[0] 0\nCY sweep[1] 1\nCZ sweep[2] 2\nCZ 3 sweep[3]\nXCZ 4 sweep[4]\nYCZ 5 sweep[5]\nM 0 1\nMX 2 3\nM 4 5\nDETECTOR rec[-6]\nDETECTOR rec[-5]\nDETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-6]\nOBSERVABLE_INCLUDE(1) rec[-5]\nOBSERVABLE_INCLUDE(2) rec[-4]\nOBSERVABLE_INCLUDE(3) rec[-3]\nOBSERVABLE_INCLUDE(4) rec[-2]\nOBSERVABLE_INCLUDE(5) rec[-1]\n}\n",
    )
    .expect("write circuit");
    std::fs::write(&sweeps, "000000\n111111\n").expect("write sweeps");
    let args = [
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--out_format=01"),
        OsString::from("--append_observables"),
        OsString::from("--sweep_format=01"),
        OsString::from("--sweep"),
        sweeps.into_os_string(),
        OsString::from("--circuit"),
        circuit.into_os_string(),
    ];
    let output = run_process(
        &stim,
        &args,
        b"000000000000000000\n000000000000000000\n",
        Some(&root.path),
    )
    .expect("execute pinned folded sweep conversion");
    assert!(
        output.success(),
        "{}",
        output.stderr.render_for_diagnostics()
    );
    assert_eq!(
        output.stdout.bytes,
        b"000000000000000000000000\n111111111111111111111111\n"
    );
}

#[test]
#[ignore = "builds and executes the pinned Stim CLI"]
fn pinned_stim_sweep_corrections_include_pauli_observables_in_both_reference_modes() {
    let root = RepoRoot::resolve(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root"),
    )
    .expect("repository root");
    let stim = ensure_stim_binary(&root, false).expect("pinned Stim binary");
    let directory = tempfile::tempdir().expect("probe directory");
    let circuit = directory.path().join("pauli-sweep.stim");
    let sweeps = directory.path().join("sweeps.01");
    std::fs::write(
        &circuit,
        "R 0 1 2 3\nCZ sweep[0] 0\nOBSERVABLE_INCLUDE(0) X0\nCX sweep[1] 1\nOBSERVABLE_INCLUDE(1) Z1\nCX sweep[2] 2\nOBSERVABLE_INCLUDE(2) Y2\nREPEAT 3 {\n    CX sweep[3] 3\n}\nOBSERVABLE_INCLUDE(3) Z3\n",
    )
    .expect("write Pauli-observable circuit");
    std::fs::write(&sweeps, "0000\n1111\n").expect("write sweeps");

    for skip_reference in [false, true] {
        let mut args = vec![
            OsString::from("m2d"),
            OsString::from("--in_format=01"),
            OsString::from("--out_format=01"),
            OsString::from("--append_observables"),
            OsString::from("--sweep_format=01"),
            OsString::from("--sweep"),
            sweeps.clone().into_os_string(),
            OsString::from("--circuit"),
            circuit.clone().into_os_string(),
        ];
        if skip_reference {
            args.push(OsString::from("--skip_reference_sample"));
        }
        let output = run_process(&stim, &args, b"\n\n", Some(&root.path))
            .expect("execute pinned Pauli-observable conversion");
        assert!(
            output.success(),
            "{}",
            output.stderr.render_for_diagnostics()
        );
        assert_eq!(output.stdout.bytes, b"0000\n1111\n");
    }
}

#[test]
#[ignore = "builds and executes the pinned Stim CLI"]
fn pinned_stim_feedback_transform_mistakes_a_sweep_target_for_a_qubit() {
    let root = RepoRoot::resolve(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root"),
    )
    .expect("repository root");
    let stim = ensure_stim_binary(&root, false).expect("pinned Stim binary");
    let sample_args = [
        OsString::from("sample"),
        OsString::from("--shots=1"),
        OsString::from("--out_format=01"),
    ];
    for source in [
        "CZ rec[-1] rec[-2]\nM 0\n",
        "CZ rec[-1] sweep[0]\nM 0\n",
        "CZ sweep[0] rec[-1]\nM 0\n",
        "CZ sweep[0] sweep[1]\nM 0\n",
    ] {
        let output = run_process(&stim, &sample_args, source.as_bytes(), Some(&root.path))
            .expect("execute pinned Stim no-op sample");
        assert!(
            output.success(),
            "pinned Stim rejected {source:?}: {}",
            output.stderr.render_for_diagnostics()
        );
        assert_eq!(output.stdout.bytes, b"0\n", "{source:?}");
    }

    let nested_record = run_process(
        &stim,
        &sample_args,
        b"X 0\nM 0\nREPEAT 2 {\n    REPEAT 2 {\n        R 1\n        CX rec[-1] 1\n        M 1\n    }\n}\n",
        Some(&root.path),
    )
    .expect("execute pinned Stim nested record feedback");
    assert!(nested_record.success());
    assert_eq!(nested_record.stdout.bytes, b"11111\n");

    let directory = tempfile::tempdir().expect("probe directory");
    let circuit = directory.path().join("mixed-classical-cz.stim");
    std::fs::write(
        &circuit,
        "M 0\nCZ rec[-1] sweep[1]\nMX 1\nDETECTOR rec[-1]\n",
    )
    .expect("write probe circuit");
    let common = [
        OsString::from("m2d"),
        OsString::from("--in_format=01"),
        OsString::from("--out_format=dets"),
        OsString::from("--circuit"),
        circuit.into_os_string(),
    ];
    let ordinary = run_process(&stim, &common, b"10\n", Some(&root.path))
        .expect("execute ordinary pinned conversion");
    assert!(ordinary.success());
    assert_eq!(ordinary.stdout.bytes, b"shot\n");

    let mut transformed = common.to_vec();
    transformed.push(OsString::from("--ran_without_feedback"));
    let transformed = run_process(&stim, &transformed, b"10\n", Some(&root.path))
        .expect("execute transformed pinned conversion");
    assert!(transformed.success());
    assert_eq!(transformed.stdout.bytes, b"shot D0\n");
}

fn run_pinned_stim_circuit_probe(name: &str, source: &[u8]) {
    let probe = PinnedStimProgram::compile(name, source);
    let result = probe.run(std::iter::empty::<&str>(), &[]);
    assert!(
        result.success(),
        "pinned Stim circuit-equality reproduction {name:?} failed: {}",
        result.stderr.render_for_diagnostics()
    );
}
