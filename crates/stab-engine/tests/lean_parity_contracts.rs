#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "parity tests use compact fixture setup and exact contract failures"
)]

use std::convert::Infallible;

use stab_engine::{
    DetectionCompileError, DetectionError, DetectionRunStatus, DetectionSamplingCompiler,
    MeasurementToDetectionCompiler, RandomPolicy, ReferenceSampleMode, SamplingCompiler,
    SamplingRunStatus, Seed, ShotCount,
};
use stab_model::Circuit;
use stab_records::{
    DetectionBatchView, DetectionSink, MeasurementBatchView, MeasurementSink, PackedShotBatch,
};

const SEED: u64 = 0x5eed;

fn circuit(source: &str) -> Circuit {
    Circuit::from_stim_str(source).expect("parse circuit fixture")
}

#[derive(Default)]
struct MeasurementCollector {
    records: Vec<Vec<bool>>,
}

impl MeasurementSink for MeasurementCollector {
    type Error = Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.records.push(
                (0..batch.width().get())
                    .map(|bit| batch.get(shot, bit).expect("valid measurement coordinate"))
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn sample(source: &str, shots: u64, reference_mode: ReferenceSampleMode) -> Vec<Vec<bool>> {
    let plan = SamplingCompiler::new()
        .compile(&circuit(source))
        .expect("compile sampling fixture");
    let mut session = plan
        .session_with_reference_mode(RandomPolicy::Seeded(Seed::new(SEED)), reference_mode)
        .expect("create sampling session");
    let mut sink = MeasurementCollector::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("sample fixture");
    assert_eq!(summary.status(), SamplingRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    sink.records
}

fn ordinary_samples(source: &str, shots: u64) -> Vec<Vec<bool>> {
    sample(source, shots, ReferenceSampleMode::UseReferenceSample)
}

#[derive(Default)]
struct DetectionCollector {
    detectors: Vec<Vec<bool>>,
    observables: Vec<Vec<bool>>,
}

impl DetectionSink for DetectionCollector {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            self.detectors.push(
                (0..batch.detector_width().get())
                    .map(|bit| {
                        batch
                            .detectors()
                            .get(shot, bit)
                            .expect("valid detector coordinate")
                    })
                    .collect(),
            );
            self.observables.push(
                (0..batch.observable_width().get())
                    .map(|bit| {
                        batch
                            .observables()
                            .get(shot, bit)
                            .expect("valid observable coordinate")
                    })
                    .collect(),
            );
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn detect(source: &str, shots: u64) -> DetectionCollector {
    let plan = DetectionSamplingCompiler::new()
        .compile(&circuit(source))
        .expect("compile detection fixture");
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(SEED)))
        .expect("create detection session");
    let mut sink = DetectionCollector::default();
    let summary = session
        .run(ShotCount::new(shots), &mut sink)
        .expect("sample detection fixture");
    assert_eq!(summary.status(), DetectionRunStatus::Completed);
    assert_eq!(summary.committed_shots(), ShotCount::new(shots));
    sink
}

#[test]
fn sampling_circuit_common_measurement_and_reset_contract() {
    struct Case {
        name: &'static str,
        source: &'static str,
        expected: &'static [bool],
    }

    let cases = [
        Case {
            name: "Z measurement and inversion",
            source: "X 0\nM 0 !0\n",
            expected: &[true, false],
        },
        Case {
            name: "basis resets and measurements",
            source: "RX 0\nMX 0\nRY 1\nMY 1\nX 2\nR 2\nM 2\n",
            expected: &[false, false, false],
        },
        Case {
            name: "measure-reset families",
            source: "X 0\nMR 0\nM 0\nRX 1\nMRX 1\nMX 1\nRY 2\nMRY 2\nMY 2\n",
            expected: &[true, false, false, false, false, false],
        },
        Case {
            name: "pair-product families",
            source: "RX 0 1\nMXX 0 1\nRY 2 3\nMYY 2 3\nR 4 5\nMZZ 4 5\n",
            expected: &[false, false, false],
        },
        Case {
            name: "mixed Pauli product",
            source: "RX 0\nRY 1\nR 2\nMPP X0*Y1*Z2\n",
            expected: &[false],
        },
        Case {
            name: "measurement padding",
            source: "MPAD 0 1 1 0\n",
            expected: &[false, true, true, false],
        },
        Case {
            name: "record feedback",
            source: "X 0\nM 0\nCX rec[-1] 1\nCY rec[-1] 2\nRX 3\nCZ rec[-1] 3\nXCZ 4 rec[-1]\nYCZ 5 rec[-1]\nM 1 2 4 5\nMX 3\n",
            expected: &[true, true, true, true, true, true],
        },
        Case {
            name: "folded repeat execution",
            source: "REPEAT 2 {\n    X 0\n    M 0\n}\n",
            expected: &[true, false],
        },
    ];

    for case in cases {
        assert_eq!(
            ordinary_samples(case.source, 1),
            vec![case.expected.to_vec()],
            "{}",
            case.name
        );
    }

    let reference_fixture = "H 0\nS 0\nS 0\nH 0\nM 0\n";
    assert_eq!(
        sample(
            reference_fixture,
            1,
            ReferenceSampleMode::UseReferenceSample
        ),
        vec![vec![true]]
    );
    assert_eq!(
        sample(
            reference_fixture,
            1,
            ReferenceSampleMode::SkipReferenceSample
        ),
        vec![vec![false]]
    );
}

#[test]
fn sampling_noise_complete_contract() {
    struct BoundaryCase {
        name: &'static str,
        source: &'static str,
        expected: &'static [bool],
    }

    let boundaries = [
        BoundaryCase {
            name: "X error",
            source: "X_ERROR(1) 0\nM 0\n",
            expected: &[true],
        },
        BoundaryCase {
            name: "Y error",
            source: "Y_ERROR(1) 0\nM 0\n",
            expected: &[true],
        },
        BoundaryCase {
            name: "Z error",
            source: "RX 0\nZ_ERROR(1) 0\nMX 0\n",
            expected: &[true],
        },
        BoundaryCase {
            name: "Z and identity errors preserve Z measurements",
            source: "Z_ERROR(1) 0\nI_ERROR(1) 0\nII_ERROR(1) 0 1\nM 0 1\n",
            expected: &[false, false],
        },
        BoundaryCase {
            name: "zero depolarization",
            source: "DEPOLARIZE1(0) 0\nDEPOLARIZE2(0) 0 1\nM 0 1\n",
            expected: &[false, false],
        },
        BoundaryCase {
            name: "single-qubit Pauli channel branches",
            source: "PAULI_CHANNEL_1(1,0,0) 0\nPAULI_CHANNEL_1(0,1,0) 1\nRX 2\nPAULI_CHANNEL_1(0,0,1) 2\nM 0 1\nMX 2\n",
            expected: &[true, true, true],
        },
        BoundaryCase {
            name: "correlated and else-correlated errors",
            source: "E(1) X0 Z1\nE(0) X2\nELSE_CORRELATED_ERROR(1) X3\nELSE_CORRELATED_ERROR(1) X4\nM 0 1 2 3 4\n",
            expected: &[true, false, false, true, false],
        },
        BoundaryCase {
            name: "heralded channel boundaries",
            source: "HERALDED_ERASE(0) 0\nHERALDED_PAULI_CHANNEL_1(1,0,0,0) 1\nHERALDED_PAULI_CHANNEL_1(0,1,0,0) 2\nHERALDED_PAULI_CHANNEL_1(0,0,1,0) 3\nRX 4\nHERALDED_PAULI_CHANNEL_1(0,0,0,1) 4\nM 0 1 2 3\nMX 4\n",
            expected: &[
                false, true, true, true, true, false, false, true, true, true,
            ],
        },
        BoundaryCase {
            name: "measurement flip families",
            source: "M(1) 0\nRX 1\nMX(1) 1\nRY 2\nMY(1) 2\nRX 3 4\nMXX(1) 3 4\nRY 5 6\nMYY(1) 5 6\nR 7 8\nMZZ(1) 7 8\nRX 9\nMPP(1) X9\nMPAD(1) 0 1\n",
            expected: &[true, true, true, true, true, true, true, true, false],
        },
        BoundaryCase {
            name: "measure-reset flip families",
            source: "MR(1) 0\nM 0\nRX 1\nMRX(1) 1\nMX 1\nRY 2\nMRY(1) 2\nMY 2\n",
            expected: &[true, false, true, false, true, false],
        },
    ];

    for case in boundaries {
        assert_eq!(
            ordinary_samples(case.source, 1),
            vec![case.expected.to_vec()],
            "{}",
            case.name
        );
    }

    const SHOTS: u64 = 8_192;
    const MAX_ABSOLUTE_ERROR: f64 = 0.04;
    // Hoeffding gives at most 2*exp(-2*8192*0.04^2) per assertion. Across this
    // matrix, the family-wide false-failure budget is below 3e-10.
    struct StatisticalCase {
        name: &'static str,
        source: &'static str,
        expected_rates: &'static [f64],
    }
    let statistical_cases = [
        StatisticalCase {
            name: "Pauli errors",
            source: "X_ERROR(0.25) 0\nRX 1\nZ_ERROR(0.25) 1\nM 0\nMX 1\n",
            expected_rates: &[0.25, 0.25],
        },
        StatisticalCase {
            name: "depolarizing errors",
            source: "DEPOLARIZE1(0.3) 0\nDEPOLARIZE2(0.3) 1 2\nM 0 1 2\n",
            expected_rates: &[0.2, 0.16, 0.16],
        },
        StatisticalCase {
            name: "single Pauli channel",
            source: "PAULI_CHANNEL_1(0.1,0.2,0.3) 0\nM 0\n",
            expected_rates: &[0.3],
        },
        StatisticalCase {
            name: "two-qubit Pauli channel",
            source: "PAULI_CHANNEL_2(0,0,0,0,0.25,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n",
            expected_rates: &[0.25, 0.25],
        },
        StatisticalCase {
            name: "correlated chain",
            source: "E(0.5) X0\nELSE_CORRELATED_ERROR(0.25) X1\nELSE_CORRELATED_ERROR(0.75) X2\nM 0 1 2\n",
            expected_rates: &[0.5, 0.125, 0.28125],
        },
        StatisticalCase {
            name: "heralded erase",
            source: "HERALDED_ERASE(0.4) 0\nM 0\n",
            expected_rates: &[0.4, 0.2],
        },
        StatisticalCase {
            name: "heralded Pauli channel",
            source: "HERALDED_PAULI_CHANNEL_1(0.05,0.1,0.15,0.25) 0\nM 0\n",
            expected_rates: &[0.55, 0.25],
        },
        StatisticalCase {
            name: "measurement and measure-reset flips",
            source: "M(0.25) 0\nRX 1\nMX(0.25) 1\nRY 2\nMY(0.25) 2\nRX 3 4\nMXX(0.25) 3 4\nRY 5 6\nMYY(0.25) 5 6\nR 7 8\nMZZ(0.25) 7 8\nRX 9\nMPP(0.25) X9\nMPAD(0.25) 0 1\nMR(0.25) 10\nM 10\nRX 11\nMRX(0.25) 11\nMX 11\nRY 12\nMRY(0.25) 12\nMY 12\n",
            expected_rates: &[
                0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.75, 0.25, 0.0, 0.25, 0.0, 0.25,
                0.0,
            ],
        },
    ];

    for case in statistical_cases {
        let records = ordinary_samples(case.source, SHOTS);
        for (bit, &expected_rate) in case.expected_rates.iter().enumerate() {
            let hits = records
                .iter()
                .filter(|record| {
                    record
                        .get(bit)
                        .copied()
                        .expect("statistical fixture produced its declared record width")
                })
                .count();
            let observed_rate = hits as f64 / SHOTS as f64;
            if expected_rate == 0.0 || expected_rate == 1.0 {
                assert_eq!(observed_rate, expected_rate, "{} bit {bit}", case.name);
            } else {
                assert!(
                    (observed_rate - expected_rate).abs() <= MAX_ABSOLUTE_ERROR,
                    "{} bit {bit}: expected {expected_rate:.5}, observed {observed_rate:.5}",
                    case.name
                );
            }
        }
    }

    let correlated = ordinary_samples(
        "E(0.5) X0\nELSE_CORRELATED_ERROR(0.25) X1\nELSE_CORRELATED_ERROR(0.75) X2\nM 0 1 2\n",
        SHOTS,
    );
    assert!(
        correlated
            .iter()
            .all(|record| record.iter().filter(|&&bit| bit).count() <= 1),
        "ELSE_CORRELATED_ERROR branches must be mutually exclusive"
    );
    for (pattern, expected_rate) in [
        ([true, false, false], 0.5),
        ([false, true, false], 0.125),
        ([false, false, true], 0.28125),
        ([false, false, false], 0.09375),
    ] {
        let observed_rate = correlated
            .iter()
            .filter(|record| record.as_slice() == pattern)
            .count() as f64
            / SHOTS as f64;
        assert!(
            (observed_rate - expected_rate).abs() <= MAX_ABSOLUTE_ERROR,
            "correlated branch {pattern:?}: expected {expected_rate:.5}, observed {observed_rate:.5}"
        );
    }

    for (name, source) in [
        ("heralded erase", "HERALDED_ERASE(0.4) 0\nM 0\n"),
        (
            "heralded Pauli channel",
            "HERALDED_PAULI_CHANNEL_1(0.05,0.1,0.15,0.25) 0\nM 0\n",
        ),
    ] {
        let records = ordinary_samples(source, SHOTS);
        assert!(
            records
                .iter()
                .all(|record| record.as_slice() != [false, true]),
            "{name} produced a data error without its herald"
        );
    }

    let reproducible = "X_ERROR(0.25) 0\nM 0\n";
    assert_eq!(
        ordinary_samples(reproducible, 257),
        ordinary_samples(reproducible, 257),
        "seeded noise must be reproducible"
    );

    let expected_two_qubit_z_measurements = [
        [false, true],
        [false, true],
        [false, false],
        [true, false],
        [true, true],
        [true, true],
        [true, false],
        [true, false],
        [true, true],
        [true, true],
        [true, false],
        [false, false],
        [false, true],
        [false, true],
        [false, false],
    ];
    for (active_branch, expected) in expected_two_qubit_z_measurements.into_iter().enumerate() {
        let probabilities = (0..15)
            .map(|branch| usize::from(branch == active_branch).to_string())
            .collect::<Vec<_>>()
            .join(",");
        let source = format!("PAULI_CHANNEL_2({probabilities}) 0 1\nM 0 1\n");
        assert_eq!(
            ordinary_samples(&source, 1),
            vec![expected.to_vec()],
            "PAULI_CHANNEL_2 branch {active_branch}"
        );
    }
}

#[test]
fn detection_common_frame_gate_surface_contract() {
    struct Case {
        name: &'static str,
        source: &'static str,
        detectors: &'static [bool],
        observables: &'static [bool],
    }

    let cases = [
        Case {
            name: "Clifford and measurement bases",
            source: "R 0 7\nH 0\nZ_ERROR(1) 0\nH 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true],
            observables: &[false],
        },
        Case {
            name: "measure-reset",
            source: "R 0 7\nX_ERROR(1) 0\nMR 0\nDETECTOR rec[-1]\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, false],
            observables: &[false],
        },
        Case {
            name: "pair-product measurements",
            source: "RX 0 1\nZ_ERROR(1) 0\nMXX 0 1\nDETECTOR rec[-1]\nRY 2 3\nX_ERROR(1) 2\nMYY 2 3\nDETECTOR rec[-1]\nR 4 5 7\nX_ERROR(1) 4\nMZZ 4 5\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, true, true],
            observables: &[false],
        },
        Case {
            name: "MPP and MPAD",
            source: "RX 0\nRY 1\nR 2 7\nZ_ERROR(1) 0\nMPP X0*Y1*Z2\nDETECTOR rec[-1]\nMPAD(1) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, true, true],
            observables: &[false],
        },
        Case {
            name: "Pauli-target observable",
            source: "R 0\nX_ERROR(1) 0\nOBSERVABLE_INCLUDE(0) Z0\n",
            detectors: &[],
            observables: &[true],
        },
        Case {
            name: "feedback",
            source: "R 0 1 7\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nCX rec[-1] 1\nM 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, true],
            observables: &[false],
        },
        Case {
            name: "bounded repeat",
            source: "R 7\nREPEAT 3 {\n    R 0\n    X_ERROR(1) 0\n    M 0\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, true, true],
            observables: &[false],
        },
        Case {
            name: "heralded noise records",
            source: "R 0 7\nHERALDED_PAULI_CHANNEL_1(0,1,0,0) 0\nDETECTOR rec[-1]\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z7\n",
            detectors: &[true, true],
            observables: &[false],
        },
    ];

    for case in cases {
        let output = detect(case.source, 1);
        assert_eq!(
            output.detectors,
            vec![case.detectors.to_vec()],
            "{}",
            case.name
        );
        assert_eq!(
            output.observables,
            vec![case.observables.to_vec()],
            "{}",
            case.name
        );
    }
}

#[test]
fn detection_sweep_feedback_and_folded_conversion_contract() {
    let sweep_circuit = circuit(
        "REPEAT 3 {\n\
             R 0 1 4 5\n\
             RX 2 3\n\
             CX sweep[0] 0\n\
             CY sweep[1] 1\n\
             CZ sweep[2] 2\n\
             CZ 3 sweep[3]\n\
             XCZ 4 sweep[4]\n\
             YCZ 5 sweep[5]\n\
             M 0 1\n\
             MX 2 3\n\
             M 4 5\n\
             DETECTOR rec[-6]\n\
             DETECTOR rec[-5]\n\
             DETECTOR rec[-4]\n\
             DETECTOR rec[-3]\n\
             DETECTOR rec[-2]\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(0) rec[-6]\n\
             OBSERVABLE_INCLUDE(1) rec[-5]\n\
             OBSERVABLE_INCLUDE(2) rec[-4]\n\
             OBSERVABLE_INCLUDE(3) rec[-3]\n\
             OBSERVABLE_INCLUDE(4) rec[-2]\n\
             OBSERVABLE_INCLUDE(5) rec[-1]\n\
         }\n",
    );
    let plan = MeasurementToDetectionCompiler::new()
        .compile(&sweep_circuit)
        .expect("compile sweep conversion");
    let measurements = PackedShotBatch::from_records(&[vec![false; 18], vec![false; 18]], 18)
        .expect("pack measurements");

    let mut omitted_session = plan.session().expect("create omitted-sweep session");
    let mut omitted = DetectionCollector::default();
    omitted_session
        .run(
            MeasurementBatchView::new(measurements.view()),
            None,
            &mut omitted,
        )
        .expect("convert with omitted all-false sweeps");
    assert_eq!(omitted.detectors, vec![vec![false; 18]; 2]);
    assert_eq!(omitted.observables, vec![vec![false; 6]; 2]);

    let sweeps = PackedShotBatch::from_records(&[vec![false; 6], vec![true; 6]], 6)
        .expect("pack sweep records");
    let mut explicit_session = plan.session().expect("create explicit-sweep session");
    let mut explicit = DetectionCollector::default();
    explicit_session
        .run(
            MeasurementBatchView::new(measurements.view()),
            Some(MeasurementBatchView::new(sweeps.view())),
            &mut explicit,
        )
        .expect("convert with per-shot sweeps");
    assert_eq!(explicit.detectors, vec![vec![false; 18], vec![true; 18]]);
    assert_eq!(explicit.observables, vec![vec![false; 6], vec![true; 6]]);

    let pauli_circuit = circuit(
        "R 0 1 2 3\n\
         CZ sweep[0] 0\n\
         OBSERVABLE_INCLUDE(0) X0\n\
         CX sweep[1] 1\n\
         OBSERVABLE_INCLUDE(1) Z1\n\
         CX sweep[2] 2\n\
         OBSERVABLE_INCLUDE(2) Y2\n\
         REPEAT 3 {\n\
             CX sweep[3] 3\n\
         }\n\
         OBSERVABLE_INCLUDE(3) Z3\n",
    );
    let empty_measurements =
        PackedShotBatch::from_records(&[Vec::new(), Vec::new()], 0).expect("pack empty records");
    let pauli_sweeps = PackedShotBatch::from_records(
        &[
            vec![false, false, false, false],
            vec![true, true, true, true],
        ],
        4,
    )
    .expect("pack Pauli-observable sweeps");
    for mode in [
        ReferenceSampleMode::UseReferenceSample,
        ReferenceSampleMode::SkipReferenceSample,
    ] {
        let plan = MeasurementToDetectionCompiler::new()
            .reference_sample_mode(mode)
            .compile(&pauli_circuit)
            .expect("compile sweep Pauli-observable conversion");
        let mut session = plan
            .session()
            .expect("create sweep Pauli-observable session");
        let mut output = DetectionCollector::default();
        session
            .run(
                MeasurementBatchView::new(empty_measurements.view()),
                Some(MeasurementBatchView::new(pauli_sweeps.view())),
                &mut output,
            )
            .expect("convert sweep Pauli observables");
        assert_eq!(output.detectors, vec![Vec::new(), Vec::new()]);
        assert_eq!(
            output.observables,
            vec![vec![false; 4], vec![true; 4]],
            "{mode:?}"
        );
    }

    for (gate, preparation, measurement) in [
        ("CX rec[-1] 1", "R 1", "M 1"),
        ("CY rec[-1] 1", "R 1", "M 1"),
        ("CZ rec[-1] 1", "RX 1", "MX 1"),
        ("CZ 1 rec[-1]", "RX 1", "MX 1"),
        ("XCZ 1 rec[-1]", "R 1", "M 1"),
        ("YCZ 1 rec[-1]", "R 1", "M 1"),
    ] {
        for active in [false, true] {
            let noise = if active { "X_ERROR(1) 0\n" } else { "" };
            let source = format!(
                "R 0 2\n{preparation}\n{noise}M 0\nDETECTOR rec[-1]\n{gate}\n{measurement}\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z2\n"
            );
            let output = detect(&source, 1);
            assert_eq!(output.detectors, vec![vec![active, active]], "{gate}");
            assert_eq!(output.observables, vec![vec![false]], "{gate}");

            let deterministic_flip = if active { "X 0\n" } else { "" };
            let conversion_source = format!(
                "R 0 2\n{preparation}\n{deterministic_flip}M 0\nDETECTOR rec[-1]\n{gate}\n{measurement}\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z2\n"
            );
            let plan = MeasurementToDetectionCompiler::new()
                .compile(&circuit(&conversion_source))
                .expect("compile feedback conversion");
            let measurements =
                PackedShotBatch::from_records(&[vec![active, active], vec![active, !active]], 2)
                    .expect("pack feedback measurements");
            let mut session = plan.session().expect("create feedback conversion session");
            let mut converted = DetectionCollector::default();
            session
                .run(
                    MeasurementBatchView::new(measurements.view()),
                    None,
                    &mut converted,
                )
                .expect("convert feedback measurements");
            assert_eq!(
                converted.detectors,
                vec![vec![false, false], vec![false, true]],
                "{gate} active={active}"
            );
            assert_eq!(
                converted.observables,
                vec![vec![false], vec![false]],
                "{gate} active={active}"
            );
        }
    }

    let folded_feedback = circuit(
        "R 0 1\n\
         X 0\n\
         M 0\n\
         REPEAT 3 {\n\
             CX rec[-1] 1\n\
             M 1\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
         }\n",
    );
    let folded_plan = MeasurementToDetectionCompiler::new()
        .compile(&folded_feedback)
        .expect("compile folded feedback conversion");
    let folded_measurements = PackedShotBatch::from_records(
        &[
            vec![true, true, false, false],
            vec![true, true, true, false],
        ],
        4,
    )
    .expect("pack folded feedback measurements");
    let mut folded_session = folded_plan
        .session()
        .expect("create folded feedback session");
    let mut folded_output = DetectionCollector::default();
    folded_session
        .run(
            MeasurementBatchView::new(folded_measurements.view()),
            None,
            &mut folded_output,
        )
        .expect("convert folded feedback measurements");
    assert_eq!(
        folded_output.detectors,
        vec![vec![false, false, false], vec![false, true, false]]
    );
    assert_eq!(folded_output.observables, vec![vec![false], vec![true]]);

    let classical_cz = circuit(
        "R 0 1\n\
         X 0\n\
         M 0 0\n\
         REPEAT 2 {\n\
             CZ rec[-1] sweep[0]\n\
             CZ sweep[0] rec[-1]\n\
             CZ rec[-1] rec[-2]\n\
             CZ sweep[0] sweep[1]\n\
         }\n\
         M 1\n\
         DETECTOR rec[-1]\n",
    );
    let noop_plan = MeasurementToDetectionCompiler::new()
        .compile(&classical_cz)
        .expect("compile all-classical CZ conversion");
    let noop_measurements = PackedShotBatch::from_records(&[vec![true, true, false]], 3)
        .expect("pack no-op measurements");
    let noop_sweeps =
        PackedShotBatch::from_records(&[vec![true, true]], 2).expect("pack no-op sweeps");
    let mut noop_session = noop_plan.session().expect("create no-op session");
    let mut noop_output = DetectionCollector::default();
    noop_session
        .run(
            MeasurementBatchView::new(noop_measurements.view()),
            Some(MeasurementBatchView::new(noop_sweeps.view())),
            &mut noop_output,
        )
        .expect("convert all-classical CZ circuit");
    assert_eq!(noop_output.detectors, vec![vec![false]]);

    let invalid_nested_reference = circuit(
        "REPEAT 2 {\n\
             REPEAT 2 {\n\
                 M 0\n\
                 DETECTOR rec[-2]\n\
             }\n\
         }\n",
    );
    let error = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
        .compile(&invalid_nested_reference)
        .expect_err("a lookback available only after a later iteration must reject");
    assert!(
        matches!(
            error,
            DetectionCompileError::InvalidCircuit(
                DetectionError::InvalidResultFormat { ref message }
            ) if message.contains("rec[-2]") && message.contains("not available")
        ),
        "unexpected nested lookback error: {error}"
    );
}
