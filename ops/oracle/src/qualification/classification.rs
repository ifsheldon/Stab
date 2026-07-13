use std::path::Path;

use super::model::{Comparator, DeferredProduct, FeatureId, UpstreamDisposition};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UpstreamClassification {
    pub(super) feature_ids: Vec<FeatureId>,
    pub(super) disposition: UpstreamDisposition,
    pub(super) deferred_product: Option<DeferredProduct>,
    pub(super) reason: &'static str,
}

impl UpstreamClassification {
    fn selected(feature_id: FeatureId) -> Self {
        Self::selected_many([feature_id])
    }

    fn selected_many(feature_ids: impl IntoIterator<Item = FeatureId>) -> Self {
        let mut feature_ids = feature_ids.into_iter().collect::<Vec<_>>();
        feature_ids.sort();
        feature_ids.dedup();
        Self {
            feature_ids,
            disposition: UpstreamDisposition::SemanticMining,
            deferred_product: None,
            reason: "Selected upstream semantics require an independently owned CQ evidence case.",
        }
    }

    fn deferred(product: DeferredProduct, reason: &'static str) -> Self {
        Self::deferred_for([], product, reason)
    }

    fn deferred_for(
        feature_ids: impl IntoIterator<Item = FeatureId>,
        product: DeferredProduct,
        reason: &'static str,
    ) -> Self {
        let mut feature_ids = feature_ids.into_iter().collect::<Vec<_>>();
        feature_ids.sort();
        feature_ids.dedup();
        Self {
            feature_ids,
            disposition: UpstreamDisposition::DeferredProduct,
            deferred_product: Some(product),
            reason,
        }
    }

    fn not_applicable(reason: &'static str) -> Self {
        Self {
            feature_ids: Vec::new(),
            disposition: UpstreamDisposition::NotApplicable,
            deferred_product: None,
            reason,
        }
    }
}

pub(super) fn classify_upstream_case(path: &Path, symbol: &str) -> UpstreamClassification {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.starts_with("glue/cirq/") {
        return UpstreamClassification::deferred(
            DeferredProduct::Stimcirq,
            "stimcirq is an explicitly deferred ecosystem product.",
        );
    }
    if value.starts_with("glue/sample/") {
        return UpstreamClassification::deferred(
            DeferredProduct::Sinter,
            "sinter is an explicitly deferred ecosystem product.",
        );
    }
    if value.starts_with("glue/stimflow/") {
        return UpstreamClassification::deferred(
            DeferredProduct::Stimflow,
            "stimflow is an explicitly deferred ecosystem product.",
        );
    }
    if value.starts_with("glue/zx/") || value.starts_with("glue/lattice_surgery/") {
        return UpstreamClassification::deferred(
            DeferredProduct::ZxAndLatticeSurgery,
            "ZX and lattice-surgery integrations are explicitly deferred ecosystem products.",
        );
    }
    if value.contains("/diagram/") || value.contains("command_diagram") {
        return UpstreamClassification::deferred_for(
            deferred_path_domains(&value),
            DeferredProduct::Diagrams,
            "Diagram and visualization products are explicitly deferred.",
        );
    }
    if value.contains("command_explain_errors") {
        return UpstreamClassification::deferred_for(
            [FeatureId::Analyzer, FeatureId::Cli],
            DeferredProduct::ExplainErrors,
            "The explain_errors CLI and full ErrorMatcher provenance are explicitly deferred.",
        );
    }
    if value.contains("export_crumble") {
        return UpstreamClassification::deferred_for(
            [FeatureId::CircuitApi],
            DeferredProduct::Crumble,
            "Crumble export is explicitly deferred.",
        );
    }
    if value.contains("export_qasm") {
        return UpstreamClassification::deferred_for(
            [FeatureId::CircuitApi],
            DeferredProduct::Qasm,
            "QASM export is explicitly deferred.",
        );
    }
    if value.contains("export_quirk") {
        return UpstreamClassification::deferred_for(
            [FeatureId::CircuitApi],
            DeferredProduct::Quirk,
            "Quirk export is explicitly deferred.",
        );
    }
    if value.ends_with("stim_included_twice.test.cc") {
        return UpstreamClassification::not_applicable(
            "C++ include-twice behavior has no Rust crate compatibility contract.",
        );
    }
    if value.contains("/util_bot/twiddle.test.cc") {
        return UpstreamClassification::selected(FeatureId::BitKernels);
    }
    if value.contains("/mem/fixed_cap_vector.test.cc")
        || value.contains("/mem/monotonic_buffer.test.cc")
        || value.contains("/util_bot/test_util.test.cc")
        || value.contains("/util_bot/str_util.test.cc")
    {
        return UpstreamClassification::not_applicable(
            "The pinned C++ implementation helper has no selected Stab public or semantic contract.",
        );
    }
    if value.ends_with("src/stim/py/stim_pybind_test.py") {
        return UpstreamClassification::deferred(
            DeferredProduct::PythonBindings,
            "Top-level Python binding shape is explicitly deferred until Python bindings exist.",
        );
    }

    if symbol.to_ascii_lowercase().contains("detector_hypergraph") {
        return UpstreamClassification::deferred_for(
            [FeatureId::Analyzer, FeatureId::Cli],
            DeferredProduct::DeprecatedDetectorHypergraph,
            "The deprecated detector_hypergraph command spelling is intentionally unsupported.",
        );
    }
    if symbol.to_ascii_lowercase().contains("explain_errors") {
        return UpstreamClassification::deferred_for(
            [FeatureId::Analyzer],
            DeferredProduct::ExplainErrors,
            "The explain_errors product and full ErrorMatcher provenance are explicitly deferred.",
        );
    }
    if symbol.to_ascii_lowercase().contains("diagram") {
        return UpstreamClassification::deferred_for(
            deferred_path_domains(&value),
            DeferredProduct::Diagrams,
            "Diagram and visualization behavior is explicitly deferred.",
        );
    }
    if value.ends_with("frame_simulator_pybind_test.py")
        || value.ends_with("tableau_simulator_pybind_test.py")
    {
        return UpstreamClassification::deferred_for(
            [FeatureId::GateContract, FeatureId::Sampling],
            DeferredProduct::InteractiveSimulators,
            "Public Python-style interactive simulator products are explicitly deferred.",
        );
    }
    if value.ends_with("_pybind_test.py") && is_python_binding_shape_only(symbol) {
        return UpstreamClassification::deferred_for(
            python_binding_domains(&value),
            DeferredProduct::PythonBindings,
            "This case tests Python binding object shape instead of a selected Rust semantic contract.",
        );
    }

    if value.ends_with("src/stim/py/compiled_measurement_sampler_pybind_test.py") {
        return UpstreamClassification::selected(FeatureId::Sampling);
    }
    if value.ends_with("src/stim/py/compiled_detector_sampler_pybind_test.py") {
        return UpstreamClassification::selected(FeatureId::Detection);
    }

    if value.contains("/cmd/") {
        return classify_command(&value);
    }
    if value.contains("/circuit/") {
        return classify_circuit(&value, symbol);
    }
    if value.contains("/dem/") {
        return classify_dem(&value, symbol);
    }
    if value.contains("/gates/") {
        return UpstreamClassification::selected(FeatureId::GateContract);
    }
    if value.contains("/gen/") {
        return UpstreamClassification::selected(FeatureId::Generation);
    }
    if value.contains("/io/") {
        return UpstreamClassification::selected(FeatureId::ResultFormats);
    }
    if value.contains("/mem/") {
        return UpstreamClassification::selected(FeatureId::BitKernels);
    }
    if value.contains("/search/") {
        return UpstreamClassification::selected(FeatureId::Search);
    }
    if value.contains("/simulators/") {
        return classify_simulator(&value);
    }
    if value.contains("/stabilizers/") {
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    if value.contains("/util_bot/") {
        return classify_util_bot(&value);
    }
    if value.contains("/util_top/") {
        return classify_util_top(&value);
    }
    if value.ends_with("src/stim.test.cc") {
        return UpstreamClassification::selected(FeatureId::GateContract);
    }
    if value.ends_with("src/stim/main_namespaced.test.cc") {
        return UpstreamClassification::selected(FeatureId::Cli);
    }

    UpstreamClassification::not_applicable(
        "The upstream case is outside the selected Stab Rust and CLI contract inventory.",
    )
}

pub(super) fn classify_upstream_path(path: &Path) -> UpstreamClassification {
    classify_upstream_case(path, "")
}

pub(super) fn classify_public_api_source(
    crate_name: &str,
    source_path: &Path,
    api_path: &str,
) -> Option<FeatureId> {
    if crate_name == "stab_cli" {
        return Some(FeatureId::Cli);
    }
    let value = source_path.to_string_lossy().replace('\\', "/");
    let api_lower = api_path.to_ascii_lowercase();
    if api_lower.contains("erroranalyzeroptions")
        || api_lower.contains("circuit_to_detector_error_model")
        || api_lower.ends_with("::detector_error_model")
        || api_lower.contains("explain_errors")
    {
        return Some(FeatureId::Analyzer);
    }
    if api_lower.ends_with("::reference_sample")
        || api_lower.ends_with("::reference_sample_tree")
        || api_lower.ends_with("::count_determined_measurements")
    {
        return Some(FeatureId::Sampling);
    }
    if api_lower.contains("time_reversed_for_flows")
        || api_lower.contains("with_inlined_feedback")
        || api_lower.contains("circuit_with_inlined_feedback")
        || api_lower.ends_with("::inverse_qec")
        || api_lower.ends_with("::inverse_qec_with_options")
        || api_lower.ends_with("::inverse_unitary")
    {
        return Some(FeatureId::FlowUtils);
    }
    if api_lower.ends_with("::to_tableau") {
        return Some(FeatureId::Algebra);
    }
    if api_lower.contains("from_stim")
        || api_lower.contains("to_stim")
        || api_lower.contains("write_stim")
    {
        return Some(FeatureId::StimFormat);
    }
    if api_lower.contains("compileddemsampler") {
        return Some(FeatureId::DemSampling);
    }
    if api_lower.contains("shortest_graphlike")
        || api_lower.contains("find_undetectable")
        || api_lower.contains("sat_problem")
        || api_lower.contains("wcnf")
    {
        return Some(FeatureId::Search);
    }

    if value == "crates/stab-core/src/ids.rs" {
        if api_lower.contains("probability") {
            return Some(FeatureId::Sampling);
        } else if api_lower.contains("measurerecordoffset") {
            return Some(FeatureId::StimFormat);
        } else {
            return Some(FeatureId::CircuitApi);
        }
    }
    if value.starts_with("crates/stab-core/src/bits/") {
        return Some(FeatureId::BitKernels);
    }
    if value.starts_with("crates/stab-core/src/circuit_generation") {
        return Some(FeatureId::Generation);
    }
    if value.starts_with("crates/stab-core/src/circuit_flow/")
        || matches!(
            value.as_str(),
            "crates/stab-core/src/circuit_detecting_regions.rs"
                | "crates/stab-core/src/circuit_feedback.rs"
                | "crates/stab-core/src/circuit_inverse.rs"
                | "crates/stab-core/src/circuit_missing_detectors.rs"
        )
    {
        return Some(FeatureId::FlowUtils);
    }
    if value.starts_with("crates/stab-core/src/dem/analyze")
        || matches!(
            value.as_str(),
            "crates/stab-core/src/error_matcher.rs" | "crates/stab-core/src/matched_error.rs"
        )
    {
        return Some(FeatureId::Analyzer);
    }
    if value == "crates/stab-core/src/dem/sat.rs" {
        return Some(FeatureId::Search);
    }
    if matches!(
        value.as_str(),
        "crates/stab-core/src/dem.rs" | "crates/stab-core/src/dem/api.rs"
    ) {
        return Some(FeatureId::DemFormat);
    }
    if value == "crates/stab-core/src/dem_sampler.rs" {
        return Some(FeatureId::DemSampling);
    }
    if value == "crates/stab-core/src/detection.rs" {
        return Some(FeatureId::Detection);
    }
    if value.starts_with("crates/stab-core/src/gate") || value == "crates/stab-core/src/target.rs" {
        return Some(FeatureId::GateContract);
    }
    if value == "crates/stab-core/src/stabilizers/flow.rs" {
        return Some(FeatureId::FlowUtils);
    }
    if value.starts_with("crates/stab-core/src/stabilizers/")
        || value == "crates/stab-core/src/circuit_tableau.rs"
    {
        return Some(FeatureId::Algebra);
    }
    if value.starts_with("crates/stab-core/src/result_format")
        || value.starts_with("crates/stab-core/src/result_stream")
    {
        return Some(FeatureId::ResultFormats);
    }
    if value.starts_with("crates/stab-core/src/sampling")
        || matches!(
            value.as_str(),
            "crates/stab-core/src/probability_util.rs"
                | "crates/stab-core/src/reference_sample_tree.rs"
        )
    {
        return Some(FeatureId::Sampling);
    }
    if matches!(
        value.as_str(),
        "crates/stab-core/src/circuit.rs"
            | "crates/stab-core/src/circuit/api.rs"
            | "crates/stab-core/src/circuit/counts.rs"
            | "crates/stab-core/src/circuit/iter.rs"
            | "crates/stab-core/src/circuit_simplify.rs"
            | "crates/stab-core/src/circuit_transforms.rs"
            | "crates/stab-core/src/error.rs"
            | "crates/stab-core/src/mbqc_decomposition.rs"
    ) {
        return Some(FeatureId::CircuitApi);
    }
    None
}

fn classify_command(value: &str) -> UpstreamClassification {
    if value.contains("command_gen") {
        UpstreamClassification::selected_many([FeatureId::Generation, FeatureId::Cli])
    } else if value.contains("command_convert") {
        UpstreamClassification::selected_many([FeatureId::ResultFormats, FeatureId::Cli])
    } else if value.contains("command_sample_dem") {
        UpstreamClassification::selected_many([FeatureId::DemSampling, FeatureId::Cli])
    } else if value.contains("command_sample") {
        UpstreamClassification::selected_many([FeatureId::Sampling, FeatureId::Cli])
    } else if value.contains("command_detect") || value.contains("command_m2d") {
        UpstreamClassification::selected_many([FeatureId::Detection, FeatureId::Cli])
    } else if value.contains("command_analyze_errors") {
        UpstreamClassification::selected_many([FeatureId::Analyzer, FeatureId::Cli])
    } else {
        UpstreamClassification::selected(FeatureId::Cli)
    }
}

fn classify_circuit(value: &str, symbol: &str) -> UpstreamClassification {
    if value.contains("gate_target") || value.contains("circuit_instruction") {
        UpstreamClassification::selected(FeatureId::StimFormat)
    } else if value.contains("gate_decomposition") {
        UpstreamClassification::selected(FeatureId::GateContract)
    } else if value.ends_with("circuit_pybind_test.py") {
        let symbol = symbol.to_ascii_lowercase();
        let secondary = if symbol.contains("shortest_graphlike")
            || symbol.contains("search_for_undetectable")
            || symbol.contains("sat_problem")
            || symbol.contains("likeliest_error")
        {
            Some(FeatureId::Search)
        } else if symbol.contains("compile_detector_sampler")
            || symbol.contains("detector_sampling")
        {
            Some(FeatureId::Detection)
        } else if symbol.contains("compile_sampler")
            || symbol.contains("measurement_sampling")
            || symbol.contains("reference_sample")
            || symbol.contains("count_determined")
        {
            Some(FeatureId::Sampling)
        } else if symbol.contains("detector_error_model") || symbol.contains("dem_conversion") {
            Some(FeatureId::Analyzer)
        } else if symbol.contains("generation") {
            Some(FeatureId::Generation)
        } else if symbol.contains("has_flow")
            || symbol.contains("detecting_region")
            || symbol.contains("time_reversed_for_flows")
            || symbol.contains("inlined_feedback")
        {
            Some(FeatureId::FlowUtils)
        } else if symbol.contains("to_tableau") {
            Some(FeatureId::Algebra)
        } else {
            None
        };
        UpstreamClassification::selected_many(
            [Some(FeatureId::CircuitApi), secondary]
                .into_iter()
                .flatten(),
        )
    } else if value.ends_with("circuit.test.cc") {
        UpstreamClassification::selected_many([FeatureId::StimFormat, FeatureId::CircuitApi])
    } else {
        UpstreamClassification::selected(FeatureId::CircuitApi)
    }
}

fn classify_dem(value: &str, symbol: &str) -> UpstreamClassification {
    if value.ends_with("detector_error_model_pybind_test.py")
        && symbol.to_ascii_lowercase().contains("shortest_graphlike")
    {
        UpstreamClassification::selected_many([FeatureId::DemFormat, FeatureId::Search])
    } else {
        UpstreamClassification::selected(FeatureId::DemFormat)
    }
}

fn classify_simulator(value: &str) -> UpstreamClassification {
    if value.contains("dem_sampler") {
        UpstreamClassification::selected(FeatureId::DemSampling)
    } else if value.contains("measurements_to_detection") {
        UpstreamClassification::selected(FeatureId::Detection)
    } else if value.contains("frame_simulator") || value.contains("tableau_simulator") {
        UpstreamClassification::selected_many([FeatureId::GateContract, FeatureId::Sampling])
    } else if value.contains("graph_simulator") || value.contains("vector_simulator") {
        UpstreamClassification {
            feature_ids: vec![FeatureId::GateContract],
            disposition: UpstreamDisposition::SemanticMining,
            deferred_product: None,
            reason: "Public graph and vector simulator products are deferred, but their bounded state-equivalence cases remain selected semantic evidence.",
        }
    } else if value.contains("sparse_rev_frame") {
        UpstreamClassification::selected(FeatureId::FlowUtils)
    } else if value.contains("error_analyzer") {
        UpstreamClassification::selected_many([FeatureId::GateContract, FeatureId::Analyzer])
    } else {
        UpstreamClassification::selected(FeatureId::Analyzer)
    }
}

fn is_python_binding_shape_only(symbol: &str) -> bool {
    let leaf = symbol
        .rsplit('.')
        .next()
        .unwrap_or(symbol)
        .to_ascii_lowercase();
    leaf == "test_repr"
        || leaf.ends_with("_repr")
        || leaf == "test_pickle"
        || leaf == "test_hash"
        || leaf.ends_with("_hash")
        || leaf == "test_hashable"
        || leaf == "test_type"
        || leaf.contains("numpy")
        || leaf.contains("output_buffer")
}

fn deferred_path_domains(value: &str) -> Vec<FeatureId> {
    if value.contains("/cmd/") || value.contains("command_") {
        vec![FeatureId::Cli]
    } else if value.contains("/circuit/") {
        vec![FeatureId::CircuitApi]
    } else if value.contains("/dem/") {
        vec![FeatureId::DemFormat]
    } else {
        Vec::new()
    }
}

fn python_binding_domains(value: &str) -> Vec<FeatureId> {
    if value.contains("compiled_measurement_sampler") {
        vec![FeatureId::Sampling]
    } else if value.contains("compiled_detector_sampler") {
        vec![FeatureId::Detection]
    } else if value.contains("dem_sampler") {
        vec![FeatureId::DemSampling]
    } else if value.contains("/circuit/") {
        if value.contains("gate_target") || value.contains("circuit_instruction") {
            vec![FeatureId::StimFormat]
        } else {
            vec![FeatureId::CircuitApi]
        }
    } else if value.contains("/dem/") {
        vec![FeatureId::DemFormat]
    } else if value.contains("/stabilizers/flow") {
        vec![FeatureId::FlowUtils]
    } else if value.contains("/stabilizers/") {
        vec![FeatureId::Algebra]
    } else {
        Vec::new()
    }
}

fn classify_util_bot(value: &str) -> UpstreamClassification {
    if value.contains("error_decomp") {
        UpstreamClassification::selected(FeatureId::Analyzer)
    } else if value.contains("probability_util") {
        UpstreamClassification::selected(FeatureId::Sampling)
    } else if value.contains("arg_parse") {
        UpstreamClassification::selected(FeatureId::Cli)
    } else {
        UpstreamClassification::selected(FeatureId::Resource)
    }
}

fn classify_util_top(value: &str) -> UpstreamClassification {
    if value.contains("circuit_to_dem") {
        UpstreamClassification::selected(FeatureId::Analyzer)
    } else if value.contains("flow")
        || value.contains("detecting_regions")
        || value.contains("missing_detectors")
        || value.contains("transform_without_feedback")
    {
        UpstreamClassification::selected(FeatureId::FlowUtils)
    } else if value.contains("reference_sample") || value.contains("count_determined") {
        UpstreamClassification::selected(FeatureId::Sampling)
    } else if value.contains("stabilizers_to_tableau")
        || value.contains("stabilizers_vs_amplitudes")
        || value.contains("circuit_vs_amplitudes")
        || value.contains("circuit_vs_tableau")
    {
        UpstreamClassification::selected(FeatureId::Algebra)
    } else {
        UpstreamClassification::selected(FeatureId::CircuitApi)
    }
}

pub(super) fn default_comparator(feature_id: FeatureId) -> Comparator {
    match feature_id {
        FeatureId::StimFormat | FeatureId::DemFormat => Comparator::Canonical,
        FeatureId::ResultFormats | FeatureId::Generation | FeatureId::Cli => Comparator::ExactBytes,
        FeatureId::GateContract => Comparator::StateEquivalence,
        FeatureId::BitKernels | FeatureId::CircuitApi | FeatureId::Algebra => Comparator::Property,
        FeatureId::Sampling | FeatureId::DemSampling => Comparator::Statistical,
        FeatureId::Detection | FeatureId::Analyzer | FeatureId::FlowUtils => {
            Comparator::SemanticInvariant
        }
        FeatureId::Search => Comparator::Structural,
        FeatureId::Resource => Comparator::Resource,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifications_keep_deferred_products_out_of_selected_features() {
        let diagram = classify_upstream_path(Path::new("src/stim/cmd/command_diagram.test.cc"));
        assert_eq!(diagram.disposition, UpstreamDisposition::DeferredProduct);
        assert_eq!(diagram.feature_ids, vec![FeatureId::Cli]);
        assert_eq!(diagram.deferred_product, Some(DeferredProduct::Diagrams));

        let cirq = classify_upstream_path(Path::new("glue/cirq/stimcirq/_stim_sampler_test.py"));
        assert_eq!(cirq.disposition, UpstreamDisposition::DeferredProduct);
        assert!(cirq.feature_ids.is_empty());
        assert_eq!(cirq.deferred_product, Some(DeferredProduct::Stimcirq));
    }

    #[test]
    fn classifications_distinguish_selected_execution_domains() {
        let result = classify_upstream_path(Path::new("src/stim/cmd/command_convert.test.cc"));
        assert_eq!(
            result.feature_ids,
            vec![FeatureId::ResultFormats, FeatureId::Cli]
        );

        let flow = classify_upstream_path(Path::new(
            "src/stim/util_top/circuit_flow_generators.test.cc",
        ));
        assert_eq!(flow.feature_ids, vec![FeatureId::FlowUtils]);

        let bits = classify_public_api_source(
            "stab_core",
            Path::new("crates/stab-core/src/bits/simd.rs"),
            "stab_core::BitBlock::xor_assign",
        );
        assert_eq!(bits, Some(FeatureId::BitKernels));

        assert_eq!(
            classify_public_api_source(
                "stab_core",
                Path::new("crates/stab-core/src/dem/analyze.rs"),
                "stab_core::ErrorAnalyzerOptions",
            ),
            Some(FeatureId::Analyzer)
        );
        assert_eq!(
            classify_public_api_source(
                "stab_core",
                Path::new("crates/stab-core/src/circuit/api.rs"),
                "stab_core::Circuit::reference_sample",
            ),
            Some(FeatureId::Sampling)
        );
        assert_eq!(
            classify_public_api_source(
                "stab_core",
                Path::new("crates/stab-core/src/circuit.rs"),
                "stab_core::Circuit::time_reversed_for_flows",
            ),
            Some(FeatureId::FlowUtils)
        );

        let unknown = classify_public_api_source(
            "stab_core",
            Path::new("crates/stab-core/src/new_domain.rs"),
            "stab_core::NewDomain",
        );
        assert_eq!(unknown, None);
    }

    #[test]
    fn classifications_split_mixed_python_cases_by_exact_symbol() {
        let path = Path::new("src/stim/circuit/circuit_pybind_test.py");
        let search = classify_upstream_case(path, "test_shortest_graphlike_error");
        assert_eq!(
            search.feature_ids,
            vec![FeatureId::CircuitApi, FeatureId::Search]
        );

        let diagram = classify_upstream_case(path, "test_tag_diagram");
        assert_eq!(diagram.disposition, UpstreamDisposition::DeferredProduct);
        assert_eq!(diagram.deferred_product, Some(DeferredProduct::Diagrams));
        assert_eq!(diagram.feature_ids, vec![FeatureId::CircuitApi]);

        let repr = classify_upstream_case(path, "test_circuit_repr");
        assert_eq!(repr.disposition, UpstreamDisposition::DeferredProduct);
        assert_eq!(repr.deferred_product, Some(DeferredProduct::PythonBindings));
    }

    #[test]
    fn classifications_reconcile_domain_matrix_sources() {
        let circuit = classify_upstream_case(
            Path::new("src/stim/circuit/circuit.test.cc"),
            "circuit.from_text",
        );
        assert_eq!(
            circuit.feature_ids,
            vec![FeatureId::StimFormat, FeatureId::CircuitApi]
        );

        let frame = classify_upstream_case(
            Path::new("src/stim/simulators/frame_simulator.test.cc"),
            "FrameSimulator.consistency_64",
        );
        assert_eq!(
            frame.feature_ids,
            vec![FeatureId::GateContract, FeatureId::Sampling]
        );

        let analyzer = classify_upstream_case(
            Path::new("src/stim/simulators/error_analyzer.test.cc"),
            "ErrorAnalyzer.mpp_ordering",
        );
        assert_eq!(
            analyzer.feature_ids,
            vec![FeatureId::GateContract, FeatureId::Analyzer]
        );

        let compiled = classify_upstream_case(
            Path::new("src/stim/py/compiled_measurement_sampler_pybind_test.py"),
            "test_measurements_vs_resets",
        );
        assert_eq!(compiled.feature_ids, vec![FeatureId::Sampling]);
    }
}
