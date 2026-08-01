use std::fmt::{self, Display};
use std::ops::ControlFlow;

use super::traversal::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemTraversal, FoldedDemVisitor,
    shifted_detector,
};
use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemObservableId, DemRepeatBlock, DemTarget,
    DetectorErrorModel,
};
use crate::{ModelError, ModelResult, Probability};

/// Caller-owned limits for semantic DEM error-mechanism traversal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DemErrorMechanismTraversalLimits {
    max_mechanisms: u64,
    max_instruction_visits: u64,
}

impl DemErrorMechanismTraversalLimits {
    pub const fn new(max_mechanisms: u64, max_instruction_visits: u64) -> Self {
        Self {
            max_mechanisms,
            max_instruction_visits,
        }
    }

    pub const fn max_mechanisms(self) -> u64 {
        self.max_mechanisms
    }

    pub const fn max_instruction_visits(self) -> u64 {
        self.max_instruction_visits
    }
}

/// One absolute target in a represented DEM error mechanism.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DemErrorTarget {
    Detector(DemDetectorId),
    Observable(DemObservableId),
    Separator,
}

/// Zero-allocation iterator over absolute targets in one error mechanism.
#[derive(Clone, Debug)]
pub struct DemErrorTargetIter<'a> {
    targets: std::slice::Iter<'a, DemTarget>,
    detector_offset: u64,
}

impl Iterator for DemErrorTargetIter<'_> {
    type Item = ModelResult<DemErrorTarget>;

    fn next(&mut self) -> Option<Self::Item> {
        self.targets.next().map(|target| match *target {
            DemTarget::RelativeDetector(detector) => {
                shifted_detector(detector, self.detector_offset).map(DemErrorTarget::Detector)
            }
            DemTarget::LogicalObservable(observable) => Ok(DemErrorTarget::Observable(observable)),
            DemTarget::Separator => Ok(DemErrorTarget::Separator),
            DemTarget::Numeric(value) => Err(ModelError::invalid_detector_error_model(format!(
                "error mechanism contains invalid numeric target {value}"
            ))),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.targets.size_hint()
    }
}

impl ExactSizeIterator for DemErrorTargetIter<'_> {}

/// Borrowed semantic view of one represented DEM error mechanism.
#[derive(Clone, Copy, Debug)]
pub struct DemErrorMechanismView<'a> {
    probability: Probability,
    targets: &'a [DemTarget],
    detector_offset: u64,
    tag_bytes: Option<&'a [u8]>,
}

impl<'a> DemErrorMechanismView<'a> {
    pub const fn probability(self) -> Probability {
        self.probability
    }

    pub fn targets(self) -> DemErrorTargetIter<'a> {
        DemErrorTargetIter {
            targets: self.targets.iter(),
            detector_offset: self.detector_offset,
        }
    }

    pub const fn tag_bytes(self) -> Option<&'a [u8]> {
        self.tag_bytes
    }
}

/// Receives represented error mechanisms in semantic execution order.
pub trait DemErrorMechanismVisitor {
    type Error;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error>;
}

/// Failure while visiting bounded semantic DEM error mechanisms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DemErrorMechanismVisitError<VisitorError> {
    Model(ModelError),
    MechanismLimit { actual: u64, limit: u64 },
    InstructionVisitLimit { actual_at_least: u64, limit: u64 },
    Visitor(VisitorError),
}

impl<VisitorError> From<ModelError> for DemErrorMechanismVisitError<VisitorError> {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl<VisitorError: Display> Display for DemErrorMechanismVisitError<VisitorError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => Display::fmt(error, formatter),
            Self::MechanismLimit { actual, limit } => write!(
                formatter,
                "DEM error-mechanism traversal supports at most {limit} represented mechanisms, got {actual}"
            ),
            Self::InstructionVisitLimit {
                actual_at_least,
                limit,
            } => write!(
                formatter,
                "DEM error-mechanism traversal supports at most {limit} represented instruction visits, got at least {actual_at_least}"
            ),
            Self::Visitor(error) => {
                write!(formatter, "DEM error-mechanism visitor failed: {error}")
            }
        }
    }
}

impl<VisitorError> std::error::Error for DemErrorMechanismVisitError<VisitorError>
where
    VisitorError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Visitor(error) => Some(error),
            Self::MechanismLimit { .. } | Self::InstructionVisitLimit { .. } => None,
        }
    }
}

impl DetectorErrorModel {
    /// Visits represented error mechanisms without materializing a flattened DEM.
    ///
    /// The represented mechanism count is admitted before the first callback. Repeat bodies with no
    /// errors are skipped in constant compact work, while all visited instructions in error-bearing
    /// paths count against the independent instruction-work limit.
    pub fn try_visit_error_mechanisms<Visitor>(
        &self,
        limits: DemErrorMechanismTraversalLimits,
        visitor: &mut Visitor,
    ) -> Result<ControlFlow<()>, DemErrorMechanismVisitError<Visitor::Error>>
    where
        Visitor: DemErrorMechanismVisitor,
    {
        let traversal = FoldedDemTraversal::new(self)?;
        traversal.validate_repeat_depth("error-mechanism")?;
        let mechanism_count = traversal.root().summary().error_count()?;
        if mechanism_count > limits.max_mechanisms() {
            return Err(DemErrorMechanismVisitError::MechanismLimit {
                actual: mechanism_count,
                limit: limits.max_mechanisms(),
            });
        }
        if mechanism_count == 0 {
            return Ok(ControlFlow::Continue(()));
        }

        let repeat_depth =
            u64::try_from(traversal.root().summary().max_repeat_depth()).map_err(|_| {
                ModelError::invalid_detector_error_model(
                    "DEM error-mechanism repeat depth does not fit u64",
                )
            })?;
        let max_expanded_iterations = mechanism_count
            .checked_mul(repeat_depth.max(1))
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model(
                    "DEM error-mechanism expansion budget overflowed",
                )
            })?;
        let mut adapter = ErrorMechanismAdapter {
            visitor,
            limits,
            instruction_visits: 0,
            max_expanded_iterations,
        };
        traversal.try_visit(&mut adapter)
    }
}

struct ErrorMechanismAdapter<'a, Visitor> {
    visitor: &'a mut Visitor,
    limits: DemErrorMechanismTraversalLimits,
    instruction_visits: u64,
    max_expanded_iterations: u64,
}

impl<Visitor> FoldedDemVisitor for ErrorMechanismAdapter<'_, Visitor>
where
    Visitor: DemErrorMechanismVisitor,
{
    type Error = DemErrorMechanismVisitError<Visitor::Error>;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> Result<ControlFlow<()>, Self::Error> {
        self.instruction_visits = self.instruction_visits.checked_add(1).ok_or(
            DemErrorMechanismVisitError::InstructionVisitLimit {
                actual_at_least: u64::MAX,
                limit: self.limits.max_instruction_visits(),
            },
        )?;
        if self.instruction_visits > self.limits.max_instruction_visits() {
            return Err(DemErrorMechanismVisitError::InstructionVisitLimit {
                actual_at_least: self.instruction_visits,
                limit: self.limits.max_instruction_visits(),
            });
        }
        if instruction.kind() != DemInstructionKind::Error {
            return Ok(ControlFlow::Continue(()));
        }

        let probability = match instruction.args() {
            [probability] => Probability::try_new(*probability)?,
            _ => {
                return Err(ModelError::invalid_detector_error_model(
                    "error instruction does not contain exactly one probability",
                )
                .into());
            }
        };
        let mechanism = DemErrorMechanismView {
            probability,
            targets: instruction.targets(),
            detector_offset: state.detector_offset(),
            tag_bytes: instruction.tag_bytes(),
        };
        self.visitor
            .visit_error_mechanism(mechanism)
            .map_err(DemErrorMechanismVisitError::Visitor)
    }

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> Result<DemRepeatSelection, Self::Error> {
        if body.summary().error_count()? == 0 {
            return Ok(DemRepeatSelection::Skip);
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: self.max_expanded_iterations,
            context: "error-mechanism",
        })
    }
}
