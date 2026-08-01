use std::error::Error;
use std::fmt::{self, Display, Formatter};

use stab_records::{CorrectionWidth, DetectorWidth};
use thiserror::Error;

use crate::DecoderLayout;

/// Request-shape failure detected before decoder implementation code runs.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DecodePreflightError {
    #[error("decoder expected detector width {expected:?}, got {actual:?}")]
    DetectorWidth {
        expected: DetectorWidth,
        actual: DetectorWidth,
    },

    #[error("decoder expected correction width {expected:?}, got {actual:?}")]
    CorrectionWidth {
        expected: CorrectionWidth,
        actual: CorrectionWidth,
    },

    #[error(
        "decoder needs prediction storage for {required} shots, but only {available} are available"
    )]
    PredictionShotCapacity { required: usize, available: usize },
}

/// Violation of the static decoder-session implementation contract.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DecodeContractError {
    #[error("decoder summary reported {actual} requested shots, expected {expected}")]
    RequestedShotCount { expected: usize, actual: usize },

    #[error("decoder summary completed {actual} shots, exceeding the requested {requested}")]
    CompletedShotCount { requested: usize, actual: usize },

    #[error(
        "decoder failure reported {actual} completed shots, exceeding the requested {requested}"
    )]
    FailureProgress { requested: usize, actual: usize },

    #[error("decoder session changed layout during one call from {expected:?} to {actual:?}")]
    SessionLayoutChanged {
        expected: DecoderLayout,
        actual: DecoderLayout,
    },

    #[error("validated prediction storage invariant failed: {message}")]
    PredictionStorage { message: String },
}

/// Decoder implementation failure with the exact committed prediction prefix.
#[derive(Debug)]
pub struct DecodeSessionFailure<E> {
    source: E,
    completed_shots: usize,
}

impl<E> DecodeSessionFailure<E> {
    pub const fn new(source: E, completed_shots: usize) -> Self {
        Self {
            source,
            completed_shots,
        }
    }

    pub const fn completed_shots(&self) -> usize {
        self.completed_shots
    }

    pub const fn source_ref(&self) -> &E {
        &self.source
    }

    pub fn into_source(self) -> E {
        self.source
    }
}

impl<E: Display> Display for DecodeSessionFailure<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "decoder implementation failed after {} completed shots: {}",
            self.completed_shots, self.source
        )
    }
}

impl<E: Error + 'static> Error for DecodeSessionFailure<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Preflight, implementation, or implementation-contract failure from [`crate::decode_batch`].
#[derive(Debug)]
pub enum DecodeBatchError<E> {
    Preflight(DecodePreflightError),
    Session(DecodeSessionFailure<E>),
    Contract(DecodeContractError),
}

impl<E> From<DecodePreflightError> for DecodeBatchError<E> {
    fn from(error: DecodePreflightError) -> Self {
        Self::Preflight(error)
    }
}

impl<E> From<DecodeSessionFailure<E>> for DecodeBatchError<E> {
    fn from(error: DecodeSessionFailure<E>) -> Self {
        Self::Session(error)
    }
}

impl<E> From<DecodeContractError> for DecodeBatchError<E> {
    fn from(error: DecodeContractError) -> Self {
        Self::Contract(error)
    }
}

impl<E: Display> Display for DecodeBatchError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(error) => Display::fmt(error, formatter),
            Self::Session(error) => Display::fmt(error, formatter),
            Self::Contract(error) => Display::fmt(error, formatter),
        }
    }
}

impl<E: Error + 'static> Error for DecodeBatchError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preflight(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Contract(error) => Some(error),
        }
    }
}
