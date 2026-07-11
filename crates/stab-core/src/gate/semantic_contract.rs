use super::{ArgRule, GateCategory, GateInfo, TargetRule};

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
            #[cfg(any(test, feature = "ops-contracts"))]
            pub(super) const NAMES: [&'static str; [$(stringify!($variant)),+].len()] = [
                $($wire_name),+
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
mod surface;

#[cfg(test)]
pub(crate) use surface::{
    GateShapeExclusion, GateSurface, GateSurfaceBehavior, GateSurfaceContract, GateTargetPattern,
};

#[cfg(feature = "ops-contracts")]
pub(super) fn gate_contract_family_names() -> &'static [&'static str] {
    &GateSemanticFamily::NAMES
}

#[cfg(feature = "ops-contracts")]
pub(super) fn gate_contract_surface_names() -> &'static [&'static str] {
    &[
        "parser",
        "measurement-sampler",
        "reference-sampler",
        "detection-converter",
        "detector-frame",
        "detection-sampler",
        "error-analyzer",
        "flow-generator",
    ]
}

#[cfg(any(test, feature = "ops-contracts"))]
mod statistical_plan;

#[cfg(all(test, not(feature = "ops-contracts")))]
pub(crate) use statistical_plan::GateContractStatisticalPlan;
#[cfg(test)]
pub(crate) use statistical_plan::gate_contract_statistical_plan;
#[cfg(feature = "ops-contracts")]
pub(super) use statistical_plan::gate_contract_statistical_plans;
#[cfg(feature = "ops-contracts")]
pub use statistical_plan::{GateContractStatisticalBucket, GateContractStatisticalPlan};

pub(super) const fn gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    gate_with_inverse(name, name, category, arg_rule, target_rule)
}

pub(super) const fn gate_with_inverse(
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    semantic_gate_with_inverse(
        name,
        inverse_name,
        category,
        arg_rule,
        target_rule,
        default_semantic_family(category),
    )
}

pub(super) const fn semantic_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    semantic_gate_with_inverse(name, name, category, arg_rule, target_rule, semantic_family)
}

pub(super) const fn semantic_gate_with_inverse(
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name,
        category,
        arg_rule,
        target_rule,
        semantic_family,
        can_fuse: true,
    }
}

pub(super) const fn not_fusable_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    semantic_not_fusable_gate(
        name,
        category,
        arg_rule,
        target_rule,
        default_semantic_family(category),
    )
}

pub(super) const fn semantic_not_fusable_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name: name,
        category,
        arg_rule,
        target_rule,
        semantic_family,
        can_fuse: false,
    }
}

#[allow(
    clippy::panic,
    reason = "ambiguous categories must choose an explicit semantic family in the canonical gate table"
)]
const fn default_semantic_family(category: GateCategory) -> GateSemanticFamily {
    match category {
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
            panic!("ambiguous gate category requires an explicit semantic family")
        }
    }
}

#[cfg(test)]
mod tests;
