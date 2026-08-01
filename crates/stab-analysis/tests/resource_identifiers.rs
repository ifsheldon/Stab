use stab_analysis::{ResourceKind, ResourceOperation};

#[test]
fn analysis_resource_identifiers_are_stable() {
    for (operation, code) in [
        (ResourceOperation::CircuitPass, "circuit-pass"),
        (ResourceOperation::CircuitFlatten, "circuit-flatten"),
        (
            ResourceOperation::DetectorErrorModelFlatten,
            "detector-error-model-flatten",
        ),
        (
            ResourceOperation::LogicalErrorSearch,
            "logical-error-search",
        ),
        (ResourceOperation::SatMaterialization, "sat-materialization"),
    ] {
        assert_eq!(operation.as_str(), code);
    }

    for (resource, code) in [
        (ResourceKind::RepeatNesting, "repeat-nesting"),
        (ResourceKind::RepresentedItems, "represented-items"),
        (ResourceKind::ExpandedOperations, "expanded-operations"),
        (ResourceKind::RepeatCount, "repeat-count"),
        (ResourceKind::RepeatIterations, "repeat-iterations"),
        (ResourceKind::MaterializedUnits, "materialized-units"),
        (ResourceKind::MaterializedBytes, "materialized-bytes"),
        (
            ResourceKind::ProjectedPayloadBytes,
            "projected-payload-bytes",
        ),
        (ResourceKind::TargetOccurrences, "target-occurrences"),
        (ResourceKind::ArgumentValues, "argument-values"),
        (ResourceKind::ErrorMechanisms, "error-mechanisms"),
        (
            ResourceKind::ErrorTargetOccurrencesPerMechanism,
            "error-target-occurrences-per-mechanism",
        ),
        (
            ResourceKind::TotalErrorTargetOccurrences,
            "total-error-target-occurrences",
        ),
        (
            ResourceKind::EffectiveDetectorNodes,
            "effective-detector-nodes",
        ),
        (ResourceKind::UniqueGraphEdges, "unique-graph-edges"),
        (ResourceKind::StoredGraphTerms, "stored-graph-terms"),
        (ResourceKind::HyperedgeDegree, "hyperedge-degree"),
        (ResourceKind::HyperedgeIncidences, "hyperedge-incidences"),
        (ResourceKind::SearchStates, "search-states"),
        (ResourceKind::SearchTransitions, "search-transitions"),
        (ResourceKind::SearchStateTerms, "search-state-terms"),
        (
            ResourceKind::StoredSearchStateTerms,
            "stored-search-state-terms",
        ),
        (ResourceKind::Variables, "variables"),
        (ResourceKind::Clauses, "clauses"),
        (ResourceKind::ClauseLiterals, "clause-literals"),
        (ResourceKind::OutputBytes, "output-bytes"),
    ] {
        assert_eq!(resource.as_str(), code);
    }
}
