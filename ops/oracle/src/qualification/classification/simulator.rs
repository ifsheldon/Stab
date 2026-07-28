use super::{DeferredProduct, FeatureId, UpstreamClassification};

pub(super) fn classify(value: &str, symbol: &str) -> UpstreamClassification {
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
