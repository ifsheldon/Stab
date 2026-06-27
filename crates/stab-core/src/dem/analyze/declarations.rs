#[derive(Clone, Debug)]
pub(super) enum Declaration {
    Detector {
        detector_id: u64,
        coordinates: Vec<f64>,
        tag: Option<String>,
    },
    Observable {
        observable: u64,
        tag: Option<String>,
    },
    Shift {
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<String>,
    },
}
