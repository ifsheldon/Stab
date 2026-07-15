use std::path::Path;

use super::model::{Comparator, DeferredProduct, FeatureId, UpstreamDisposition};

mod stabilizer;

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
    if value.ends_with("circuit_instruction_pybind_test.py")
        && symbol.rsplit('.').next() == Some("test_init_from_str")
    {
        return UpstreamClassification::deferred_for(
            [FeatureId::StimFormat],
            DeferredProduct::PythonBindings,
            "The overloaded Python CircuitInstruction string constructor is deferred with Python bindings; Stab exposes circuit-level Stim parsing instead.",
        );
    }

    if value.ends_with("src/stim/py/compiled_measurement_sampler_pybind_test.py") {
        return UpstreamClassification::selected(FeatureId::Sampling);
    }
    if value.ends_with("src/stim/py/compiled_detector_sampler_pybind_test.py") {
        return UpstreamClassification::selected(FeatureId::Detection);
    }

    if value.ends_with("src/stim/circuit/circuit_pybind_test.py")
        && symbol == "test_circuit_generation_errors"
    {
        return UpstreamClassification::deferred_for(
            [FeatureId::Generation],
            DeferredProduct::PythonBindings,
            "This mixed case enters through Python Circuit.generated string dispatch; Stab owns the portable Generation distance, round, task, and family constraints independently, while Probability value validation remains in its own qualification domain. The complete upstream symbol is deferred until Python bindings exist.",
        );
    }

    if value.ends_with("src/stim/cmd/command_gen.test.cc") {
        return if symbol == "command_gen.execute" {
            UpstreamClassification::selected(FeatureId::Cli)
        } else {
            UpstreamClassification::selected(FeatureId::Generation)
        };
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
        return classify_gates(&value, symbol);
    }
    if value.contains("/gen/") {
        return UpstreamClassification::selected(FeatureId::Generation);
    }
    if value.contains("/io/") {
        return UpstreamClassification::selected(FeatureId::ResultFormats);
    }
    if value.contains("/mem/") {
        return classify_mem(&value, symbol);
    }
    if value.contains("/search/") {
        return UpstreamClassification::selected(FeatureId::Search);
    }
    if value.contains("/simulators/") {
        return classify_simulator(&value, symbol);
    }
    if value.contains("/stabilizers/") {
        return stabilizer::classify(&value, symbol);
    }
    if value.contains("/util_bot/") {
        return classify_util_bot(&value);
    }
    if value.contains("/util_top/") {
        return classify_util_top(&value, symbol);
    }
    if value.ends_with("src/stim.test.cc") {
        return UpstreamClassification::not_applicable(
            "C++ public-header include behavior has no selected Rust compatibility contract.",
        );
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
    let leaf = symbol.rsplit('.').next().unwrap_or(symbol);
    if value.contains("gate_target") && matches!(leaf, "equality" | "test_init_and_equality") {
        UpstreamClassification::selected(FeatureId::GateContract)
    } else if value.ends_with("circuit_instruction_pybind_test.py") {
        classify_circuit_instruction_binding(leaf)
    } else if value.ends_with("circuit_repeat_block_test.py") {
        classify_circuit_repeat_binding(leaf)
    } else if value.contains("gate_target") || value.contains("circuit_instruction") {
        UpstreamClassification::selected(FeatureId::StimFormat)
    } else if value.contains("gate_decomposition") {
        classify_gate_decomposition(symbol)
    } else if value.ends_with("circuit_pybind_test.py") {
        classify_circuit_binding(symbol)
    } else if value.ends_with("circuit.test.cc") {
        classify_circuit_cpp(symbol)
    } else {
        UpstreamClassification::selected(FeatureId::CircuitApi)
    }
}

fn classify_circuit_instruction_binding(leaf: &str) -> UpstreamClassification {
    if matches!(leaf, "test_init_and_equality" | "test_num_measurements") {
        UpstreamClassification::selected(FeatureId::CircuitApi)
    } else if leaf == "test_target_groups" {
        UpstreamClassification::selected_many([FeatureId::StimFormat, FeatureId::CircuitApi])
    } else if matches!(leaf, "test_repr" | "test_hashable" | "test_init_from_str") {
        deferred_python_circuit_binding()
    } else {
        UpstreamClassification::selected(FeatureId::StimFormat)
    }
}

fn classify_circuit_repeat_binding(leaf: &str) -> UpstreamClassification {
    if leaf == "test_init_and_equality" {
        UpstreamClassification::selected(FeatureId::CircuitApi)
    } else {
        deferred_python_circuit_binding()
    }
}

fn classify_circuit_binding(symbol: &str) -> UpstreamClassification {
    let symbol = symbol.to_ascii_lowercase();
    if symbol.contains("diagram") || symbol.contains("detslice") {
        return UpstreamClassification::deferred_for(
            [FeatureId::CircuitApi],
            DeferredProduct::Diagrams,
            "Circuit diagram and detector-slice rendering are explicitly deferred products.",
        );
    }
    if symbol.contains("append_from_stim_program_text") {
        return UpstreamClassification::selected(FeatureId::StimFormat);
    }
    if symbol.contains("shortest_graphlike")
        || symbol.contains("search_for_undetectable")
        || symbol.contains("sat_problem")
        || symbol.contains("likeliest_error")
    {
        return UpstreamClassification::selected(FeatureId::Search);
    }
    if symbol.contains("compile_detector_sampler") || symbol.contains("detector_sampling") {
        return UpstreamClassification::selected(FeatureId::Detection);
    }
    if symbol == "test_tag_compile_samplers" {
        return UpstreamClassification::selected_many([FeatureId::Sampling, FeatureId::Detection]);
    }
    if symbol.contains("compile_sampler")
        || symbol.contains("measurement_sampling")
        || symbol.contains("reference_sample")
        || symbol.contains("count_determined")
    {
        return UpstreamClassification::selected(FeatureId::Sampling);
    }
    if symbol.contains("detector_error_model")
        || symbol.contains("dem_conversion")
        || symbol.contains("explain_errors")
        || symbol.contains("anti_commuting_mpp")
        || symbol.contains("blocked_remnant_edge")
    {
        return UpstreamClassification::selected(FeatureId::Analyzer);
    }
    if symbol.contains("generation") {
        return UpstreamClassification::selected(FeatureId::Generation);
    }
    if symbol.contains("has_flow")
        || symbol.contains("detecting_region")
        || symbol.contains("time_reversed_for_flows")
        || symbol.contains("inlined_feedback")
        || symbol == "test_circuit_inverse"
        || symbol == "test_tag_inverse"
    {
        return UpstreamClassification::selected(FeatureId::FlowUtils);
    }
    if symbol.contains("to_tableau") {
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    if symbol == "test_circuit_create_with_odd_cx" {
        return UpstreamClassification::selected(FeatureId::StimFormat);
    }
    if is_python_only_circuit_binding(&symbol) {
        return deferred_python_circuit_binding();
    }
    UpstreamClassification::selected(FeatureId::CircuitApi)
}

fn is_python_only_circuit_binding(symbol: &str) -> bool {
    matches!(
        symbol,
        "test_approx_equals"
            | "test_append_instructions_and_blocks"
            | "test_append_tag"
            | "test_append_extended_cases"
            | "test_append_pauli_string"
            | "test_backwards_compatibility_vs_safety_append_vs_append_operation"
            | "test_circuit_add"
            | "test_circuit_add_tags"
            | "test_circuit_append_operation"
            | "test_circuit_eq"
            | "test_circuit_from_file"
            | "test_circuit_get_item_tags"
            | "test_circuit_iadd"
            | "test_circuit_mul"
            | "test_circuit_repr"
            | "test_circuit_slice_reverse"
            | "test_circuit_to_file"
            | "test_complex_slice_does_not_seg_fault"
            | "test_copy"
            | "test_hash"
            | "test_indexing_operations"
            | "test_insert"
            | "test_pickle"
            | "test_pop"
            | "test_reappend_gate_targets"
            | "test_reference_detector_and_observable_signs"
            | "test_slicing"
            | "test_tag_approx_equals"
            | "test_tag_copy"
            | "test_tag_from_file"
            | "test_tags_iadd"
            | "test_tags_imul"
            | "test_tags_mul"
    )
}

fn deferred_python_circuit_binding() -> UpstreamClassification {
    UpstreamClassification::deferred_for(
        [FeatureId::CircuitApi],
        DeferredProduct::PythonBindings,
        "Python Circuit object shape, operators, indexing, copying, hashing, pickling, flexible target coercion, and binding-only helpers are deferred; selected Rust APIs own their semantic contracts independently.",
    )
}

fn classify_circuit_cpp(symbol: &str) -> UpstreamClassification {
    if symbol.is_empty() {
        return UpstreamClassification::selected_many([
            FeatureId::StimFormat,
            FeatureId::CircuitApi,
        ]);
    }
    let leaf = symbol.strip_prefix("circuit.").unwrap_or(symbol);
    if leaf == "py_get_slice" {
        return deferred_python_circuit_binding();
    }
    if leaf == "inverse" {
        return UpstreamClassification::selected(FeatureId::FlowUtils);
    }
    if matches!(
        leaf,
        "count_detectors_num_observables" | "count_measurements"
    ) {
        return UpstreamClassification::not_applicable(
            "This aggregate mixes selected non-overflow count semantics with C++ UINT64_MAX saturation. Exact Rust Circuit count owners prove the shared semantics and Stab's documented checked-overflow contract without claiming the incompatible saturation assertion.",
        );
    }
    if matches!(
        leaf,
        "addition_shares_blocks"
            | "aliased_noiseless_circuit"
            | "approx_equals"
            | "concat_self_fuse"
            | "generate_test_circuit_with_all_operations"
            | "max_lookback"
            | "self_addition"
    ) {
        return UpstreamClassification::not_applicable(
            "This case exercises a C++ storage-sharing, aliasing, approximate-comparison, private helper, or unexposed summary API outside the selected Stab Rust circuit contract.",
        );
    }

    let format = circuit_case_has_stim_format_contract(symbol);
    let circuit_api = matches!(
        leaf,
        "append_circuit"
            | "append_op_fuse"
            | "append_repeat_block"
            | "assignment_copies_operations"
            | "big_rep_count"
            | "concat_fuse"
            | "coords_of_detector"
            | "count_qubits"
            | "count_sweep_bits"
            | "count_ticks"
            | "equality"
            | "final_coord_shift"
            | "flattened"
            | "for_each_operation"
            | "for_each_operation_reverse"
            | "get_final_qubit_coords"
            | "get_final_qubit_coords_huge_repetition_count_efficiency"
            | "insert_circuit"
            | "insert_instruction"
            | "multiplication_repeats"
            | "noiseless_heralded_erase"
            | "preserves_repetition_blocks"
            | "without_tags"
    );
    match (format, circuit_api) {
        (true, true) => {
            UpstreamClassification::selected_many([FeatureId::StimFormat, FeatureId::CircuitApi])
        }
        (true, false) => UpstreamClassification::selected(FeatureId::StimFormat),
        (false, true) => UpstreamClassification::selected(FeatureId::CircuitApi),
        (false, false) => UpstreamClassification::selected(FeatureId::CircuitApi),
    }
}

fn classify_gate_decomposition(symbol: &str) -> UpstreamClassification {
    if symbol.is_empty()
        || symbol
            .rsplit('.')
            .next()
            .is_some_and(|leaf| leaf.starts_with("decompose_spp_or_spp_dag_operation_"))
    {
        UpstreamClassification::selected(FeatureId::GateContract)
    } else {
        UpstreamClassification::not_applicable(
            "This case exercises a private C++ gate-decomposition traversal helper; selected Stab parity is owned by public decomposition metadata and executable gate semantics.",
        )
    }
}

fn classify_gates(value: &str, symbol: &str) -> UpstreamClassification {
    if value.ends_with("gates.test.cc")
        && matches!(
            symbol,
            "gate_data.zero_flag_means_not_a_gate"
                | "gate_data.hash_matches_storage_location"
                | "gate_data.to_euler_angles"
                | "gate_data.to_axis_angle"
                | "gate_data.to_euler_angles_axis_reference"
                | "gate_data.hadamard_conjugated_vs_flow_generators_of_two_qubit_gates"
        )
    {
        UpstreamClassification::not_applicable(
            "This case exercises C++ GateData storage or helper APIs that are outside the selected public Rust Gate contract.",
        )
    } else if value.ends_with("gates_test.py") && symbol == "test_gate_hadamard_conjugated" {
        UpstreamClassification::deferred_for(
            [FeatureId::GateContract],
            DeferredProduct::PythonBindings,
            "GateData.hadamard_conjugated is part of the explicitly deferred Python GateData surface and has no selected Rust Gate analogue.",
        )
    } else {
        UpstreamClassification::selected(FeatureId::GateContract)
    }
}

fn classify_mem(value: &str, symbol: &str) -> UpstreamClassification {
    if symbol.is_empty() {
        return UpstreamClassification::selected(FeatureId::BitKernels);
    }
    let leaf = symbol.rsplit('.').next().unwrap_or(symbol);
    let base = strip_stim_word_size_suffix(leaf);

    let selected = if value.ends_with("bit_ref.test.cc") {
        matches!(base, "get" | "set" | "bit_xor" | "bit_andr" | "bit_or")
    } else if value.ends_with("simd_bit_table.test.cc") {
        matches!(base, "equality" | "transposed" | "xor_row_into")
    } else if value.ends_with("simd_bits_range_ref.test.cc") {
        matches!(
            base,
            "assignment" | "clear" | "equality" | "not_zero256" | "popcnt" | "xor_assignment"
        )
    } else if value.ends_with("simd_bits.test.cc") {
        matches!(
            base,
            "assignment"
                | "clear"
                | "equality"
                | "mask_assignment_and"
                | "mask_assignment_or"
                | "not_zero"
                | "popcnt"
                | "xor_assignment"
        )
    } else if value.ends_with("simd_util.test.cc") {
        matches!(
            base,
            "inplace_transpose" | "inplace_transpose_64x64" | "simd_bit_table_transpose"
        )
    } else if value.ends_with("simd_word.test.cc") {
        matches!(base, "equality" | "from_u64_array" | "masking" | "popcount")
    } else if value.ends_with("sparse_xor_vec.test.cc") {
        symbol.starts_with("sparse_xor_vec.")
    } else {
        return UpstreamClassification::selected(FeatureId::BitKernels);
    };

    if selected {
        UpstreamClassification::selected(FeatureId::BitKernels)
    } else {
        UpstreamClassification::not_applicable(
            "This case exercises a C++-specific SIMD storage, ownership, aliasing, resizing, lane-layout, arithmetic, shift, or raw-randomization helper with no selected Stab Rust bit-kernel contract.",
        )
    }
}

fn strip_stim_word_size_suffix(symbol: &str) -> &str {
    for suffix in ["_64", "_128", "_256"] {
        if let Some(base) = symbol.strip_suffix(suffix) {
            return base;
        }
    }
    symbol
}

fn circuit_case_has_stim_format_contract(symbol: &str) -> bool {
    let leaf = symbol.strip_prefix("circuit.").unwrap_or(symbol);
    leaf.starts_with("parse_")
        || leaf.ends_with("_validation")
        || leaf.starts_with("validate_")
        || matches!(
            leaf,
            "append_op_fuse"
                | "big_rep_count"
                | "classical_controls"
                | "concat_fuse"
                | "from_text"
                | "negative_float_coordinates"
                | "parse_windows_newlines"
                | "preserves_repetition_blocks"
                | "qubit_coords"
                | "str"
                | "without_tags"
                | "zero_repetitions_not_allowed"
        )
}

fn classify_dem(value: &str, symbol: &str) -> UpstreamClassification {
    let leaf = symbol
        .rsplit('.')
        .next()
        .unwrap_or(symbol)
        .to_ascii_lowercase();

    if value.ends_with("detector_error_model_pybind_test.py") && leaf.contains("shortest_graphlike")
    {
        return UpstreamClassification::selected(FeatureId::Search);
    }

    let binding_only_python_case = if value.ends_with("dem_instruction_pybind_test.py") {
        matches!(
            leaf.as_str(),
            "test_args_copy" | "test_targets_copy" | "test_init_from_str"
        )
    } else if value.ends_with("detector_error_model_pybind_test.py") {
        matches!(
            leaf.as_str(),
            "test_init_get"
                | "test_approx_equals"
                | "test_append"
                | "test_append_bad"
                | "test_coords"
                | "test_dem_from_file"
                | "test_dem_to_file"
                | "test_append_dem_to_dem"
                | "test_init_parse"
        )
    } else {
        false
    };
    let deferred_convenience_case = value.ends_with("detector_error_model.test.cc")
        && matches!(
            leaf.as_str(),
            "from_file" | "py_get_slice" | "mul" | "imul" | "add" | "iadd"
        );
    if binding_only_python_case || deferred_convenience_case {
        return UpstreamClassification::deferred_for(
            [FeatureId::DemFormat],
            DeferredProduct::PythonBindings,
            "Python-style DEM copying, indexing, operators, overloaded append, and file helpers are deferred with Python bindings; selected Rust APIs own their semantic contracts independently.",
        );
    }

    if value.ends_with("detector_error_model.test.cc") && leaf == "movement" {
        return UpstreamClassification::not_applicable(
            "C++ moved-from object state has no Rust value-semantic compatibility contract.",
        );
    }

    if value.ends_with("detector_error_model.test.cc")
        && leaf == "general"
        && symbol.to_ascii_lowercase().starts_with("dem_instruction.")
    {
        return UpstreamClassification::not_applicable(
            "This mixed C++ utility case includes DemInstruction::approx_equals, which is not part of the selected Rust API; exact Rust instruction equality, validation, and canonical printing have independent API and semantic owners.",
        );
    }

    UpstreamClassification::selected(FeatureId::DemFormat)
}

fn classify_simulator(value: &str, symbol: &str) -> UpstreamClassification {
    if value.contains("dem_sampler") {
        UpstreamClassification::selected(FeatureId::DemSampling)
    } else if value.contains("measurements_to_detection") || value.contains("frame_simulator_util")
    {
        UpstreamClassification::selected(FeatureId::Detection)
    } else if value.contains("frame_simulator") {
        if is_frame_gate_semantic_case(symbol) {
            UpstreamClassification::selected_many([FeatureId::GateContract, FeatureId::Sampling])
        } else {
            UpstreamClassification::selected(FeatureId::Sampling)
        }
    } else if value.contains("tableau_simulator") {
        if is_tableau_gate_semantic_case(symbol) {
            UpstreamClassification::selected_many([FeatureId::GateContract, FeatureId::Sampling])
        } else {
            deferred_interactive_simulator([FeatureId::GateContract, FeatureId::Sampling])
        }
    } else if value.contains("graph_simulator") || value.contains("vector_simulator") {
        deferred_interactive_simulator([FeatureId::GateContract])
    } else if value.contains("sparse_rev_frame") {
        UpstreamClassification::selected(FeatureId::FlowUtils)
    } else if value.contains("error_analyzer") {
        if is_analyzer_gate_semantic_case(symbol) {
            UpstreamClassification::selected_many([FeatureId::GateContract, FeatureId::Analyzer])
        } else {
            UpstreamClassification::selected(FeatureId::Analyzer)
        }
    } else {
        UpstreamClassification::selected(FeatureId::Analyzer)
    }
}

fn deferred_interactive_simulator(
    feature_ids: impl IntoIterator<Item = FeatureId>,
) -> UpstreamClassification {
    UpstreamClassification::deferred_for(
        feature_ids,
        DeferredProduct::InteractiveSimulators,
        "This case exercises an explicitly deferred public interactive simulator surface instead of a selected Rust sampler or gate contract.",
    )
}

fn simulator_symbol_base(symbol: &str) -> &str {
    ["_64", "_128", "_256"]
        .into_iter()
        .find_map(|suffix| symbol.strip_suffix(suffix))
        .unwrap_or(symbol)
}

fn is_frame_gate_semantic_case(symbol: &str) -> bool {
    let symbol = simulator_symbol_base(symbol);
    matches!(
        symbol,
        "FrameSimulator.bulk_operations_consistent_with_tableau_data"
            | "FrameSimulator.correlated_error"
            | "FrameSimulator.quantum_cannot_control_classical"
            | "FrameSimulator.classical_can_control_quantum"
            | "FrameSimulator.classical_controls"
            | "FrameSimulator.measure_y_without_reset_doesnt_reset"
            | "FrameSimulator.resets_vs_measurements"
            | "FrameSimulator.measure_pauli_product_4body"
            | "FrameSimulator.non_deterministic_pauli_product_detectors"
            | "FrameSimulator.ignores_sweep_controls_when_given_no_sweep_data"
            | "FrameSimulator.mpad"
            | "FrameSimulator.mxxyyzz_basis"
            | "FrameSimulator.mxxyyzz_inversion"
            | "FrameSimulator.runs_on_general_circuit"
            | "FrameSimulator.heralded_erase_detect_statistics"
            | "FrameSimulator.heralded_pauli_channel_1_statistics"
            | "FrameSimulator.heralded_erase_statistics_offset_by_2"
            | "FrameSimulator.heralded_pauli_channel_1_statistics_offset_by_2"
            | "FrameSimulator<W>::do_MPAD"
            | "case GateType::I_ERROR:"
    ) || symbol.starts_with("FrameSimulator.noisy_measurement_")
        || symbol.starts_with("FrameSimulator.noisy_measurement_reset_")
        || symbol.starts_with("FrameSimulator.observable_include_paulis_")
}

fn is_tableau_gate_semantic_case(symbol: &str) -> bool {
    let symbol = simulator_symbol_base(symbol);
    matches!(
        symbol,
        "TableauSimulator.identity"
            | "TableauSimulator.identity2"
            | "TableauSimulator.bit_flip"
            | "TableauSimulator.bit_flip_2"
            | "TableauSimulator.epr"
            | "TableauSimulator.big_determinism"
            | "TableauSimulator.unitary_gates_consistent_with_tableau_data"
            | "TableauSimulator.certain_errors_consistent_with_gates"
            | "TableauSimulator.simulate"
            | "TableauSimulator.simulate_reset"
            | "TableauSimulator.measurement_vs_vector_sim"
            | "TableauSimulator.correlated_error"
            | "TableauSimulator.quantum_cannot_control_classical"
            | "TableauSimulator.classical_can_control_quantum"
            | "TableauSimulator.classical_control_cases"
            | "TableauSimulator.mr_repeated_target"
            | "TableauSimulator.measure_pauli_product_1"
            | "TableauSimulator.measure_pauli_product_4body"
            | "TableauSimulator.measure_pauli_product_bad"
            | "TableauSimulator.measure_pauli_product_epr"
            | "TableauSimulator.measure_pauli_product_inversions"
            | "TableauSimulator.measure_pauli_product_noisy"
            | "TableauSimulator.mpad"
            | "TableauSimulator.mxx_myy_mzz_vs_mpp_unsigned"
            | "TableauSimulator.mxx"
            | "TableauSimulator.myy"
            | "TableauSimulator.mzz"
            | "TableauSimulator.ignores_sweep_controls"
            | "TableauSimulator.reset_pure"
            | "TableauSimulator.reset_random"
            | "TableauSimulator.reset_vs_measurements"
            | "TableauSimulator.reset_x_entangled"
            | "TableauSimulator.reset_y_entangled"
            | "TableauSimulator.reset_z_entangled"
            | "TableauSimulator.measure_x_entangled"
            | "TableauSimulator.measure_y_entangled"
            | "TableauSimulator.measure_z_entangled"
            | "TableauSimulator.measure_reset_x_entangled"
            | "TableauSimulator.measure_reset_y_entangled"
            | "TableauSimulator.measure_reset_z_entangled"
            | "TableauSimulator.runs_on_general_circuit"
            | "TableauSimulator.heralded_erase"
            | "TableauSimulator.heralded_pauli_channel_1"
    ) || symbol.starts_with("TableauSimulator.noisy_measurement_")
        || symbol.starts_with("TableauSimulator.noisy_measure_reset_")
}

fn is_analyzer_gate_semantic_case(symbol: &str) -> bool {
    let symbol = simulator_symbol_base(symbol);
    matches!(
        symbol,
        "ErrorAnalyzer.unitary_gates_match_frame_simulator"
            | "ErrorAnalyzer.reversed_operation_order"
            | "ErrorAnalyzer.classical_error_propagation"
            | "ErrorAnalyzer.measure_reset_basis"
            | "ErrorAnalyzer.repeated_measure_reset"
            | "ErrorAnalyzer.period_3_gates"
            | "ErrorAnalyzer.composite_error_analysis"
            | "ErrorAnalyzer.exact_solved_pauli_channel_1_is_let_through"
            | "ErrorAnalyzer.pauli_channel_threshold"
            | "ErrorAnalyzer.pauli_channel_composite_errors"
            | "ErrorAnalyzer.measure_pauli_product_4body"
            | "ErrorAnalyzer.ignores_sweep_controls"
            | "ErrorAnalyzer.mpp_ordering"
            | "ErrorAnalyzer.else_correlated_error_block"
            | "ErrorAnalyzer.mpad"
            | "ErrorAnalyzer.mxx"
            | "ErrorAnalyzer.myy"
            | "ErrorAnalyzer.mzz"
            | "ErrorAnalyzer.heralded_erase_conditional_division"
            | "ErrorAnalyzer.heralded_erase"
            | "ErrorAnalyzer.runs_on_general_circuit"
            | "ErrorAnalyzer.heralded_pauli_channel_1"
            | "ErrorAnalyzer.OBS_INCLUDE_PAULIS"
    ) || symbol.starts_with("ErrorAnalyzer.noisy_measurement_m")
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

fn classify_util_top(value: &str, symbol: &str) -> UpstreamClassification {
    if (value.contains("mbqc_decomposition") && symbol == "mbqc_decomposition.all_gates")
        || (value.contains("simplified_circuit")
            && symbol == "gate_decomposition.simplifications_are_correct")
    {
        UpstreamClassification::not_applicable(
            "This aggregate C++ all-gates test exceeds the selected scoped Rust transform contract; independently selectable Rust qualification cases own every implemented MBQC or simplification behavior without claiming the unselected aggregate.",
        )
    } else if value.contains("circuit_to_dem") {
        UpstreamClassification::selected(FeatureId::Analyzer)
    } else if value.contains("flow")
        || value.contains("circuit_inverse_qec")
        || value.contains("circuit_inverse_unitary")
        || value.contains("detecting_regions")
        || value.contains("missing_detectors")
        || value.contains("transform_without_feedback")
    {
        UpstreamClassification::selected(FeatureId::FlowUtils)
    } else if value.contains("reference_sample") || value.contains("count_determined") {
        UpstreamClassification::selected(FeatureId::Sampling)
    } else if value.contains("stabilizers_to_tableau") {
        UpstreamClassification::selected(FeatureId::Algebra)
    } else if value.contains("circuit_vs_amplitudes") {
        stabilizer::deferred_interactive(
            "State-vector-to-Circuit and Circuit-to-state-vector conversions are explicitly deferred interactive simulator products.",
        )
    } else if value.contains("circuit_vs_tableau") {
        let leaf = strip_stim_word_size_suffix(symbol)
            .rsplit('.')
            .next()
            .unwrap_or(symbol);
        if matches!(
            leaf,
            "circuit_to_tableau" | "circuit_to_tableau_ignoring_gates"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            stabilizer::deferred_interactive(
                "Tableau-to-Circuit synthesis and state-vector-backed MPP circuit checks are explicitly deferred interactive simulator products.",
            )
        }
    } else if value.contains("stabilizers_vs_amplitudes") {
        let leaf = strip_stim_word_size_suffix(symbol)
            .rsplit('.')
            .next()
            .unwrap_or(symbol);
        if matches!(
            leaf,
            "unitary_to_tableau_fail"
                | "unitary_to_tableau_vs_gate_data"
                | "unitary_vs_tableau_basic"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            stabilizer::deferred_interactive(
                "Tableau-to-unitary and state-vector round trips are explicitly deferred interactive simulator products.",
            )
        }
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
#[path = "classification/tests.rs"]
mod tests;
