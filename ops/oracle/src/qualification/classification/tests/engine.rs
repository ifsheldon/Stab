use std::path::Path;

use super::super::{FeatureId, classify_public_api_source};

#[test]
fn extracted_engine_foundations_keep_sampling_ownership() {
    for (source, api) in [
        (
            "crates/stab-engine/src/fingerprint.rs",
            "stab_engine::CompilationRequestFingerprint::for_sampling",
        ),
        (
            "crates/stab-engine/src/fingerprint.rs",
            "stab_engine::fingerprint::CompilationOperation::as_str",
        ),
        (
            "crates/stab-engine/src/probability.rs",
            "stab_engine::probability::biased_randomize_bits",
        ),
    ] {
        assert_eq!(
            classify_public_api_source("stab_engine", Path::new(source), api),
            Some(FeatureId::Sampling),
            "{api}"
        );
    }
}

#[test]
fn extracted_sampling_engine_keeps_sampling_ownership() {
    for (source, api) in [
        (
            "crates/stab-engine/src/sampling/api.rs",
            "stab_engine::SamplingCompiler::compile",
        ),
        (
            "crates/stab-engine/src/sampling/api.rs",
            "stab_engine::SamplingSession::run",
        ),
        (
            "crates/stab-engine/src/sampling/mod.rs",
            "stab_engine::count_determined_measurements",
        ),
        (
            "crates/stab-core/src/lib.rs",
            "stab_core::SamplingCompiler::compile",
        ),
    ] {
        assert_eq!(
            classify_public_api_source("stab_engine", Path::new(source), api),
            Some(FeatureId::Sampling),
            "{api}"
        );
    }
}

#[test]
fn sampling_descriptor_keeps_capability_ownership() {
    for api in [
        "stab_engine::SamplingCompilationDescriptor",
        "stab_engine::COMPILATION_DESCRIPTOR",
    ] {
        assert_eq!(
            classify_public_api_source(
                "stab_engine",
                Path::new("crates/stab-engine/src/sampling/mod.rs"),
                api,
            ),
            Some(FeatureId::CircuitApi),
            "{api}"
        );
    }
}
