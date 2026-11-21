/// The analysis operation that refused its input, exceeded a limit, or failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AnalysisOperation {
    CircuitToDetectorErrorModel,
    CircuitPass,
    CircuitFlatten,
    CircuitGeneration,
    CircuitInverse,
    CircuitSimplification,
    DetectingRegions,
    DetectorErrorModelFlatten,
    FeedbackInlining,
    FlowCheck,
    FlowGeneration,
    FlowMeasurementSolving,
    LogicalErrorSearch,
    MissingDetectorDiscovery,
    PauliConjugation,
    /// The sparse reverse frame tracker shared by error analysis, detecting regions, feedback
    /// inlining, and flow checking.
    ReverseFrameTracking,
    SatMaterialization,
    TableauConversion,
    TableauSynthesis,
    TimeReversal,
}

impl AnalysisOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitToDetectorErrorModel => "circuit-to-detector-error-model",
            Self::CircuitPass => "circuit-pass",
            Self::CircuitFlatten => "circuit-flatten",
            Self::CircuitGeneration => "circuit-generation",
            Self::CircuitInverse => "circuit-inverse",
            Self::CircuitSimplification => "circuit-simplification",
            Self::DetectingRegions => "detecting-regions",
            Self::DetectorErrorModelFlatten => "detector-error-model-flatten",
            Self::FeedbackInlining => "feedback-inlining",
            Self::FlowCheck => "flow-check",
            Self::FlowGeneration => "flow-generation",
            Self::FlowMeasurementSolving => "flow-measurement-solving",
            Self::LogicalErrorSearch => "logical-error-search",
            Self::MissingDetectorDiscovery => "missing-detector-discovery",
            Self::PauliConjugation => "pauli-conjugation",
            Self::ReverseFrameTracking => "reverse-frame-tracking",
            Self::SatMaterialization => "sat-materialization",
            Self::TableauConversion => "tableau-conversion",
            Self::TableauSynthesis => "tableau-synthesis",
            Self::TimeReversal => "time-reversal",
        }
    }

    /// The text that opens a free-form failure description of this operation.
    ///
    /// Stab's human-readable messages predate this taxonomy and grouped failures by the model
    /// they concern: detector-error-model work, tableau and flow algebra, circuit simplification,
    /// and the parameter checks of flattening and generation. The prefix keeps those messages
    /// byte-identical while the variant, not the text, now says what happened.
    pub(crate) const fn message_prefix(self) -> &'static str {
        match self {
            Self::CircuitToDetectorErrorModel
            | Self::DetectingRegions
            | Self::DetectorErrorModelFlatten
            | Self::FeedbackInlining
            | Self::FlowCheck
            | Self::LogicalErrorSearch
            | Self::MissingDetectorDiscovery
            | Self::ReverseFrameTracking
            | Self::SatMaterialization => "invalid detector error model: ",
            Self::CircuitInverse
            | Self::FlowGeneration
            | Self::FlowMeasurementSolving
            | Self::PauliConjugation
            | Self::TableauConversion
            | Self::TableauSynthesis
            | Self::TimeReversal => "cannot convert circuit to tableau: ",
            Self::CircuitSimplification => "cannot simplify circuit: ",
            Self::CircuitFlatten | Self::CircuitGeneration | Self::CircuitPass => "invalid ",
        }
    }
}
