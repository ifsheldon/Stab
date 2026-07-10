#![allow(
    clippy::expect_used,
    reason = "integration tests use direct assertions for compact parity diagnostics"
)]

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use stab_core::{
    Circuit, CompiledDemSampler, DemDetectorId, DemRepeatBlock, DetectorErrorModel, RepeatCount,
    explain_errors_from_circuit, find_undetectable_logical_error, likeliest_error_sat_problem,
    shortest_error_sat_problem, shortest_graphlike_undetectable_logical_error,
};

const GENERATED_DIFFERENTIAL_CASES: u32 = 96;
const GENERATED_DIFFERENTIAL_SEED: [u8; 32] = [0xB3; 32];

#[derive(Clone, Debug)]
enum GeneratedDemItem {
    Error {
        active: bool,
        shape: u8,
        detector_a: u8,
        detector_b: u8,
        observable: u8,
        tagged: bool,
    },
    Detector {
        detector: u8,
        coordinates: Vec<i8>,
        tagged: bool,
    },
    Shift {
        detectors: u8,
        coordinates: Vec<i8>,
    },
    Logical {
        observable: u8,
    },
    Repeat {
        count: u8,
        body: Vec<GeneratedDemItem>,
        tagged: bool,
    },
}

fn generated_dem_item_strategy() -> BoxedStrategy<GeneratedDemItem> {
    let coordinates = proptest::collection::vec(-1_i8..=2, 0..=3);
    let leaf = prop_oneof![
        (
            any::<bool>(),
            0_u8..6,
            0_u8..5,
            0_u8..5,
            0_u8..4,
            any::<bool>()
        )
            .prop_map(
                |(active, shape, detector_a, detector_b, observable, tagged)| {
                    GeneratedDemItem::Error {
                        active,
                        shape,
                        detector_a,
                        detector_b,
                        observable,
                        tagged,
                    }
                }
            ),
        (0_u8..5, coordinates.clone(), any::<bool>()).prop_map(
            |(detector, coordinates, tagged)| GeneratedDemItem::Detector {
                detector,
                coordinates,
                tagged,
            }
        ),
        (0_u8..3, coordinates).prop_map(|(detectors, coordinates)| {
            GeneratedDemItem::Shift {
                detectors,
                coordinates,
            }
        }),
        (0_u8..4).prop_map(|observable| GeneratedDemItem::Logical { observable }),
    ];
    leaf.prop_recursive(3, 48, 4, |inner| {
        (
            1_u8..=3,
            proptest::collection::vec(inner, 0..=4),
            any::<bool>(),
        )
            .prop_map(|(count, body, tagged)| GeneratedDemItem::Repeat {
                count,
                body,
                tagged,
            })
    })
    .boxed()
}

fn generated_dem_strategy() -> impl Strategy<Value = Vec<GeneratedDemItem>> {
    proptest::collection::vec(generated_dem_item_strategy(), 0..=6)
}

fn expand_generated_dem(items: &[GeneratedDemItem]) -> Vec<GeneratedDemItem> {
    let mut expanded = Vec::new();
    for item in items {
        match item {
            GeneratedDemItem::Repeat { count, body, .. } => {
                let expanded_body = expand_generated_dem(body);
                for _ in 0..*count {
                    expanded.extend(expanded_body.iter().cloned());
                }
            }
            item => expanded.push(item.clone()),
        }
    }
    expanded
}

fn render_generated_dem(items: &[GeneratedDemItem]) -> String {
    fn coordinate(value: i8) -> &'static str {
        match value {
            -1 => "-0.25",
            0 => "0",
            1 => "0.5",
            2 => "1.25",
            _ => unreachable!("coordinate strategy is bounded"),
        }
    }

    fn render_items(items: &[GeneratedDemItem], indent: usize, out: &mut String) {
        let prefix = "    ".repeat(indent);
        for item in items {
            match item {
                GeneratedDemItem::Error {
                    active,
                    shape,
                    detector_a,
                    detector_b,
                    observable,
                    tagged,
                } => {
                    out.push_str(&prefix);
                    out.push_str("error");
                    if *tagged {
                        out.push_str("[generated]");
                    }
                    out.push_str(if *active { "(1)" } else { "(0)" });
                    match shape {
                        0 => {}
                        1 => out.push_str(&format!(" D{detector_a}")),
                        2 => out.push_str(&format!(" L{observable}")),
                        3 => out.push_str(&format!(" D{detector_a} L{observable}")),
                        4 => out.push_str(&format!(" D{detector_a} ^ D{detector_b} L{observable}")),
                        5 => out.push_str(&format!(" D{detector_a} D{detector_b}")),
                        _ => unreachable!("error shape strategy is bounded"),
                    }
                    out.push('\n');
                }
                GeneratedDemItem::Detector {
                    detector,
                    coordinates,
                    tagged,
                } => {
                    out.push_str(&prefix);
                    out.push_str("detector");
                    if *tagged {
                        out.push_str("[generated]");
                    }
                    if !coordinates.is_empty() {
                        out.push('(');
                        out.push_str(
                            &coordinates
                                .iter()
                                .map(|value| coordinate(*value))
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                        out.push(')');
                    }
                    out.push_str(&format!(" D{detector}\n"));
                }
                GeneratedDemItem::Shift {
                    detectors,
                    coordinates,
                } => {
                    out.push_str(&prefix);
                    out.push_str("shift_detectors");
                    if !coordinates.is_empty() {
                        out.push('(');
                        out.push_str(
                            &coordinates
                                .iter()
                                .map(|value| coordinate(*value))
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                        out.push(')');
                    }
                    out.push_str(&format!(" {detectors}\n"));
                }
                GeneratedDemItem::Logical { observable } => {
                    out.push_str(&format!("{prefix}logical_observable L{observable}\n"));
                }
                GeneratedDemItem::Repeat {
                    count,
                    body,
                    tagged,
                } => {
                    out.push_str(&prefix);
                    out.push_str("repeat");
                    if *tagged {
                        out.push_str("[generated]");
                    }
                    out.push_str(&format!(" {count} {{\n"));
                    render_items(body, indent + 1, out);
                    out.push_str(&format!("{prefix}}}\n"));
                }
            }
        }
    }

    let mut out = String::new();
    render_items(items, 0, &mut out);
    out
}

fn run_generated_folded_differential_corpus() {
    let config = Config {
        cases: GENERATED_DIFFERENTIAL_CASES,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &GENERATED_DIFFERENTIAL_SEED);
    let mut runner = TestRunner::new_with_rng(config, rng);
    let matcher_circuit =
        Circuit::from_stim_str("M(0.125) 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .expect("generated differential matcher circuit");

    runner
        .run(&generated_dem_strategy(), |items| {
            let text = render_generated_dem(&items);
            let compact = DetectorErrorModel::from_dem_str(&text).map_err(|error| {
                TestCaseError::fail(format!("generated DEM did not parse: {error}\n{text}"))
            })?;
            let materialized_text = render_generated_dem(&expand_generated_dem(&items));
            let materialized =
                DetectorErrorModel::from_dem_str(&materialized_text).map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated materialized DEM did not parse: {error}\n{materialized_text}"
                    ))
                })?;

            prop_assert_eq!(
                compact.total_detector_shift(),
                materialized.total_detector_shift()
            );
            prop_assert_eq!(compact.count_detectors(), materialized.count_detectors());
            prop_assert_eq!(
                compact.count_observables(),
                materialized.count_observables()
            );
            prop_assert_eq!(compact.count_errors(), materialized.count_errors());
            prop_assert_eq!(
                compact.final_coordinate_shift(),
                materialized.final_coordinate_shift()
            );
            prop_assert_eq!(
                compact.detector_coordinates(),
                materialized.detector_coordinates()
            );

            let compact_rounded = compact
                .rounded(2)
                .and_then(|model| model.flattened())
                .map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated compact rounding failed: {error}\n{text}"
                    ))
                })?;
            let materialized_rounded = materialized
                .rounded(2)
                .and_then(|model| model.flattened())
                .map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated materialized rounding failed: {error}\n{text}"
                    ))
                })?;
            prop_assert_eq!(compact_rounded, materialized_rounded);
            let compact_without_tags = compact.without_tags().flattened().map_err(|error| {
                TestCaseError::fail(format!(
                    "generated compact tag removal failed: {error}\n{text}"
                ))
            })?;
            let materialized_without_tags =
                materialized.without_tags().flattened().map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated materialized tag removal failed: {error}\n{text}"
                    ))
                })?;
            prop_assert_eq!(compact_without_tags, materialized_without_tags);

            let compact_samples = CompiledDemSampler::compile(&compact)
                .and_then(|sampler| sampler.sample_detection_events_with_seed(8, Some(0xB3)))
                .map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated compact deterministic sampling failed: {error}\n{text}"
                    ))
                })?;
            let materialized_samples = CompiledDemSampler::compile(&materialized)
                .and_then(|sampler| sampler.sample_detection_events_with_seed(8, Some(0xB3)))
                .map_err(|error| {
                    TestCaseError::fail(format!(
                        "generated materialized deterministic sampling failed: {error}\n{text}"
                    ))
                })?;
            prop_assert_eq!(compact_samples, materialized_samples);

            let compact_graphlike = shortest_graphlike_undetectable_logical_error(&compact, false)
                .map(|model| model.to_dem_string())
                .map_err(|error| error.to_string());
            let materialized_graphlike =
                shortest_graphlike_undetectable_logical_error(&materialized, false)
                    .map(|model| model.to_dem_string())
                    .map_err(|error| error.to_string());
            prop_assert_eq!(compact_graphlike, materialized_graphlike);
            let compact_hyper =
                find_undetectable_logical_error(&compact, usize::MAX, usize::MAX, false)
                    .map(|model| model.to_dem_string())
                    .map_err(|error| error.to_string());
            let materialized_hyper =
                find_undetectable_logical_error(&materialized, usize::MAX, usize::MAX, false)
                    .map(|model| model.to_dem_string())
                    .map_err(|error| error.to_string());
            prop_assert_eq!(compact_hyper, materialized_hyper);

            let normalize_matcher = |filter: &DetectorErrorModel| {
                explain_errors_from_circuit(&matcher_circuit, Some(filter), false)
                    .map(|errors| {
                        errors
                            .into_iter()
                            .map(|error| error.to_string())
                            .collect::<Vec<_>>()
                    })
                    .map_err(|error| error.to_string())
            };
            prop_assert_eq!(
                normalize_matcher(&compact),
                normalize_matcher(&materialized)
            );
            Ok(())
        })
        .expect("deterministic generated folded DEM differential corpus");
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

fn detector(id: u64) -> DemDetectorId {
    DemDetectorId::try_new(id).expect("valid detector id")
}

#[test]
fn pfm_b3_folded_traversal_counts() {
    let huge = dem("repeat 1000000000 {\n\
             repeat 1 {\n\
                 error(0) D3 ^ D1 L5\n\
                 detector(1, 2) D1\n\
                 shift_detectors(3, 4) 2\n\
             }\n\
             logical_observable L7\n\
         }\n");
    assert_eq!(
        huge.total_detector_shift().expect("detector shift"),
        2_000_000_000
    );
    assert_eq!(
        huge.count_detectors().expect("detector count"),
        2_000_000_002
    );
    assert_eq!(huge.count_observables().expect("observable count"), 8);
    assert_eq!(huge.count_errors().expect("error count"), 1_000_000_000);
    assert_eq!(
        huge.final_coordinate_shift().expect("coordinate shift"),
        vec![3_000_000_000.0, 4_000_000_000.0]
    );

    for text in [
        "error(0.1) D0 L0\n",
        "repeat 1 {\n    error(0.1) D2 ^ L3\n    shift_detectors 2\n}\n",
        "repeat 3 {\n    repeat 2 {\n        error(0) D1 L2\n        shift_detectors 1\n    }\n    detector D0\n}\n",
        "repeat 7 {\n}\nlogical_observable L4\n",
    ] {
        let compact = dem(text);
        let materialized = compact.flattened().expect("small materialized reference");
        assert_eq!(
            compact.count_detectors(),
            materialized.count_detectors(),
            "{text}"
        );
        assert_eq!(
            compact.count_observables(),
            materialized.count_observables(),
            "{text}"
        );
        assert_eq!(
            compact.count_errors(),
            materialized.count_errors(),
            "{text}"
        );
    }

    for outer in 1..=4_u64 {
        for inner in 1..=3_u64 {
            let text = format!(
                "repeat {outer} {{\n    error(0.25) D1 ^ D0 L2\n    repeat {inner} {{\n        detector(3) D0\n        shift_detectors(1) 1\n    }}\n    logical_observable L4\n}}\n"
            );
            let compact = dem(&text);
            let materialized = compact
                .flattened()
                .expect("generated materialized reference");
            assert_eq!(
                compact.count_detectors(),
                materialized.count_detectors(),
                "{text}"
            );
            assert_eq!(
                compact.count_observables(),
                materialized.count_observables(),
                "{text}"
            );
            assert_eq!(
                compact.count_errors(),
                materialized.count_errors(),
                "{text}"
            );
            assert_eq!(
                compact.detector_coordinates(),
                materialized.detector_coordinates(),
                "{text}"
            );
        }
    }

    run_generated_folded_differential_corpus();

    let overflow = dem("repeat 5 {\n    shift_detectors 4611686018427387903\n}\n")
        .total_detector_shift()
        .expect_err("checked repeat shift overflow");
    assert!(overflow.to_string().contains("overflowed"), "{overflow}");

    let coordinate_overflow = dem("repeat 2 {\n    shift_detectors(1e308) 0\n}\n");
    assert_eq!(coordinate_overflow.count_detectors(), Ok(0));
    let error = coordinate_overflow
        .final_coordinate_shift()
        .expect_err("coordinate overflow belongs only to coordinate queries");
    assert!(
        error.to_string().contains("coordinate shift overflowed"),
        "{error}"
    );

    let wide_coordinates = std::iter::repeat_n("1", 32_000)
        .collect::<Vec<_>>()
        .join(",");
    let mut deep_coordinate = dem(&format!("shift_detectors({wide_coordinates}) 0\n"));
    for _ in 0..256 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            RepeatCount::try_new(1).expect("repeat count"),
            deep_coordinate,
            None,
        ));
        deep_coordinate = outer;
    }
    assert_eq!(deep_coordinate.count_detectors(), Ok(0));
    let error = deep_coordinate
        .final_coordinate_shift()
        .expect_err("aggregate coordinate scalar work must be bounded");
    assert!(
        error.to_string().contains("coordinate scalar updates"),
        "{error}"
    );
}

#[test]
fn pfm_b3_folded_traversal_coordinates() {
    let compact = dem("repeat 3 {\n\
             detector(10) D2\n\
             shift_detectors(1) 1\n\
             repeat 2 {\n\
                 detector(20) D0\n\
                 shift_detectors(2) 2\n\
             }\n\
         }\n\
         error(0.1) D1\n");
    let materialized = compact
        .flattened()
        .expect("small materialized coordinate reference");
    assert_eq!(
        compact.detector_coordinates().expect("compact coordinates"),
        materialized
            .detector_coordinates()
            .expect("materialized coordinates")
    );
    assert_eq!(
        compact
            .detector_coordinates_for([detector(0), detector(2), detector(7), detector(15)])
            .expect("selected compact coordinates"),
        materialized
            .detector_coordinates_for([detector(0), detector(2), detector(7), detector(15)])
            .expect("selected materialized coordinates")
    );

    let huge_sparse = dem("repeat 4000000 {\n\
             repeat 1 {\n\
                 detector(7) D0\n\
             }\n\
             detector(99) D2000000\n\
             shift_detectors(1) 1\n\
         }\n");
    assert_eq!(
        huge_sparse
            .coordinates_of_detector(detector(1_500_000))
            .expect("folded sparse coordinate"),
        vec![1_500_007.0]
    );
    let ambiguous =
        dem("repeat 10 {\n    detector(100) D2\n    detector(0) D0\n    shift_detectors(1) 1\n}\n");
    assert_eq!(
        ambiguous
            .coordinates_of_detector(detector(9))
            .expect("first repeated declaration"),
        vec![107.0]
    );

    let huge_full = dem("repeat 1000001 {\n    detector(1) D0\n    shift_detectors 1\n}\n");
    let error = huge_full
        .detector_coordinates()
        .expect_err("full coordinate map has inherently expanded output");
    assert!(
        error.to_string().contains("at most 1000000 detectors"),
        "{error}"
    );

    let declaration_overflow = dem(
        "error(0) D1\nrepeat 2 {\n    repeat 2 {\n        repeat 18446744073709551615 {\n            detector(5) D0\n        }\n    }\n}\n",
    );
    assert_eq!(declaration_overflow.count_detectors(), Ok(2));
    assert_eq!(
        declaration_overflow
            .coordinates_of_detector(detector(0))
            .expect("selected declaration survives irrelevant count overflow"),
        vec![5.0]
    );
    assert_eq!(
        declaration_overflow
            .coordinates_of_detector(detector(1))
            .expect("selected sparse hole survives irrelevant count overflow"),
        Vec::<f64>::new()
    );

    let fractional = dem("repeat 100 {\n    detector(0) D0\n    shift_detectors(0.1) 1\n}\n");
    let coordinate = fractional
        .coordinates_of_detector(detector(99))
        .expect("fractional selected coordinate");
    assert_eq!(coordinate.len(), 1);
    let coordinate = *coordinate.first().expect("one fractional coordinate");
    assert!(
        (coordinate - 9.899_999_999_999_98).abs() <= 1e-12,
        "coordinate must be semantically equivalent to pinned Stim accumulation: {coordinate}"
    );
}

#[test]
fn pfm_b3_folded_traversal_transforms() {
    let source = dem("repeat[outer] 1000000000 {\n\
             error[first](0.123456) D0 L0\n\
             detector[coords](1, 2) D0\n\
             repeat[inner] 3 {\n\
                 error[tiny](0.0004) D1\n\
             }\n\
         }\n");
    let rounded = source.rounded(3).expect("compact rounded transform");
    assert_eq!(
        rounded,
        dem("repeat[outer] 1000000000 {\n\
                 error[first](0.123) D0 L0\n\
                 detector[coords](1, 2) D0\n\
                 repeat[inner] 3 {\n\
                     error[tiny](0) D1\n\
                 }\n\
             }\n")
    );
    let stripped = source.without_tags().to_dem_string();
    assert!(stripped.starts_with("repeat 1000000000"), "{stripped}");
    assert!(!stripped.contains('['), "{stripped}");
    let flatten_error = source
        .flattened()
        .expect_err("materialized flattening keeps its explicit cap");
    assert!(
        flatten_error.to_string().contains("supports repeat counts"),
        "{flatten_error}"
    );

    let mut deep = dem("error[tag](0.1234) D0\n");
    for _ in 0..257 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            RepeatCount::try_new(1).expect("repeat count"),
            deep,
            Some("nested".to_string()),
        ));
        deep = outer;
    }
    assert_eq!(deep.count_errors(), Ok(1));
    assert_eq!(
        deep.rounded(2).expect("deep rounded model").count_errors(),
        Ok(1)
    );
    assert!(!deep.without_tags().to_dem_string().contains('['));
}

#[test]
fn pfm_b3_folded_traversal_sampler() {
    const SHOTS: usize = 100_000;
    let stochastic = dem("repeat 1000001 {\n\
             error(0.1) D0 L0\n\
             shift_detectors 0\n\
         }\n");
    let sampler = CompiledDemSampler::compile(&stochastic).expect("folded sampler compilation");
    assert_eq!(sampler.error_count(), 1_000_001);
    let output = sampler
        .sample_detection_events_with_seed(SHOTS, Some(12_648_437))
        .expect("seeded folded sampling");
    let mut all_zero = 0_usize;
    let mut joint_nonzero = 0_usize;
    let mut unexpected = 0_usize;
    for record in &output.records {
        match (record.detectors.as_slice(), record.observables.as_slice()) {
            ([false], [false]) => all_zero += 1,
            ([true], [true]) => joint_nonzero += 1,
            _ => unexpected += 1,
        }
    }
    assert_eq!(unexpected, 0, "unexpected joint detector-observable bucket");
    assert_probability(all_zero, SHOTS, 0.5);
    assert_probability(joint_nonzero, SHOTS, 0.5);

    let combinations = CompiledDemSampler::compile(&dem(
        "error(0.1) D0 D1\nerror(0.2) D1 D2\nerror(0.3) D2 D0\n",
    ))
    .expect("pinned combination sampler");
    let combination_output = combinations
        .sample_detection_events_with_seed(SHOTS, Some(12_648_437))
        .expect("seeded combination sampling");
    let mut detector_hits = [0_usize; 3];
    for record in &combination_output.records {
        for (index, detector) in record.detectors.iter().copied().enumerate() {
            if detector {
                *detector_hits
                    .get_mut(index)
                    .expect("combination sampler has exactly three detectors") += 1;
            }
        }
        assert!(
            !record
                .detectors
                .iter()
                .copied()
                .fold(false, |parity, bit| parity ^ bit),
            "the three pair mechanisms must preserve even detector parity"
        );
    }
    assert_probability(detector_hits[0], SHOTS, 0.34);
    assert_probability(detector_hits[1], SHOTS, 0.26);
    assert_probability(detector_hits[2], SHOTS, 0.38);

    let deterministic = CompiledDemSampler::compile(&dem("repeat 1000000000 {\n\
             repeat 3 {\n\
                 error(1) D1 L1\n\
             }\n\
         }\n"))
    .expect("deterministic folded sampler compilation");
    let record = deterministic
        .sample_detection_events_with_seed(1, Some(12_648_437))
        .expect("deterministic folded sample")
        .records
        .into_iter()
        .next()
        .expect("one record");
    assert_eq!(record.detectors, vec![false, false]);
    assert_eq!(record.observables, vec![false, false]);
}

fn assert_probability(observed: usize, shots: usize, expected: f64) {
    let observed = observed as f64 / shots as f64;
    let sigma = (expected * (1.0 - expected) / shots as f64).sqrt();
    let tolerance = 0.01_f64.max(6.0 * sigma);
    assert!(
        (observed - expected).abs() <= tolerance,
        "observed={observed} expected={expected} tolerance={tolerance}"
    );
}

#[test]
fn pfm_b3_folded_traversal_search() {
    let repeated = dem("repeat 100001 {\n\
             detector(1) D0\n\
             logical_observable L2\n\
             error(0) D999999 L999\n\
             error(0.1) D0\n\
             repeat 17 {\n\
                 shift_detectors 0\n\
                 error(0.2) D0 L0\n\
             }\n\
             error(0.3) D0 ^ D1\n\
         }\n");
    let compact = dem("detector(1) D0\n\
         logical_observable L2\n\
         error(0) D999999 L999\n\
         error(0.1) D0\n\
         shift_detectors 0\n\
         error(0.2) D0 L0\n\
         error(0.3) D0 ^ D1\n");
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&repeated, false)
            .expect("folded graphlike search")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(&compact, false)
            .expect("compact graphlike search")
            .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&repeated, usize::MAX, usize::MAX, false)
            .expect("folded hypergraph search")
            .to_dem_string(),
        find_undetectable_logical_error(&compact, usize::MAX, usize::MAX, false)
            .expect("compact hypergraph search")
            .to_dem_string()
    );

    let neutral_nested = dem(
        "repeat 100001 {\n    error(0.1) D0\n    error(0.1) D0 L0\n    repeat 100001 {\n    }\n}\n",
    );
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&neutral_nested, false)
            .expect("nested neutral repeat is skipped")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            false,
        )
        .expect("compact neutral-repeat reference")
        .to_dem_string()
    );
    assert_eq!(
        find_undetectable_logical_error(&neutral_nested, usize::MAX, usize::MAX, false)
            .expect("nested neutral repeat is skipped by hypergraph collection")
            .to_dem_string(),
        find_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            usize::MAX,
            usize::MAX,
            false,
        )
        .expect("compact neutral hypergraph reference")
        .to_dem_string()
    );

    let wide_shift = std::iter::repeat_n("1", 4096).collect::<Vec<_>>().join(",");
    let coordinate_irrelevant = dem(&format!(
        "repeat 100001 {{\n    error(0.1) D0\n    error(0.1) D0 L0\n    shift_detectors({wide_shift}) 0\n}}\n"
    ));
    assert_eq!(
        shortest_graphlike_undetectable_logical_error(&coordinate_irrelevant, false)
            .expect("search does not allocate or update irrelevant coordinate state")
            .to_dem_string(),
        shortest_graphlike_undetectable_logical_error(
            &dem("error(0.1) D0\nerror(0.1) D0 L0\n"),
            false,
        )
        .expect("compact coordinate-irrelevant reference")
        .to_dem_string()
    );

    let shifted = dem("repeat 100001 {\n    error(0.1) D0 L0\n    shift_detectors 1\n}\n");
    let error = shortest_graphlike_undetectable_logical_error(&shifted, false)
        .expect_err("shifted active repeat exceeds bounded search traversal");
    assert!(
        error.to_string().contains("supports repeat counts"),
        "{error}"
    );
}

#[test]
fn pfm_b3_folded_traversal_sat() {
    const EXPECTED_UNWEIGHTED: &str = "\
p wcnf 3 8 9
1 -1 0
9 1 2 -3 0
9 1 -2 3 0
9 -1 2 3 0
9 -1 -2 -3 0
1 -2 0
9 -3 0
9 1 0
";
    const EXPECTED_WEIGHTED: &str = "\
p wcnf 3 8 801
100 -1 0
801 1 2 -3 0
801 1 -2 3 0
801 -1 2 3 0
801 -1 -2 -3 0
100 -2 0
801 -3 0
801 1 0
";
    let repeated = dem("repeat 100001 {\n\
             detector D0\n\
             error(0.1) D0 L0\n\
             repeat 2 {\n\
                 error(0.9) D0\n\
                 shift_detectors 0\n\
             }\n\
             logical_observable L0\n\
         }\n");
    let compact = dem("detector D0\n\
         error(0.1) D0 L0\n\
         error(0.1) D0\n\
         shift_detectors 0\n\
         logical_observable L0\n");
    let repeated_unweighted = shortest_error_sat_problem(&repeated).expect("folded SAT");
    let compact_unweighted = shortest_error_sat_problem(&compact).expect("compact SAT");
    assert_eq!(repeated_unweighted, EXPECTED_UNWEIGHTED);
    assert_eq!(compact_unweighted, EXPECTED_UNWEIGHTED);
    let repeated_weighted = likeliest_error_sat_problem(&repeated, 100).expect("folded WCNF");
    let compact_weighted = likeliest_error_sat_problem(&compact, 100).expect("compact WCNF");
    assert_eq!(repeated_weighted, EXPECTED_WEIGHTED);
    assert_eq!(compact_weighted, EXPECTED_WEIGHTED);

    let neutral = dem("repeat 100001 {\n}\n");
    assert_eq!(
        shortest_error_sat_problem(&neutral).expect("neutral SAT"),
        shortest_error_sat_problem(&DetectorErrorModel::new()).expect("empty SAT reference")
    );

    let shifted = dem("repeat 100001 {\n    error(0.1) D0 L0\n    shift_detectors 1\n}\n");
    let error = shortest_error_sat_problem(&shifted)
        .expect_err("shifted active repeat exceeds bounded SAT traversal");
    assert!(
        error.to_string().contains("supports repeat counts"),
        "{error}"
    );
}

#[test]
fn pfm_b3_folded_traversal_matcher_filter() {
    let circuit = Circuit::from_stim_str(
        "MPAD 0\n\
         DETECTOR rec[-1]\n\
         M(0.125) 0\n\
         M(0.25) 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n\
         OBSERVABLE_INCLUDE(1) rec[-2]\n",
    )
    .expect("matcher circuit");
    let compact = dem("shift_detectors 1\n\
         error(0.1) D0\n\
         error(0.1) D0 D0 D1 ^ L0\n\
         error(0.1) L1\n");
    let repeated = dem("shift_detectors 1\n\
         repeat 100001 {\n\
             detector(2, 3) D0\n\
             logical_observable L0\n\
             error(0.1) D0\n\
             repeat 17 {\n\
                 detector(7) D1\n\
                 error(0.1) D0 D0 D1 ^ L0\n\
                 error(0.1) L1\n\
                 shift_detectors 0\n\
             }\n\
         }\n");
    let normalize = |filter: &DetectorErrorModel| {
        explain_errors_from_circuit(&circuit, Some(filter), false)
            .expect("matcher filter traversal")
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(normalize(&repeated), normalize(&compact));

    let neutral = dem("repeat 100001 {\n}\n");
    assert_eq!(normalize(&neutral), normalize(&DetectorErrorModel::new()));

    let shifted = dem("repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\n");
    let error = explain_errors_from_circuit(&circuit, Some(&shifted), false)
        .expect_err("shifted filter repeat exceeds bounded traversal");
    assert!(
        error.to_string().contains("supports repeat counts"),
        "{error}"
    );
}
