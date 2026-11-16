//! Execution of error-major replay for one batch of at most 64 records.

use std::ops::Range;

use stab_records::{BitPlaneBatchView, PackedShotBatchView};

use super::{DemSamplingExecutionError, DemSamplingSession, internal_format_error};
use crate::dem_sampling::program::DemPoll;
use crate::dem_sampling::replay_planes::replay_error_words;
use crate::sampling::session_cancellation;

/// Records of one error-major replay batch.
pub(super) enum ErrorMajorRecords<'a> {
    /// Records `range` of a shot-major batch, transposed before replay.
    Packed(PackedShotBatchView<'a>, Range<usize>),
    /// Up to 64 records already represented as bit planes, copied directly without transpose.
    BitPlanes(BitPlaneBatchView<'a>),
    /// One word per error, already transposed, as a record store retains them.
    Words(&'a [u64]),
}

impl DemSamplingSession {
    /// Replays one batch of at most 64 records into the session's detector, observable, and
    /// sampled-error planes, one word per plane.
    pub(super) fn fill_replay_planes(
        &mut self,
        records: ErrorMajorRecords<'_>,
    ) -> Result<(), DemSamplingExecutionError> {
        let error_planes = self.error_planes.as_mut().ok_or_else(|| {
            DemSamplingExecutionError::InternalInvariant {
                message: "DEM replay omitted its reusable sampled-error planes".to_owned(),
            }
        })?;
        // Each vector was admitted and allocated with at least one word per plane, so these
        // resizes stay within the reserved capacity.
        error_planes.resize(self.plan.error_count(), 0);
        self.detector_planes.resize(self.plan.detector_count(), 0);
        self.observable_planes
            .resize(self.plan.observable_count(), 0);
        match records {
            ErrorMajorRecords::Packed(records, range) => records
                .copy_rows_into_bit_plane_words(range.start, range.len(), error_planes)
                .map_err(internal_format_error)?,
            ErrorMajorRecords::BitPlanes(planes) => {
                if !error_planes.is_empty() && planes.shot_count() > 0 {
                    planes
                        .copy_word_column_into(0, error_planes)
                        .map_err(internal_format_error)?;
                }
            }
            ErrorMajorRecords::Words(words) => {
                if words.len() != error_planes.len() {
                    return Err(DemSamplingExecutionError::InternalInvariant {
                        message: format!(
                            "retained DEM replay batch holds {} error words for {} errors",
                            words.len(),
                            error_planes.len()
                        ),
                    });
                }
                error_planes.copy_from_slice(words);
            }
        }
        replay_error_words(
            &self.plan.inner.operations,
            error_planes,
            &mut self.detector_planes,
            &mut self.observable_planes,
            &mut DemPoll::new(session_cancellation(&self.cancellation)),
        )
        .map_err(DemSamplingExecutionError::InvalidRequest)
    }
}
