//! Stable decoder interoperability contracts.
//!
//! Decoder implementations compile their own representation from [`DecoderModelView`] and expose
//! reusable state through [`DecoderSession`]. The canonical [`decode_batch`] entry point validates
//! every input and prediction dimension before implementation code receives a mutable output view.

mod batch;
mod cancellation;
mod error;
mod layout;
mod session;

pub use batch::{DecoderInputBatchView, ValidatedDecodeBatch};
pub use cancellation::DecodeCancellation;
pub use error::{
    DecodeBatchError, DecodeContractError, DecodePreflightError, DecodeSessionFailure,
};
pub use layout::{DecoderLayout, DecoderModelView, DecoderModelViewError};
pub use session::{DecodeBatchStatus, DecodeBatchSummary, DecoderSession, decode_batch};
