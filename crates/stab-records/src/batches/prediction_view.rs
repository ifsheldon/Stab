use super::{ObservablePredictionBatch, PackedShotBatch, PackedShotBatchView};
use crate::{BitIndex, CorrectionWidth, FormatError, RecordResult, ShotIndex};

/// Mutable correction-typed borrow over a checked prefix of prediction records.
///
/// The view never exposes records beyond [`Self::shot_count`], even when the owned batch has
/// additional reusable capacity. All writes replace or update only admitted prediction records.
#[derive(Debug)]
pub struct ObservablePredictionBatchViewMut<'a> {
    records: &'a mut PackedShotBatch,
    shot_count: usize,
    width: CorrectionWidth,
}

impl ObservablePredictionBatchViewMut<'_> {
    pub const fn shot_count(&self) -> usize {
        self.shot_count
    }

    pub const fn correction_width(&self) -> CorrectionWidth {
        self.width
    }

    pub fn get(&self, shot: ShotIndex, bit: BitIndex) -> Option<bool> {
        if shot.0 >= self.shot_count {
            return None;
        }
        self.records.get(shot, bit)
    }

    pub fn set(&mut self, shot: ShotIndex, bit: BitIndex, value: bool) -> RecordResult<()> {
        self.ensure_shot(shot.0)?;
        self.records.set(shot, bit, value)
    }

    pub fn copy_shot_from_bools(&mut self, shot_index: usize, record: &[bool]) -> RecordResult<()> {
        self.ensure_shot(shot_index)?;
        self.records.copy_shot_from_bools(shot_index, record)
    }

    pub fn view(&self) -> PackedShotBatchView<'_> {
        PackedShotBatchView {
            storage: &self.records.storage,
            shot_count: self.shot_count,
        }
    }

    fn ensure_shot(&self, shot_index: usize) -> RecordResult<()> {
        if shot_index >= self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "prediction shot index {shot_index} is out of range for {} admitted records",
                self.shot_count
            )));
        }
        Ok(())
    }
}

impl ObservablePredictionBatch {
    /// Borrows a checked immutable prefix without resizing the reusable owned storage.
    pub fn view_prefix(&self, shot_count: usize) -> RecordResult<PackedShotBatchView<'_>> {
        self.records.view_prefix(shot_count)
    }

    /// Borrows every record mutably, through a view that keeps the batch's correction width.
    pub fn view_mut(&mut self) -> ObservablePredictionBatchViewMut<'_> {
        ObservablePredictionBatchViewMut {
            shot_count: self.records.shot_count(),
            records: &mut self.records,
            width: self.width,
        }
    }

    /// Borrows a checked mutable prefix without exposing reusable suffix records.
    pub fn view_prefix_mut(
        &mut self,
        shot_count: usize,
    ) -> RecordResult<ObservablePredictionBatchViewMut<'_>> {
        if shot_count > self.records.shot_count() {
            return Err(FormatError::invalid_data(format!(
                "prediction prefix has {shot_count} records but storage contains only {}",
                self.records.shot_count()
            )));
        }
        Ok(ObservablePredictionBatchViewMut {
            records: &mut self.records,
            shot_count,
            width: self.width,
        })
    }
}
