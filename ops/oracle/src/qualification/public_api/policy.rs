pub(super) fn is_evidence_only_stab_core_export(name: &str) -> bool {
    matches!(
        name,
        "ErrorAnalyzerDiagnostics"
            | "GateContractStatisticalBucket"
            | "GateContractStatisticalPlan"
            | "__circuit_to_detector_error_model_with_diagnostics"
            | "__gate_contract_family_names"
            | "__gate_contract_statistical_plans"
            | "__gate_contract_statistical_rejection_boundaries"
            | "__gate_contract_surface_names"
    )
}
