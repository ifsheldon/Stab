use crate::{FormatError, RecordResult};

mod capabilities;
mod dets;
#[cfg(test)]
pub(crate) mod reference;

pub use capabilities::codec_capabilities;
pub use capabilities::{CodecCapability, RecordEncoding, RecordFormat};
pub(crate) use dets::DetsToken;
pub use dets::{DetsLayout, DetsResultType};

pub fn validate_ptb64_shot_count(shots: usize) -> RecordResult<()> {
    if !shots.is_multiple_of(64) {
        return Err(FormatError::invalid_data(
            "shots must be a multiple of 64 to use ptb64 format",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
