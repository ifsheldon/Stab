#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "the gate-table fixtures must construct, and a rule without a view is a test failure"
)]

//! The typed instruction views are keyed by the gate table's argument and target rules. These
//! tests build one representative instruction for every gate and check that each view names the
//! shape the gate's rule promises and round-trips the instruction's stored data.

use std::assert_matches;

use stab_model::{
    CircuitInstruction, ClassicalControl, ControlledPauliTargetPair, Gate, GateArgumentRule,
    GateArguments, GateCategory, GateKind, GateTargetRule, GateTargets, MeasureRecordOffset,
    ObservableId, ObservableTarget, Pauli, Probability, QubitId, Target,
};

fn q(id: u32) -> QubitId {
    QubitId::new(id).expect("small qubit id")
}

fn qubit(id: u32, inverted: bool) -> Target {
    Target::qubit(q(id), inverted)
}

fn rec(offset: i32) -> Target {
    Target::measurement_record(MeasureRecordOffset::try_new(offset).expect("lookback"))
}

fn pauli(pauli: Pauli, id: u32, inverted: bool) -> Target {
    Target::pauli(pauli, q(id), inverted)
}

fn representative_arguments(rule: GateArgumentRule) -> Vec<f64> {
    match rule {
        GateArgumentRule::Exact(0) => Vec::new(),
        GateArgumentRule::Any => vec![1.5, -2.0],
        GateArgumentRule::OptionalProbability => vec![0.125],
        // Distinct values whose sum stays a probability for every fixed list length.
        GateArgumentRule::ProbabilityList(count) => {
            (1..=count).map(|index| 0.005 * index as f64).collect()
        }
        GateArgumentRule::AnyProbabilityList => vec![0.1, 0.2],
        GateArgumentRule::UnsignedInteger => vec![7.0],
        GateArgumentRule::Exact(count) => {
            panic!("no gate takes {count} exact arguments, so no view covers that rule")
        }
    }
}

fn representative_targets(gate: Gate) -> Vec<Target> {
    match gate.target_rule() {
        GateTargetRule::None => Vec::new(),
        GateTargetRule::AnySingleQubit => vec![qubit(0, false), qubit(5, false)],
        GateTargetRule::MeasurementQubits => vec![qubit(0, false), qubit(1, true)],
        GateTargetRule::MeasurementPads => vec![qubit(0, false), qubit(1, false), qubit(1, false)],
        GateTargetRule::PlainPairs => {
            vec![
                qubit(0, false),
                qubit(1, false),
                qubit(3, false),
                qubit(2, false),
            ]
        }
        GateTargetRule::ClassicalControlPairs => {
            // One quantum pair, then a record control in the gate's control slot.
            let control_second = matches!(gate.kind(), GateKind::XCZ | GateKind::YCZ);
            let classical = if control_second {
                [qubit(2, false), rec(-1)]
            } else {
                [rec(-1), qubit(2, false)]
            };
            let mut targets = vec![qubit(0, false), qubit(1, false)];
            targets.extend(classical);
            targets
        }
        GateTargetRule::MeasurementPairs => {
            vec![
                qubit(0, false),
                qubit(1, true),
                qubit(2, false),
                qubit(3, false),
            ]
        }
        GateTargetRule::RecOnly => vec![rec(-1), rec(-3)],
        GateTargetRule::RecOrPauli => {
            vec![rec(-1), pauli(Pauli::X, 0, false), pauli(Pauli::Z, 1, true)]
        }
        GateTargetRule::QubitCoords => vec![qubit(3, false)],
        GateTargetRule::PauliProducts => vec![
            pauli(Pauli::X, 0, false),
            Target::combiner(),
            pauli(Pauli::Y, 1, true),
            pauli(Pauli::Z, 2, false),
        ],
        // Correlated-error combiners are ignored decoration wherever they appear.
        GateTargetRule::PauliList => vec![
            pauli(Pauli::X, 0, false),
            Target::combiner(),
            Target::combiner(),
            pauli(Pauli::Y, 1, true),
            pauli(Pauli::Z, 2, false),
        ],
    }
}

fn instruction_gates() -> impl Iterator<Item = Gate> {
    // REPEAT is block-only control flow and never forms an instruction.
    Gate::all().filter(|gate| gate.category() != GateCategory::ControlFlow)
}

fn representative_instruction(gate: Gate) -> CircuitInstruction {
    CircuitInstruction::new(
        gate,
        representative_arguments(gate.argument_rule()),
        representative_targets(gate),
        None,
    )
    .unwrap_or_else(|error| panic!("construct {}: {error}", gate.canonical_name()))
}

#[test]
fn every_gate_argument_view_matches_its_rule_and_round_trips() {
    for gate in instruction_gates() {
        let name = gate.canonical_name();
        let instruction = representative_instruction(gate);
        let args = instruction.args();
        match (gate.argument_rule(), instruction.gate_arguments()) {
            (GateArgumentRule::Exact(0), GateArguments::None) => assert!(args.is_empty(), "{name}"),
            (GateArgumentRule::Any, GateArguments::Coordinates(coordinates)) => {
                assert_eq!(coordinates, args, "{name}");
            }
            (GateArgumentRule::OptionalProbability, GateArguments::OptionalProbability(value)) => {
                assert_eq!(value.map(Probability::get), Some(0.125), "{name}");
                let absent =
                    CircuitInstruction::new(gate, Vec::new(), representative_targets(gate), None)
                        .expect("the probability is optional");
                assert_eq!(
                    absent.gate_arguments(),
                    GateArguments::OptionalProbability(None),
                    "{name}"
                );
            }
            (GateArgumentRule::ProbabilityList(1), GateArguments::Probability(probability)) => {
                assert_eq!([probability.get()], args, "{name}");
            }
            (
                GateArgumentRule::ProbabilityList(_) | GateArgumentRule::AnyProbabilityList,
                GateArguments::Probabilities(probabilities),
            ) => {
                assert_eq!(probabilities.len(), args.len(), "{name}");
                assert_eq!(probabilities.as_f64_slice(), args, "{name}");
                assert!(
                    probabilities.map(Probability::get).eq(args.iter().copied()),
                    "{name}"
                );
            }
            (GateArgumentRule::UnsignedInteger, GateArguments::Observable(observable)) => {
                assert_eq!(observable, ObservableId::from(7), "{name}");
            }
            (rule, view) => panic!("{name}: argument rule {rule:?} produced {view:?}"),
        }
        assert_eq!(
            instruction.probability().is_some(),
            matches!(
                instruction.gate_arguments(),
                GateArguments::Probability(_) | GateArguments::OptionalProbability(Some(_))
            ),
            "{name}"
        );
    }
}

/// Rebuilds the raw target list a target view describes.
fn targets_from_view(view: GateTargets<'_>) -> Vec<Target> {
    match view {
        GateTargets::None => Vec::new(),
        GateTargets::Qubits(qubits) | GateTargets::QubitCoords(qubits) => {
            qubits.map(|id| Target::qubit(id, false)).collect()
        }
        GateTargets::MeasuredQubits(qubits) => qubits.map(Target::from).collect(),
        GateTargets::MeasurementPads(pads) => pads
            .map(|pad| Target::qubit(q(u32::from(pad)), false))
            .collect(),
        GateTargets::QubitPairs(pairs) => pairs
            .flat_map(|pair| pair.map(|id| Target::qubit(id, false)))
            .collect(),
        GateTargets::MeasuredQubitPairs(pairs) => {
            pairs.flat_map(|pair| pair.map(Target::from)).collect()
        }
        GateTargets::ControlledPairs(pairs) => {
            pairs.as_target_pairs().iter().flatten().copied().collect()
        }
        GateTargets::Records(records) => records.map(Target::measurement_record).collect(),
        GateTargets::ObservableTargets(targets) => targets
            .map(|target| match target {
                ObservableTarget::MeasurementRecord(offset) => Target::measurement_record(offset),
                ObservableTarget::Pauli(term) => Target::from(term),
            })
            .collect(),
        GateTargets::PauliProducts(products) => {
            let mut targets = Vec::new();
            for product in products {
                for (index, term) in product.enumerate() {
                    if index > 0 {
                        targets.push(Target::combiner());
                    }
                    targets.push(Target::from(term));
                }
            }
            targets
        }
        GateTargets::PauliTerms(terms) => terms.map(Target::from).collect(),
    }
}

#[test]
fn every_gate_target_view_matches_its_rule_and_round_trips() {
    for gate in instruction_gates() {
        let name = gate.canonical_name();
        let instruction = representative_instruction(gate);
        let view = instruction.gate_targets();
        assert_eq!(view.rule(), gate.target_rule(), "{name}");

        let expected = match gate.target_rule() {
            // The Pauli-list view drops its ignored combiners.
            GateTargetRule::PauliList => instruction
                .targets()
                .iter()
                .copied()
                .filter(|target| !target.is_combiner())
                .collect(),
            _ => instruction.targets().to_vec(),
        };
        assert_eq!(targets_from_view(view.clone()), expected, "{name}");

        // Each view reports its exact length up front, and iterating from the back yields the
        // same items in reverse.
        match view {
            GateTargets::Qubits(qubits) | GateTargets::QubitCoords(qubits) => {
                assert_eq!(qubits.len(), qubits.clone().count(), "{name}");
                assert!(
                    qubits
                        .clone()
                        .rev()
                        .eq(qubits.collect::<Vec<_>>().into_iter().rev())
                );
            }
            GateTargets::PauliProducts(products) => {
                assert_eq!(products.len(), 2, "{name}");
                let forward = products
                    .clone()
                    .map(|product| product.as_targets())
                    .collect::<Vec<_>>();
                let mut backward = products
                    .rev()
                    .map(|product| product.as_targets())
                    .collect::<Vec<_>>();
                backward.reverse();
                assert_eq!(forward, backward, "{name}");
            }
            GateTargets::PauliTerms(terms) => {
                assert_eq!(terms.len(), 3, "{name}");
                assert!(
                    terms
                        .clone()
                        .rev()
                        .eq(terms.collect::<Vec<_>>().into_iter().rev())
                );
            }
            GateTargets::ControlledPairs(pairs) => {
                assert_eq!(pairs.len(), 2, "{name}");
                assert_matches!(
                    pairs.clone().collect::<Vec<_>>().as_slice(),
                    [
                        ControlledPauliTargetPair::Quantum { first, second },
                        ControlledPauliTargetPair::Classical {
                            control: ClassicalControl::Record(offset),
                            target,
                        },
                    ] if first.get() == 0 && second.get() == 1 && offset.get() == -1 && target.get() == 2,
                    "{name}"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn every_gate_target_groups_partition_targets_by_group_kind() {
    for gate in instruction_gates() {
        let name = gate.canonical_name();
        let instruction = representative_instruction(gate);
        let groups = instruction.target_groups();
        let count = groups.len();
        let collected = groups.clone().collect::<Vec<_>>();
        assert_eq!(collected.len(), count, "{name}");
        assert!(collected.iter().all(|group| !group.is_empty()), "{name}");
        assert_eq!(
            collected.concat(),
            instruction.targets(),
            "{name} groups must partition the targets in order"
        );
        let mut reversed = groups.rev().collect::<Vec<_>>();
        reversed.reverse();
        assert_eq!(reversed, collected, "{name}");
    }
}
