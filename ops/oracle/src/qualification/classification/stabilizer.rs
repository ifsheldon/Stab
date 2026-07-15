use super::{DeferredProduct, FeatureId, UpstreamClassification};

pub(super) fn classify(value: &str, symbol: &str) -> UpstreamClassification {
    if symbol.is_empty() {
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    let base = super::strip_stim_word_size_suffix(symbol);
    let leaf = base.rsplit('.').next().unwrap_or(base);

    if value.ends_with("clifford_string.test.cc") {
        return if leaf == "to_from_circuit" {
            not_applicable(
                "Stab has no selected CliffordString-to-Circuit or Circuit-to-CliffordString API; focused Rust owners prove the shared Clifford group semantics.",
            )
        } else {
            UpstreamClassification::selected(FeatureId::Algebra)
        };
    }
    if value.ends_with("clifford_string_pybind_test.py") {
        return if matches!(leaf, "test_random" | "test_all_cliffords_string") {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            deferred_python(
                "This complete case exercises Python CliffordString construction, indexing, mutation, coercion, slicing, or output-list shape; focused Rust owners prove the portable Clifford semantics.",
            )
        };
    }
    if value.ends_with("flex_pauli_string.test.cc") || value.ends_with("flow.test.cc") {
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    if value.ends_with("flow_pybind_test.py") {
        return match leaf {
            "test_flow_canonicalization" | "test_flow_multiplication" => {
                UpstreamClassification::selected(FeatureId::Algebra)
            }
            "test_obs_flows" => UpstreamClassification::selected(FeatureId::FlowUtils),
            "test_obs_include_pauli_terms_sensitivity" => {
                UpstreamClassification::selected(FeatureId::Detection)
            }
            _ => deferred_python(
                "This complete case mixes portable Flow semantics with Python constructors, repr, comparison, or foreign-object behavior; focused Rust owners prove the portable value contract.",
            ),
        };
    }
    if value.ends_with("pauli_string.test.cc") {
        if matches!(leaf, "py_get_item" | "py_get_slice") {
            return deferred_python(
                "Python PauliString indexing and slicing are deferred with the Python binding product.",
            );
        }
        if leaf == "after_circuit"
            || leaf == "before_circuit"
            || leaf.starts_with("before_after_circuit_")
        {
            return not_applicable(
                "Stab has no selected PauliString circuit-propagation API; Tableau and circuit-conversion owners prove the shared implemented semantics independently.",
            );
        }
        if matches!(leaf, "after_tableau" | "before_tableau" | "left_mul_pauli") {
            return not_applicable(
                "Stab has no selected target-scatter or target-growing Pauli mutation API; full-width Tableau action and owned Pauli multiplication are qualified independently.",
            );
        }
        if matches!(
            leaf,
            "ensure_num_qubits"
                | "ensure_num_qubits_padded"
                | "foreign_memory"
                | "gather"
                | "move_copy_assignment"
                | "scatter"
                | "swap_with_overwrite_with"
        ) {
            return not_applicable(
                "This case exercises a C++ PauliString storage, ownership, resizing, gather, scatter, or overwrite helper with no selected Rust API contract.",
            );
        }
        return if matches!(
            leaf,
            "pauli_xyz_to_xz"
                | "pauli_xz_to_xyz"
                | "commutes"
                | "equality"
                | "identity"
                | "left_mul_pauli_mul_table"
                | "log_i_scalar_byproduct"
                | "multiplication"
                | "right_mul_pauli_mul_table"
                | "sparse_str"
                | "str"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            not_applicable(
                "The pinned C++ PauliString case has no selected Stab public or semantic contract.",
            )
        };
    }
    if value.ends_with("pauli_string_ref.test.cc") {
        return if matches!(
            leaf,
            "for_each_active_pauli" | "has_no_pauli_terms" | "intersects" | "weight"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            not_applicable(
                "The borrowed mutable PauliStringRef instruction-simulation helper has no selected Rust public contract.",
            )
        };
    }
    if value.ends_with("pauli_string_iter.test.cc") {
        return if leaf == "small_cases" {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else {
            not_applicable(
                "NestedLooper is a Stim-internal iterator helper; focused Rust iterator owners prove public ordering, filtering, restart, and cardinality semantics.",
            )
        };
    }
    if value.ends_with("pauli_string_pybind_test.py") {
        return if matches!(
            leaf,
            "test_identity"
                | "test_from_str"
                | "test_random"
                | "test_str"
                | "test_to_tableau"
                | "test_commutes"
                | "test_commutes_different_lengths"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else if matches!(
            leaf,
            "test_to_unitary_matrix"
                | "test_from_unitary_matrix"
                | "test_from_unitary_matrix_detect_bad_matrix"
                | "test_fuzz_to_from_unitary_matrix"
                | "test_before_after"
                | "test_before_reset"
        ) {
            deferred_interactive(
                "Arbitrary Pauli unitary conversion and circuit-propagation products are outside the selected Rust Algebra surface.",
            )
        } else {
            deferred_python(
                "This complete case exercises Python PauliString operators, aliasing, constructors, indexing, iteration, coercion, or collection shape; focused Rust owners prove shared portable semantics.",
            )
        };
    }
    if value.ends_with("tableau.test.cc") {
        if matches!(
            leaf,
            "unitary_big_endian" | "unitary_little_endian" | "unitary_vs_gate_data"
        ) {
            return deferred_interactive(
                "Tableau-to-unitary materialization is part of the deferred interactive simulator and state-vector product surface.",
            );
        }
        if matches!(
            leaf,
            "apply_within"
                | "direct_sum"
                | "expand"
                | "expand_pad"
                | "expand_pad_equals"
                | "inplace_scatter_append"
                | "inplace_scatter_prepend"
                | "is_conjugation_by_pauli"
                | "prepend_pauli_product"
                | "raised_to"
                | "specialized_operation"
                | "transposed_access"
                | "transposed_xz_input"
        ) {
            return not_applicable(
                "This case exercises an unexposed C++ Tableau mutation, expansion, scatter, transpose, power, or specialized helper with no selected Rust API contract.",
            );
        }
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    if value.ends_with("tableau_iter.test.cc") {
        return UpstreamClassification::selected(FeatureId::Algebra);
    }
    if value.ends_with("tableau_pybind_test.py") {
        if matches!(
            leaf,
            "test_from_state_vector_fuzz"
                | "test_unitary"
                | "test_to_circuit_vs_from_circuit"
                | "test_to_circuit_graph_state_preserves_stabilizers"
                | "test_to_circuit_mpp_preserves_stabilizers"
                | "test_to_circuit_mpp_unsigned_preserves_stabilizers"
        ) {
            return deferred_interactive(
                "State-vector conversion, Tableau-to-unitary output, and Tableau-to-Circuit synthesis are explicitly deferred product surfaces.",
            );
        }
        return if matches!(
            leaf,
            "test_composition"
                | "test_from_named_gate"
                | "test_from_stabilizers_error_messages"
                | "test_from_unitary_matrix"
                | "test_identity"
                | "test_init_equality"
                | "test_inverse"
                | "test_inverse_xyz_output"
                | "test_inverse_xyz_output_pauli"
                | "test_iter_0q"
                | "test_iter_1q"
                | "test_iter_2q"
                | "test_iter_3q"
                | "test_pauli_output"
                | "test_random"
                | "test_signs"
                | "test_str"
                | "test_to_pauli_string"
                | "test_xyz_output_pauli"
        ) {
            UpstreamClassification::selected(FeatureId::Algebra)
        } else if matches!(
            leaf,
            "test_from_conjugated_generators" | "test_to_stabilizers"
        ) {
            deferred_python(
                "Arbitrary conjugated-generator construction and Tableau-to-stabilizer collection output are not exposed by the selected Rust Tableau API.",
            )
        } else {
            deferred_python(
                "This complete case exercises Python Tableau operators, aliasing, constructors, calls, copying, coercion, or collection shape; focused Rust owners prove the portable Tableau semantics.",
            )
        };
    }

    UpstreamClassification::selected(FeatureId::Algebra)
}

fn deferred_python(reason: &'static str) -> UpstreamClassification {
    UpstreamClassification::deferred_for(
        [FeatureId::Algebra],
        DeferredProduct::PythonBindings,
        reason,
    )
}

pub(super) fn deferred_interactive(reason: &'static str) -> UpstreamClassification {
    UpstreamClassification::deferred_for(
        [FeatureId::Algebra],
        DeferredProduct::InteractiveSimulators,
        reason,
    )
}

fn not_applicable(reason: &'static str) -> UpstreamClassification {
    UpstreamClassification::not_applicable(reason)
}
