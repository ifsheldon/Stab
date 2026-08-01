use stab_records::{
    DetectionBatchView, DetectorWidth, ObservablePredictionBatchViewMut, PackedShotBatchView,
    RecordResult,
};

use crate::DecoderLayout;

/// Borrowed detector records presented to a decoder without observable truth.
#[derive(Clone, Copy, Debug)]
pub struct DecoderInputBatchView<'a> {
    detectors: PackedShotBatchView<'a>,
    detector_width: DetectorWidth,
}

impl<'a> DecoderInputBatchView<'a> {
    pub fn from_detectors(detectors: PackedShotBatchView<'a>) -> Self {
        Self {
            detectors,
            detector_width: DetectorWidth::new(detectors.bits_per_shot()),
        }
    }

    /// Drops observable truth while borrowing detector records from a detection batch.
    pub fn from_detection(detection: DetectionBatchView<'a>) -> Self {
        Self::from_detectors(detection.detectors())
    }

    pub const fn detectors(self) -> PackedShotBatchView<'a> {
        self.detectors
    }

    pub fn shot_count(self) -> usize {
        self.detectors.shot_count()
    }

    pub const fn detector_width(self) -> DetectorWidth {
        self.detector_width
    }

    pub fn detector(self, shot_index: usize, detector_index: usize) -> Option<bool> {
        self.detectors.get(shot_index, detector_index)
    }
}

/// Non-forgeable input and prediction views admitted by [`crate::decode_batch`].
///
/// External decoder implementations can inspect detector records and update only the checked
/// prediction prefix. They cannot construct this value or access observable truth through it.
#[derive(Debug)]
pub struct ValidatedDecodeBatch<'input, 'output> {
    input: DecoderInputBatchView<'input>,
    predictions: ObservablePredictionBatchViewMut<'output>,
    layout: DecoderLayout,
}

impl<'input, 'output> ValidatedDecodeBatch<'input, 'output> {
    pub(crate) const fn new(
        input: DecoderInputBatchView<'input>,
        predictions: ObservablePredictionBatchViewMut<'output>,
        layout: DecoderLayout,
    ) -> Self {
        Self {
            input,
            predictions,
            layout,
        }
    }

    pub const fn layout(&self) -> DecoderLayout {
        self.layout
    }

    pub const fn input(&self) -> DecoderInputBatchView<'input> {
        self.input
    }

    pub fn shot_count(&self) -> usize {
        self.input.shot_count()
    }

    pub fn detector(&self, shot_index: usize, detector_index: usize) -> Option<bool> {
        self.input.detector(shot_index, detector_index)
    }

    pub fn prediction(&self, shot_index: usize, observable_index: usize) -> Option<bool> {
        self.predictions.get(shot_index, observable_index)
    }

    pub fn set_prediction(
        &mut self,
        shot_index: usize,
        observable_index: usize,
        value: bool,
    ) -> RecordResult<()> {
        self.predictions.set(shot_index, observable_index, value)
    }

    pub fn copy_prediction_from_bools(
        &mut self,
        shot_index: usize,
        prediction: &[bool],
    ) -> RecordResult<()> {
        self.predictions
            .copy_shot_from_bools(shot_index, prediction)
    }

    pub fn prediction_records(&self) -> PackedShotBatchView<'_> {
        self.predictions.view()
    }
}
