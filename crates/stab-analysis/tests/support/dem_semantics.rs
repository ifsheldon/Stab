#![allow(
    dead_code,
    clippy::expect_used,
    clippy::panic,
    reason = "each test crate uses a subset of the independent semantic DEM oracle"
)]

//! Independent semantic view of a detector error model.
//!
//! Repeat blocks are expanded and `shift_detectors` offsets are applied so folded and unfolded
//! analyzer outputs can be compared as the probability distribution they describe rather than as
//! text. Errors that share a symptom set are combined as independent mechanisms
//! (`p ^ q = p(1-q) + q(1-p)`), which is the only freedom the analyzer has when it chooses whether
//! to merge mechanisms across loop iterations.

use std::collections::{BTreeMap, BTreeSet};

use stab_model::{DemInstructionKind, DemItem, DemTargetKind, DetectorErrorModel};

/// One error-symptom target after detector offsets are resolved.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Symptom {
    Detector(u64),
    Observable(u64),
}

/// The symptoms of one mechanism, split at `^` separators.
pub(crate) type Components = Vec<BTreeSet<Symptom>>;

/// One `error` instruction after offsets are resolved.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedError {
    pub(crate) probability: f64,
    pub(crate) components: Components,
}

impl ResolvedError {
    /// The XOR of every decomposition component: the mechanism's real symptom set.
    pub(crate) fn symptoms(&self) -> BTreeSet<Symptom> {
        let mut symptoms = BTreeSet::new();
        for component in &self.components {
            for symptom in component {
                if !symptoms.remove(symptom) {
                    symptoms.insert(*symptom);
                }
            }
        }
        symptoms
    }
}

/// Resolved contents of a DEM in execution order.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedDem {
    pub(crate) errors: Vec<ResolvedError>,
    pub(crate) detector_coordinates: BTreeMap<u64, Vec<f64>>,
    pub(crate) declared_observables: BTreeSet<u64>,
}

impl ResolvedDem {
    pub(crate) fn new(model: &DetectorErrorModel) -> Self {
        let mut resolved = Self::default();
        let mut offset = 0_u64;
        let mut coordinate_shift = Vec::new();
        resolved.collect(model, &mut offset, &mut coordinate_shift);
        resolved
    }

    fn collect(
        &mut self,
        model: &DetectorErrorModel,
        offset: &mut u64,
        coordinate_shift: &mut Vec<f64>,
    ) {
        for item in model.items() {
            match item {
                DemItem::RepeatBlock(repeat) => {
                    for _ in 0..repeat.repeat_count().get() {
                        self.collect(repeat.body(), offset, coordinate_shift);
                    }
                }
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::Error => {
                        let mut components = vec![BTreeSet::new()];
                        for target in instruction.targets() {
                            let symptom = match target.kind() {
                                DemTargetKind::RelativeDetector(detector) => {
                                    Symptom::Detector(*offset + detector.get())
                                }
                                DemTargetKind::LogicalObservable(observable) => {
                                    Symptom::Observable(observable.get())
                                }
                                DemTargetKind::Separator => {
                                    components.push(BTreeSet::new());
                                    continue;
                                }
                            };
                            let component = components.last_mut().expect("open component");
                            assert!(component.insert(symptom), "repeated symptom {symptom:?}");
                        }
                        self.errors.push(ResolvedError {
                            probability: *instruction.args().first().expect("error probability"),
                            components,
                        });
                    }
                    DemInstructionKind::Detector => {
                        for target in instruction.targets() {
                            let Some(detector) = target.detector() else {
                                panic!("detector declaration target {target:?}");
                            };
                            let coordinates = instruction
                                .args()
                                .iter()
                                .enumerate()
                                .map(|(index, coordinate)| {
                                    coordinate + coordinate_shift.get(index).copied().unwrap_or(0.0)
                                })
                                .collect();
                            self.detector_coordinates
                                .insert(*offset + detector.get(), coordinates);
                        }
                    }
                    DemInstructionKind::LogicalObservable => {
                        for target in instruction.targets() {
                            let Some(observable) = target.observable() else {
                                panic!("observable declaration target {target:?}");
                            };
                            self.declared_observables.insert(observable.get());
                        }
                    }
                    DemInstructionKind::ShiftDetectors => {
                        if coordinate_shift.len() < instruction.args().len() {
                            coordinate_shift.resize(instruction.args().len(), 0.0);
                        }
                        for (shift, amount) in coordinate_shift.iter_mut().zip(instruction.args()) {
                            *shift += amount;
                        }
                        let shift = instruction.detector_shift().expect("detector shift");
                        *offset += shift;
                    }
                },
            }
        }
    }

    /// Combined probability of each distinct symptom set, ignoring decomposition structure.
    pub(crate) fn symptom_distribution(&self) -> BTreeMap<BTreeSet<Symptom>, f64> {
        let mut distribution = BTreeMap::<BTreeSet<Symptom>, f64>::new();
        for error in &self.errors {
            let symptoms = error.symptoms();
            if symptoms.is_empty() {
                continue;
            }
            let combined = distribution.entry(symptoms).or_insert(0.0);
            *combined =
                *combined * (1.0 - error.probability) + error.probability * (1.0 - *combined);
        }
        distribution
    }
}

/// Asserts that two symptom distributions describe the same independent-error model.
pub(crate) fn assert_same_distribution(
    actual: &BTreeMap<BTreeSet<Symptom>, f64>,
    expected: &BTreeMap<BTreeSet<Symptom>, f64>,
    tolerance: f64,
    context: &str,
) {
    let keys = actual
        .keys()
        .chain(expected.keys())
        .collect::<BTreeSet<_>>();
    for key in keys {
        let left = actual.get(key).copied().unwrap_or(0.0);
        let right = expected.get(key).copied().unwrap_or(0.0);
        assert!(
            (left - right).abs() <= tolerance,
            "{context}: symptom set {key:?} has probability {left} but expected {right}"
        );
    }
}
