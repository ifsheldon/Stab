use std::collections::BTreeSet;

use serde::Deserialize;

use super::{BlockerCase, BlockerRecord};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GateContractFamily {
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

impl GateContractFamily {
    const ALL: [Self; 19] = [
        Self::Annotation,
        Self::ControlFlow,
        Self::MeasurementPad,
        Self::Measurement,
        Self::MeasureReset,
        Self::Reset,
        Self::FixedTableau,
        Self::ForwardClassicalControl,
        Self::SymmetricClassicalControl,
        Self::ReverseClassicalControl,
        Self::Depolarization,
        Self::PauliNoise,
        Self::IdentityNoise,
        Self::PauliChannel,
        Self::CorrelatedError,
        Self::HeraldedNoise,
        Self::PauliProductMeasurement,
        Self::PauliProductPhase,
        Self::PairMeasurement,
    ];

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Annotation => "annotation",
            Self::ControlFlow => "control-flow",
            Self::MeasurementPad => "measurement-pad",
            Self::Measurement => "measurement",
            Self::MeasureReset => "measure-reset",
            Self::Reset => "reset",
            Self::FixedTableau => "fixed-tableau",
            Self::ForwardClassicalControl => "forward-classical-control",
            Self::SymmetricClassicalControl => "symmetric-classical-control",
            Self::ReverseClassicalControl => "reverse-classical-control",
            Self::Depolarization => "depolarization",
            Self::PauliNoise => "pauli-noise",
            Self::IdentityNoise => "identity-noise",
            Self::PauliChannel => "pauli-channel",
            Self::CorrelatedError => "correlated-error",
            Self::HeraldedNoise => "heralded-noise",
            Self::PauliProductMeasurement => "pauli-product-measurement",
            Self::PauliProductPhase => "pauli-product-phase",
            Self::PairMeasurement => "pair-measurement",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GateContractSurface {
    Parser,
    MeasurementSampler,
    ReferenceSampler,
    DetectionConverter,
    DetectorFrame,
    DetectionSampler,
    ErrorAnalyzer,
    FlowGenerator,
}

impl GateContractSurface {
    const ALL: [Self; 8] = [
        Self::Parser,
        Self::MeasurementSampler,
        Self::ReferenceSampler,
        Self::DetectionConverter,
        Self::DetectorFrame,
        Self::DetectionSampler,
        Self::ErrorAnalyzer,
        Self::FlowGenerator,
    ];

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Parser => "parser",
            Self::MeasurementSampler => "measurement-sampler",
            Self::ReferenceSampler => "reference-sampler",
            Self::DetectionConverter => "detection-converter",
            Self::DetectorFrame => "detector-frame",
            Self::DetectionSampler => "detection-sampler",
            Self::ErrorAnalyzer => "error-analyzer",
            Self::FlowGenerator => "flow-generator",
        }
    }
}

pub(super) fn validate_gate_contract_case(
    blocker: &BlockerRecord,
    case: &BlockerCase,
    violations: &mut Vec<String>,
) {
    if blocker.id != "pfm3-gate-execution" {
        if !case.gate_surfaces.is_empty() || !case.gate_families.is_empty() {
            violations.push(format!(
                "non-gate blocker {:?} case {:?} declares gate contract coverage",
                blocker.id, case.id
            ));
        }
        return;
    }

    let actual = case.gate_surfaces.iter().copied().collect::<BTreeSet<_>>();
    if actual.len() != case.gate_surfaces.len() {
        violations.push(format!(
            "gate contract case {:?} has duplicate gate_surfaces",
            case.id
        ));
    }
    let expected = GateContractSurface::ALL
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected
            .difference(&actual)
            .map(|surface| surface.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let extra = actual
            .difference(&expected)
            .map(|surface| surface.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        violations.push(format!(
            "gate contract case {:?} must cover all eight gate surfaces; missing=[{missing}] extra=[{extra}]",
            case.id
        ));
    }

    if case.gate_families.is_empty() {
        violations.push(format!(
            "gate contract case {:?} must own at least one gate family",
            case.id
        ));
    }
    let families = case.gate_families.iter().copied().collect::<BTreeSet<_>>();
    if families.len() != case.gate_families.len() {
        violations.push(format!(
            "gate contract case {:?} has duplicate gate_families",
            case.id
        ));
    }
}

pub(super) fn validate_gate_schema(violations: &mut Vec<String>) {
    let core_surfaces = stab_core::__gate_contract_surface_names()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let ledger_surfaces = GateContractSurface::ALL
        .into_iter()
        .map(GateContractSurface::as_str)
        .collect::<BTreeSet<_>>();
    if ledger_surfaces != core_surfaces {
        violations.push(format!(
            "gate contract surface schema differs from canonical core metadata; ledger={ledger_surfaces:?} core={core_surfaces:?}"
        ));
    }

    let core_families = stab_core::__gate_contract_family_names()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let ledger_families = GateContractFamily::ALL
        .into_iter()
        .map(GateContractFamily::as_str)
        .collect::<BTreeSet<_>>();
    if ledger_families != core_families {
        violations.push(format!(
            "gate contract family schema differs from canonical core metadata; ledger={ledger_families:?} core={core_families:?}"
        ));
    }
}

pub(super) fn validate_gate_family_coverage(blocker: &BlockerRecord, violations: &mut Vec<String>) {
    if blocker.id != "pfm3-gate-execution" {
        return;
    }
    let actual = blocker
        .cases
        .iter()
        .flat_map(|case| case.gate_families.iter().copied())
        .collect::<BTreeSet<_>>();
    let expected = GateContractFamily::ALL.into_iter().collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected
            .difference(&actual)
            .map(|family| family.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let extra = actual
            .difference(&expected)
            .map(|family| family.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        violations.push(format!(
            "gate contract cases must cover all nineteen semantic families; missing=[{missing}] extra=[{extra}]"
        ));
    }

    let actual_statistical_cases = blocker
        .cases
        .iter()
        .filter(|case| {
            case.statistical_plan.is_some()
                && matches!(case.comparator, super::ComparatorKind::Statistical)
        })
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    let core_statistical_plans = stab_core::__gate_contract_statistical_plans();
    let expected_statistical_cases = core_statistical_plans
        .iter()
        .map(|plan| plan.case_id)
        .collect::<BTreeSet<_>>();
    if expected_statistical_cases.len() != core_statistical_plans.len() {
        violations.push("canonical core gate contract repeats a statistical case id".to_string());
    }
    if actual_statistical_cases != expected_statistical_cases {
        violations.push(format!(
            "gate contract statistical case set differs from canonical core metadata; ledger={actual_statistical_cases:?} core={expected_statistical_cases:?}"
        ));
    }
}
