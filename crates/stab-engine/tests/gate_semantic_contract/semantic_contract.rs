use super::{Gate, GateCategory};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum GateSemanticFamily {
    Annotation,
    ControlFlow,
    MeasurementPad,
    Measurement,
    MeasureReset,
    Reset,
    FixedTableau,
    ForwardClassicalControl,
    SymmetricClassicalControl,
    ReverseClassicalControl,
    Depolarization,
    PauliNoise,
    IdentityNoise,
    PauliChannel,
    CorrelatedError,
    HeraldedNoise,
    PauliProductMeasurement,
    PauliProductPhase,
    PairMeasurement,
}

/// Declares the enum and its exhaustive `ALL` list from one variant list.
macro_rules! define_gate_surfaces {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub(crate) enum GateSurface {
            $($variant),+
        }

        impl GateSurface {
            pub(super) const ALL: [Self; [$(stringify!($variant)),+].len()] = [
                $(Self::$variant),+
            ];
        }
    };
}

#[cfg(test)]
define_gate_surfaces! {
    Parser,
    MeasurementSampler,
    ReferenceSampler,
    DetectionConverter,
    DetectorFrame,
    DetectionSampler,
    ErrorAnalyzer,
    FlowGenerator,
}

#[cfg(test)]
mod surface;

#[cfg(test)]
pub(crate) use surface::{
    GateShapeExclusion, GateSurfaceBehavior, GateSurfaceContract, GateSurfaceContractExt,
    GateTargetPattern,
};

#[cfg(test)]
mod statistical_plan;

#[cfg(test)]
pub(crate) use statistical_plan::{GateContractStatisticalPlan, PAULI_CHANNEL_2_PROBABILITIES};
#[cfg(test)]
pub(crate) use statistical_plan::{
    gate_contract_statistical_count_is_accepted, gate_contract_statistical_plan,
    gate_contract_statistical_tolerance,
};

#[allow(
    clippy::panic,
    reason = "the exhaustive canonical gate contract test proves every ambiguous gate is classified"
)]
fn gate_semantic_family(gate: Gate) -> GateSemanticFamily {
    match gate.canonical_name() {
        "MPAD" => GateSemanticFamily::MeasurementPad,
        "MX" | "MY" | "M" => GateSemanticFamily::Measurement,
        "MRX" | "MRY" | "MR" => GateSemanticFamily::MeasureReset,
        "RX" | "RY" | "R" => GateSemanticFamily::Reset,
        "XCZ" | "YCZ" => GateSemanticFamily::ReverseClassicalControl,
        "CX" | "CY" => GateSemanticFamily::ForwardClassicalControl,
        "CZ" => GateSemanticFamily::SymmetricClassicalControl,
        "DEPOLARIZE1" | "DEPOLARIZE2" => GateSemanticFamily::Depolarization,
        "X_ERROR" | "Y_ERROR" | "Z_ERROR" => GateSemanticFamily::PauliNoise,
        "I_ERROR" | "II_ERROR" => GateSemanticFamily::IdentityNoise,
        "PAULI_CHANNEL_1" | "PAULI_CHANNEL_2" => GateSemanticFamily::PauliChannel,
        "E" | "ELSE_CORRELATED_ERROR" => GateSemanticFamily::CorrelatedError,
        "MPP" => GateSemanticFamily::PauliProductMeasurement,
        "SPP" | "SPP_DAG" => GateSemanticFamily::PauliProductPhase,
        _ => match gate.category() {
            GateCategory::Annotation => GateSemanticFamily::Annotation,
            GateCategory::ControlFlow => GateSemanticFamily::ControlFlow,
            GateCategory::Controlled
            | GateCategory::HadamardLike
            | GateCategory::Pauli
            | GateCategory::Period3
            | GateCategory::Period4
            | GateCategory::ParityPhasing
            | GateCategory::Swap => GateSemanticFamily::FixedTableau,
            GateCategory::HeraldedNoise => GateSemanticFamily::HeraldedNoise,
            GateCategory::PairMeasurement => GateSemanticFamily::PairMeasurement,
            GateCategory::Collapsing | GateCategory::Noise | GateCategory::PauliProduct => {
                panic!(
                    "ambiguous gate {} requires an explicit semantic family",
                    gate.canonical_name()
                )
            }
        },
    }
}

#[cfg(test)]
mod tests;
