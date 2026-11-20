use std::fmt::{Debug, Display, Formatter, NumBuffer};
use std::str::FromStr;

use super::parser::parse_unsigned_dem_text_value;
use crate::{ModelError, ModelResult};

/// Largest relative detector id a DEM target can name.
pub(super) const MAX_DEM_DETECTOR_ID: u64 = (1_u64 << 62) - 1;

/// Bits above the detector id range that select the target kind.
const KIND_SHIFT: u32 = 62;
const OBSERVABLE_KIND: u64 = 1;
const OBSERVABLE_BITS: u64 = OBSERVABLE_KIND << KIND_SHIFT;
const SEPARATOR_BITS: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemDetectorId(u64);

impl DemDetectorId {
    pub fn try_new(value: u64) -> ModelResult<Self> {
        if value > MAX_DEM_DETECTOR_ID {
            return Err(ModelError::invalid_detector_error_model(format!(
                "detector id {value} exceeds {MAX_DEM_DETECTOR_ID}"
            )));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns this id lowered by `base`, or `None` when it is below `base`.
    pub(crate) const fn checked_sub(self, base: u64) -> Option<Self> {
        match self.0.checked_sub(base) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemObservableId(u32);

impl DemObservableId {
    pub fn try_new(value: u64) -> ModelResult<Self> {
        let value = u32::try_from(value).map_err(|_| {
            ModelError::invalid_detector_error_model(format!(
                "observable id {value} exceeds {}",
                u32::MAX
            ))
        })?;
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0 as u64
    }
}

/// One target of a DEM instruction: a relative detector, a logical observable, or the `^`
/// separator between the components of a decomposed error.
///
/// The target packs into eight bytes. Its order, equality, and hash follow [`DemTargetKind`]:
/// every detector sorts before every observable, and both sort before the separator, each by id.
/// Use [`Self::kind`] to match on a target.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemTarget(u64);

const _: () = assert!(size_of::<DemTarget>() == 8);

/// Matchable view of a [`DemTarget`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DemTargetKind {
    RelativeDetector(DemDetectorId),
    LogicalObservable(DemObservableId),
    Separator,
}

impl DemTarget {
    /// The `^` separator between the components of a decomposed error.
    pub const SEPARATOR: Self = Self(SEPARATOR_BITS);

    pub fn relative_detector(id: u64) -> ModelResult<Self> {
        DemDetectorId::try_new(id).map(Self::from_detector)
    }

    pub fn logical_observable(id: u64) -> ModelResult<Self> {
        DemObservableId::try_new(id).map(Self::from_observable)
    }

    pub const fn separator() -> Self {
        Self::SEPARATOR
    }

    pub const fn from_detector(detector: DemDetectorId) -> Self {
        Self(detector.0)
    }

    pub const fn from_observable(observable: DemObservableId) -> Self {
        Self(OBSERVABLE_BITS | observable.0 as u64)
    }

    pub const fn kind(self) -> DemTargetKind {
        match self.0 >> KIND_SHIFT {
            0 => DemTargetKind::RelativeDetector(DemDetectorId(self.0)),
            OBSERVABLE_KIND => {
                DemTargetKind::LogicalObservable(DemObservableId(self.observable_bits()))
            }
            _ => DemTargetKind::Separator,
        }
    }

    pub const fn detector(self) -> Option<DemDetectorId> {
        if self.is_detector() {
            Some(DemDetectorId(self.0))
        } else {
            None
        }
    }

    pub const fn observable(self) -> Option<DemObservableId> {
        if self.is_observable() {
            Some(DemObservableId(self.observable_bits()))
        } else {
            None
        }
    }

    pub const fn is_detector(self) -> bool {
        self.0 >> KIND_SHIFT == 0
    }

    pub const fn is_observable(self) -> bool {
        self.0 >> KIND_SHIFT == OBSERVABLE_KIND
    }

    pub const fn is_separator(self) -> bool {
        self.0 == SEPARATOR_BITS
    }

    /// Returns this target with a detector id raised by `detector_offset`; other targets are
    /// returned unchanged.
    ///
    /// # Errors
    ///
    /// Fails when the shifted detector id exceeds the DEM detector id range.
    pub fn shifted(self, detector_offset: u64) -> ModelResult<Self> {
        let Some(detector) = self.detector() else {
            return Ok(self);
        };
        let shifted = detector.get().checked_add(detector_offset).ok_or_else(|| {
            ModelError::invalid_detector_error_model("relative detector id overflowed")
        })?;
        DemDetectorId::try_new(shifted).map(Self::from_detector)
    }

    /// The low bits of an observable target, which hold its `u32` id.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "observable targets store a u32 id in their low 32 bits"
    )]
    const fn observable_bits(self) -> u32 {
        self.0 as u32
    }

    /// Splits this target's DEM text into its letter prefix and its decimal digits.
    pub(super) fn text_parts(self, digits: &mut NumBuffer<u64>) -> (&'static str, &str) {
        match self.kind() {
            DemTargetKind::RelativeDetector(id) => ("D", id.get().format_into(digits)),
            DemTargetKind::LogicalObservable(id) => ("L", id.get().format_into(digits)),
            DemTargetKind::Separator => ("^", ""),
        }
    }
}

/// A [`DemTarget`] that is a relative detector by construction, so its id needs no check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DetectorTarget(DemTarget);

impl DetectorTarget {
    pub(super) const fn new(detector: DemDetectorId) -> Self {
        Self(DemTarget::from_detector(detector))
    }

    pub(super) const fn id(self) -> DemDetectorId {
        // Only `new` builds this target, from a checked id stored unchanged.
        DemDetectorId(self.0.0)
    }

    pub(super) const fn as_target(&self) -> &DemTarget {
        &self.0
    }

    /// Returns this detector raised by `detector_offset`.
    ///
    /// # Errors
    ///
    /// Fails when the shifted id exceeds the DEM detector id range.
    pub(super) fn shifted(self, detector_offset: u64) -> ModelResult<Self> {
        let shifted = self
            .id()
            .get()
            .checked_add(detector_offset)
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model("relative detector id overflowed")
            })?;
        DemDetectorId::try_new(shifted).map(Self::new)
    }
}

/// A [`DemTarget`] that is a logical observable by construction, so its id needs no check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ObservableTarget(DemTarget);

impl ObservableTarget {
    pub(super) const fn new(observable: DemObservableId) -> Self {
        Self(DemTarget::from_observable(observable))
    }

    pub(super) const fn id(self) -> DemObservableId {
        // Only `new` builds this target, so its low bits hold the observable id.
        DemObservableId(self.0.observable_bits())
    }

    pub(super) const fn as_target(&self) -> &DemTarget {
        &self.0
    }
}

impl From<DemDetectorId> for DemTarget {
    fn from(detector: DemDetectorId) -> Self {
        Self::from_detector(detector)
    }
}

impl From<DemObservableId> for DemTarget {
    fn from(observable: DemObservableId) -> Self {
        Self::from_observable(observable)
    }
}

impl From<DemTargetKind> for DemTarget {
    fn from(kind: DemTargetKind) -> Self {
        match kind {
            DemTargetKind::RelativeDetector(detector) => Self::from_detector(detector),
            DemTargetKind::LogicalObservable(observable) => Self::from_observable(observable),
            DemTargetKind::Separator => Self::SEPARATOR,
        }
    }
}

impl Debug for DemTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.kind(), f)
    }
}

impl Display for DemTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut digits = NumBuffer::new();
        let (prefix, number) = self.text_parts(&mut digits);
        f.write_str(prefix)?;
        f.write_str(number)
    }
}

impl FromStr for DemTarget {
    type Err = ModelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw == "^" {
            return Ok(Self::SEPARATOR);
        }
        if let Some(value) = raw.strip_prefix('D') {
            return Self::relative_detector(parse_unsigned_dem_text_value(
                value,
                "relative detector target",
            )?);
        }
        if let Some(value) = raw.strip_prefix('L') {
            return Self::logical_observable(parse_unsigned_dem_text_value(
                value,
                "logical observable target",
            )?);
        }
        Err(ModelError::invalid_detector_error_model(format!(
            "invalid DEM target {raw:?}"
        )))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "target fixtures are valid by construction"
)]
mod tests {
    use super::*;

    #[test]
    fn packed_order_matches_kind_order_at_every_boundary() {
        let targets = [
            DemTarget::relative_detector(0).unwrap(),
            DemTarget::relative_detector(1).unwrap(),
            DemTarget::relative_detector(MAX_DEM_DETECTOR_ID).unwrap(),
            DemTarget::logical_observable(0).unwrap(),
            DemTarget::logical_observable(1).unwrap(),
            DemTarget::logical_observable(u64::from(u32::MAX)).unwrap(),
            DemTarget::SEPARATOR,
        ];
        for left in targets {
            for right in targets {
                assert_eq!(left.cmp(&right), left.kind().cmp(&right.kind()));
                assert_eq!(left == right, left.kind() == right.kind());
                assert_eq!(DemTarget::from(left.kind()), left);
            }
        }
        assert!(DemTarget::relative_detector(MAX_DEM_DETECTOR_ID + 1).is_err());
        assert!(DemTarget::logical_observable(u64::from(u32::MAX) + 1).is_err());
    }
}
