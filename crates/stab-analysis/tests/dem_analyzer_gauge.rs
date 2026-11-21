#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_analysis::{
    ErrorAnalyzerOptions, GaugeDetectors, LoopFolding, circuit_to_detector_error_model,
};
use stab_model::{Circuit, DemInstructionKind, DemItem};

fn analyze_allowing_gauge_detectors(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        &ErrorAnalyzerOptions {
            gauge_detectors: GaugeDetectors::Allow,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string()
}

fn analyze_error(text: &str, options: ErrorAnalyzerOptions) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, &options)
        .expect_err("analysis should reject gauge sensitivity")
        .to_string()
}

#[test]
fn dem_analyzer_rejects_gauge_observables_like_upstream() {
    let circuit = "
        R 0
        H 0
        M 0
        OBSERVABLE_INCLUDE(0) rec[-1]
    ";

    for options in [
        ErrorAnalyzerOptions::default(),
        ErrorAnalyzerOptions {
            gauge_detectors: GaugeDetectors::Allow,
            ..ErrorAnalyzerOptions::default()
        },
    ] {
        let error = analyze_error(circuit, options);
        assert!(error.contains("non-deterministic observables"));
        assert!(error.contains("L0"));
    }
}

#[test]
fn dem_analyzer_rejects_gauge_detectors_like_upstream() {
    for circuit in [
        "
        R 0
        H 0
        M 0
        DETECTOR rec[-1]
        ",
        "
        M 0
        H 0
        M 0
        DETECTOR rec[-1]
        ",
        "
        MZ 0
        MX 0
        DETECTOR rec[-1]
        ",
        "
        MY 0
        MX 0
        DETECTOR rec[-1]
        ",
        "
        MX 0
        MZ 0
        DETECTOR rec[-1]
        ",
        "
        RX 0
        MZ 0
        DETECTOR rec[-1]
        ",
        "
        RY 0
        MX 0
        DETECTOR rec[-1]
        ",
        "
        RZ 0
        MX 0
        DETECTOR rec[-1]
        ",
        "
        MX 0
        DETECTOR rec[-1]
        ",
    ] {
        let error = analyze_error(circuit, ErrorAnalyzerOptions::default());
        assert!(error.contains("non-deterministic detectors"));
        assert!(error.contains("D0"));
    }
}

#[test]
fn dem_analyzer_allows_simple_gauge_detector_variants_like_upstream() {
    for circuit in [
        "
        H 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        R 0
        H 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        RX 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        RY 0
        H_XY 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        MR 0
        H 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        MRX 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        MRY 0
        H_XY 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        M 0
        H 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        MX 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
        "
        MY 0
        H_XY 0
        CNOT 0 1
        M 0 1
        DETECTOR rec[-1]
        DETECTOR rec[-2]
        ",
    ] {
        assert_eq!(
            analyze_allowing_gauge_detectors(circuit),
            "error(0.5) D0 D1\n"
        );
    }
}

#[test]
fn dem_analyzer_multi_round_gauge_detectors_do_not_grow_upstream_subset() {
    let dem = analyze_allowing_gauge_detectors(
        "
        ZCX 0 10 1 10
        ZCX 2 11 3 11
        XCX 0 12 2 12
        XCX 1 13 3 13
        MR 10 11 12 13
        REPEAT 5 {
            ZCX 0 10 1 10
            ZCX 2 11 3 11
            XCX 0 12 2 12
            XCX 1 13 3 13
            MR 10 11 12 13
            DETECTOR rec[-1] rec[-5]
            DETECTOR rec[-2] rec[-6]
            DETECTOR rec[-3] rec[-7]
            DETECTOR rec[-4] rec[-8]
        }
        ",
    );

    assert_eq!(
        dem,
        "error(0.5) D0 D1\nerror(0.5) D2 D3\nerror(0.5) D4 D5\nerror(0.5) D6 D7\nerror(0.5) D8 D9\nerror(0.5) D10 D11\nerror(0.5) D12 D13\nerror(0.5) D14 D15\nerror(0.5) D16 D17\nerror(0.5) D18 D19\n"
    );
}

#[test]
fn dem_analyzer_noisy_multi_round_gauge_detectors_match_upstream_subset_semantics() {
    let circuit = Circuit::from_stim_str(
        "
        ZCX 0 10 1 10
        ZCX 2 11 3 11
        XCX 0 12 2 12
        XCX 1 13 3 13
        MR 10 11 12 13
        REPEAT 5 {
            DEPOLARIZE1(0.01) 0 1 2 3
            ZCX 0 10 1 10
            ZCX 2 11 3 11
            XCX 0 12 2 12
            XCX 1 13 3 13
            MR 10 11 12 13
            DETECTOR rec[-1] rec[-5]
            DETECTOR rec[-2] rec[-6]
            DETECTOR rec[-3] rec[-7]
            DETECTOR rec[-4] rec[-8]
        }
        ",
    )
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        &ErrorAnalyzerOptions {
            gauge_detectors: GaugeDetectors::Allow,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze");

    let errors = dem
        .items()
        .iter()
        .filter_map(|item| {
            let DemItem::Instruction(instruction) = item else {
                return None;
            };
            (instruction.kind() == DemInstructionKind::Error).then_some(instruction)
        })
        .collect::<Vec<_>>();

    assert_eq!(errors.len(), 50);
    assert_eq!(dem.count_detectors().expect("detector count"), 20);
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.args().first().copied() == Some(0.5))
            .count(),
        10
    );
    assert_eq!(
        errors
            .iter()
            .filter(|error| {
                error
                    .args()
                    .first()
                    .is_some_and(|probability| (probability - 0.003344519141621982).abs() < 1e-18)
            })
            .count(),
        20
    );
    assert_eq!(
        errors
            .iter()
            .filter(|error| {
                error
                    .args()
                    .first()
                    .is_some_and(|probability| (probability - (1.0 / 150.0)).abs() < 1e-16)
            })
            .count(),
        20
    );
}

fn analyze_folded_allowing_gauge_detectors(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        &ErrorAnalyzerOptions {
            gauge_detectors: GaugeDetectors::Allow,
            loops: LoopFolding::Fold,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string()
}

/// Pinned Stim resolves the products and pairs of one instruction one at a time in reverse,
/// so a later product's merged record anticommutes with an earlier product on a shared qubit.
#[test]
fn dem_analyzer_checks_each_measured_product_after_later_products_like_upstream() {
    for (circuit, expected) in [
        ("MPP X0 Z0\nDETECTOR rec[-1]\n", "error(0.5) D0\n"),
        (
            "MPP Z0 X0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
            "error(0.5) D0\ndetector D1\n",
        ),
        (
            "MXX 0 1 1 2 0 2\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nDETECTOR rec[-3]\n",
            "error(0.5) D0 D1\nerror(0.5) D0 D2\n",
        ),
    ] {
        let error = analyze_error(circuit, ErrorAnalyzerOptions::default());
        assert!(error.contains("non-deterministic detectors"), "{circuit}");
        assert_eq!(
            analyze_allowing_gauge_detectors(circuit),
            expected,
            "{circuit}"
        );
    }
    assert_eq!(
        analyze_allowing_gauge_detectors(
            "
            R 0 1
            X_ERROR(0.125) 0
            Z_ERROR(0.25) 1
            MPP X0*X1 Z0*Z1 Y0*Y1 X0 Z1
            DETECTOR rec[-1]
            DETECTOR rec[-2]
            DETECTOR rec[-3]
            DETECTOR rec[-4]
            DETECTOR rec[-5]
            DETECTOR rec[-1] rec[-5]
            ",
        ),
        "error(0.5) D0 D1 D5\nerror(0.5) D1\nerror(0.125) D2 D3\nerror(0.5) D2 D4 D5\n"
    );
}

/// Pinned Stim's `remove_gauge` rewrites only qubit frames: pending record sensitivities keep
/// the eliminated detector, which later records reintroduce, and per-target elimination order
/// decides which later gauges are found.
#[test]
fn dem_analyzer_gauge_elimination_keeps_record_sensitivities_like_upstream() {
    for (circuit, expected) in [
        (
            "
            X_ERROR(0.125) 1
            M 1
            RX 0
            M 0
            DETECTOR rec[-1]
            DETECTOR rec[-1] rec[-2]
            ",
            "error(0.5) D0 D1\nerror(0.125) D1\n",
        ),
        (
            "
            X_ERROR(0.1) 0 1
            MX 0 1
            M(0.05) 0 1
            DETECTOR rec[-1] rec[-2]
            DETECTOR rec[-2] rec[-3]
            DETECTOR rec[-1] rec[-4]
            ",
            "error(0.5) D0\nerror(0.5) D0 D1\nerror(0.5) D0 D2\nerror(0.5) D2\n",
        ),
        (
            "
            R 0 1
            X_ERROR(0.125) 0
            Z_ERROR(0.25) 1
            MPP[p] Z0*Z1 X1 Z0
            DETECTOR rec[-2]
            DETECTOR rec[-1] rec[-3]
            OBSERVABLE_INCLUDE(0) rec[-1]
            M 1
            DETECTOR rec[-1] rec[-3]
            OBSERVABLE_INCLUDE(1) rec[-4]
            ",
            "error[p](0.5) D0\nerror[p](0.5) D2\nerror(0.125) L0 L1\ndetector D1\n",
        ),
    ] {
        assert_eq!(
            analyze_allowing_gauge_detectors(circuit),
            expected,
            "{circuit}"
        );
    }
}

/// Pinned Stim passes the collapsing instruction's tag to each gauge error and leaves the
/// implicit start-of-circuit reset untagged.
#[test]
fn dem_analyzer_gauge_errors_carry_collapsing_instruction_tag_like_upstream() {
    for (circuit, expected) in [
        (
            "R[foo] 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
            "error[foo](0.5) D0\n",
        ),
        ("H[h] 0\nM[m] 0\nDETECTOR rec[-1]\n", "error(0.5) D0\n"),
        (
            "RY[y] 0\nM 0\nDETECTOR rec[-1]\nMRY[w] 0\nH 0\nM 0\nDETECTOR rec[-1]\n",
            "error[y](0.5) D0\nerror[w](0.5) D1\n",
        ),
        (
            "MX[a] 0 1\nM[b](0.25) 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nDETECTOR rec[-3] rec[-1]\n",
            "error(0.5) D0\nerror[a](0.5) D0 D2\nerror[b](0.25) D0 D2\nerror[a](0.5) D1\nerror[b](0.25) D1\n",
        ),
        (
            "R 0 1 2\nMXX[q] 0 1 1 2\nMZZ[z] 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nDETECTOR rec[-3]\n",
            "error[q](0.5) D0\nerror(0.5) D1\nerror(0.5) D2\n",
        ),
    ] {
        assert_eq!(
            analyze_allowing_gauge_detectors(circuit),
            expected,
            "{circuit}"
        );
    }
    assert_eq!(
        analyze_folded_allowing_gauge_detectors(
            "
            RX 0
            REPEAT 30 {
                Z_ERROR(0.02) 0
                MPP[t] Z0 X0
                DETECTOR rec[-1]
                DETECTOR rec[-2]
                DETECTOR rec[-1] rec[-2]
                SHIFT_COORDS(1)
            }
            M 0
            DETECTOR rec[-1]
            ",
        ),
        "error(0.5) D0 D1\nrepeat 29 {\n    error[t](0.5) D0 D2\n    error[t](0.5) D3 D4\n    shift_detectors(1) 3\n}\nerror[t](0.5) D0 D2\nerror[t](0.5) D3\ndetector D1\nshift_detectors(1) 0\n"
    );
}
