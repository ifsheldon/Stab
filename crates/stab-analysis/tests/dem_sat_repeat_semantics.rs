#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "the independent SAT oracle uses compact parser assertions"
)]

use std::collections::BTreeSet;

use stab_analysis::{
    AnalysisResult, ResourceKind, ResourceOperation, SatMaterializationLimits,
    likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
};
use stab_model::DetectorErrorModel;

#[path = "support/sat_wcnf.rs"]
mod sat_wcnf;

const QUANTIZATION: u32 = 100;

#[derive(Clone, Copy, Debug)]
struct DirectError {
    probability: f64,
    detector: Option<u64>,
    observable: bool,
}

#[derive(Clone, Copy)]
struct TargetPattern {
    dem: &'static str,
    detector: bool,
    observable: bool,
}

const TARGET_PATTERNS: [TargetPattern; 3] = [
    TargetPattern {
        dem: "D0",
        detector: true,
        observable: false,
    },
    TargetPattern {
        dem: "L0",
        detector: false,
        observable: true,
    },
    TargetPattern {
        dem: "D0 L0",
        detector: true,
        observable: true,
    },
];

const PROBABILITIES: [(f64, &str); 5] = [
    (0.0, "0"),
    (0.1, "0.1"),
    (0.5, "0.5"),
    (0.9, "0.9"),
    (1.0, "1"),
];

fn direct_optimum(errors: &[DirectError]) -> Option<usize> {
    assert!(errors.len() <= 5);
    let max_weight = errors
        .iter()
        .filter_map(|error| soft_weight(error.probability))
        .fold(0.0_f64, f64::max);
    let mut optimum = None;
    for assignment in 0..(1usize << errors.len()) {
        let mut detectors = BTreeSet::new();
        let mut observable = false;
        let mut cost = 0usize;
        let mut feasible = true;
        for (index, error) in errors.iter().enumerate() {
            let selected = assignment & (1usize << index) != 0;
            if (error.probability == 0.0 && selected) || (error.probability == 1.0 && !selected) {
                feasible = false;
                break;
            }
            if let Some(weight) = soft_weight(error.probability) {
                let preferred = error.probability > 0.5;
                if selected != preferred {
                    let quantized = (weight / max_weight * f64::from(QUANTIZATION)).round();
                    cost += format!("{quantized:.0}")
                        .parse::<usize>()
                        .expect("finite nonnegative direct weight");
                }
            }
            if selected {
                if let Some(detector) = error.detector
                    && !detectors.insert(detector)
                {
                    detectors.remove(&detector);
                }
                observable ^= error.observable;
            }
        }
        if feasible && detectors.is_empty() && observable {
            optimum = Some(optimum.map_or(cost, |current: usize| current.min(cost)));
        }
    }
    optimum
}

fn soft_weight(probability: f64) -> Option<f64> {
    if probability <= 0.0 || probability == 0.5 || probability >= 1.0 {
        return None;
    }
    let odds = if probability < 0.5 {
        probability / (1.0 - probability)
    } else {
        (1.0 - probability) / probability
    };
    Some(-odds.ln())
}

fn nested_repeat_case(
    probability: f64,
    probability_text: &str,
    outer_count: u64,
    inner_count: u64,
    detector_shift: u64,
    target: TargetPattern,
) -> (String, String, Vec<DirectError>) {
    let compact = format!(
        "repeat {outer_count} {{\n    repeat {inner_count} {{\n        error({probability_text}) {}\n        shift_detectors {detector_shift}\n    }}\n}}\nerror(0.1) D0 L0\nerror(0.9) D0\n",
        target.dem
    );
    let mut detector_offset = 0;
    let mut errors = Vec::new();
    for _ in 0..outer_count {
        for _ in 0..inner_count {
            errors.push(DirectError {
                probability,
                detector: target.detector.then_some(detector_offset),
                observable: target.observable,
            });
            detector_offset += detector_shift;
        }
    }
    errors.push(DirectError {
        probability: 0.1,
        detector: Some(detector_offset),
        observable: true,
    });
    errors.push(DirectError {
        probability: 0.9,
        detector: Some(detector_offset),
        observable: false,
    });

    let mut unrolled = String::new();
    for error in &errors {
        let probability_text = PROBABILITIES
            .iter()
            .find_map(|&(value, text)| (value == error.probability).then_some(text))
            .expect("known probability");
        unrolled.push_str(&format!("error({probability_text})"));
        if let Some(detector) = error.detector {
            unrolled.push_str(&format!(" D{detector}"));
        }
        if error.observable {
            unrolled.push_str(" L0");
        }
        unrolled.push('\n');
    }
    (compact, unrolled, errors)
}

fn dem(source: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(source).expect("valid semantic SAT fixture")
}

#[test]
fn likeliest_wcnf_repeat_semantics_match_direct_exhaustion() -> AnalysisResult<()> {
    let mut cases = 0;
    for &(probability, probability_text) in &PROBABILITIES {
        for outer_count in 0..=3 {
            for inner_count in 0..=3 {
                if outer_count * inner_count > 3 {
                    continue;
                }
                for detector_shift in 0..=1 {
                    for target in TARGET_PATTERNS {
                        let (compact, unrolled, errors) = nested_repeat_case(
                            probability,
                            probability_text,
                            outer_count,
                            inner_count,
                            detector_shift,
                            target,
                        );
                        let compact_wcnf = likeliest_error_sat_problem_with_limits(
                            &dem(&compact),
                            QUANTIZATION,
                            SatMaterializationLimits::default(),
                        )?;
                        let unrolled_wcnf =
                            likeliest_error_sat_problem(&dem(&unrolled), QUANTIZATION)?;
                        let expected = direct_optimum(&errors);
                        assert_eq!(
                            compact_wcnf, unrolled_wcnf,
                            "compact DEM did not preserve exact flattened WCNF:\n{compact}"
                        );
                        assert_eq!(
                            sat_wcnf::optimum(&compact_wcnf),
                            expected,
                            "compact DEM:\n{compact}"
                        );
                        assert_eq!(
                            sat_wcnf::optimum(&unrolled_wcnf),
                            expected,
                            "unrolled DEM:\n{unrolled}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 360);

    let shifted = dem("repeat 100001 {\n    error(0.1) L0\n    shift_detectors 1\n}\n");
    let first_excess = likeliest_error_sat_problem_with_limits(
        &shifted,
        QUANTIZATION,
        SatMaterializationLimits::default().with_max_repeat_iterations(100_000),
    )
    .expect_err("the first aggregate repeat-iteration excess must reject");
    let resource = first_excess
        .resource_limit_error()
        .expect("typed aggregate repeat-iteration error");
    assert_eq!(resource.operation(), ResourceOperation::SatMaterialization);
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert_eq!(resource.actual(), 100_001);
    assert_eq!(resource.limit(), 100_000);

    let admitted = likeliest_error_sat_problem_with_limits(
        &shifted,
        QUANTIZATION,
        SatMaterializationLimits::default()
            .with_max_repeat_iterations(100_001)
            .with_max_expanded_instructions(0),
    )
    .expect_err("admitted expansion should reach the next aggregate limit");
    let resource = admitted
        .resource_limit_error()
        .expect("typed expanded-instruction error");
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 1);
    assert_eq!(resource.limit(), 0);
    Ok(())
}
