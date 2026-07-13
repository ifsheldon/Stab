use super::inventory::stable_id;
use super::model::QualificationManifest;
use super::model::{
    BehavioralSurface, Comparator, EvidenceCase, EvidenceProvenance, EvidenceSelector,
    EvidenceState, EvidenceStatus, FeatureId, PropertyPlanRef, PropertyPlanSource,
    ResourceContract, ResourceKind, SelectorKind, StableCaseDomain,
};
use super::validation::ValidationIssues;

pub(super) const EXISTING_RESOURCE_SOURCE_ID: &str =
    "safe_file::tests::descriptor_walk_rejects_symlinked_parent";
pub(super) const EXISTING_PROPERTY_SOURCE_ID: &str = super::property::PASS_TARGET_ID;

struct PlannedResourceCaseSpec {
    source_id: &'static str,
    kind: ResourceKind,
    detail: &'static str,
    negative_axes: &'static [&'static str],
}

const PLANNED_RESOURCE_CASES: [PlannedResourceCaseSpec; 13] = [
    PlannedResourceCaseSpec {
        source_id: "resource-parser-input-admission",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Parser entry points prove exact byte, line, and nesting acceptance limits plus first rejection without reading or allocating beyond the admitted input.",
        negative_axes: &["input-size", "line-count", "nesting-depth"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-count-arithmetic-overflow",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Width, shot, repeat, detector, observable, and allocation size products reject checked-arithmetic overflow before allocation or traversal.",
        negative_axes: &["count-overflow", "size-product-overflow"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-result-record-admission",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Result readers and writers prove exact record-width, record-count, packed-padding, and transposed-group admission boundaries.",
        negative_axes: &["record-width", "record-count", "partial-group"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-materialized-expansion-admission",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Circuit, DEM, sample, conversion, replay, and generated-output materialization rejects the first expansion beyond each public cap.",
        negative_axes: &["circuit-expansion", "dem-expansion", "sample-count"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-streaming-buffer-slope",
        kind: ResourceKind::Streaming,
        detail: "Streaming sample, detection, conversion, DEM, and result-format paths retain bounded per-record or per-64-record buffering as total records grow.",
        negative_axes: &["large-shot-count", "bounded-buffering"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-streaming-writer-failure",
        kind: ResourceKind::Streaming,
        detail: "Every public streaming writer stops promptly, propagates the original writer or broken-pipe error, and documents partial-output behavior.",
        negative_axes: &["writer-failure", "broken-pipe", "partial-output"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-streaming-visitor-failure",
        kind: ResourceKind::Streaming,
        detail: "Every public visitor API stops promptly on callback failure without producing later records or retaining shot-proportional state.",
        negative_axes: &["visitor-failure", "early-stop"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-replay-and-side-input-admission",
        kind: ResourceKind::Streaming,
        detail: "Replay, sweep, observable-side, and sampled-error inputs reject truncation, trailing data, and width mismatches with bounded lookahead.",
        negative_axes: &["replay-truncation", "trailing-data", "width-mismatch"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-folded-traversal-work",
        kind: ResourceKind::BoundedSearch,
        detail: "Folded circuit and DEM traversals enforce independent repeat-work and traversal-step limits without silently materializing repeats.",
        negative_axes: &["repeat-work", "traversal-work"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-search-and-solver-admission",
        kind: ResourceKind::BoundedSearch,
        detail: "Analyzer, graph, matching, SAT, WCNF, and flow solvers enforce independent edge, state, clause, literal, and matrix admission limits.",
        negative_axes: &[
            "graph-edges",
            "search-states",
            "clauses",
            "literals",
            "solver-matrix",
        ],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-allocation-scaling",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Allocation probes measure the declared constant, width-proportional, or output-proportional state slope for selected public engines and transforms.",
        negative_axes: &["allocation-count", "peak-live-bytes", "input-scale"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-path-boundary",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Repository, fixture, scratch, generated-artifact, and CLI path boundaries reject absolute, parent-traversal, and wrong-root inputs before file access.",
        negative_axes: &["absolute-path", "parent-traversal", "wrong-root"],
    },
    PlannedResourceCaseSpec {
        source_id: "resource-output-file-lifecycle",
        kind: ResourceKind::BoundedMaterialized,
        detail: "Output workflows prove fail-closed creation, replacement, cleanup, hard-link, and partial-write behavior at their public filesystem boundaries.",
        negative_axes: &[
            "existing-output",
            "hard-link",
            "cleanup-failure",
            "partial-write",
        ],
    },
];

pub(super) fn required_source_ids() -> impl Iterator<Item = &'static str> {
    std::iter::once(EXISTING_RESOURCE_SOURCE_ID)
        .chain(std::iter::once(EXISTING_PROPERTY_SOURCE_ID))
        .chain(PLANNED_RESOURCE_CASES.iter().map(|spec| spec.source_id))
}

pub(super) fn validate_inventory(
    manifest: &QualificationManifest,
    violations: &mut ValidationIssues,
) {
    let expected = required_source_ids().collect::<std::collections::BTreeSet<_>>();
    let resource_cases = manifest
        .evidence_cases
        .iter()
        .filter(|case| case.feature_id == FeatureId::Resource)
        .collect::<Vec<_>>();
    let actual = resource_cases
        .iter()
        .map(|case| case.source_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected || resource_cases.len() != expected.len() {
        violations
            .push("CQ-RESOURCE source-owned case inventory is incomplete or stale".to_string());
    }
    for case in resource_cases {
        let existing = matches!(
            case.source_id.as_str(),
            EXISTING_RESOURCE_SOURCE_ID | EXISTING_PROPERTY_SOURCE_ID
        );
        let expected_status = if existing {
            EvidenceStatus::Implemented
        } else {
            EvidenceStatus::Planned
        };
        if case.status != expected_status {
            violations.push(format!(
                "CQ-RESOURCE case {:?} has status {:?}, expected {:?}",
                case.id, case.status, expected_status
            ));
        }
        if !existing
            && (case.primary_selector.kind != SelectorKind::PropertyTarget
                || case.primary_selector.value.as_slice() != [case.source_id.as_str()])
        {
            violations.push(format!(
                "CQ-RESOURCE case {:?} has a stale planned selector",
                case.id
            ));
        }
    }
}

pub(super) fn planned_evidence() -> Vec<EvidenceCase> {
    let feature_id = FeatureId::Resource;
    PLANNED_RESOURCE_CASES
        .iter()
        .map(|spec| EvidenceCase {
            id: stable_id(StableCaseDomain::EvidenceResource, spec.source_id),
            feature_id,
            behavioral_surface: BehavioralSurface::ResourceBoundary,
            provenance: EvidenceProvenance::QualificationPlan,
            source_id: spec.source_id.to_string(),
            comparator: Comparator::Resource,
            execution: super::execution_contract::for_status(EvidenceStatus::Planned),
            statistical_plan: None,
            property_plan: None,
            primary_selector: EvidenceSelector {
                state: EvidenceState::Planned,
                kind: SelectorKind::PropertyTarget,
                value: vec![spec.source_id.to_string()],
            },
            supporting_selectors: Vec::new(),
            resource_contract: ResourceContract {
                kind: spec.kind,
                detail: spec.detail.to_string(),
            },
            negative_axes: spec
                .negative_axes
                .iter()
                .map(|axis| (*axis).to_string())
                .collect(),
            performance_groups: feature_id
                .performance_groups()
                .iter()
                .map(|group| (*group).to_string())
                .collect(),
            deferred_product: None,
            status: EvidenceStatus::Planned,
        })
        .collect()
}

pub(super) fn existing_regression() -> EvidenceCase {
    let feature_id = FeatureId::Resource;
    EvidenceCase {
        id: stable_id(StableCaseDomain::EvidenceResource, "safe-file"),
        feature_id,
        behavioral_surface: BehavioralSurface::ResourceBoundary,
        provenance: EvidenceProvenance::RustRegression,
        source_id: EXISTING_RESOURCE_SOURCE_ID.to_string(),
        comparator: Comparator::Resource,
        execution: super::execution_contract::for_status(EvidenceStatus::Implemented),
        statistical_plan: None,
        property_plan: None,
        primary_selector: EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::CargoTest,
            value: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "stab-oracle".to_string(),
                EXISTING_RESOURCE_SOURCE_ID.to_string(),
                "--quiet".to_string(),
                "--exact".to_string(),
            ],
        },
        supporting_selectors: Vec::new(),
        resource_contract: ResourceContract {
            kind: ResourceKind::BoundedMaterialized,
            detail: "Descriptor-relative traversal rejects a symlinked parent before opening the owned file."
                .to_string(),
        },
        negative_axes: vec!["symlink-parent".to_string(), "path-traversal".to_string()],
        performance_groups: feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect(),
        deferred_product: None,
        status: EvidenceStatus::Implemented,
    }
}

pub(super) fn existing_property_regression() -> EvidenceCase {
    let feature_id = FeatureId::Resource;
    EvidenceCase {
        id: stable_id(
            StableCaseDomain::EvidenceResource,
            EXISTING_PROPERTY_SOURCE_ID,
        ),
        feature_id,
        behavioral_surface: BehavioralSurface::ResourceBoundary,
        provenance: EvidenceProvenance::QualificationPlan,
        source_id: EXISTING_PROPERTY_SOURCE_ID.to_string(),
        comparator: Comparator::Property,
        execution: super::execution_contract::for_status(EvidenceStatus::Implemented),
        statistical_plan: None,
        property_plan: Some(PropertyPlanRef {
            state: EvidenceState::Existing,
            source: PropertyPlanSource::QualificationCase,
            id: EXISTING_PROPERTY_SOURCE_ID.to_string(),
            plan: super::property::registered_execution_plan(EXISTING_PROPERTY_SOURCE_ID),
        }),
        primary_selector: EvidenceSelector {
            state: EvidenceState::Existing,
            kind: SelectorKind::PropertyTarget,
            value: vec![EXISTING_PROPERTY_SOURCE_ID.to_string()],
        },
        supporting_selectors: Vec::new(),
        resource_contract: ResourceContract {
            kind: ResourceKind::BoundedMaterialized,
            detail: "The registered property worker binds its manifest seed, case-count, size, persistence, timeout, and replay contract before execution."
                .to_string(),
        },
        negative_axes: vec![
            "property-plan-drift".to_string(),
            "property-worker-timeout".to_string(),
            "property-replay-failure".to_string(),
        ],
        performance_groups: feature_id
            .performance_groups()
            .iter()
            .map(|group| (*group).to_string())
            .collect(),
        deferred_product: None,
        status: EvidenceStatus::Implemented,
    }
}
