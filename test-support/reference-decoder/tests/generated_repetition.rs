#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "generated-code tests use bounded independently constructed tables"
)]

use std::ops::ControlFlow;

use stab_analysis::{
    CodeDistance, ErrorAnalyzerOptions, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    circuit_to_detector_error_model, generate_repetition_code_circuit,
};
use stab_model::{
    DemErrorMechanismTraversalLimits, DemErrorMechanismView, DemErrorMechanismVisitor,
    DemErrorTarget, DetectorErrorModel, ModelError, Probability,
};
use stab_reference_decoder::{ExactMlDecodeError, ExactMlDecoderSession};
use thiserror::Error;

#[test]
fn generated_and_diagnostic_models_match_independent_oracle() {
    for distance in [3, 5] {
        let model = generated_repetition_model(distance);
        predictions_match_independent_oracle(&model, &format!("distance={distance}"));
    }

    for (label, model) in diagnostic_compile_models() {
        predictions_match_independent_oracle(&model, label);
    }
    predictions_match_independent_oracle(&diagnostic_model(14, 64), "reused-decode");
}

fn predictions_match_independent_oracle(model: &DetectorErrorModel, label: &str) {
    let session = ExactMlDecoderSession::try_compile_model(model).expect("compile exact ML");
    let oracle = IndependentProbabilityOracle::compile(model);

    assert_eq!(session.syndrome_count(), oracle.predictions.len());
    for (syndrome, expected) in oracle.predictions.iter().copied().enumerate() {
        let syndrome = u64::try_from(syndrome).expect("bounded syndrome");
        match expected {
            Some(expected) => assert_eq!(
                session
                    .prediction_for_syndrome(syndrome)
                    .expect("possible diagnostic syndrome"),
                expected,
                "{label} syndrome={syndrome}"
            ),
            None => assert!(
                matches!(
                    session.prediction_for_syndrome(syndrome),
                    Err(ExactMlDecodeError::ImpossibleSyndrome { .. })
                ),
                "{label} syndrome={syndrome}"
            ),
        }
    }
}

fn diagnostic_compile_models() -> [(&'static str, DetectorErrorModel); 3] {
    [
        ("compile-small", diagnostic_model(6, 12)),
        ("compile-medium", diagnostic_model(10, 32)),
        (
            "compile-accepted-maximum",
            DetectorErrorModel::from_dem_str("error(0) D19\nerror(0.5) L0\n")
                .expect("accepted-maximum DEM"),
        ),
    ]
}

fn diagnostic_model(detector_count: usize, mechanism_count: usize) -> DetectorErrorModel {
    let mut text = String::new();
    for mechanism in 0..mechanism_count {
        let probability_millis = 5 + (mechanism * 17) % 190;
        text.push_str(&format!("error(0.{probability_millis:03})"));
        if mechanism < detector_count {
            text.push_str(&format!(" D{mechanism}"));
            if mechanism == 0 {
                text.push_str(" L0");
            }
        } else if mechanism == detector_count {
            text.push_str(" L0");
        } else {
            let first = mechanism % detector_count;
            let second = (first + 1 + mechanism / detector_count) % detector_count;
            text.push_str(&format!(" D{first} D{second}"));
            if mechanism % 3 == 0 {
                text.push_str(" L0");
            }
        }
        text.push('\n');
    }
    DetectorErrorModel::from_dem_str(&text).expect("diagnostic DEM")
}

fn generated_repetition_model(distance: u32) -> DetectorErrorModel {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(distance).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("repetition parameters")
    .with_before_round_data_depolarization(probability(0.03125))
    .with_before_measure_flip_probability(probability(0.015625))
    .with_after_reset_flip_probability(probability(0.0078125));
    let generated = generate_repetition_code_circuit(&params).expect("generate repetition circuit");
    circuit_to_detector_error_model(generated.circuit(), ErrorAnalyzerOptions::default())
        .expect("lower repetition circuit")
}

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("probability")
}

struct IndependentProbabilityOracle {
    predictions: Vec<Option<bool>>,
}

impl IndependentProbabilityOracle {
    fn compile(model: &DetectorErrorModel) -> Self {
        let detector_count = usize::try_from(model.count_detectors().expect("detector count"))
            .expect("bounded detector count");
        let observable_count =
            usize::try_from(model.count_observables().expect("observable count"))
                .expect("bounded observable count");
        assert_eq!(observable_count, 1);
        let mut collector = EffectCollector {
            detector_count,
            effects: Vec::new(),
        };
        let traversal = model
            .try_visit_error_mechanisms(
                DemErrorMechanismTraversalLimits::new(256, 65_536),
                &mut collector,
            )
            .expect("bounded public mechanism traversal");
        assert_eq!(traversal, ControlFlow::Continue(()));

        let syndrome_count = 1_usize << detector_count;
        let state_count = syndrome_count << 1;
        let mut current = vec![0.0_f64; state_count];
        current[0] = 1.0;
        for effect in collector.effects {
            let mut next = vec![0.0_f64; state_count];
            for (state, probability) in current.iter().copied().enumerate() {
                next[state] += probability * (1.0 - effect.probability);
                next[state ^ effect.mask] += probability * effect.probability;
            }
            current = next;
        }

        let predictions = (0..syndrome_count)
            .map(|syndrome| {
                let observable_zero = current[syndrome];
                let observable_one = current[syndrome | syndrome_count];
                if observable_zero == 0.0 && observable_one == 0.0 {
                    None
                } else {
                    Some(observable_one > observable_zero)
                }
            })
            .collect();
        Self { predictions }
    }
}

#[derive(Clone, Copy)]
struct Effect {
    probability: f64,
    mask: usize,
}

struct EffectCollector {
    detector_count: usize,
    effects: Vec<Effect>,
}

impl DemErrorMechanismVisitor for EffectCollector {
    type Error = OracleError;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        let mut mask = 0_usize;
        for target in mechanism.targets() {
            match target.map_err(OracleError::Model)? {
                DemErrorTarget::Detector(detector) => {
                    let detector_id = detector.get();
                    let detector = usize::try_from(detector_id).map_err(|_| {
                        OracleError::DetectorOutOfRange {
                            detector: detector_id,
                            detector_count: self.detector_count,
                        }
                    })?;
                    if detector >= self.detector_count {
                        return Err(OracleError::DetectorOutOfRange {
                            detector: detector_id,
                            detector_count: self.detector_count,
                        });
                    }
                    mask ^= 1_usize << detector;
                }
                DemErrorTarget::Observable(observable) => {
                    if observable.get() != 0 {
                        return Err(OracleError::ObservableOutOfRange {
                            observable: observable.get(),
                        });
                    }
                    mask ^= 1_usize << self.detector_count;
                }
                DemErrorTarget::Separator => {}
            }
        }
        self.effects.push(Effect {
            probability: mechanism.probability().get(),
            mask,
        });
        Ok(ControlFlow::Continue(()))
    }
}

#[derive(Debug, Error)]
enum OracleError {
    #[error("invalid generated model target: {0}")]
    Model(#[source] ModelError),
    #[error("detector {detector} is outside generated detector count {detector_count}")]
    DetectorOutOfRange {
        detector: u64,
        detector_count: usize,
    },
    #[error("observable {observable} is outside the one-observable oracle")]
    ObservableOutOfRange { observable: u64 },
}
