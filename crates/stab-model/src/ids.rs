use crate::{ModelError, ModelResult};

pub(crate) const STIM_TARGET_VALUE_LIMIT: u32 = 1 << 24;
const MIN_MEASUREMENT_RECORD_OFFSET: i32 = -(1 << 24) + 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QubitId(u32);

impl QubitId {
    /// The largest qubit id a Stim target can name.
    pub const MAX: Self = Self(STIM_TARGET_VALUE_LIMIT - 1);

    pub fn new(value: u32) -> ModelResult<Self> {
        if value >= STIM_TARGET_VALUE_LIMIT {
            return Err(ModelError::invalid_domain_value("qubit id", value));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    /// Rebuilds an id from the 24 value bits of a packed target, which are always in range.
    #[inline(always)]
    pub(crate) const fn from_packed(value: u32) -> Self {
        Self(value & (STIM_TARGET_VALUE_LIMIT - 1))
    }
}

/// A logical observable id, shared by circuit `OBSERVABLE_INCLUDE` arguments and DEM `L` targets.
///
/// The domain is `0..=u32::MAX`, the range the DEM file format accepts, so every id a circuit
/// holds is one a detector error model can represent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObservableId(u32);

impl ObservableId {
    /// The largest observable id.
    pub const MAX: Self = Self(u32::MAX);

    /// Builds an id, refusing values above [`Self::MAX`].
    pub fn try_new(value: u64) -> ModelResult<Self> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| ModelError::invalid_domain_value("observable id", value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub(crate) const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    /// Converts a gate argument to an id when it is an integral value in the id domain.
    pub(crate) fn from_gate_arg(arg: f64) -> Option<Self> {
        if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 || arg > f64::from(u32::MAX) {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the finite, integral, nonnegative, and at-most-u32::MAX checks make this exact"
        )]
        Some(Self(arg as u32))
    }
}

impl From<u32> for ObservableId {
    fn from(value: u32) -> Self {
        Self(value)
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
    /// The impossible event, `0`.
    pub const ZERO: Self = Self(0.0);
    /// The certain event, `1`.
    pub const ONE: Self = Self(1.0);

    #[inline(always)]
    pub fn try_new(value: f64) -> ModelResult<Self> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ModelError::invalid_domain_value("probability", value));
        }
        Ok(Self(value))
    }

    #[inline(always)]
    pub fn get(self) -> f64 {
        self.0
    }

    /// Borrows the stored value, so a model can lend it as a one-element argument slice.
    #[inline(always)]
    pub(crate) const fn as_f64_ref(&self) -> &f64 {
        &self.0
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

    /// Rebuilds an offset from the lookback distance a packed target stores, where distance zero
    /// is Stim's distinct `rec[-0]` and every distance is below `2^24`.
    #[inline(always)]
    pub(crate) const fn from_packed_distance(distance: u32) -> Self {
        let lookback = 0_i32.wrapping_sub_unsigned(distance & (STIM_TARGET_VALUE_LIMIT - 1));
        match std::num::NonZeroI32::new(lookback) {
            Some(lookback) => Self(MeasureRecordOffsetRepr::Lookback(lookback)),
            None => Self(MeasureRecordOffsetRepr::NegativeZero),
        }
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
