use super::*;

#[test]
fn classifications_route_each_advanced_facade_tier_to_its_semantic_domain() {
    let source = Path::new("crates/stab-core/src/advanced.rs");
    for (api, expected) in [
        (
            "stab_core::advanced::storage::BitMatrix::transpose",
            FeatureId::BitKernels,
        ),
        (
            "stab_core::advanced::algebra::pauli_from_bases_unchecked",
            FeatureId::Algebra,
        ),
        (
            "stab_core::advanced::records::read_dets_records",
            FeatureId::ResultFormats,
        ),
        (
            "stab_core::advanced::backend::BackendPreference",
            FeatureId::Sampling,
        ),
        (
            "stab_core::advanced::backend::SamplingCompilationDescriptor",
            FeatureId::CircuitApi,
        ),
        (
            "stab_core::advanced::traversal::CircuitFlattenedInstructionIter",
            FeatureId::CircuitApi,
        ),
        (
            "stab_core::advanced::traversal::FoldedDemTraversal",
            FeatureId::DemFormat,
        ),
        (
            "stab_core::advanced::compat::CompiledSampler",
            FeatureId::Sampling,
        ),
        (
            "stab_core::advanced::compat::CompiledDetectionConverter",
            FeatureId::Detection,
        ),
        (
            "stab_core::advanced::compat::CompiledDemSampler",
            FeatureId::DemSampling,
        ),
        (
            "stab_core::advanced::compat::CompiledDemSampler::session_with_limits",
            FeatureId::Resource,
        ),
    ] {
        assert_eq!(
            classify_public_api_source("stab_core", source, api),
            Some(expected),
            "{api}"
        );
    }
}
