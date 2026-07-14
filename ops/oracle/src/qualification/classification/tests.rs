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

    let instruction_path = Path::new("src/stim/circuit/circuit_instruction_pybind_test.py");
    let string_constructor = classify_upstream_case(instruction_path, "test_init_from_str");
    assert_eq!(
        string_constructor.disposition,
        UpstreamDisposition::DeferredProduct
    );
    assert_eq!(
        string_constructor.deferred_product,
        Some(DeferredProduct::PythonBindings)
    );
    assert_eq!(string_constructor.feature_ids, vec![FeatureId::StimFormat]);

    let instruction_printer = classify_upstream_case(instruction_path, "test_str");
    assert_eq!(
        instruction_printer.disposition,
        UpstreamDisposition::SemanticMining
    );
    assert_eq!(instruction_printer.feature_ids, vec![FeatureId::StimFormat]);

    let instruction_value = classify_upstream_case(instruction_path, "test_init_and_equality");
    assert_eq!(instruction_value.feature_ids, vec![FeatureId::CircuitApi]);
    let instruction_count = classify_upstream_case(instruction_path, "test_num_measurements");
    assert_eq!(instruction_count.feature_ids, vec![FeatureId::CircuitApi]);

    let gate_target_path = Path::new("src/stim/circuit/gate_target_pybind_test.py");
    let gate_target_value = classify_upstream_case(gate_target_path, "test_init_and_equality");
    assert_eq!(gate_target_value.feature_ids, vec![FeatureId::GateContract]);
}

#[test]
fn classifications_split_dem_binding_search_and_value_cases_by_exact_symbol() {
    let model_python = Path::new("src/stim/dem/detector_error_model_pybind_test.py");
    let search = classify_upstream_case(model_python, "test_shortest_graphlike_error_line");
    assert_eq!(search.feature_ids, vec![FeatureId::Search]);
    assert_eq!(search.disposition, UpstreamDisposition::SemanticMining);

    let binding_append = classify_upstream_case(model_python, "test_append_bad");
    assert_eq!(
        binding_append.disposition,
        UpstreamDisposition::DeferredProduct
    );
    assert_eq!(
        binding_append.deferred_product,
        Some(DeferredProduct::PythonBindings)
    );
    assert_eq!(binding_append.feature_ids, vec![FeatureId::DemFormat]);

    let shared_transform = classify_upstream_case(model_python, "test_rounded");
    assert_eq!(
        shared_transform.disposition,
        UpstreamDisposition::SemanticMining
    );
    assert_eq!(shared_transform.feature_ids, vec![FeatureId::DemFormat]);

    let binding_coordinate_shapes = classify_upstream_case(model_python, "test_coords");
    assert_eq!(
        binding_coordinate_shapes.disposition,
        UpstreamDisposition::DeferredProduct
    );
    assert_eq!(
        binding_coordinate_shapes.deferred_product,
        Some(DeferredProduct::PythonBindings)
    );

    let instruction_python = Path::new("src/stim/dem/dem_instruction_pybind_test.py");
    let copied_args = classify_upstream_case(instruction_python, "test_args_copy");
    assert_eq!(
        copied_args.disposition,
        UpstreamDisposition::DeferredProduct
    );
    let instruction_validation = classify_upstream_case(instruction_python, "test_validation");
    assert_eq!(
        instruction_validation.disposition,
        UpstreamDisposition::SemanticMining
    );

    let model_cpp = Path::new("src/stim/dem/detector_error_model.test.cc");
    let operator = classify_upstream_case(model_cpp, "detector_error_model.mul");
    assert_eq!(operator.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(operator.feature_ids, vec![FeatureId::DemFormat]);

    let moved_from = classify_upstream_case(model_cpp, "detector_error_model.movement");
    assert_eq!(moved_from.disposition, UpstreamDisposition::NotApplicable);
    assert!(moved_from.feature_ids.is_empty());

    let mixed_instruction = classify_upstream_case(model_cpp, "dem_instruction.general");
    assert_eq!(
        mixed_instruction.disposition,
        UpstreamDisposition::NotApplicable
    );
    assert!(mixed_instruction.feature_ids.is_empty());

    let parser = classify_upstream_case(model_cpp, "detector_error_model.parse");
    assert_eq!(parser.disposition, UpstreamDisposition::SemanticMining);
    assert_eq!(parser.feature_ids, vec![FeatureId::DemFormat]);
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

    let count = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.count_qubits",
    );
    assert_eq!(count.feature_ids, vec![FeatureId::CircuitApi]);

    let windows = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.parse_windows_newlines",
    );
    assert_eq!(
        windows.feature_ids,
        vec![FeatureId::StimFormat, FeatureId::CircuitApi]
    );

    let approximate = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.approx_equals",
    );
    assert_eq!(approximate.feature_ids, vec![FeatureId::CircuitApi]);

    let frame = classify_upstream_case(
        Path::new("src/stim/simulators/frame_simulator.test.cc"),
        "FrameSimulator.consistency_64",
    );
    assert_eq!(frame.feature_ids, vec![FeatureId::Sampling]);

    let frame_gate = classify_upstream_case(
        Path::new("src/stim/simulators/frame_simulator.test.cc"),
        "FrameSimulator.noisy_measurement_x_64",
    );
    assert_eq!(
        frame_gate.feature_ids,
        vec![FeatureId::GateContract, FeatureId::Sampling]
    );

    let detection = classify_upstream_case(
        Path::new("src/stim/simulators/frame_simulator_util.test.cc"),
        "DetectionSimulator.stream_results_64",
    );
    assert_eq!(detection.feature_ids, vec![FeatureId::Detection]);

    let analyzer = classify_upstream_case(
        Path::new("src/stim/simulators/error_analyzer.test.cc"),
        "ErrorAnalyzer.mpp_ordering",
    );
    assert_eq!(
        analyzer.feature_ids,
        vec![FeatureId::GateContract, FeatureId::Analyzer]
    );

    let analyzer_only = classify_upstream_case(
        Path::new("src/stim/simulators/error_analyzer.test.cc"),
        "ErrorAnalyzer.brute_force_decomp_simple",
    );
    assert_eq!(analyzer_only.feature_ids, vec![FeatureId::Analyzer]);

    let tableau_internal = classify_upstream_case(
        Path::new("src/stim/simulators/tableau_simulator.test.cc"),
        "TableauSimulator.amortized_resizing_64",
    );
    assert_eq!(
        tableau_internal.disposition,
        UpstreamDisposition::DeferredProduct
    );
    assert_eq!(
        tableau_internal.deferred_product,
        Some(DeferredProduct::InteractiveSimulators)
    );

    let tableau_gate = classify_upstream_case(
        Path::new("src/stim/simulators/tableau_simulator.test.cc"),
        "TableauSimulator.unitary_gates_consistent_with_tableau_data_64",
    );
    assert_eq!(
        tableau_gate.feature_ids,
        vec![FeatureId::GateContract, FeatureId::Sampling]
    );

    let vector_internal = classify_upstream_case(
        Path::new("src/stim/simulators/vector_simulator.test.cc"),
        "vector_sim.approximate_equals",
    );
    assert_eq!(
        vector_internal.disposition,
        UpstreamDisposition::DeferredProduct
    );

    let vector_gate = classify_upstream_case(
        Path::new("src/stim/simulators/vector_simulator.test.cc"),
        "vector_sim.do_unitary_circuit",
    );
    assert_eq!(
        vector_gate.disposition,
        UpstreamDisposition::DeferredProduct
    );

    let graph_internal = classify_upstream_case(
        Path::new("src/stim/simulators/graph_simulator.test.cc"),
        "graph_simulator.do_complementation",
    );
    assert_eq!(
        graph_internal.disposition,
        UpstreamDisposition::DeferredProduct
    );

    let graph_gate = classify_upstream_case(
        Path::new("src/stim/simulators/graph_simulator.test.cc"),
        "graph_simulator.all_unitary_gates_work",
    );
    assert_eq!(graph_gate.disposition, UpstreamDisposition::DeferredProduct);

    let cpp_gate_internal = classify_upstream_case(
        Path::new("src/stim/gates/gates.test.cc"),
        "gate_data.hash_matches_storage_location",
    );
    assert_eq!(
        cpp_gate_internal.disposition,
        UpstreamDisposition::NotApplicable
    );

    let python_gate_deferred = classify_upstream_case(
        Path::new("src/stim/gates/gates_test.py"),
        "test_gate_hadamard_conjugated",
    );
    assert_eq!(
        python_gate_deferred.disposition,
        UpstreamDisposition::DeferredProduct
    );

    let spp_decomposition = classify_upstream_case(
        Path::new("src/stim/circuit/gate_decomposition.test.cc"),
        "gate_decomposition.decompose_spp_or_spp_dag_operation_complex",
    );
    assert_eq!(spp_decomposition.feature_ids, vec![FeatureId::GateContract]);

    let internal_decomposition = classify_upstream_case(
        Path::new("src/stim/circuit/gate_decomposition.test.cc"),
        "gate_decomposition.for_each_combined_targets_group",
    );
    assert_eq!(
        internal_decomposition.disposition,
        UpstreamDisposition::NotApplicable
    );

    let include = classify_upstream_case(Path::new("src/stim.test.cc"), "stim.include1");
    assert_eq!(include.disposition, UpstreamDisposition::NotApplicable);
    assert!(include.feature_ids.is_empty());

    let compiled = classify_upstream_case(
        Path::new("src/stim/py/compiled_measurement_sampler_pybind_test.py"),
        "test_measurements_vs_resets",
    );
    assert_eq!(compiled.feature_ids, vec![FeatureId::Sampling]);
}

#[test]
fn classifications_split_portable_bit_contracts_from_cpp_storage_helpers() {
    let bits = Path::new("src/stim/mem/simd_bits.test.cc");
    for symbol in [
        "simd_bits.assignment_64",
        "simd_bits.xor_assignment_128",
        "simd_bits.mask_assignment_and_256",
        "simd_bits.popcnt_64",
    ] {
        let classified = classify_upstream_case(bits, symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::SemanticMining);
        assert_eq!(classified.feature_ids, vec![FeatureId::BitKernels]);
    }
    for symbol in [
        "simd_bits.randomize_64",
        "simd_bits.destructive_resize_128",
        "simd_bits.fuzz_left_shift_assignment_256",
        "simd_bits.move_64",
        "simd_bits.prefix_ref_256",
        "simd_bits.truncated_overwrite_from_64",
    ] {
        let classified = classify_upstream_case(bits, symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::NotApplicable);
        assert!(classified.feature_ids.is_empty());
    }

    let range = Path::new("src/stim/mem/simd_bits_range_ref.test.cc");
    assert_eq!(
        classify_upstream_case(range, "simd_bits_range_ref.xor_assignment_256").feature_ids,
        vec![FeatureId::BitKernels]
    );
    assert_eq!(
        classify_upstream_case(range, "simd_bits_range_ref.intersects_256").disposition,
        UpstreamDisposition::NotApplicable
    );

    let table = Path::new("src/stim/mem/simd_bit_table.test.cc");
    assert_eq!(
        classify_upstream_case(table, "simd_bit_table.transposed_128").feature_ids,
        vec![FeatureId::BitKernels]
    );
    assert_eq!(
        classify_upstream_case(table, "simd_bit_table.from_quadrants_128").disposition,
        UpstreamDisposition::NotApplicable
    );

    let word = Path::new("src/stim/mem/simd_word.test.cc");
    assert_eq!(
        classify_upstream_case(word, "simd_word_pick.popcount_64").feature_ids,
        vec![FeatureId::BitKernels]
    );
    assert_eq!(
        classify_upstream_case(word, "simd_word.shifting_64").disposition,
        UpstreamDisposition::NotApplicable
    );

    let sparse = Path::new("src/stim/mem/sparse_xor_vec.test.cc");
    assert_eq!(
        classify_upstream_case(sparse, "sparse_xor_vec.inplace_xor_sort").feature_ids,
        vec![FeatureId::BitKernels]
    );
    assert_eq!(
        classify_upstream_case(sparse, "sparse_xor_table.inplace_xor").disposition,
        UpstreamDisposition::NotApplicable
    );
}
