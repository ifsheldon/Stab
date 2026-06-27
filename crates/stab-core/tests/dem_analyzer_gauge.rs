#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    Circuit, DemInstructionKind, DemItem, ErrorAnalyzerOptions, circuit_to_detector_error_model,
};

fn analyze_allowing_gauge_detectors(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            allow_gauge_detectors: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string()
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
        ErrorAnalyzerOptions {
            allow_gauge_detectors: true,
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
