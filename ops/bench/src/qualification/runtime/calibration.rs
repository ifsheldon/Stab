use std::num::NonZeroU64;
use std::time::Duration;

use thiserror::Error;

const DEFAULT_MAX_PROBES: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CalibrationPolicy {
    pub(super) minimum: Duration,
    pub(super) maximum: Duration,
    pub(super) timeout: Duration,
    pub(super) maximum_iterations: NonZeroU64,
}

impl CalibrationPolicy {
    pub(super) fn validate(self) -> Result<Self, CalibrationError> {
        if self.minimum.is_zero() || self.minimum > self.maximum {
            return Err(CalibrationError::InvalidBounds);
        }
        if self.timeout < self.maximum {
            return Err(CalibrationError::InvalidTimeout);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CalibrationProbe {
    pub(super) measured: Duration,
    pub(super) wall: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CalibrationDecision {
    pub(super) iterations: NonZeroU64,
    pub(super) measured: Duration,
    pub(super) probes: Vec<CalibrationProbe>,
}

pub(crate) fn calibrate(
    policy: CalibrationPolicy,
    mut probe: impl FnMut(NonZeroU64) -> Result<CalibrationProbe, String>,
) -> Result<CalibrationDecision, CalibrationError> {
    let policy = policy.validate()?;
    let mut iterations = NonZeroU64::MIN;
    let mut probes = Vec::new();
    let mut wall = Duration::ZERO;
    for _ in 0..DEFAULT_MAX_PROBES {
        let result = probe(iterations).map_err(CalibrationError::Probe)?;
        if result.measured.is_zero() {
            return Err(CalibrationError::ZeroDuration);
        }
        wall = wall
            .checked_add(result.wall)
            .ok_or(CalibrationError::DurationOverflow)?;
        probes.push(result);
        if wall > policy.timeout {
            return Err(CalibrationError::TimedOut);
        }
        if result.measured >= policy.minimum && result.measured <= policy.maximum {
            return Ok(CalibrationDecision {
                iterations,
                measured: result.measured,
                probes,
            });
        }
        if result.measured > policy.maximum {
            return Err(CalibrationError::NoFeasibleBatch {
                iterations: iterations.get(),
            });
        }
        iterations = next_iterations(iterations, result.measured, policy)?;
    }
    Err(CalibrationError::TooManyProbes {
        maximum: DEFAULT_MAX_PROBES,
    })
}

fn next_iterations(
    current: NonZeroU64,
    measured: Duration,
    policy: CalibrationPolicy,
) -> Result<NonZeroU64, CalibrationError> {
    let numerator = u128::from(current.get())
        .checked_mul(policy.minimum.as_nanos())
        .ok_or(CalibrationError::IterationOverflow)?;
    let denominator = measured.as_nanos();
    let rounded = numerator
        .checked_add(denominator.saturating_sub(1))
        .ok_or(CalibrationError::IterationOverflow)?
        / denominator;
    let at_least_next = rounded.max(u128::from(current.get()) + 1);
    let next = u64::try_from(at_least_next).map_err(|_| CalibrationError::IterationOverflow)?;
    if next > policy.maximum_iterations.get() {
        return Err(CalibrationError::MaximumIterations {
            required: next,
            maximum: policy.maximum_iterations.get(),
        });
    }
    NonZeroU64::new(next).ok_or(CalibrationError::IterationOverflow)
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CalibrationError {
    #[error("calibration minimum must be positive and no greater than its maximum")]
    InvalidBounds,
    #[error("calibration timeout must be at least the maximum batch duration")]
    InvalidTimeout,
    #[error("calibration probe failed: {0}")]
    Probe(String),
    #[error("calibration probe reported zero elapsed time")]
    ZeroDuration,
    #[error("calibration duration accounting overflowed")]
    DurationOverflow,
    #[error("calibration iteration arithmetic overflowed")]
    IterationOverflow,
    #[error("calibration exceeded its wall-time budget")]
    TimedOut,
    #[error("no feasible calibrated batch exists at or below {iterations} iterations")]
    NoFeasibleBatch { iterations: u64 },
    #[error("calibration needs {required} iterations, exceeding the cap of {maximum}")]
    MaximumIterations { required: u64, maximum: u64 },
    #[error("calibration did not converge within {maximum} probes")]
    TooManyProbes { maximum: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> CalibrationPolicy {
        CalibrationPolicy {
            minimum: Duration::from_millis(250),
            maximum: Duration::from_secs(2),
            timeout: Duration::from_secs(10),
            maximum_iterations: NonZeroU64::new(1_000_000).expect("positive cap"),
        }
    }

    #[test]
    fn calibrates_to_the_lower_bound_without_hiding_probes() {
        let decision = calibrate(policy(), |iterations| {
            Ok(CalibrationProbe {
                measured: Duration::from_millis(iterations.get()),
                wall: Duration::from_millis(1),
            })
        })
        .expect("calibration succeeds");
        assert_eq!(decision.iterations.get(), 250);
        assert_eq!(decision.measured, Duration::from_millis(250));
        assert_eq!(decision.probes.len(), 2);
    }

    #[test]
    fn rejects_zero_slow_overflow_timeout_and_iteration_caps() {
        let zero = calibrate(policy(), |_| {
            Ok(CalibrationProbe {
                measured: Duration::ZERO,
                wall: Duration::ZERO,
            })
        });
        assert_eq!(
            zero.expect_err("zero must fail"),
            CalibrationError::ZeroDuration
        );

        let slow = calibrate(policy(), |_| {
            Ok(CalibrationProbe {
                measured: Duration::from_secs(3),
                wall: Duration::from_secs(3),
            })
        });
        assert!(matches!(
            slow,
            Err(CalibrationError::NoFeasibleBatch { .. })
        ));

        let mut capped = policy();
        capped.maximum_iterations = NonZeroU64::new(10).expect("positive cap");
        let cap = calibrate(capped, |_| {
            Ok(CalibrationProbe {
                measured: Duration::from_millis(1),
                wall: Duration::from_millis(1),
            })
        });
        assert!(matches!(
            cap,
            Err(CalibrationError::MaximumIterations { .. })
        ));

        let mut timeout = policy();
        timeout.timeout = Duration::from_secs(2);
        let timed_out = calibrate(timeout, |_| {
            Ok(CalibrationProbe {
                measured: Duration::from_millis(1),
                wall: Duration::from_secs(3),
            })
        });
        assert_eq!(
            timed_out.expect_err("timeout must fail"),
            CalibrationError::TimedOut
        );
    }
}
