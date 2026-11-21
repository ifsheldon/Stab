#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "property tests index fixed generator tables and report generated models through direct assertion diagnostics"
)]

//! The fast path at both maxima 2 must agree with the hypergraph search it replaces.

use std::fmt::Write as _;

use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_model::{DemInstructionView, DemTargetKind};

use super::super::graphlike::shortest_graphlike_undetectable_logical_error;
use super::super::outcome::{GraphlikeSearchOptions, UngraphlikeErrors};
use super::*;

const DETECTORS: u64 = 6;
const OBSERVABLES: u64 = 2;

/// One error target: a detector below [`DETECTORS`], an observable below [`OBSERVABLES`], or a
/// `^` separator.
#[derive(Clone, Copy, Debug)]
enum Token {
    Detector(u64),
    Observable(u64),
    Separator,
}

/// An error's targets: one to three detectors, repeats allowed, then an observable two times in
/// five, with a `^` separator in a quarter of the gaps between targets.
fn error_targets() -> impl Strategy<Value = Vec<Token>> {
    (
        prop::collection::vec(0..DETECTORS, 1..4),
        prop::option::weighted(0.4, 0..OBSERVABLES),
        prop::collection::vec(0..4_u8, 3),
    )
        .prop_map(|(detectors, observable, gaps)| {
            let terms = detectors
                .into_iter()
                .map(Token::Detector)
                .chain(observable.map(Token::Observable));
            let mut targets = Vec::new();
            for (index, term) in terms.enumerate() {
                if index > 0 && gaps.get(index - 1) == Some(&0) {
                    targets.push(Token::Separator);
                }
                targets.push(term);
            }
            targets
        })
}

/// Models of up to fourteen errors, a few of probability zero, half of them without separators,
/// and sometimes a declared detector or observable that no error touches.
fn model_text() -> impl Strategy<Value = String> {
    (
        prop::collection::vec((error_targets(), 0..8_u8), 0..15),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(errors, separators, declare_detector, declare_observable)| {
                let mut text = String::new();
                for (targets, probability_pick) in errors {
                    let probability = if probability_pick == 0 { "0" } else { "0.125" };
                    let _ = write!(text, "error({probability})");
                    for target in targets {
                        match target {
                            Token::Detector(id) => write!(text, " D{id}"),
                            Token::Observable(id) => write!(text, " L{id}"),
                            Token::Separator if separators => write!(text, " ^"),
                            Token::Separator => Ok(()),
                        }
                        .expect("writing to a String succeeds");
                    }
                    text.push('\n');
                }
                if declare_detector {
                    let _ = writeln!(text, "detector D{DETECTORS}");
                }
                if declare_observable {
                    let _ = writeln!(text, "logical_observable L{OBSERVABLES}");
                }
                text
            },
        )
}

/// The folded edges of `model`'s nonzero errors that the hypergraph search at maxima 2 keeps,
/// as sorted target lists.
fn folded_two_detector_edges(model: &DetectorErrorModel) -> BTreeSet<Vec<DemTarget>> {
    let mut edges = BTreeSet::new();
    for item in model.items() {
        let DemItem::Instruction(instruction) = item else {
            panic!("generated models have no repeat blocks");
        };
        let DemInstructionView::Error {
            probability,
            targets,
        } = instruction.view()
        else {
            continue;
        };
        if probability.get() == 0.0 {
            continue;
        }
        let targets: Vec<DemTarget> = targets.to_vec();
        let (detectors, observables) =
            folded_error_targets(&targets, &stab_model::Cancellation::never())
                .expect("folding never cancels");
        if detectors.len() <= 2 && !(detectors.is_empty() && observables.is_empty()) {
            edges.insert(edge_targets(&detectors, &observables));
        }
    }
    edges
}

fn edge_targets(
    detectors: &BTreeSet<DemDetectorId>,
    observables: &ObservableMask,
) -> Vec<DemTarget> {
    let mut targets: Vec<DemTarget> = detectors
        .iter()
        .map(|detector| DemTarget::relative_detector(detector.get()).expect("small detector"))
        .collect();
    observables.push_targets(&mut targets);
    targets
}

/// Checks that `witness` is an undetectable logical error made of `edges`, and returns its length.
fn witness_length(
    witness: &DetectorErrorModel,
    edges: &BTreeSet<Vec<DemTarget>>,
    text: &str,
) -> usize {
    let mut detectors = BTreeSet::new();
    let mut observables = ObservableMask::new();
    for item in witness.items() {
        let DemItem::Instruction(instruction) = item else {
            panic!("a witness has no repeat blocks\n{text}");
        };
        let DemInstructionView::Error {
            probability,
            targets,
        } = instruction.view()
        else {
            panic!("a witness holds only errors\n{text}");
        };
        assert_eq!(probability.get(), 1.0, "{text}");
        assert!(instruction.tag().is_none(), "{text}");
        let targets: Vec<DemTarget> = targets.to_vec();
        assert!(
            edges.contains(&targets),
            "witness step {targets:?} is not an edge of the model\n{text}"
        );
        for target in &targets {
            match target.kind() {
                DemTargetKind::RelativeDetector(detector) => {
                    if !detectors.insert(detector) {
                        detectors.remove(&detector);
                    }
                }
                DemTargetKind::LogicalObservable(observable) => observables.toggle(observable),
                DemTargetKind::Separator => panic!("a witness has no separators\n{text}"),
            }
        }
    }
    assert!(detectors.is_empty(), "the witness is detected\n{text}");
    assert!(
        !observables.is_empty(),
        "the witness flips no observable\n{text}"
    );
    witness.items().len()
}

fn has_separator_or_repeated_target(model: &DetectorErrorModel) -> bool {
    model.items().iter().any(|item| {
        let DemItem::Instruction(instruction) = item else {
            return true;
        };
        let targets = instruction.targets();
        targets.iter().any(|target| target.is_separator())
            || targets
                .iter()
                .enumerate()
                .any(|(index, target)| targets[..index].contains(target))
    })
}

fn assert_fast_path_matches_hypergraph_search(text: &str) {
    let model = DetectorErrorModel::from_dem_str(text).expect("generated models parse");
    let fast = find_undetectable_logical_error(
        &model,
        &HypergraphSearchOptions {
            max_detection_event_set_size: 2,
            max_edge_degree: 2,
            symptom_growth: SymptomGrowth::Explore,
            limits: LogicalErrorSearchLimits::default(),
        },
    )
    .expect("the fast path runs to completion");
    let edges = folded_two_detector_edges(&model);
    for symptom_growth in [SymptomGrowth::Explore, SymptomGrowth::Skip] {
        let reference = hypergraph_search(
            &model,
            &HypergraphSearchOptions {
                max_detection_event_set_size: 2,
                max_edge_degree: 2,
                symptom_growth,
                limits: LogicalErrorSearchLimits::default(),
            },
        )
        .expect("the hypergraph search runs to completion");
        match (&fast, &reference) {
            (LogicalErrorSearch::Found(fast), LogicalErrorSearch::Found(reference)) => {
                assert_eq!(
                    witness_length(fast, &edges, text),
                    witness_length(reference, &edges, text),
                    "fast {}reference {}{text}",
                    fast.to_dem_string(),
                    reference.to_dem_string()
                );
            }
            (LogicalErrorSearch::NotFound(fast), LogicalErrorSearch::NotFound(reference)) => {
                assert_eq!(fast, reference, "{text}");
            }
            _ => {
                panic!("fast path {fast:?} disagrees with hypergraph search {reference:?}\n{text}")
            }
        }
    }

    // Without separators or repeated targets, the folded edges are Stim's graphlike edges, so the
    // witness is byte-identical to the one Stim's reroute to the graphlike search returns.
    if !has_separator_or_repeated_target(&model) {
        let graphlike = shortest_graphlike_undetectable_logical_error(
            &model,
            &GraphlikeSearchOptions {
                ungraphlike_errors: UngraphlikeErrors::Ignore,
                limits: LogicalErrorSearchLimits::default(),
            },
        )
        .expect("the graphlike search runs to completion");
        assert_eq!(
            fast.found().map(DetectorErrorModel::to_dem_string),
            graphlike.found().map(DetectorErrorModel::to_dem_string),
            "{text}"
        );
    }
}

#[test]
fn fast_path_keeps_the_first_distance_1_error_where_the_hypergraph_search_keeps_the_last() {
    // The documented witness choice: the fast path keeps the first distance-1 error, as Stim's
    // graphlike search does, and the hypergraph search keeps the last.
    let text = "error(0.1) D0 ^ D0 L0\nerror(0.1) L1\n";
    let model = DetectorErrorModel::from_dem_str(text).expect("valid model");
    let options = HypergraphSearchOptions {
        max_detection_event_set_size: 2,
        max_edge_degree: 2,
        symptom_growth: SymptomGrowth::Explore,
        limits: LogicalErrorSearchLimits::default(),
    };
    let found = |search: LogicalErrorSearch| {
        search
            .into_found()
            .expect("a distance-1 error is found")
            .to_dem_string()
    };
    assert_eq!(
        found(find_undetectable_logical_error(&model, &options).expect("fast path")),
        "error(1) L0\n"
    );
    assert_eq!(
        found(hypergraph_search(&model, &options).expect("hypergraph search")),
        "error(1) L1\n"
    );
    assert_fast_path_matches_hypergraph_search(text);
}

#[test]
fn fast_path_fails_on_the_hypergraph_limits_and_cancellation_as_the_hypergraph_search() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.1) D0 D1 L0\nerror(0.1) D0 ^ D1\nerror(0.1) D2 D3\n",
    )
    .expect("valid model");
    let cancelled = stab_model::Cancellation::new();
    cancelled.cancel();
    let cases = [
        LogicalErrorSearchLimits {
            max_hyperedge_degree: 1,
            ..LogicalErrorSearchLimits::default()
        },
        LogicalErrorSearchLimits {
            max_hyperedge_incidences: 3,
            ..LogicalErrorSearchLimits::default()
        },
        LogicalErrorSearchLimits {
            max_effective_detector_nodes: 1,
            ..LogicalErrorSearchLimits::default()
        },
        LogicalErrorSearchLimits {
            cancellation: cancelled,
            ..LogicalErrorSearchLimits::default()
        },
    ];
    for limits in cases {
        let options = HypergraphSearchOptions {
            max_detection_event_set_size: 2,
            max_edge_degree: 2,
            symptom_growth: SymptomGrowth::Explore,
            limits,
        };
        let fast = find_undetectable_logical_error(&model, &options).expect_err("fast path stops");
        let reference = hypergraph_search(&model, &options).expect_err("hypergraph search stops");
        assert_eq!(fast, reference, "{fast}");
    }
}

proptest! {
    // Fixed seed: the case set is reproducible across runs and machines.
    #![proptest_config(ProptestConfig {
        cases: 2048,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x2d2d_fa57_9a7e_0001),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..ProptestConfig::default()
    })]

    #[test]
    fn fast_path_at_both_maxima_two_matches_the_hypergraph_search(text in model_text()) {
        assert_fast_path_matches_hypergraph_search(&text);
    }
}
