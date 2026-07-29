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
