use crate::{CircuitError, CircuitResult};

pub(crate) const STIM_TARGET_VALUE_LIMIT: u32 = 1 << 24;
const MIN_MEASUREMENT_RECORD_OFFSET: i32 = -(1 << 24) + 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QubitId(u32);

impl QubitId {
    pub fn new(value: u32) -> CircuitResult<Self> {
        if value >= STIM_TARGET_VALUE_LIMIT {
            return Err(CircuitError::invalid_domain_value("qubit id", value));
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MeasureRecordOffset(i32);

impl MeasureRecordOffset {
    pub fn try_new(value: i32) -> CircuitResult<Self> {
        if !(MIN_MEASUREMENT_RECORD_OFFSET..0).contains(&value) {
            return Err(CircuitError::invalid_domain_value(
                "measurement record offset",
                value,
            ));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RepeatCount(u64);

impl RepeatCount {
    pub fn try_new(value: u64) -> CircuitResult<Self> {
        if value == 0 {
            return Err(CircuitError::invalid_domain_value("repeat count", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Probability(f64);

impl Probability {
    pub fn try_new(value: f64) -> CircuitResult<Self> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(CircuitError::invalid_domain_value("probability", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}
