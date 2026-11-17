//! Replay records as a delivery call supplies them.

use stab_bits::BitSlice;
use stab_records::{BitPlaneBatchView, FormatError as RecordsFormatError, PackedShotBatchView};

use super::{DemSamplingExecutionError, internal_format_error};
use crate::dem_sampling::replay::SparseErrorRecord;
use crate::dem_sampling::{DemError, DemSamplingPlan};

/// Replay records supplied as dense caller records, packed shot-major rows, bit planes, retained
/// error-major words, or retained sparse set-error ordinals.
#[derive(Clone, Copy)]
pub(super) enum ReplayRecords<'a> {
    Dense(&'a [Vec<bool>]),
    Packed(PackedShotBatchView<'a>),
    BitPlanes(BitPlaneBatchView<'a>),
    ErrorMajor {
        words: &'a [u64],
        shots: usize,
    },
    Sparse {
        error_count: usize,
        hits: &'a [u32],
        shot_starts: &'a [usize],
    },
}

/// One replay record borrowed from [`ReplayRecords`].
#[derive(Clone, Copy)]
pub(super) enum ReplayRecord<'a> {
    Dense(&'a [bool]),
    Packed(BitSlice<'a>),
    Sparse(SparseErrorRecord<'a>),
}

impl<'a> ReplayRecords<'a> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Dense(records) => records.len(),
            Self::Packed(records) => records.shot_count(),
            Self::BitPlanes(records) => records.shot_count(),
            Self::ErrorMajor { shots, .. } => shots,
            Self::Sparse { shot_starts, .. } => shot_starts.len().saturating_sub(1),
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
            Self::BitPlanes(_) | Self::ErrorMajor { .. } => Err(RecordsFormatError::new(
                stab_records::FormatErrorCode::InvalidData,
                "error-major replay records have no shot-major record",
                None,
            )),
            Self::Sparse {
                error_count,
                hits,
                shot_starts,
            } => {
                let (&start, &end) = shot_starts
                    .get(index)
                    .zip(shot_starts.get(index + 1))
                    .ok_or_else(|| {
                        RecordsFormatError::new(
                            stab_records::FormatErrorCode::InvalidData,
                            format!("sparse replay record {index} is out of range"),
                            None,
                        )
                    })?;
                let record_hits = hits.get(start..end).ok_or_else(|| {
                    RecordsFormatError::new(
                        stab_records::FormatErrorCode::InvalidData,
                        format!("sparse replay record {index} escaped its hit storage"),
                        None,
                    )
                })?;
                Ok(ReplayRecord::Sparse(SparseErrorRecord::new(
                    error_count,
                    record_hits,
                )))
            }
        }
    }

    pub(super) fn validate_width(
        self,
        plan: &DemSamplingPlan,
        index: usize,
        shot_index: usize,
    ) -> Result<(), DemSamplingExecutionError> {
        match self {
            Self::BitPlanes(planes) => {
                if planes.bits_per_shot() == plan.error_count() {
                    return Ok(());
                }
                return Err(DemSamplingExecutionError::InvalidRequest(
                    DemError::invalid_result_format(format!(
                        "DEM error record {shot_index} expected {} bits, got {}",
                        plan.error_count(),
                        planes.bits_per_shot()
                    )),
                ));
            }
            Self::ErrorMajor { words, .. } => {
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
            Self::Sparse { error_count, .. } => {
                if error_count == plan.error_count() {
                    return Ok(());
                }
                return Err(DemSamplingExecutionError::InvalidRequest(
                    DemError::invalid_result_format(format!(
                        "DEM error record {shot_index} expected {} bits, got {error_count}",
                        plan.error_count()
                    )),
                ));
            }
            Self::Dense(_) | Self::Packed(_) => {}
        }
        let record = self.record(index).map_err(internal_format_error)?;
        match record {
            ReplayRecord::Dense(bits) => plan.validate_error_record_width(bits, Some(shot_index)),
            ReplayRecord::Packed(bits) => plan.validate_error_record_width(&bits, Some(shot_index)),
            ReplayRecord::Sparse(bits) => plan.validate_error_record_width(&bits, Some(shot_index)),
        }
        .map_err(DemSamplingExecutionError::InvalidRequest)
    }
}
