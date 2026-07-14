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
    assert_eq!(search.feature_ids, vec![FeatureId::Search]);

    let diagram = classify_upstream_case(path, "test_tag_diagram");
    assert_eq!(diagram.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(diagram.deferred_product, Some(DeferredProduct::Diagrams));
    assert_eq!(diagram.feature_ids, vec![FeatureId::CircuitApi]);

    let repr = classify_upstream_case(path, "test_circuit_repr");
    assert_eq!(repr.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(repr.deferred_product, Some(DeferredProduct::PythonBindings));

    let generation_errors = classify_upstream_case(path, "test_circuit_generation_errors");
    assert_eq!(
        generation_errors.disposition,
        UpstreamDisposition::DeferredProduct
    );
    assert_eq!(generation_errors.feature_ids, vec![FeatureId::Generation]);
    assert_eq!(
        generation_errors.deferred_product,
        Some(DeferredProduct::PythonBindings)
    );

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
fn classifications_split_generation_cli_dispatch_from_core_semantics() {
    let path = Path::new("src/stim/cmd/command_gen.test.cc");
    let execute = classify_upstream_case(path, "command_gen.execute");
    assert_eq!(execute.feature_ids, vec![FeatureId::Cli]);

    let no_noise = classify_upstream_case(path, "command_gen.no_noise_no_detections_256");
    assert_eq!(no_noise.feature_ids, vec![FeatureId::Generation]);
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
    assert_eq!(circuit.feature_ids, vec![FeatureId::StimFormat]);

    let count = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.count_qubits",
    );
    assert_eq!(count.feature_ids, vec![FeatureId::CircuitApi]);

    let windows = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.parse_windows_newlines",
    );
    assert_eq!(windows.feature_ids, vec![FeatureId::StimFormat]);

    let approximate = classify_upstream_case(
        Path::new("src/stim/circuit/circuit.test.cc"),
        "circuit.approx_equals",
    );
    assert_eq!(approximate.disposition, UpstreamDisposition::NotApplicable);
    assert!(approximate.feature_ids.is_empty());

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

#[test]
fn classifications_split_circuit_rust_semantics_from_bindings_and_other_domains() {
    let pybind = Path::new("src/stim/circuit/circuit_pybind_test.py");
    for (symbol, feature) in [
        ("test_circuit_compile_sampler", FeatureId::Sampling),
        (
            "test_circuit_compile_detector_sampler",
            FeatureId::Detection,
        ),
        ("test_circuit_generation", FeatureId::Generation),
        ("test_shortest_graphlike_error", FeatureId::Search),
        ("test_has_flow_ry", FeatureId::FlowUtils),
        ("test_to_tableau", FeatureId::Algebra),
    ] {
        let classified = classify_upstream_case(pybind, symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::SemanticMining);
        assert_eq!(classified.feature_ids, vec![feature], "symbol={symbol}");
    }

    for symbol in [
        "test_circuit_iadd",
        "test_circuit_repr",
        "test_copy",
        "test_hash",
        "test_slicing",
        "test_approx_equals",
        "test_pickle",
        "test_append_pauli_string",
        "test_reference_detector_and_observable_signs",
        "test_circuit_append_operation",
        "test_insert",
        "test_pop",
        "test_circuit_from_file",
        "test_circuit_to_file",
        "test_tag_from_file",
        "test_append_instructions_and_blocks",
        "test_reappend_gate_targets",
        "test_append_tag",
    ] {
        let classified = classify_upstream_case(pybind, symbol);
        assert_eq!(
            classified.disposition,
            UpstreamDisposition::DeferredProduct,
            "symbol={symbol}"
        );
        assert_eq!(
            classified.deferred_product,
            Some(DeferredProduct::PythonBindings),
            "symbol={symbol}"
        );
    }

    let explain = classify_upstream_case(pybind, "test_explain_errors");
    assert_eq!(explain.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(
        explain.deferred_product,
        Some(DeferredProduct::ExplainErrors)
    );

    for symbol in [
        "test_circuit_init_num_measurements_num_qubits",
        "test_num_ticks",
        "test_coords",
        "test_flattened",
        "test_without_noise",
        "test_decomposed",
        "test_circuit_tags",
        "test_without_tags",
        "test_append_circuit_to_circuit",
    ] {
        let classified = classify_upstream_case(pybind, symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::SemanticMining);
        assert_eq!(
            classified.feature_ids,
            vec![FeatureId::CircuitApi],
            "symbol={symbol}"
        );
    }

    let diagram = classify_upstream_case(pybind, "test_diagram");
    assert_eq!(diagram.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(diagram.deferred_product, Some(DeferredProduct::Diagrams));
    let detslice = classify_upstream_case(pybind, "test_detslice_filter_coords_flexibility");
    assert_eq!(detslice.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(detslice.deferred_product, Some(DeferredProduct::Diagrams));
    assert_eq!(
        classify_upstream_case(pybind, "test_tags_append_from_stim_program_text").feature_ids,
        vec![FeatureId::StimFormat]
    );
    assert_eq!(
        classify_upstream_case(pybind, "test_circuit_create_with_odd_cx").feature_ids,
        vec![FeatureId::StimFormat]
    );
}

#[test]
fn classifications_split_circuit_cpp_and_value_object_contracts_exactly() {
    let circuit = Path::new("src/stim/circuit/circuit.test.cc");
    for symbol in [
        "circuit.append_circuit",
        "circuit.for_each_operation",
        "circuit.count_qubits",
        "circuit.multiplication_repeats",
        "circuit.get_final_qubit_coords",
        "circuit.coords_of_detector",
        "circuit.flattened",
        "circuit.equality",
        "circuit.insert_instruction",
        "circuit.without_tags",
    ] {
        let classified = classify_upstream_case(circuit, symbol);
        assert!(
            classified.feature_ids.contains(&FeatureId::CircuitApi),
            "symbol={symbol}"
        );
    }

    for symbol in [
        "circuit.max_lookback",
        "circuit.addition_shares_blocks",
        "circuit.aliased_noiseless_circuit",
        "circuit.approx_equals",
        "circuit.concat_self_fuse",
        "circuit.count_detectors_num_observables",
        "circuit.count_measurements",
        "circuit.generate_test_circuit_with_all_operations",
        "circuit.self_addition",
    ] {
        assert_eq!(
            classify_upstream_case(circuit, symbol).disposition,
            UpstreamDisposition::NotApplicable,
            "symbol={symbol}"
        );
    }
    let inverse = classify_upstream_case(circuit, "circuit.inverse");
    assert_eq!(inverse.disposition, UpstreamDisposition::SemanticMining);
    assert_eq!(inverse.feature_ids, vec![FeatureId::FlowUtils]);
    let slice = classify_upstream_case(circuit, "circuit.py_get_slice");
    assert_eq!(slice.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(
        slice.deferred_product,
        Some(DeferredProduct::PythonBindings)
    );

    let instruction = Path::new("src/stim/circuit/circuit_instruction_pybind_test.py");
    let target_groups = classify_upstream_case(instruction, "test_target_groups");
    assert_eq!(
        target_groups.feature_ids,
        vec![FeatureId::StimFormat, FeatureId::CircuitApi]
    );
    assert_eq!(
        classify_upstream_case(instruction, "test_repr").disposition,
        UpstreamDisposition::DeferredProduct
    );

    let repeat = Path::new("src/stim/circuit/circuit_repeat_block_test.py");
    assert_eq!(
        classify_upstream_case(repeat, "test_init_and_equality").feature_ids,
        vec![FeatureId::CircuitApi]
    );
    assert_eq!(
        classify_upstream_case(repeat, "test_name").disposition,
        UpstreamDisposition::DeferredProduct
    );
}

#[test]
fn classifications_route_inverse_circuit_utilities_with_flow_transforms() {
    for path in [
        "src/stim/util_top/circuit_inverse_qec.test.cc",
        "src/stim/util_top/circuit_inverse_qec_test.py",
        "src/stim/util_top/circuit_inverse_unitary.test.cc",
    ] {
        let classified = classify_upstream_case(Path::new(path), "representative_case");
        assert_eq!(classified.disposition, UpstreamDisposition::SemanticMining);
        assert_eq!(
            classified.feature_ids,
            vec![FeatureId::FlowUtils],
            "path={path}"
        );
    }
}

#[test]
fn classifications_do_not_promote_broad_all_gate_transform_aggregates() {
    for (path, symbol) in [
        (
            "src/stim/util_top/mbqc_decomposition.test.cc",
            "mbqc_decomposition.all_gates",
        ),
        (
            "src/stim/util_top/simplified_circuit.test.cc",
            "gate_decomposition.simplifications_are_correct",
        ),
    ] {
        let classified = classify_upstream_case(Path::new(path), symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::NotApplicable);
        assert!(classified.feature_ids.is_empty());
    }
}

#[test]
fn classifications_keep_flow_values_in_algebra_and_flow_engines_separate() {
    assert_eq!(
        classify_public_api_source(
            "stab_core",
            Path::new("crates/stab-core/src/stabilizers/flow.rs"),
            "stab_core::Flow::multiply",
        ),
        Some(FeatureId::Algebra)
    );

    let path = Path::new("src/stim/stabilizers/flow_pybind_test.py");
    assert_eq!(
        classify_upstream_case(path, "test_flow_multiplication").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(path, "test_obs_flows").feature_ids,
        vec![FeatureId::FlowUtils]
    );
    assert_eq!(
        classify_upstream_case(path, "test_obs_include_pauli_terms_sensitivity").feature_ids,
        vec![FeatureId::Detection]
    );
    let repr = classify_upstream_case(path, "test_repr");
    assert_eq!(repr.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(repr.feature_ids, vec![FeatureId::Algebra]);
}

#[test]
fn classifications_split_pauli_and_clifford_semantics_from_language_helpers() {
    let pauli = Path::new("src/stim/stabilizers/pauli_string.test.cc");
    for symbol in [
        "pauli_string.multiplication_64",
        "pauli_string.after_tableau_128",
        "PauliString.pauli_xyz_to_xz",
    ] {
        let classified = classify_upstream_case(pauli, symbol);
        assert_eq!(classified.disposition, UpstreamDisposition::SemanticMining);
        assert_eq!(classified.feature_ids, vec![FeatureId::Algebra]);
    }
    for symbol in [
        "pauli_string.gather_64",
        "pauli_string.before_circuit_128",
        "pauli_string.ensure_num_qubits_256",
    ] {
        assert_eq!(
            classify_upstream_case(pauli, symbol).disposition,
            UpstreamDisposition::NotApplicable,
            "symbol={symbol}"
        );
    }
    assert_eq!(
        classify_upstream_case(pauli, "pauli_string.py_get_slice_64").deferred_product,
        Some(DeferredProduct::PythonBindings)
    );

    let clifford = Path::new("src/stim/stabilizers/clifford_string.test.cc");
    assert_eq!(
        classify_upstream_case(clifford, "clifford_string.known_identities_64").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(clifford, "clifford_string.to_from_circuit_64").disposition,
        UpstreamDisposition::NotApplicable
    );

    let iterator = Path::new("src/stim/stabilizers/pauli_string_iter.test.cc");
    assert_eq!(
        classify_upstream_case(iterator, "pauli_string_iter.small_cases_64").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(iterator, "pauli_string_iter.NestedLooper_simple").disposition,
        UpstreamDisposition::NotApplicable
    );
}

#[test]
fn classifications_split_tableau_semantics_from_unselected_products() {
    let tableau = Path::new("src/stim/stabilizers/tableau.test.cc");
    assert_eq!(
        classify_upstream_case(tableau, "tableau.inverse_256").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(tableau, "tableau.expand_64").disposition,
        UpstreamDisposition::NotApplicable
    );
    let unitary = classify_upstream_case(tableau, "tableau.unitary_big_endian_128");
    assert_eq!(unitary.disposition, UpstreamDisposition::DeferredProduct);
    assert_eq!(
        unitary.deferred_product,
        Some(DeferredProduct::InteractiveSimulators)
    );

    let python = Path::new("src/stim/stabilizers/tableau_pybind_test.py");
    assert_eq!(
        classify_upstream_case(python, "test_from_unitary_matrix").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(python, "test_append").deferred_product,
        Some(DeferredProduct::PythonBindings)
    );
    assert_eq!(
        classify_upstream_case(python, "test_from_state_vector_fuzz").deferred_product,
        Some(DeferredProduct::InteractiveSimulators)
    );
}

#[test]
fn classifications_bound_util_top_algebra_to_selected_conversion_direction() {
    let circuit = Path::new("src/stim/util_top/circuit_vs_tableau.test.cc");
    assert_eq!(
        classify_upstream_case(circuit, "conversions.circuit_to_tableau_64").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(circuit, "conversions.tableau_to_circuit_64").deferred_product,
        Some(DeferredProduct::InteractiveSimulators)
    );

    let amplitudes = Path::new("src/stim/util_top/stabilizers_vs_amplitudes.test.cc");
    assert_eq!(
        classify_upstream_case(amplitudes, "conversions.unitary_to_tableau_fail_64").feature_ids,
        vec![FeatureId::Algebra]
    );
    assert_eq!(
        classify_upstream_case(amplitudes, "conversions.tableau_to_unitary_vs_gate_data_64")
            .deferred_product,
        Some(DeferredProduct::InteractiveSimulators)
    );
}
