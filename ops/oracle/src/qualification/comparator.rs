#![allow(
    dead_code,
    reason = "CQ1 comparator adapters are exercised adversarially before CQ2-CQ5 register product observations"
)]

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::model::{Comparator, ResourceKind};

const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_TAG_BYTES: usize = 2_048;

macro_rules! bounded_identifier {
    ($name:ident, $description:literal) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub(super) struct $name(Box<str>);

        impl $name {
            pub(super) fn try_new(value: impl Into<Box<str>>) -> Result<Self, &'static str> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > MAX_IDENTIFIER_BYTES
                    || value.chars().any(char::is_control)
                {
                    Err(concat!(
                        $description,
                        " must be nonempty, control-free, and at most 256 bytes"
                    ))
                } else {
                    Ok(Self(value))
                }
            }

            pub(super) fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

bounded_identifier!(ObservationId, "observation id");
bounded_identifier!(FieldName, "field name");
bounded_identifier!(ErrorClassId, "error class id");
bounded_identifier!(ArtifactId, "artifact id");
bounded_identifier!(InvariantId, "invariant id");
bounded_identifier!(BucketId, "bucket id");
bounded_identifier!(PropertyId, "property id");
bounded_identifier!(ResourceMetricId, "resource metric id");

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Tag(Box<str>);

impl Tag {
    pub(super) fn try_new(value: impl Into<Box<str>>) -> Result<Self, &'static str> {
        let value = value.into();
        if value.len() > MAX_TAG_BYTES || value.chars().any(char::is_control) {
            Err("tag must be control-free and at most 2048 bytes")
        } else {
            Ok(Self(value))
        }
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Count(u64);

impl Count {
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(super) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Weight(u64);

impl Weight {
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(super) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Coordinate(u64);

impl Coordinate {
    pub(super) fn try_new(value: f64) -> Result<Self, &'static str> {
        if value.is_finite() {
            Ok(Self(value.to_bits()))
        } else {
            Err("coordinate must be finite")
        }
    }

    pub(super) fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum PauliAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Target {
    Qubit(u64),
    Pauli { axis: PauliAxis, qubit: u64 },
    Detector(u64),
    Observable(u64),
    MeasurementRecord(u64),
    SweepBit(u64),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Sign {
    Positive,
    Negative,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ExactValue {
    Unit,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    FloatBits(u64),
    Text(Box<str>),
    Bytes(Vec<u8>),
    Sequence(Vec<Self>),
    Record(Vec<ExactField>),
}

impl ExactValue {
    pub(super) fn from_f64(value: f64) -> Self {
        Self::FloatBits(value.to_bits())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ExactField {
    pub(super) name: FieldName,
    pub(super) value: ExactValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExactBytesOutput {
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExactValueOutput {
    pub(super) value: ExactValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalOutput {
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ExitStatus {
    Code(i32),
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum StderrClass {
    Empty,
    NonEmpty,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ArtifactState {
    Missing,
    Complete,
    Partial,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ArtifactOutcome {
    pub(super) id: ArtifactId,
    pub(super) state: ArtifactState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ErrorClassOutput {
    pub(super) error_class: Option<ErrorClassId>,
    pub(super) exit_status: ExitStatus,
    pub(super) stderr_class: StderrClass,
    pub(super) artifacts: Vec<ArtifactOutcome>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ObservationSequence<T> {
    Contractual(Vec<T>),
    NonContractual(Vec<T>),
}

impl<T: Ord> ObservationSequence<T> {
    fn equivalent(&self, actual: &Self) -> bool {
        match (self, actual) {
            (Self::Contractual(expected), Self::Contractual(actual)) => expected == actual,
            (Self::NonContractual(expected), Self::NonContractual(actual)) => {
                multiset(expected) == multiset(actual)
            }
            _ => false,
        }
    }
}

fn multiset<T: Ord>(values: &[T]) -> BTreeMap<&T, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        counts
            .entry(value)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
    counts
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StructuralObservation {
    pub(super) id: ObservationId,
    pub(super) targets: Vec<Target>,
    pub(super) sign: Sign,
    pub(super) weight: Option<Weight>,
    pub(super) count: Count,
    pub(super) tag: Option<Tag>,
    pub(super) coordinates: Vec<Coordinate>,
    pub(super) declarations: Vec<ObservationId>,
    pub(super) memberships: Vec<ObservationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StructuralOutput {
    pub(super) declared_count: Count,
    pub(super) minimum_weight: Option<Weight>,
    pub(super) observations: ObservationSequence<StructuralObservation>,
}

impl StructuralOutput {
    fn equivalent(&self, actual: &Self) -> bool {
        self.declared_count == actual.declared_count
            && self.minimum_weight == actual.minimum_weight
            && self.observations.equivalent(&actual.observations)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StateComponent {
    pub(super) sign: Sign,
    pub(super) targets: Vec<Target>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StateObservation {
    pub(super) input: ObservationId,
    pub(super) output: Vec<StateComponent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StateEquivalenceOutput {
    pub(super) observations: Vec<StateObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SemanticOutcome {
    Effect {
        before: ExactValue,
        after: ExactValue,
    },
    NoOp {
        value: ExactValue,
    },
    Rejected {
        error: ErrorClassOutput,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SemanticObservation {
    pub(super) id: ObservationId,
    pub(super) outcome: SemanticOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SemanticInvariantOutput {
    pub(super) invariant: InvariantId,
    pub(super) observations: Vec<SemanticObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BucketObservation {
    pub(super) id: BucketId,
    pub(super) count: Count,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StatisticalObservation {
    pub(super) id: ObservationId,
    pub(super) seed: u64,
    pub(super) shots: Count,
    pub(super) buckets: Vec<BucketObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StatisticalOutput {
    pub(super) observations: Vec<StatisticalObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PropertyOutcome {
    Passed,
    Rejected {
        error_class: ErrorClassId,
    },
    Failed {
        counterexample: ExactValue,
        shrink_steps: Count,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PropertyObservation {
    pub(super) id: ObservationId,
    pub(super) seed: u64,
    pub(super) case_index: u64,
    pub(super) outcome: PropertyOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PropertyOutput {
    pub(super) property: PropertyId,
    pub(super) observations: Vec<PropertyObservation>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ResourceMetric {
    Bytes,
    Allocations,
    WorkItems,
    NestingDepth,
    RecordWidth,
    ShotCount,
    Files,
    Other(ResourceMetricId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ResourceOutcome {
    WithinLimit,
    Rejected { error_class: ErrorClassId },
    Propagated { error_class: ErrorClassId },
    Violated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResourceObservation {
    pub(super) id: ObservationId,
    pub(super) kind: ResourceKind,
    pub(super) metric: ResourceMetric,
    pub(super) limit: Count,
    pub(super) observed: Count,
    pub(super) outcome: ResourceOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ResourceOutput {
    pub(super) observations: Vec<ResourceObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ComparatorOutput {
    ExactBytes(ExactBytesOutput),
    ExactValue(ExactValueOutput),
    Canonical(CanonicalOutput),
    ErrorClass(ErrorClassOutput),
    Structural(StructuralOutput),
    StateEquivalence(StateEquivalenceOutput),
    SemanticInvariant(SemanticInvariantOutput),
    Statistical(StatisticalOutput),
    Property(PropertyOutput),
    Resource(ResourceOutput),
}

impl ComparatorOutput {
    pub(super) const fn comparator(&self) -> Comparator {
        match self {
            Self::ExactBytes(_) => Comparator::ExactBytes,
            Self::ExactValue(_) => Comparator::ExactValue,
            Self::Canonical(_) => Comparator::Canonical,
            Self::ErrorClass(_) => Comparator::ErrorClass,
            Self::Structural(_) => Comparator::Structural,
            Self::StateEquivalence(_) => Comparator::StateEquivalence,
            Self::SemanticInvariant(_) => Comparator::SemanticInvariant,
            Self::Statistical(_) => Comparator::Statistical,
            Self::Property(_) => Comparator::Property,
            Self::Resource(_) => Comparator::Resource,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComparisonSide {
    Expected,
    Actual,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(super) enum ComparatorMismatch {
    #[error("expected {expected:?} output, got {actual:?} output")]
    Class {
        expected: Comparator,
        actual: Comparator,
    },

    #[error("{comparator:?} comparator outputs differ")]
    Output { comparator: Comparator },

    #[error("{side:?} statistical observation {observation:?} has duplicate bucket {bucket:?}")]
    DuplicateStatisticalBucket {
        side: ComparisonSide,
        observation: ObservationId,
        bucket: BucketId,
    },

    #[error("{side:?} statistical observation {observation:?} count total overflowed")]
    StatisticalCountOverflow {
        side: ComparisonSide,
        observation: ObservationId,
    },

    #[error(
        "{side:?} statistical observation {observation:?} records {count_total} counts for {shots} shots"
    )]
    StatisticalCountTotal {
        side: ComparisonSide,
        observation: ObservationId,
        shots: u64,
        count_total: u64,
    },
}

pub(super) fn compare_outputs(
    expected: &ComparatorOutput,
    actual: &ComparatorOutput,
) -> Result<(), ComparatorMismatch> {
    let expected_comparator = expected.comparator();
    let actual_comparator = actual.comparator();
    if expected_comparator != actual_comparator {
        return Err(ComparatorMismatch::Class {
            expected: expected_comparator,
            actual: actual_comparator,
        });
    }

    let equivalent = match (expected, actual) {
        (ComparatorOutput::ExactBytes(expected), ComparatorOutput::ExactBytes(actual)) => {
            expected == actual
        }
        (ComparatorOutput::ExactValue(expected), ComparatorOutput::ExactValue(actual)) => {
            expected == actual
        }
        (ComparatorOutput::Canonical(expected), ComparatorOutput::Canonical(actual)) => {
            expected == actual
        }
        (ComparatorOutput::ErrorClass(expected), ComparatorOutput::ErrorClass(actual)) => {
            expected == actual
        }
        (ComparatorOutput::Structural(expected), ComparatorOutput::Structural(actual)) => {
            expected.equivalent(actual)
        }
        (
            ComparatorOutput::StateEquivalence(expected),
            ComparatorOutput::StateEquivalence(actual),
        ) => expected == actual,
        (
            ComparatorOutput::SemanticInvariant(expected),
            ComparatorOutput::SemanticInvariant(actual),
        ) => expected == actual,
        (ComparatorOutput::Statistical(expected), ComparatorOutput::Statistical(actual)) => {
            return compare_statistical_shape(expected, actual);
        }
        (ComparatorOutput::Property(expected), ComparatorOutput::Property(actual)) => {
            expected == actual
        }
        (ComparatorOutput::Resource(expected), ComparatorOutput::Resource(actual)) => {
            expected == actual
        }
        _ => false,
    };

    equivalent.then_some(()).ok_or(ComparatorMismatch::Output {
        comparator: expected_comparator,
    })
}

fn compare_statistical_shape(
    expected: &StatisticalOutput,
    actual: &StatisticalOutput,
) -> Result<(), ComparatorMismatch> {
    validate_statistical_counts(ComparisonSide::Expected, expected)?;
    validate_statistical_counts(ComparisonSide::Actual, actual)?;

    let same_shape = expected.observations.len() == actual.observations.len()
        && expected
            .observations
            .iter()
            .zip(&actual.observations)
            .all(|(expected, actual)| {
                expected.id == actual.id
                    && expected.seed == actual.seed
                    && expected.shots == actual.shots
                    && expected.buckets.len() == actual.buckets.len()
                    && expected
                        .buckets
                        .iter()
                        .zip(&actual.buckets)
                        .all(|(expected, actual)| expected.id == actual.id)
            });

    same_shape.then_some(()).ok_or(ComparatorMismatch::Output {
        comparator: Comparator::Statistical,
    })
}

fn validate_statistical_counts(
    side: ComparisonSide,
    output: &StatisticalOutput,
) -> Result<(), ComparatorMismatch> {
    for observation in &output.observations {
        let mut bucket_ids = BTreeSet::new();
        let mut count_total = 0_u64;
        for bucket in &observation.buckets {
            if !bucket_ids.insert(&bucket.id) {
                return Err(ComparatorMismatch::DuplicateStatisticalBucket {
                    side,
                    observation: observation.id.clone(),
                    bucket: bucket.id.clone(),
                });
            }
            count_total = count_total.checked_add(bucket.count.get()).ok_or_else(|| {
                ComparatorMismatch::StatisticalCountOverflow {
                    side,
                    observation: observation.id.clone(),
                }
            })?;
        }
        if count_total != observation.shots.get() {
            return Err(ComparatorMismatch::StatisticalCountTotal {
                side,
                observation: observation.id.clone(),
                shots: observation.shots.get(),
                count_total,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation_id(value: &str) -> ObservationId {
        ObservationId(value.into())
    }

    fn field_name(value: &str) -> FieldName {
        FieldName(value.into())
    }

    fn error_class(value: &str) -> ErrorClassId {
        ErrorClassId(value.into())
    }

    fn artifact_id(value: &str) -> ArtifactId {
        ArtifactId(value.into())
    }

    fn invariant_id(value: &str) -> InvariantId {
        InvariantId(value.into())
    }

    fn bucket_id(value: &str) -> BucketId {
        BucketId(value.into())
    }

    fn property_id(value: &str) -> PropertyId {
        PropertyId(value.into())
    }

    fn tag(value: &str) -> Tag {
        Tag(value.into())
    }

    fn coordinate(value: f64) -> Coordinate {
        Coordinate(value.to_bits())
    }

    fn assert_mismatch(expected: &ComparatorOutput, actual: &ComparatorOutput) {
        assert!(compare_outputs(expected, actual).is_err());
    }

    #[test]
    fn exact_bytes_rejects_one_wrong_byte() {
        let expected = ComparatorOutput::ExactBytes(ExactBytesOutput {
            bytes: b"D0 L1\n".to_vec(),
        });
        let actual = ComparatorOutput::ExactBytes(ExactBytesOutput {
            bytes: b"D0 L2\n".to_vec(),
        });

        assert_mismatch(&expected, &actual);
    }

    #[test]
    fn exact_value_preserves_record_and_sequence_order() {
        let expected = ComparatorOutput::ExactValue(ExactValueOutput {
            value: ExactValue::Record(vec![
                ExactField {
                    name: field_name("targets"),
                    value: ExactValue::Sequence(vec![
                        ExactValue::Unsigned(1),
                        ExactValue::Unsigned(2),
                    ]),
                },
                ExactField {
                    name: field_name("tag"),
                    value: ExactValue::Text("kept".into()),
                },
            ]),
        });
        let reordered = ComparatorOutput::ExactValue(ExactValueOutput {
            value: ExactValue::Record(vec![
                ExactField {
                    name: field_name("targets"),
                    value: ExactValue::Sequence(vec![
                        ExactValue::Unsigned(2),
                        ExactValue::Unsigned(1),
                    ]),
                },
                ExactField {
                    name: field_name("tag"),
                    value: ExactValue::Text("kept".into()),
                },
            ]),
        });

        assert_mismatch(&expected, &reordered);
    }

    #[test]
    fn canonical_rejects_noncanonical_output_bytes() {
        let expected = ComparatorOutput::Canonical(CanonicalOutput {
            bytes: b"M 0 1\n".to_vec(),
        });
        let reordered = ComparatorOutput::Canonical(CanonicalOutput {
            bytes: b"M 1 0\n".to_vec(),
        });

        assert_mismatch(&expected, &reordered);
    }

    fn error_output(
        error: &str,
        exit_status: ExitStatus,
        stderr_class: StderrClass,
    ) -> ComparatorOutput {
        ComparatorOutput::ErrorClass(ErrorClassOutput {
            error_class: Some(error_class(error)),
            exit_status,
            stderr_class,
            artifacts: vec![ArtifactOutcome {
                id: artifact_id("result-file"),
                state: ArtifactState::Partial,
            }],
        })
    }

    #[test]
    fn error_class_rejects_exit_stderr_error_and_artifact_mutations() {
        let expected = error_output("invalid-target", ExitStatus::Code(2), StderrClass::NonEmpty);
        let wrong_exit = error_output("invalid-target", ExitStatus::Code(1), StderrClass::NonEmpty);
        let wrong_stderr = error_output("invalid-target", ExitStatus::Code(2), StderrClass::Empty);
        let wrong_error = error_output(
            "invalid-probability",
            ExitStatus::Code(2),
            StderrClass::NonEmpty,
        );
        let wrong_artifact = ComparatorOutput::ErrorClass(ErrorClassOutput {
            error_class: Some(error_class("invalid-target")),
            exit_status: ExitStatus::Code(2),
            stderr_class: StderrClass::NonEmpty,
            artifacts: vec![ArtifactOutcome {
                id: artifact_id("result-file"),
                state: ArtifactState::Complete,
            }],
        });

        assert_mismatch(&expected, &wrong_exit);
        assert_mismatch(&expected, &wrong_stderr);
        assert_mismatch(&expected, &wrong_error);
        assert_mismatch(&expected, &wrong_artifact);
    }

    fn structural_observation(
        id: &str,
        target: Target,
        weight: u64,
        count: u64,
        tag_value: &str,
        coordinate_value: f64,
    ) -> StructuralObservation {
        StructuralObservation {
            id: observation_id(id),
            targets: vec![target],
            sign: Sign::Positive,
            weight: Some(Weight::new(weight)),
            count: Count::new(count),
            tag: Some(tag(tag_value)),
            coordinates: vec![coordinate(coordinate_value)],
            declarations: vec![observation_id("declared-detector")],
            memberships: vec![observation_id("component-a")],
        }
    }

    fn structural_output(observations: Vec<StructuralObservation>) -> ComparatorOutput {
        ComparatorOutput::Structural(StructuralOutput {
            declared_count: Count::new(2),
            minimum_weight: Some(Weight::new(3)),
            observations: ObservationSequence::Contractual(observations),
        })
    }

    #[test]
    fn structural_rejects_wrong_target_weight_count_tag_and_coordinate() {
        let expected = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(1),
            3,
            2,
            "logical-x",
            1.5,
        )]);
        let wrong_target = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(2),
            3,
            2,
            "logical-x",
            1.5,
        )]);
        let wrong_weight = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(1),
            4,
            2,
            "logical-x",
            1.5,
        )]);
        let wrong_count = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(1),
            3,
            3,
            "logical-x",
            1.5,
        )]);
        let wrong_tag = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(1),
            3,
            2,
            "logical-z",
            1.5,
        )]);
        let wrong_coordinate = structural_output(vec![structural_observation(
            "edge-a",
            Target::Detector(1),
            3,
            2,
            "logical-x",
            2.5,
        )]);

        assert_mismatch(&expected, &wrong_target);
        assert_mismatch(&expected, &wrong_weight);
        assert_mismatch(&expected, &wrong_count);
        assert_mismatch(&expected, &wrong_tag);
        assert_mismatch(&expected, &wrong_coordinate);
    }

    #[test]
    fn structural_only_ignores_order_when_explicitly_noncontractual() {
        let first = structural_observation("first", Target::Detector(1), 3, 1, "a", 1.0);
        let second = structural_observation("second", Target::Detector(2), 3, 1, "b", 2.0);
        let contractual = structural_output(vec![first.clone(), second.clone()]);
        let reordered_contractual = structural_output(vec![second.clone(), first.clone()]);
        assert_mismatch(&contractual, &reordered_contractual);

        let expected = ComparatorOutput::Structural(StructuralOutput {
            declared_count: Count::new(2),
            minimum_weight: Some(Weight::new(3)),
            observations: ObservationSequence::NonContractual(vec![first.clone(), second.clone()]),
        });
        let reordered = ComparatorOutput::Structural(StructuralOutput {
            declared_count: Count::new(2),
            minimum_weight: Some(Weight::new(3)),
            observations: ObservationSequence::NonContractual(vec![second.clone(), first.clone()]),
        });
        let missing_duplicate = ComparatorOutput::Structural(StructuralOutput {
            declared_count: Count::new(2),
            minimum_weight: Some(Weight::new(3)),
            observations: ObservationSequence::NonContractual(vec![first.clone(), first]),
        });

        assert_eq!(compare_outputs(&expected, &reordered), Ok(()));
        assert_mismatch(&expected, &missing_duplicate);
    }

    fn state_output(sign: Sign, observation_count: usize) -> ComparatorOutput {
        let observation = StateObservation {
            input: observation_id("plus-x"),
            output: vec![StateComponent {
                sign,
                targets: vec![Target::Pauli {
                    axis: PauliAxis::X,
                    qubit: 0,
                }],
            }],
        };
        ComparatorOutput::StateEquivalence(StateEquivalenceOutput {
            observations: vec![observation; observation_count],
        })
    }

    #[test]
    fn state_equivalence_rejects_wrong_sign_and_missing_or_extra_observations() {
        let expected = state_output(Sign::Positive, 1);
        let wrong_sign = state_output(Sign::Negative, 1);
        let missing = state_output(Sign::Positive, 0);
        let extra = state_output(Sign::Positive, 2);

        assert_mismatch(&expected, &wrong_sign);
        assert_mismatch(&expected, &missing);
        assert_mismatch(&expected, &extra);
    }

    fn semantic_output(observations: Vec<SemanticObservation>) -> ComparatorOutput {
        ComparatorOutput::SemanticInvariant(SemanticInvariantOutput {
            invariant: invariant_id("folded-unrolled-equivalence"),
            observations,
        })
    }

    #[test]
    fn semantic_invariant_rejects_wrong_outcome_and_observation_shape() {
        let effect = SemanticObservation {
            id: observation_id("positive"),
            outcome: SemanticOutcome::Effect {
                before: ExactValue::Unsigned(1),
                after: ExactValue::Unsigned(2),
            },
        };
        let no_op = SemanticObservation {
            id: observation_id("no-op"),
            outcome: SemanticOutcome::NoOp {
                value: ExactValue::Unsigned(0),
            },
        };
        let expected = semantic_output(vec![effect.clone(), no_op.clone()]);
        let wrong_outcome = semantic_output(vec![
            SemanticObservation {
                id: observation_id("positive"),
                outcome: SemanticOutcome::NoOp {
                    value: ExactValue::Unsigned(2),
                },
            },
            no_op.clone(),
        ]);
        let missing = semantic_output(vec![effect.clone()]);
        let extra = semantic_output(vec![effect, no_op.clone(), no_op]);

        assert_mismatch(&expected, &wrong_outcome);
        assert_mismatch(&expected, &missing);
        assert_mismatch(&expected, &extra);
    }

    fn statistical_observation(
        id: &str,
        seed: u64,
        shots: u64,
        buckets: &[(&str, u64)],
    ) -> StatisticalObservation {
        StatisticalObservation {
            id: observation_id(id),
            seed,
            shots: Count::new(shots),
            buckets: buckets
                .iter()
                .map(|(id, count)| BucketObservation {
                    id: bucket_id(id),
                    count: Count::new(*count),
                })
                .collect(),
        }
    }

    fn statistical_output(observations: Vec<StatisticalObservation>) -> ComparatorOutput {
        ComparatorOutput::Statistical(StatisticalOutput { observations })
    }

    #[test]
    fn statistical_adapter_checks_shape_and_totals_without_probability_planning() {
        let expected_observation =
            statistical_observation("primary", 17, 5, &[("zero", 2), ("one", 3)]);
        let different_valid_counts =
            statistical_observation("primary", 17, 5, &[("zero", 1), ("one", 4)]);
        let expected = statistical_output(vec![expected_observation.clone()]);
        let actual = statistical_output(vec![different_valid_counts]);
        assert_eq!(compare_outputs(&expected, &actual), Ok(()));

        let wrong_total = statistical_output(vec![statistical_observation(
            "primary",
            17,
            5,
            &[("zero", 1), ("one", 3)],
        )]);
        let wrong_bucket_order = statistical_output(vec![statistical_observation(
            "primary",
            17,
            5,
            &[("one", 3), ("zero", 2)],
        )]);
        let duplicate_bucket = statistical_output(vec![statistical_observation(
            "primary",
            17,
            5,
            &[("zero", 2), ("zero", 3)],
        )]);
        let missing = statistical_output(Vec::new());
        let extra = statistical_output(vec![
            expected_observation.clone(),
            statistical_observation("secondary", 19, 1, &[("zero", 1)]),
        ]);

        assert_mismatch(&expected, &wrong_total);
        assert_mismatch(&expected, &wrong_bucket_order);
        assert_mismatch(&expected, &duplicate_bucket);
        assert_mismatch(&expected, &missing);
        assert_mismatch(&expected, &extra);
    }

    fn property_observation(id: &str, case_index: u64) -> PropertyObservation {
        PropertyObservation {
            id: observation_id(id),
            seed: 31,
            case_index,
            outcome: PropertyOutcome::Passed,
        }
    }

    fn property_output(observations: Vec<PropertyObservation>) -> ComparatorOutput {
        ComparatorOutput::Property(PropertyOutput {
            property: property_id("parse-print-parse"),
            observations,
        })
    }

    #[test]
    fn property_adapter_rejects_wrong_outcome_order_and_observation_shape() {
        let first = property_observation("case-zero", 0);
        let second = property_observation("case-one", 1);
        let expected = property_output(vec![first.clone(), second.clone()]);
        let wrong_outcome = property_output(vec![
            PropertyObservation {
                id: observation_id("case-zero"),
                seed: 31,
                case_index: 0,
                outcome: PropertyOutcome::Failed {
                    counterexample: ExactValue::Text("M !".into()),
                    shrink_steps: Count::new(2),
                },
            },
            second.clone(),
        ]);
        let reordered = property_output(vec![second.clone(), first.clone()]);
        let missing = property_output(vec![first.clone()]);
        let extra = property_output(vec![first, second.clone(), second]);

        assert_mismatch(&expected, &wrong_outcome);
        assert_mismatch(&expected, &reordered);
        assert_mismatch(&expected, &missing);
        assert_mismatch(&expected, &extra);
    }

    fn resource_observation(id: &str, limit: u64, outcome: ResourceOutcome) -> ResourceObservation {
        ResourceObservation {
            id: observation_id(id),
            kind: ResourceKind::BoundedMaterialized,
            metric: ResourceMetric::Bytes,
            limit: Count::new(limit),
            observed: Count::new(1_024),
            outcome,
        }
    }

    fn resource_output(observations: Vec<ResourceObservation>) -> ComparatorOutput {
        ComparatorOutput::Resource(ResourceOutput { observations })
    }

    #[test]
    fn resource_adapter_rejects_wrong_contract_outcome_and_observation_shape() {
        let primary = resource_observation("cap", 2_048, ResourceOutcome::WithinLimit);
        let expected = resource_output(vec![primary.clone()]);
        let wrong_limit = resource_output(vec![resource_observation(
            "cap",
            4_096,
            ResourceOutcome::WithinLimit,
        )]);
        let wrong_outcome = resource_output(vec![resource_observation(
            "cap",
            2_048,
            ResourceOutcome::Violated,
        )]);
        let missing = resource_output(Vec::new());
        let extra = resource_output(vec![
            primary.clone(),
            resource_observation("cap-plus-one", 2_048, ResourceOutcome::WithinLimit),
        ]);

        assert_mismatch(&expected, &wrong_limit);
        assert_mismatch(&expected, &wrong_outcome);
        assert_mismatch(&expected, &missing);
        assert_mismatch(&expected, &extra);
    }

    #[test]
    fn comparator_class_mismatch_is_typed() {
        let expected = ComparatorOutput::ExactBytes(ExactBytesOutput { bytes: Vec::new() });
        let actual = ComparatorOutput::Canonical(CanonicalOutput { bytes: Vec::new() });

        assert_eq!(
            compare_outputs(&expected, &actual),
            Err(ComparatorMismatch::Class {
                expected: Comparator::ExactBytes,
                actual: Comparator::Canonical,
            })
        );
    }
}
