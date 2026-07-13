use crate::qualification::model::{
    BehavioralSurface, CaseId, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
    EvidenceState, EvidenceStatus, FeatureId, ResourceContract, ResourceKind, SelectorKind,
    StatisticalPlanRef, StatisticalPlanSource,
};

use super::property_plan;

pub(super) fn infer_feature_from_oracle_argv(argv: &str) -> Option<FeatureId> {
    let first = argv.split('|').next()?;
    match first {
        "--help" => Some(FeatureId::Cli),
        "gen" => Some(FeatureId::Generation),
        "convert" => Some(FeatureId::ResultFormats),
        "sample" => Some(FeatureId::Sampling),
        "detect" | "m2d" => Some(FeatureId::Detection),
        "analyze_errors" => Some(FeatureId::Analyzer),
        "sample_dem" => Some(FeatureId::DemSampling),
        "core-parse-print" | "core-circuit-parse-print" => Some(FeatureId::StimFormat),
        "core-dem-parse-print" => Some(FeatureId::DemFormat),
        _ if first.starts_with("--gen=") => Some(FeatureId::Generation),
        _ if first.starts_with("--sample=") => Some(FeatureId::Sampling),
        _ => None,
    }
}

pub(super) fn oracle_behavioral_surface(argv: &str) -> BehavioralSurface {
    match argv.split('|').next().unwrap_or("") {
        "core-parse-print" | "core-circuit-parse-print" | "core-dem-parse-print" => {
            BehavioralSurface::FileFormat
        }
        "cargo-test" => BehavioralSurface::Engine,
        _ => BehavioralSurface::Cli,
    }
}

pub(super) fn behavioral_surface_for_feature(
    feature_id: FeatureId,
    provenance: EvidenceProvenance,
) -> BehavioralSurface {
    if provenance == EvidenceProvenance::PublicRustApi {
        return BehavioralSurface::RustApi;
    }
    match feature_id {
        FeatureId::Cli => BehavioralSurface::Cli,
        FeatureId::StimFormat | FeatureId::DemFormat | FeatureId::ResultFormats => {
            BehavioralSurface::FileFormat
        }
        FeatureId::Resource => BehavioralSurface::ResourceBoundary,
        _ => BehavioralSurface::Engine,
    }
}

pub(super) fn statistical_plan_reference(
    comparator: Comparator,
    status: EvidenceStatus,
    provenance: EvidenceProvenance,
    source_id: &str,
    case_id: &CaseId,
) -> Option<StatisticalPlanRef> {
    if comparator != Comparator::Statistical {
        return None;
    }
    let (state, source, id) = match status {
        EvidenceStatus::Planned => (
            EvidenceState::Planned,
            StatisticalPlanSource::QualificationCase,
            case_id.to_string(),
        ),
        EvidenceStatus::Implemented | EvidenceStatus::EvidenceClose => (
            EvidenceState::Existing,
            match provenance {
                EvidenceProvenance::OracleFixture => StatisticalPlanSource::OracleFixture,
                EvidenceProvenance::BlockerLedger => StatisticalPlanSource::BlockerLedger,
                EvidenceProvenance::UpstreamSemanticCase
                | EvidenceProvenance::PublicRustApi
                | EvidenceProvenance::RustRegression
                | EvidenceProvenance::QualificationPlan => StatisticalPlanSource::QualificationCase,
            },
            source_id.to_string(),
        ),
        EvidenceStatus::Deferred => return None,
    };
    Some(StatisticalPlanRef { state, source, id })
}

pub(super) fn semantic_only_resource_contract() -> ResourceContract {
    ResourceContract {
        kind: ResourceKind::NotApplicable,
        detail: "This atomic semantic case makes no negative-axis or resource-boundary claim; dedicated CQ cases must own those contracts."
            .to_string(),
    }
}

pub(super) fn make_planned_evidence_case(
    id: CaseId,
    feature_id: FeatureId,
    provenance: EvidenceProvenance,
    source_id: String,
    comparator: Comparator,
    primary_selector: EvidenceSelector,
) -> EvidenceCase {
    let statistical_plan = statistical_plan_reference(
        comparator,
        EvidenceStatus::Planned,
        provenance,
        "planned",
        &id,
    );
    let property_plan = property_plan::planned_reference(comparator, &id);
    EvidenceCase {
        id,
        feature_id,
        behavioral_surface: behavioral_surface_for_feature(feature_id, provenance),
        provenance,
        source_id,
        comparator,
        execution: super::super::execution_contract::for_status(EvidenceStatus::Planned),
        statistical_plan,
        property_plan,
        primary_selector,
        supporting_selectors: Vec::new(),
        resource_contract: semantic_only_resource_contract(),
        negative_axes: Vec::new(),
        performance_groups: feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect(),
        deferred_product: None,
        status: EvidenceStatus::Planned,
    }
}

pub(super) fn planned_selector(feature_id: FeatureId, source_id: &str) -> EvidenceSelector {
    let package = if feature_id == FeatureId::Cli {
        "stab-cli"
    } else {
        "stab-core"
    };
    let test_name = format!(
        "{}_{}",
        source_id.replace('-', "_"),
        feature_id.as_str().to_ascii_lowercase().replace('-', "_")
    );
    EvidenceSelector {
        state: EvidenceState::Planned,
        kind: SelectorKind::CargoTest,
        value: vec![
            "cargo".to_string(),
            "test".to_string(),
            "-p".to_string(),
            package.to_string(),
            test_name,
            "--quiet".to_string(),
            "--exact".to_string(),
        ],
    }
}

pub(super) fn planned_api_selector(crate_name: &str, owner_case_id: &CaseId) -> EvidenceSelector {
    let package = crate_name.replace('_', "-");
    let test_name = owner_case_id.as_str().replace('-', "_");
    EvidenceSelector {
        state: EvidenceState::Planned,
        kind: SelectorKind::CargoTest,
        value: vec![
            "cargo".to_string(),
            "test".to_string(),
            "-p".to_string(),
            package,
            test_name,
            "--quiet".to_string(),
            "--exact".to_string(),
        ],
    }
}
