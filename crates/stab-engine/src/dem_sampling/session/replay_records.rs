//! Replay records as a delivery call supplies them.

use stab_bits::BitSlice;
use stab_records::{FormatError as RecordsFormatError, PackedShotBatchView};

use super::{DemSamplingExecutionError, internal_format_error};
use crate::dem_sampling::{DemError, DemSamplingPlan};

/// Replay records supplied as dense caller records, as one packed shot-major batch, or as one
/// retained error-major batch of `shots` records.
#[derive(Clone, Copy)]
pub(super) enum ReplayRecords<'a> {
    Dense(&'a [Vec<bool>]),
    Packed(PackedShotBatchView<'a>),
    ErrorMajor { words: &'a [u64], shots: usize },
}

/// One replay record borrowed from [`ReplayRecords`].
#[derive(Clone, Copy)]
pub(super) enum ReplayRecord<'a> {
    Dense(&'a [bool]),
    Packed(BitSlice<'a>),
}

impl<'a> ReplayRecords<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Dense(records) => records.len(),
            Self::Packed(records) => records.shot_count(),
            Self::ErrorMajor { shots, .. } => shots,
        }
    }

    pub(super) fn record(self, index: usize) -> Result<ReplayRecord<'a>, RecordsFormatError> {
        match self {
            Self::Dense(records) => records
                .get(index)
                .map(|record| ReplayRecord::Dense(record))
                .ok_or_else(|| {
                    RecordsFormatError::new(
                        stab_records::FormatErrorCode::InvalidData,
                        format!("replay record {index} is out of range"),
                        None,
                    )
                }),
            Self::Packed(records) => records.shot(index).map(ReplayRecord::Packed),
            Self::ErrorMajor { .. } => Err(RecordsFormatError::new(
                stab_records::FormatErrorCode::InvalidData,
                "error-major replay records have no shot-major record",
                None,
            )),
        }
    }

    pub(super) fn validate_width(
        self,
        plan: &DemSamplingPlan,
        index: usize,
        shot_index: usize,
    ) -> Result<(), DemSamplingExecutionError> {
        if let Self::ErrorMajor { words, .. } = self {
            if words.len() == plan.error_count() {
                return Ok(());
            }
            return Err(DemSamplingExecutionError::InvalidRequest(
                DemError::invalid_result_format(format!(
                    "DEM error-major replay batch expected {} error words, got {}",
                    plan.error_count(),
                    words.len()
                )),
            ));
        }
        let record = self.record(index).map_err(internal_format_error)?;
        match record {
            ReplayRecord::Dense(bits) => plan.validate_error_record_width(bits, Some(shot_index)),
            ReplayRecord::Packed(bits) => plan.validate_error_record_width(&bits, Some(shot_index)),
        }
        .map_err(DemSamplingExecutionError::InvalidRequest)
    }
}
