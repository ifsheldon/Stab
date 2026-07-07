# PFM3 Gate Semantic Boundary Scope

## Summary

This scope note locks the current PFM3 gate semantic execution boundary.
The selected implementation covers all 46 fixed-tableau gates across the current sampler, detection-conversion, and analyzer surfaces, plus supported Hermitian `SPP` and `SPP_DAG` execution across the promoted sampler, detection-conversion, detector-frame, and analyzer paths.
It does not claim full legal-gate execution parity for every parser-accepted non-tableau operation or future execution surface.

## Selected Surface

- Core sampler surface: `CompiledSampler` and `Circuit::reference_sample` through the fixed-tableau contract plus supported Hermitian `SPP` or `SPP_DAG` decomposition lowering.
- Core detection-conversion surface: `CompiledDetectionConverter` and public conversion helpers through the same fixed-tableau and supported Hermitian `SPP` or `SPP_DAG` subset.
- Core detection-sampling surface: `sample_detection_events` frame and non-frame paths for the selected fixed-tableau and supported Hermitian `SPP` or `SPP_DAG` cases.
- Core analyzer surface: `circuit_to_detector_error_model` fixed-tableau propagation plus supported Hermitian `SPP` or `SPP_DAG` state and gauge propagation.
- Public CLI dependency: selected `stab analyze_errors` `SPP` success and nondeterministic rejection behavior.

## Selected Positive Cases

- All 46 canonical gates with fixed tableau metadata execute in inverse-canceling sampler, detection-conversion, and analyzer circuits.
- Supported Hermitian `SPP` and `SPP_DAG` products execute in sampler and detection-conversion paths by matching decomposed-circuit behavior.
- Supported Hermitian `SPP` and `SPP_DAG` products execute in detector-frame sampling where the circuit requires the frame path because of Pauli-target observables.
- Supported Hermitian `SPP` and `SPP_DAG` products execute in analyzer state and gauge-tracker paths by matching explicit small-circuit expansions.
- Public `stab analyze_errors` covers selected Hermitian `SPP` state propagation and selected nondeterministic-detector failure behavior against pinned Stim v1.16.0.

## Selected Negative Cases

- Anti-Hermitian `SPP` and `SPP_DAG` products fail closed in sampler, detection-conversion, detector-frame, and analyzer tests.
- Parser-accepted gates that are not selected for a specific execution surface remain unsupported unless another milestone owns that surface with exact positive, negative, resource-boundary, oracle, and benchmark evidence.

## Evidence

- `fixed_tableau_gates_execute_across_current_public_surfaces` covers all 46 fixed-tableau gates through sampler, detection conversion, and analyzer circuits.
- `variable_target_spp_execution_matches_decomposed_circuit`, `variable_target_spp_matches_hard_coded_phase_product_decompositions`, and `variable_target_spp_executes_in_frame_detection_path` cover supported Hermitian `SPP` or `SPP_DAG` execution in sampler, detection-conversion, and detector-frame paths.
- `anti_hermitian_spp_execution_is_rejected_by_sampler_and_detection_conversion` covers sampler, detection-conversion, and detector-frame rejection for unsupported anti-Hermitian products.
- `dem_analyzer_spp_matches_explicit_phase_product_expansions`, `dem_analyzer_spp_nondeterministic_detector_matches_explicit_expansion`, `dem_analyzer_spp_nondeterministic_observable_matches_explicit_expansion`, and `dem_analyzer_rejects_anti_hermitian_spp_products` cover analyzer state, gauge, multiple product groups, and rejection behavior.
- `gate_execution_contract_accepts_supported_spp_execution_paths` and `gate_metadata_api_contract_table_matches_rust_accessors` keep the support-contract table synchronized with the promoted execution surfaces.
- Oracle rows `pf3-gate-semantic-wide-rust`, `pf3-gate-spp-analyzer-rust`, `pf3-gate-spp-contract-rust`, `pf3-analyze-errors-spp-state-propagation-cli`, and `pf3-analyze-errors-spp-nondeterministic-cli` select the promoted evidence.
- Benchmark row `pf3-gate-semantic-wide` is report-only and measures only the selected fixed-tableau plus supported Hermitian `SPP` or `SPP_DAG` sampler, detection-conversion, and analyzer execution contract.

## Explicit Non-Goals

- This slice does not select full legal non-tableau operation execution parity.
- This slice does not select public interactive simulator APIs, Python simulator products, exact random-stream parity, future detector-sampler sweep APIs, or full analyzer/search consumption of every legal operation.
- This slice does not add a primary benchmark gate or a new benchmark row.

## Future Selection Rule

Do not implement or claim another PF3 legal-gate execution family until a future exact-subcase plan names the gate family, execution surfaces, expected accepted and rejected behavior, comparator class, positive and negative tests, resource-boundary behavior, oracle metadata, benchmark policy, and documentation updates.

## Verification Commands

- `cargo test -p stab-core --test gate_semantic_execution --quiet`
- `cargo test -p stab-core --test dem_analyzer_mpp spp --quiet`
- `cargo test -p stab-core --test gate_metadata gate_execution_contract --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF3 --structural`
- `just bench::smoke`
