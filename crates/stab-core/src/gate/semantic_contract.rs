use super::{Gate, GateCategory};

macro_rules! define_gate_contract_enum {
    (
        $(#[$meta:meta])*
        $visibility:vis enum $name:ident {
            $($variant:ident => $wire_name:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $visibility enum $name {
            $($variant),+
        }

        impl $name {
            #[cfg(test)]
            pub(super) const ALL: [Self; [$(stringify!($variant)),+].len()] = [
                $(Self::$variant),+
            ];
            #[cfg(test)]
            pub(super) const NAMES: [&'static str; [$(stringify!($variant)),+].len()] = [
                $(Self::$variant.as_str()),+
            ];

            #[cfg(test)]
            pub(super) const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire_name),+
                }
            }
        }
    };
}

define_gate_contract_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub(super) enum GateSemanticFamily {
        Annotation => "annotation",
        ControlFlow => "control-flow",
        MeasurementPad => "measurement-pad",
        Measurement => "measurement",
        MeasureReset => "measure-reset",
        Reset => "reset",
        FixedTableau => "fixed-tableau",
        ForwardClassicalControl => "forward-classical-control",
        SymmetricClassicalControl => "symmetric-classical-control",
        ReverseClassicalControl => "reverse-classical-control",
        Depolarization => "depolarization",
        PauliNoise => "pauli-noise",
        IdentityNoise => "identity-noise",
        PauliChannel => "pauli-channel",
        CorrelatedError => "correlated-error",
        HeraldedNoise => "heralded-noise",
        PauliProductMeasurement => "pauli-product-measurement",
        PauliProductPhase => "pauli-product-phase",
        PairMeasurement => "pair-measurement",
    }
}

#[cfg(test)]
define_gate_contract_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub(crate) enum GateSurface {
        Parser => "parser",
        MeasurementSampler => "measurement-sampler",
        ReferenceSampler => "reference-sampler",
        DetectionConverter => "detection-converter",
        DetectorFrame => "detector-frame",
        DetectionSampler => "detection-sampler",
        ErrorAnalyzer => "error-analyzer",
        FlowGenerator => "flow-generator",
    }
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
pub(crate) use statistical_plan::GateContractStatisticalPlan;
#[cfg(test)]
pub(crate) use statistical_plan::{
    gate_contract_statistical_count_is_accepted, gate_contract_statistical_plan,
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
