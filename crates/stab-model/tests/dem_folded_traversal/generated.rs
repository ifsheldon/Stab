//! Deterministic folded-DEM corpus for the model owner tests.

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

const GENERATED_DIFFERENTIAL_CASES: u32 = 96;
const GENERATED_DIFFERENTIAL_SEED: [u8; 32] = [0xB3; 32];

#[derive(Clone, Debug)]
pub(crate) enum GeneratedDemItem {
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

pub(crate) fn generated_dem_strategy() -> impl Strategy<Value = Vec<GeneratedDemItem>> {
    proptest::collection::vec(generated_dem_item_strategy(), 0..=6)
}

pub(crate) fn generated_dem_runner() -> TestRunner {
    let config = Config {
        cases: GENERATED_DIFFERENTIAL_CASES,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &GENERATED_DIFFERENTIAL_SEED);
    TestRunner::new_with_rng(config, rng)
}

pub(crate) fn expand_generated_dem(items: &[GeneratedDemItem]) -> Vec<GeneratedDemItem> {
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

pub(crate) fn render_generated_dem(items: &[GeneratedDemItem]) -> String {
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
