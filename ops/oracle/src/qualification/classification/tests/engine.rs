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
fn extracted_detection_engine_keeps_detection_ownership() {
    for (crate_name, source, api) in [
        (
            "stab_engine",
            "crates/stab-engine/src/detection/api.rs",
            "stab_engine::DetectionSamplingCompiler::compile",
        ),
        (
            "stab_engine",
            "crates/stab-engine/src/detection/api.rs",
            "stab_engine::MeasurementToDetectionSession::run",
        ),
        (
            "stab_engine",
            "crates/stab-engine/src/detection/mod.rs",
            "stab_engine::CompiledDetectionConverter::convert_record",
        ),
        (
            "stab_core",
            "crates/stab-core/src/lib.rs",
            "stab_core::execution::DetectionSamplingCompiler::compile",
        ),
    ] {
        assert_eq!(
            classify_public_api_source(crate_name, Path::new(source), api),
            Some(FeatureId::Detection),
            "{api}"
        );
    }
}

#[test]
fn detection_measurement_sink_adapters_keep_record_boundary_ownership() {
    for (crate_name, source, api) in [
        (
            "stab_engine",
            "crates/stab-engine/src/detection/api/delivery.rs",
            "stab_engine::MeasurementToDetectionSinkAdapter::finish",
        ),
        (
            "stab_core",
            "crates/stab-core/src/detection/api/delivery.rs",
            "stab_core::execution::MeasurementToDetectionSinkAdapter::write_batch_with_sweep",
        ),
    ] {
        assert_eq!(
            classify_public_api_source(crate_name, Path::new(source), api),
            Some(FeatureId::ResultFormats),
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
