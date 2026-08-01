use stab_model::{DetectorErrorModel, ModelError, ModelFingerprint};
use stab_records::{CorrectionWidth, DetectorWidth, ObservableWidth};
use thiserror::Error;

/// Detector-input and observable-correction dimensions of one decoder model.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DecoderLayout {
    detector_width: DetectorWidth,
    observable_width: ObservableWidth,
}

impl DecoderLayout {
    pub const fn new(detector_width: DetectorWidth, observable_width: ObservableWidth) -> Self {
        Self {
            detector_width,
            observable_width,
        }
    }

    pub const fn detector_width(self) -> DetectorWidth {
        self.detector_width
    }

    pub const fn observable_width(self) -> ObservableWidth {
        self.observable_width
    }

    pub const fn correction_width(self) -> CorrectionWidth {
        CorrectionWidth::new(self.observable_width.get())
    }
}

/// Borrowed exact detector-error model and the decoder dimensions derived from it.
#[derive(Clone, Copy, Debug)]
pub struct DecoderModelView<'a> {
    model: &'a DetectorErrorModel,
    layout: DecoderLayout,
    fingerprint: ModelFingerprint,
}

impl<'a> DecoderModelView<'a> {
    pub fn try_new(model: &'a DetectorErrorModel) -> Result<Self, DecoderModelViewError> {
        let detector_count = model.count_detectors()?;
        let observable_count = model.count_observables()?;
        let detector_width = usize::try_from(detector_count).map_err(|_| {
            DecoderModelViewError::DetectorWidthOverflow {
                actual: detector_count,
            }
        })?;
        let observable_width = usize::try_from(observable_count).map_err(|_| {
            DecoderModelViewError::ObservableWidthOverflow {
                actual: observable_count,
            }
        })?;
        Ok(Self {
            model,
            layout: DecoderLayout::new(
                DetectorWidth::new(detector_width),
                ObservableWidth::new(observable_width),
            ),
            fingerprint: model.fingerprint(),
        })
    }

    pub const fn model(self) -> &'a DetectorErrorModel {
        self.model
    }

    pub const fn layout(self) -> DecoderLayout {
        self.layout
    }

    pub const fn fingerprint(self) -> ModelFingerprint {
        self.fingerprint
    }
}

/// Failure while deriving a decoder view from an exact detector-error model.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DecoderModelViewError {
    #[error("invalid detector-error model: {0}")]
    Model(#[from] ModelError),

    #[error("detector count {actual} does not fit the platform decoder width")]
    DetectorWidthOverflow { actual: u64 },

    #[error("observable count {actual} does not fit the platform decoder width")]
    ObservableWidthOverflow { actual: u64 },
}
