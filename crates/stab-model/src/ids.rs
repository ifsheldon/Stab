use crate::{ModelError, ModelResult};

pub(crate) const STIM_TARGET_VALUE_LIMIT: u32 = 1 << 24;
const MIN_MEASUREMENT_RECORD_OFFSET: i32 = -(1 << 24) + 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QubitId(u32);

impl QubitId {
    pub fn new(value: u32) -> ModelResult<Self> {
        if value >= STIM_TARGET_VALUE_LIMIT {
            return Err(ModelError::invalid_domain_value("qubit id", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservableId(u64);

impl ObservableId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct MeasureRecordOffset(MeasureRecordOffsetRepr);

impl MeasureRecordOffset {
    pub fn try_new(value: i32) -> ModelResult<Self> {
        if !(MIN_MEASUREMENT_RECORD_OFFSET..0).contains(&value) {
            return Err(ModelError::invalid_domain_value(
                "measurement record offset",
                value,
            ));
        }
        Self::from_valid_lookback(value)
    }

    pub fn get(self) -> i32 {
        self.raw_value()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CircuitDetectorId(u64);

impl CircuitDetectorId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// Zero-based index of a `TICK` boundary within a circuit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CircuitTick(u64);

impl CircuitTick {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A finite, nonnegative absolute tolerance for approximate model comparison.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct AbsoluteTolerance(f64);

impl AbsoluteTolerance {
    pub fn try_new(value: f64) -> ModelResult<Self> {
        if !value.is_finite() || value < 0.0 {
            return Err(ModelError::invalid_domain_value(
                "absolute tolerance",
                value,
            ));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RepeatCount(u64);

impl RepeatCount {
    pub fn try_new(value: u64) -> ModelResult<Self> {
        if value == 0 {
            return Err(ModelError::invalid_domain_value("repeat count", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemRepeatCount(u64);

impl DemRepeatCount {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Probability(f64);

impl Probability {
    #[inline(always)]
    pub fn try_new(value: f64) -> ModelResult<Self> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ModelError::invalid_domain_value("probability", value));
        }
        Ok(Self(value))
    }

    #[inline(always)]
    pub(crate) fn from_valid_probability(value: f64) -> Self {
        debug_assert!(value.is_finite());
        debug_assert!((0.0..=1.0).contains(&value));
        Self(value)
    }

    #[inline(always)]
    pub fn get(self) -> f64 {
        self.0
    }

    /// Formats this probability exactly as pinned Stim prints doubles in
    /// generated-circuit headers and instruction arguments (C++ ostream
    /// default formatting: six significant digits, scientific outside the
    /// fixed range).
    pub fn stim_text(self) -> ProbabilityStimText {
        ProbabilityStimText(self)
    }
}

/// Stim-canonical text form of a [`Probability`].
#[derive(Debug)]
pub struct ProbabilityStimText(Probability);

impl std::fmt::Display for ProbabilityStimText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(crate::circuit::printing::format_float(self.0.get()).as_str())
    }
}

impl MeasureRecordOffset {
    fn from_valid_lookback(value: i32) -> ModelResult<Self> {
        let value = std::num::NonZeroI32::new(value)
            .ok_or_else(|| ModelError::invalid_domain_value("measurement record offset", value))?;
        Ok(Self(MeasureRecordOffsetRepr::Lookback(value)))
    }

    /// Preserves Stim v1.16's distinct parsed `rec[-0]` target.
    pub(crate) fn from_stim_text(value: i32) -> ModelResult<Self> {
        if value == 0 {
            return Ok(Self(MeasureRecordOffsetRepr::NegativeZero));
        }
        Self::try_new(value)
    }

    fn raw_value(self) -> i32 {
        match self.0 {
            MeasureRecordOffsetRepr::Lookback(value) => value.get(),
            MeasureRecordOffsetRepr::NegativeZero => 0,
        }
    }

    /// Returns whether this value preserves Stim's distinct parsed `rec[-0]` spelling.
    pub fn is_negative_zero(self) -> bool {
        matches!(self.0, MeasureRecordOffsetRepr::NegativeZero)
    }

    /// Formats this offset exactly as it appears inside a Stim target.
    pub fn stim_text(self) -> MeasureRecordOffsetText {
        MeasureRecordOffsetText(self)
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum MeasureRecordOffsetRepr {
    Lookback(std::num::NonZeroI32),
    NegativeZero,
}

impl std::fmt::Debug for MeasureRecordOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MeasureRecordOffset")
            .field(&self.get())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasureRecordOffsetText(MeasureRecordOffset);

impl std::fmt::Display for MeasureRecordOffsetText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_negative_zero() {
            f.write_str("-0")
        } else {
            std::fmt::Display::fmt(&self.0.get(), f)
        }
    }
}
