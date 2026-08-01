use super::CorrectnessApi;
use crate::qualification::model::{ApiDisposition, PerformanceDisposition};

pub(super) const BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING";
pub(super) const BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID: &str =
    "PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE";
pub(super) const PAULI_STRING_MULTIPLY_GROUP_ID: &str = "PERFQ-M6-PAULI-STRING";
pub(super) const PAULI_STRING_ITER_GROUP_ID: &str = "PERFQ-M6-PAULI-ITER";
pub(super) const CLIFFORD_STRING_NON_IDENTITY_GROUP_ID: &str =
    "PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY";
pub(in crate::qualification::discovery) const A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID: &str =
    "PERFQ-A2-CIRCUIT-MODEL-FINGERPRINT";
pub(in crate::qualification::discovery) const A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID: &str =
    "PERFQ-A2-SAMPLING-REQUEST-FINGERPRINT";
pub(in crate::qualification::discovery) const A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID: &str =
    "PERFQ-A2-SAMPLING-REQUEST-ESTIMATE";
pub(in crate::qualification::discovery) const A2_SAMPLER_COMPILE_GROUP_ID: &str =
    "PERFQ-A2-SAMPLER-COMPILE";
pub(in crate::qualification::discovery) const A7_EXACT_ML_COMPILE_GROUP_ID: &str =
    "PERFQ-A7-EXACT-ML-COMPILE";
pub(in crate::qualification::discovery) const A7_EXACT_ML_REUSED_DECODE_GROUP_ID: &str =
    "PERFQ-A7-EXACT-ML-REUSED-DECODE";
pub(in crate::qualification::discovery) const A7_PIPELINE_GROUP_ID: &str =
    "PERFQ-A7-SAMPLE-DETECT-DECODE-PIPELINE";
pub(in crate::qualification::discovery) const A8_EXTERNAL_NOISE_PASS_GROUP_ID: &str =
    "PERFQ-A8-EXTERNAL-NOISE-PASS";

pub(super) fn make_disposition(item: &CorrectnessApi) -> ApiDisposition {
    let performance_feature = item
        .performance_groups
        .first()
        .cloned()
        .unwrap_or_else(|| "PERF-RESOURCE-BOUNDARIES".to_string());
    let behavioral = is_behavioral(item)
        && !is_fixed_fingerprint_metadata(item)
        && !is_decoder_boundary_plumbing(item);
    let supporting_performance_features = item.performance_groups.iter().skip(1).cloned().collect();
    let diagnostic_parent = behavioral
        .then(|| diagnostic_group_id(&item.path))
        .flatten();
    let mut parent_group_ids = if let Some(parent) = diagnostic_parent {
        vec![parent.to_string()]
    } else if behavioral {
        item.performance_groups
            .iter()
            .filter_map(|feature| qualification_group_id(item, feature))
            .collect()
    } else {
        Vec::new()
    };
    let has_complete_measured_parent = behavioral
        && (diagnostic_parent.is_some() || parent_group_ids.len() == item.performance_groups.len());
    if !has_complete_measured_parent {
        parent_group_ids.clear();
    }
    ApiDisposition {
        id: item.id.clone(),
        path: item.path.clone(),
        kind: item.kind.clone(),
        performance_feature,
        supporting_performance_features,
        correctness_case_id: item.owner_case_id.clone(),
        disposition: if has_complete_measured_parent {
            PerformanceDisposition::CoveredByParent
        } else if behavioral {
            PerformanceDisposition::FutureCandidate
        } else {
            PerformanceDisposition::NotPerformanceRelevant
        },
        parent_group_ids,
        reason: if has_complete_measured_parent {
            if diagnostic_parent.is_some() {
                "Behavioral operation is covered by the listed executable Stab-only product diagnostic."
                    .to_string()
            } else {
                "Behavioral operation is covered by the listed executable release workload."
                    .to_string()
            }
        } else if behavioral {
            "Behavioral operation remains visible as a future workload candidate without creating a speculative benchmark product."
                .to_string()
        } else {
            "Declaration-only, derived, marker, accessor, error-plumbing, or boundary-support operation has no independent runtime workload."
                .to_string()
        },
    }
}

fn is_decoder_boundary_plumbing(item: &CorrectnessApi) -> bool {
    let path = item.path.as_str();
    let is_decoder_item = path.starts_with("stab_decoder::")
        || path.starts_with("stab_core::Decoder")
        || path.starts_with("stab_core::Decode");
    is_decoder_item
        && !matches!(
            path,
            "stab_decoder::decode_batch" | "stab_core::decode_batch"
        )
}

fn diagnostic_group_id(path: &str) -> Option<&'static str> {
    match path {
        "stab_core::Circuit::fingerprint" => Some(A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID),
        "stab_engine::CompilationRequestFingerprint::for_sampling"
        | "stab_core::CompilationRequestFingerprint::for_sampling" => {
            Some(A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID)
        }
        "stab_core::estimate_sampling_request" => Some(A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID),
        "stab_engine::SamplingCompiler::compile"
        | "stab_core::SamplingCompiler::compile"
        | "stab_core::execution::SamplingCompiler::compile"
        | "stab_core::advanced::compat::CompiledSampler::compile" => {
            Some(A2_SAMPLER_COMPILE_GROUP_ID)
        }
        "stab_decoder::decode_batch" | "stab_core::decode_batch" => {
            Some(A7_EXACT_ML_REUSED_DECODE_GROUP_ID)
        }
        "stab_analysis::run_circuit_pass" | "stab_core::experimental::run_circuit_pass" => {
            Some(A8_EXTERNAL_NOISE_PASS_GROUP_ID)
        }
        _ => None,
    }
}

pub(super) fn is_behavioral(item: &CorrectnessApi) -> bool {
    matches!(item.kind.as_str(), "function" | "method")
        || item.kind == "trait-impl" && behavioral_trait_impl(&item.path)
}

fn is_fixed_fingerprint_metadata(item: &CorrectnessApi) -> bool {
    matches!(
        item.path.as_str(),
        "stab_core::ModelDialect::as_str"
            | "stab_core::ModelDialect::all"
            | "stab_core::ModelFingerprint::schema_version"
            | "stab_core::ModelFingerprint::dialect"
            | "stab_core::ModelFingerprint::digest"
            | "stab_core::ModelFingerprint::digest_hex"
            | "stab_core::CompilationOperation::as_str"
            | "stab_core::CompilationRequestFingerprint::schema_version"
            | "stab_core::CompilationRequestFingerprint::compiler_schema_version"
            | "stab_core::CompilationRequestFingerprint::operation"
            | "stab_core::CompilationRequestFingerprint::model_fingerprint"
            | "stab_core::CompilationRequestFingerprint::digest"
            | "stab_core::CompilationRequestFingerprint::digest_hex"
            | "stab_engine::CompilationOperation::as_str"
            | "stab_engine::CompilationRequestFingerprint::schema_version"
            | "stab_engine::CompilationRequestFingerprint::compiler_schema_version"
            | "stab_engine::CompilationRequestFingerprint::operation"
            | "stab_engine::CompilationRequestFingerprint::model_fingerprint"
            | "stab_engine::CompilationRequestFingerprint::digest"
            | "stab_engine::CompilationRequestFingerprint::digest_hex"
            | "stab_core::CapabilitySet::current"
            | "stab_core::CapabilitySet::dialects"
            | "stab_core::CapabilitySet::gates"
            | "stab_core::CapabilitySet::record_formats"
            | "stab_core::CapabilitySet::codecs"
            | "stab_core::CapabilitySet::compilation_operations"
            | "stab_core::CapabilitySet::selectable_backend_ids"
            | "stab_core::CapabilitySet::default_parse_limits"
            | "stab_core::CompilationCapability::operation"
            | "stab_core::CompilationCapability::input_dialect"
            | "stab_core::CompilationCapability::compiler_schema_version"
            | "stab_core::CompilationCapability::request_fingerprint_schema_version"
            | "stab_core::CompilationCapability::has_configurable_limits"
            | "stab_core::CompilationCapability::supports_backend_selection"
            | "stab_core::RecordEncoding::as_str"
            | "stab_core::RecordFormat::all"
            | "stab_core::RecordFormat::as_str"
            | "stab_core::RecordFormat::encoding"
            | "stab_core::RecordFormat::records_per_group"
            | "stab_core::RecordFormat::estimate_output_bytes"
            | "stab_core::CodecCapability::format"
            | "stab_core::CodecCapability::can_decode"
            | "stab_core::CodecCapability::can_encode"
            | "stab_core::CodecCapability::requires_typed_layout"
    )
}

fn qualification_group_id(item: &CorrectnessApi, performance_feature: &str) -> Option<String> {
    if performance_feature == "PERF-BIT-KERNELS" {
        match item.path.as_str() {
            "stab_bits::BitMatrix::transpose"
            | "stab_core::advanced::storage::BitMatrix::transpose" => {
                return Some(BIT_MATRIX_TRANSPOSE_ALLOCATING_GROUP_ID.to_string());
            }
            "stab_bits::BitMatrix::transpose_square_in_place"
            | "stab_core::advanced::storage::BitMatrix::transpose_square_in_place" => {
                return Some(BIT_MATRIX_TRANSPOSE_IN_PLACE_GROUP_ID.to_string());
            }
            _ => {}
        }
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_algebra::CliffordString::right_multiply_in_place"
                | "stab_core::CliffordString::right_multiply_in_place"
        )
    {
        return Some(CLIFFORD_STRING_NON_IDENTITY_GROUP_ID.to_string());
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_algebra::PauliString::right_multiply_in_place_returning_log_i_scalar"
                | "stab_core::PauliString::right_multiply_in_place_returning_log_i_scalar"
        )
    {
        return Some(PAULI_STRING_MULTIPLY_GROUP_ID.to_string());
    }
    if performance_feature == "PERF-STABILIZER-ALGEBRA"
        && matches!(
            item.path.as_str(),
            "stab_algebra::PauliStringIterator::new"
                | "stab_algebra::PauliStringIterator::iter_next"
                | "stab_algebra::PauliStringIterator::result"
                | "stab_core::advanced::algebra::PauliStringIterator::new"
                | "stab_core::advanced::algebra::PauliStringIterator::iter_next"
                | "stab_core::advanced::algebra::PauliStringIterator::result"
        )
    {
        return Some(PAULI_STRING_ITER_GROUP_ID.to_string());
    }
    None
}

fn behavioral_trait_impl(path: &str) -> bool {
    let Some((_, rest)) = path.split_once(" as ") else {
        return false;
    };
    let trait_name = rest
        .split_once(" for@")
        .or_else(|| rest.split_once(" for "))
        .map_or(rest, |(name, _)| name)
        .split('@')
        .next()
        .unwrap_or(rest);
    matches!(
        trait_name,
        "Display" | "From" | "FromStr" | "Iterator" | "TryFrom"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_work_stays_visible_without_speculative_accessor_workloads() {
        for path in [
            "stab_core::ModelDialect::as_str",
            "stab_core::ModelDialect::all",
            "stab_core::ModelFingerprint::schema_version",
            "stab_core::ModelFingerprint::dialect",
            "stab_core::ModelFingerprint::digest",
            "stab_core::ModelFingerprint::digest_hex",
            "stab_core::CompilationRequestFingerprint::schema_version",
            "stab_core::CompilationRequestFingerprint::model_fingerprint",
            "stab_engine::CompilationOperation::as_str",
            "stab_engine::CompilationRequestFingerprint::schema_version",
            "stab_engine::CompilationRequestFingerprint::model_fingerprint",
            "stab_core::CapabilitySet::codecs",
            "stab_core::CompilationCapability::compiler_schema_version",
            "stab_core::RecordFormat::records_per_group",
            "stab_core::RecordFormat::estimate_output_bytes",
            "stab_core::CodecCapability::requires_typed_layout",
        ] {
            let disposition = make_disposition(&api(path, "method"));
            assert_eq!(
                disposition.disposition,
                PerformanceDisposition::NotPerformanceRelevant,
                "{path}"
            );
            assert!(disposition.parent_group_ids.is_empty(), "{path}");
        }

        for (path, parent) in [
            "stab_core::Circuit::fingerprint",
            "stab_engine::CompilationRequestFingerprint::for_sampling",
            "stab_core::CompilationRequestFingerprint::for_sampling",
            "stab_core::estimate_sampling_request",
            "stab_core::advanced::compat::CompiledSampler::compile",
        ]
        .into_iter()
        .zip([
            A2_CIRCUIT_MODEL_FINGERPRINT_GROUP_ID,
            A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
            A2_SAMPLING_REQUEST_FINGERPRINT_GROUP_ID,
            A2_SAMPLING_REQUEST_ESTIMATE_GROUP_ID,
            A2_SAMPLER_COMPILE_GROUP_ID,
        ]) {
            let disposition = make_disposition(&api(path, "method"));
            assert_eq!(
                disposition.disposition,
                PerformanceDisposition::CoveredByParent,
                "{path}"
            );
            assert_eq!(disposition.parent_group_ids, [parent], "{path}");
        }

        let path = "stab_core::DetectorErrorModel::fingerprint";
        let disposition = make_disposition(&api(path, "method"));
        assert_eq!(
            disposition.disposition,
            PerformanceDisposition::FutureCandidate,
            "{path}"
        );
        let path = "stab_engine::biased_randomize_bits";
        let disposition = make_disposition(&api(path, "function"));
        assert_eq!(
            disposition.disposition,
            PerformanceDisposition::FutureCandidate,
            "{path}"
        );
    }

    #[test]
    fn decoder_boundary_plumbing_does_not_create_speculative_workloads() {
        for path in [
            "stab_decoder::DecoderLayout::new",
            "stab_decoder::DecoderInputBatchView::detector",
            "stab_decoder::ValidatedDecodeBatch::set_prediction",
            "stab_decoder::DecodeBatchError as Display for@hash",
            "stab_core::DecoderModelView::try_new",
            "stab_core::DecodeCancellation::cancel",
        ] {
            let disposition = make_disposition(&api(path, "method"));
            assert_eq!(
                disposition.disposition,
                PerformanceDisposition::NotPerformanceRelevant,
                "{path}"
            );
            assert!(disposition.parent_group_ids.is_empty(), "{path}");
        }

        for path in ["stab_decoder::decode_batch", "stab_core::decode_batch"] {
            let disposition = make_disposition(&api(path, "function"));
            assert_eq!(
                disposition.disposition,
                PerformanceDisposition::CoveredByParent,
                "{path}"
            );
            assert_eq!(
                disposition.parent_group_ids,
                [A7_EXACT_ML_REUSED_DECODE_GROUP_ID],
                "{path}"
            );
        }
    }

    #[test]
    fn circuit_pass_execution_is_owned_by_the_external_pass_diagnostic() {
        for path in [
            "stab_analysis::run_circuit_pass",
            "stab_core::experimental::run_circuit_pass",
        ] {
            let disposition = make_disposition(&api(path, "function"));
            assert_eq!(
                disposition.disposition,
                PerformanceDisposition::CoveredByParent,
                "{path}"
            );
            assert_eq!(
                disposition.parent_group_ids,
                [A8_EXTERNAL_NOISE_PASS_GROUP_ID],
                "{path}"
            );
        }
    }

    #[test]
    fn extracted_algebra_and_facade_paths_share_release_workloads() {
        for path in [
            "stab_algebra::CliffordString::right_multiply_in_place",
            "stab_core::CliffordString::right_multiply_in_place",
        ] {
            assert_eq!(
                qualification_group_id(&api(path, "method"), "PERF-STABILIZER-ALGEBRA").as_deref(),
                Some(CLIFFORD_STRING_NON_IDENTITY_GROUP_ID),
                "{path}"
            );
        }
        for path in [
            "stab_algebra::PauliString::right_multiply_in_place_returning_log_i_scalar",
            "stab_core::PauliString::right_multiply_in_place_returning_log_i_scalar",
        ] {
            assert_eq!(
                qualification_group_id(&api(path, "method"), "PERF-STABILIZER-ALGEBRA").as_deref(),
                Some(PAULI_STRING_MULTIPLY_GROUP_ID),
                "{path}"
            );
        }
        for path in [
            "stab_algebra::PauliStringIterator::new",
            "stab_algebra::PauliStringIterator::iter_next",
            "stab_algebra::PauliStringIterator::result",
            "stab_core::advanced::algebra::PauliStringIterator::new",
        ] {
            assert_eq!(
                qualification_group_id(&api(path, "method"), "PERF-STABILIZER-ALGEBRA").as_deref(),
                Some(PAULI_STRING_ITER_GROUP_ID),
                "{path}"
            );
        }
    }

    fn api(path: &str, kind: &str) -> CorrectnessApi {
        CorrectnessApi {
            id: format!("test-{path}"),
            path: path.to_string(),
            kind: kind.to_string(),
            owner_case_id: "test-correctness-owner".to_string(),
            performance_groups: vec!["PERF-CIRCUIT-MODEL".to_string()],
        }
    }
}
