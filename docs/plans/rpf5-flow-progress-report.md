# RPF5 Flow Progress Report

## Summary

This RPF5 report now covers the promoted unsigned `has_flow` and `has_all_flows` Rust helper subset for measurement-record and observable dependencies, the additive unsigned diagnostic checker for the current helper subset, the scoped signed sampled flow checker, the scoped `circuit_flow_generators` subset including pinned variable-target `SPP` and `SPP_DAG` unitary examples, Python multi-target measurement and `MPP` examples with idle-qubit identity rows, nonconstant and constant single-instruction `MPP`, inverted result-target `MR`/`MRX`/`MRY`, selected unitary-mixed composed measurement-rich instruction sequences, selected all-operation no-op annotation and ordinary-noise traversal, selected composed `SPP` and `SPP_DAG` unitary decomposition, exact signed-flow-set and checker-satisfaction coverage for the pinned generated all-operations fixture, bounded repeat-contained measurement-rich instruction sequences, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, and selected single- and multi-target heralded-noise MPP generator cases, the pinned Stim `solve_for_flow_measurements` empty, `MX`, measured-idle, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code examples, the scoped unitary `time_reversed_for_flows` transform bridge, and the selected measurement-rich `time_reversed_for_flows` bridge for pinned `M`, `R`, `MPAD`, `MZZ`, `dont_turn_measurements_into_resets`, and `flow_flip` examples plus selected multi-record measurement-ordering evidence, selected plain `R`/`RX`/`RY` reset-to-measurement conversion over one or more unique qubit targets, selected single-target `M`/`MX`/`MY` measurement-to-reset conversion, `MR`/`MRX`/`MRY` measure-reset flow reversal over one or more unique qubit targets including inverted result targets, selected empty-flow plus Pauli-only, measurement-record, and observable MPAD record-tail reversal, including selected duplicate MPAD observable-id record parity tracked by `pf2-inverse-qec-mpad-rust`, and the selected `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s`.
Supersession note, 2026-07-11: PFM-B4 in `docs/plans/non-deferred-partial-feature-milestones.md` implements the selected Rust flow-engine closure, and PFM-B1 completes the selected nineteen-case Rust reverse-flow transform closure. Full generator-table GF(2) measurement solving uses no exhaustive fallback, all owned cases have independent selectors, and behavior outside either finite ledger requires an explicit plan revision. Python binding behavior remains deferred.
PFM-B4 review correction note, 2026-07-11: reset and measure-reset fast paths preserve untouched idle-qubit rows; flow generators support duplicate reset and measure-reset targets with pinned final-record and inversion-parity semantics while the narrower time-reversal transform keeps its separately documented duplicate-target rejection; mixed feedback-capable controlled-Pauli instructions process record-to-qubit, plain-qubit, and accepted classical-only pairs independently without depending on an unrelated measurement; pure ignored annotation or ordinary-noise circuits use a folded identity path, while mixed unitary-plus-noise circuits ignore noise during tableau propagation; `MPAD` pad values are excluded from internal simulated-qubit width and asymmetric pad values use pinned reverse-value/forward-record association; flow ordering compares Pauli bases and width before sign like Stim; feedback inlining preserves sweep-only groups only for `CZ`; wide idle and sparse high-index unitary repeats build touched-qubit transforms; the generated all-operations fixture is locked to its signed 40-flow set; and exact ledger selectors replace substring-based independence claims.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- `circuit_has_unsigned_stabilizer_flow` and `circuit_has_all_unsigned_stabilizer_flows` are additive Rust helpers over the same deterministic unsigned checker semantics and intentionally do not implement Stim's randomized signed Python `has_flow` or `has_all_flows` mode.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.
- `check_unsigned_stabilizer_flows_with_diagnostics` is an additive Rust diagnostic helper over the same unsigned checker subset. It reports success, unsigned unitary output mismatches, sparse-tracker input mismatches, out-of-range measurement-record references, and unsupported-circuit reasons while preserving the existing boolean helper semantics.
- `sample_if_circuit_has_stabilizer_flows` is an additive Rust sampled signed checker over the promoted scope. It builds one augmented noiseless circuit per queried flow, uses an ancilla witness measurement to preserve input signs, output signs, selected measurement-record terms, and selected observable terms, and returns one boolean per flow.
- The signed sampled checker ports selected pinned Stim sign-sensitive cases for unitary flows, measurement-record flows, record-backed observables, Pauli-backed observables, inverted Pauli observables, and inverted record-backed observables. It rejects malformed measurement-record references with a domain error and lets unsupported augmented sampler shapes fail through a clear `CircuitError`.
- `circuit_flow_generators` supports exact single-instruction generators for `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, including inverted result targets for `MR`, `MRX`, and `MRY`, nonconstant and constant `MPP`, Python multi-target `M`, `MX`, `MYY`, and `MPP` examples with idle-qubit identity rows, pinned variable-target `SPP` and `SPP_DAG` unitary examples through existing decomposition, and `MPAD`, plus selected unitary-mixed composed measurement-rich instruction sequences with tableau-backed plain-qubit Clifford gates, selected all-operation annotation and ordinary-noise instructions ignored in Stim's generator sense, selected composed `SPP` and `SPP_DAG` unitary instructions through existing decomposition, exact signed-flow-set and checker-satisfaction coverage for the pinned generated all-operations fixture from Stim's `generate_test_circuit_with_all_operations`, bounded repeat-contained measurement-rich instruction sequences through the 4096-row flow-generator cap and `Circuit::flattened_operations` materialization limit, the scoped measurement-record feedback examples `M; CX rec`, `MPP; CX rec`, `M; XCZ rec`, `M; CY rec`, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, and selected single- and multi-target `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` MPP generator cases.
- Mixed feedback-capable controlled-Pauli instructions process target groups independently: valid record-to-qubit feedback contributes measurement parity, plain-qubit pairs apply the gate tableau, and parser-accepted classical-only pairs such as record-to-sweep controls are sign-only no-ops. Flow generation supports duplicate reset and measure-reset targets with pinned final-record parity and inversion semantics; malformed controlled-Pauli targets, excessive flow-generator rows or repeat expansion beyond current caps, and unselected heralded-noise composition fail closed with an explicit unsupported generator or resource-limit error.
- `solve_for_flow_measurements` is exposed as a Rust helper for the promoted unsigned scope and always uses deterministic GF(2) reduction over generator rows when the current `circuit_flow_generators` subset applies. Like Stim v1.16.0, it solves only each query's input/output Pauli projection and ignores measurement or observable terms already present on the query. Unsupported generator shapes preserve their typed generator error instead of enumerating measurement subsets.
- The promoted solver scope covers empty input, simple `MX`, measured-idle identity and collapse flows, multi-target measurement and `MPP` products, fewer-measurements heuristic cases, repetition-code measurement solving, empty-Pauli query rejection, a supported 33-measurement product case, rank-deficient and underdetermined rows, sparse high qubits, nonempty ignored query measurement and observable terms, fixed-seed generated cross-engine agreement, and measurement-count-independent unsupported-shape rejection. Duplicate term canonicalization remains covered by the `Flow` value-object and checker suites that consume those terms.
- `Circuit::time_reversed_for_flows` is exposed for the scoped unitary flow-transform subset, validates unsigned Pauli-only flows against the original unitary circuit with bounded tableau validation or folded sparse validation for supported large repeats including the owned `H`, `SQRT_X`, and `CY` regression cases, and supports idle flow qubits beyond the circuit width.
- The selected measurement-rich flow-transform subset validates flows through the sparse tracker or the selected MPAD record-tail QEC inverse path and reverses flow endpoints for one noiseless plain unique-target `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, with pinned Stim `M` and `MZZ` evidence, source-owned basis coverage for `MX`, `MY`, `MXX`, and `MYY`, selected multi-record measurement-ordering evidence, selected plain `R`, `RX`, and `RY` reset-to-measurement conversion over one or more unique qubit targets, selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion when the full flow batch has record dependence but no future Pauli dependence on the measured qubit, selected `dont_turn_measurements_into_resets` single-measurement preservation, selected empty-flow plus Pauli-only, measurement-record, and observable MPAD record-tail reversal with selected duplicate MPAD observable-id record parity and unsatisfied observable-flow terms rejected, the selected `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s`, and the exact pinned `flow_flip` packet; the selected measure-reset subset covers one noiseless `MR`, `MRX`, or `MRY` instruction over one or more unique qubit targets, including inverted result targets, by mapping reset-effect output terms into reversed measurement records and measurement-record dependencies back into reset effects.

## Composed-Measurement Scope

The current PFM5 composed-measurement slice promotes composed measurement-rich flow generators for top-level or bounded repeat-contained instruction sequences handled by the existing reverse measurement-flow solver: measurement, reset, measure-reset including inverted result targets, pair-measurement, Pauli-product measurement, `MPAD`, `TICK`, supported heralded record-producing instructions, supported measurement-record feedback gates, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, selected tableau-backed unitary gates with plain qubit targets, selected all-operation annotation and ordinary-noise no-op traversal, selected `SPP` or `SPP_DAG` unitary decomposition, and exact signed-flow-set and checker-satisfaction coverage for the pinned generated all-operations fixture.
It supports selected mixed sweep-controlled, measurement-record-feedback, classical-only, and plain-qubit groups through the pairwise transition contract locked by PFM-B4. Invalid feedback placement, malformed or otherwise unsupported classical-control shapes, excessive repeat expansion, broad stochastic-noise checking semantics, and broader generated all-operation families beyond the pinned fixture remain fail-closed until their exact flow semantics and resource rules are specified.
Completion evidence for this slice includes exact generator tests for repeated same-qubit measurements across instructions, inverted measure-reset result targets, reset/measurement ordering, selected unitary-mixed compositions, no-op annotation and ordinary-noise equivalence, composed `SPP` decomposition equivalence, exact signed-flow-set and checker-satisfaction coverage for the pinned generated all-operations fixture, repeat-versus-expanded equivalence tests for bounded repeat-contained measurement-rich instruction sequences, generated-flow checker satisfaction for independent composed measurements and mixed composed measurement families where the unsigned checker owns the shape, oracle metadata updates, refreshed `pf5-flow-generators-measurement-rich` benchmark corpus work units, milestone-audit, full-code-review, and targeted verification.

## Unsigned Has-All Scope

The current PFM5 has-all slice promotes only deterministic unsigned Rust helpers over the existing supported flow checker: `circuit_has_unsigned_stabilizer_flow` for one flow and `circuit_has_all_unsigned_stabilizer_flows` for an empty or non-empty batch.
Owned positive and negative subcases are unitary sign-insensitive flows, false unitary flows, unsigned `SPP`/`SPP_DAG` product propagation, anti-Hermitian `SPP` failure, measurement-record and observable dependencies, and folded-repeat measurement references.
The helpers return booleans and deliberately preserve the existing fail-closed checker behavior for sparse-tracker shapes that remain unsupported outside the promoted unsigned path.
They do not expose Stim's signed randomized `has_flow` or `has_all_flows` behavior and do not claim Python binding parity.
The report-only `pf5-has-all-flows-batch` benchmark now calls the public batch helper while still validating the vector checker's per-flow expected results inside the benchmark closure.

## Signed Sampled Flow-Checking Scope

This follow-up slice promotes a scoped Rust counterpart to Stim v1.16.0 `sample_if_circuit_has_stabilizer_flows`.
The owned public API is additive: it samples an augmented noiseless circuit for each requested `Flow`, preserves signs, `rec[...]` terms, and selected `obs[...]` terms, rounds the effective sample count up to 256 to match Stim's public Python-path confidence behavior, and returns one boolean per flow.
The comparator class is structural Rust parity against the pinned Stim signed sampled flow tests for sign-sensitive unitary, measurement-record, record-backed observable, Pauli-backed observable, inverted Pauli-backed observable, and inverted record-backed observable cases.
The API intentionally remains probabilistic and does not replace the deterministic unsigned helper; tests use deterministic positive cases plus negative cases that either fail deterministically or are sampled with enough fixed-seed shots to catch the bad flow.
Malformed measurement-record references return a domain error instead of silently failing, and unsupported augmented sampler shapes fail closed through a clear `CircuitError`.
The owned resource behavior is bounded by the caller-provided sample count after 256-shot rounding and one augmented sampler compilation per flow; this historical slice did not add Python `Circuit.has_flow` or `has_all_flows` binding parity, the general generator-table solver later closed by PFM-B4, signed diagnostics, or a new benchmark gate.

## Tests

Implemented Rust tests:

- `check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies`
- `circuit_has_unsigned_stabilizer_flow_helpers_match_supported_batch_semantics`
- `unsigned_stabilizer_flow_diagnostics_explain_unitary_mismatches`
- `unsigned_stabilizer_flow_diagnostics_explain_sparse_tracker_failures`
- `unsigned_stabilizer_flow_diagnostics_keep_unsupported_circuits_fail_closed`
- `unsigned_stabilizer_flow_diagnostics_match_bool_checker`
- `sample_if_circuit_has_stabilizer_flows_checks_signed_unitary_flows`
- `sample_if_circuit_has_stabilizer_flows_checks_signed_measurement_records`
- `sample_if_circuit_has_stabilizer_flows_checks_signed_observables`
- `sample_if_circuit_has_stabilizer_flows_checks_inverted_record_observables`
- `sample_if_circuit_has_stabilizer_flows_rejects_malformed_measurement_refs`
- `pf6_sparse_rev_spp_circuit_has_unsigned_stabilizer_flow_helpers_support_unsigned_semantics`
- `circuit_flow_generators_promotes_single_instruction_measurement_subset`
- `circuit_flow_generators_measurement_promotes_multi_target_heralded_noise_mpp_subset`
- `circuit_flow_generators_promotes_python_multi_target_examples`
- `circuit_flow_generators_promotes_spp_measurement_row_unitary_examples`
- `circuit_flow_generators_promotes_composed_measurement_subset`
- `circuit_flow_generators_promotes_unitary_mixed_measurement_subset`
- `circuit_flow_generators_measurement_subset_ignores_annotations_and_noise`
- `circuit_flow_generators_measurement_subset_composes_spp_unitaries`
- `circuit_flow_generators_measurement_subset_promotes_generated_all_operations_fixture`
- `circuit_flow_generators_promotes_bounded_repeat_measurement_subset`
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker`
- `circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes`
- `circuit_flow_generators_measurement_subset_supports_measurement_free_mixed_sweep_groups`
- `circuit_flow_generators_measurement_subset_ignores_noise_without_measurements`
- `circuit_flow_generators_measurement_subset_excludes_mpad_values_from_simulated_qubits`
- `circuit_flow_generators_rejects_excessive_repeat_measurement_expansion`
- `circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products`
- `solve_for_flow_measurements_cpp_empty_and_simple_examples`
- `solve_for_flow_measurements_python_measured_idle_examples`
- `solve_for_flow_measurements_python_multi_target_examples`
- `solve_for_flow_measurements_python_fewer_measurements_heuristic_examples`
- `solve_for_flow_measurements_cpp_repetition_code_example`
- `pfm_b4_flow_solve_over_sixteen`
- `pfm_b4_flow_solver_rejects_unsupported_circuits_without_exhaustive_fallback`
- `pfm_b4_flow_solver_treats_mpad_values_as_non_qubit_records`
- `pfm_b4_flow_solver_rank_deficient_inconsistent_and_underdetermined`
- `pfm_b4_flow_solver_generated_cross_engine_corpus`
- `time_reversed_for_flows_unitary_subset_matches_qec_inverse`
- `time_reversed_for_flows_unitary_subset_supports_flow_past_end`
- `time_reversed_for_flows_unitary_subset_supports_extra_idle_qubits`
- `time_reversed_for_flows_unitary_subset_folds_large_repeats`
- `time_reversed_for_flows_unitary_subset_folds_large_cy_repeats`
- `time_reversed_for_flows_unitary_subset_rejects_unsatisfied_flow`
- `time_reversed_for_flows_measurement_rich_subset_reverses_single_measurement`
- `time_reversed_for_flows_measurement_rich_subset_reverses_pair_measurement`
- `time_reversed_for_flows_measurement_rich_subset_covers_selected_bases`
- `time_reversed_for_flows_measurement_rich_subset_preserves_measurement_ordering`
- `time_reversed_for_flows_measurement_rich_subset_turns_measurements_into_resets`
- `time_reversed_for_flows_measurement_rich_subset_can_keep_measurements`
- `time_reversed_for_flows_measurement_rich_subset_reverses_measure_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_measure_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_inverted_measure_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_resets`
- `time_reversed_for_flows_measurement_rich_subset_supports_flow_flip`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_flow_flip_variants`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unsatisfied_flows`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_terms`
- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measurement_targets`
- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets`
- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unscoped_reset_terms`
- `mzz_unitary_suffix_matches_pinned_stim_flow_through_h_cx_s`
- `mzz_unitary_suffix_rejects_unsatisfied_flows`
- `mzz_unitary_suffix_rejects_observable_terms`
- `mzz_unitary_suffix_rejects_unscoped_shapes`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, unsigned single-flow and all-flow helper success and failure cases, empty all-flow batches, folded-repeat measurement references, diagnostic reasons for unitary output mismatches, sparse-tracker input mismatches, out-of-range record references, unsupported circuits, signed sampled flow checks for sign-sensitive unitary flows, measurement-record flows, record-backed observables, Pauli-backed observables, inverted Pauli observables, inverted record-backed observables, and malformed measurement references, unsigned `SPP`/`SPP_DAG` product propagation with false identity-flow rejection, exact measurement, reset, measure-reset including inverted result targets, pair-measurement, nonconstant and constant `MPP`, Python multi-target measurement and `MPP` flow-generator examples with idle-qubit identity rows, pinned variable-target `SPP` and `SPP_DAG` unitary flow-generator examples with anti-Hermitian rejection, selected unitary-mixed composed measurement-rich instruction sequences, selected all-operation annotation and ordinary-noise no-op traversal in the generator solver, selected composed `SPP` and `SPP_DAG` unitary decomposition, exact signed-flow-set and checker-satisfaction coverage for the pinned generated all-operations fixture, bounded repeat-contained measurement-rich instruction sequences, measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, `MPAD`, and selected single- and multi-target heralded-noise MPP generators, generated-flow satisfaction checks for the supported checker subset, pinned Stim measurement-solver examples for empty input, simple `MX`, measured-idle collapse, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code solving, scoped unitary flow time reversal, selected single-instruction measurement-rich flow time reversal for all promoted measurement bases with selected multi-record ordering, selected plain reset-to-measurement conversion for `R`, `RX`, and `RY` over one or more unique qubit targets, selected single-target measurement-to-reset conversion for `M`, `MX`, and `MY` including the Rust-native `dont_turn_measurements_into_resets` option case, selected measure-reset flow time reversal for `MR`, `MRX`, and `MRY` over one or more unique qubit targets including inverted result targets, selected `MZZ` plus plain-qubit unitary suffix reversal matching pinned `flow_through_mzz_h_cx_s`, exact pinned `flow_flip` packet reversal and nearby exact-scope rejection, time-reversal duplicate measurement target rejection, time-reversal duplicate reset target rejection under the locked duplicate-target boundary, time-reversal duplicate measure-reset target rejection under the locked duplicate-target boundary, selected-flow unsatisfied observable-term rejection and reset measurement-record-term rejection, and negative cases ported from pinned Stim v1.16.0 `has_flow`, `has_all_flows`, `circuit_flow_generators`, and `circuit_inverse_qec` tests.

## Oracle Rows

Implemented rows include:

- `pf5-has-flow-record-observable-rust`
- `pf5-has-all-flows-rust`
- `pf5-has-flow-diagnostics-rust`
- `pf5-signed-sampled-flows-rust`
- `pf5-flow-generators-measurement-rust`
- `pf5-flow-generators-measurement-python-rust`
- `pf5-flow-solve-measurement-rust`
- `pf5-flow-solve-measurement-python-rust`
- `pfm-b4-flow-generators-various-rust`
- `pfm-b4-flow-solver-cpp-rust`
- `pfm-b4-flow-solver-python-rust`
- `pfm-b4-flow-solver-matrix-rust`
- `pf2-inverse-qec-mpad-rust`
- `pf2-time-reverse-flow-unitary-rust`
- `pf2-time-reverse-flow-measurement-rust`
- `pf2-time-reverse-flow-mzz-unitary-suffix-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`
- `pf5-flow-generators-measurement-rich`
- `pf5-flow-generators-measurement-python`
- `pf5-flow-solve-measurement-rich`
- `pf5-flow-solve-measurement-python`
- `pfm-b4-flow-solve-matrix-sizes`
- `pf2-time-reverse-flow`
- `pf2-time-reverse-flow-measurement`

The PFM-B4 matrix row times end-to-end public solver calls over measurement-rich `32x64`, `128x256`, and sparse high-qubit `512x1024` Pauli bases carrying 7, 24, and 12 measurement signatures. It uses 17, 65, and 33 three-row-composed queries with nonempty solved parity, enforces dense, sparse, and active-submatrix density contracts plus exact active support, and executes literal production-contract construction in benchmark tests. The medium case exceeds the former sixteen-measurement boundary; deterministic fixture construction and validation stay outside timing.

No benchmark row is added for the signed sampled checker because this slice is an additive compatibility and diagnostic helper that compiles one augmented sampler per queried flow and is not yet a throughput contract. Future Python-style `has_flow` or `has_all_flows` parity should add a source-owned benchmark row if it makes sampled flow checking a public hot path.

The row measures the promoted unsigned has-all-flow corpus through the Rust public batch helper while validating per-flow expected results through the vector checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
The generator row measures the promoted measurement, reset, inverted measure-reset, pair-measurement, nonconstant and constant `MPP`, pinned variable-target `SPP` and `SPP_DAG` unitary, selected unitary-mixed composed measurement-rich, selected all-operation annotation and ordinary-noise no-op traversal, selected composed `SPP` and `SPP_DAG` unitary decomposition, bounded repeat-contained measurement-rich, measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, `MPAD`, and selected single- and multi-target heralded-noise MPP generator corpus through the Rust public `circuit_flow_generators` helper.
It reports `stab_pf5_flow_generators_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_flows`, normalized as flows per second.
The refreshed corpus covers 51 cases and 168 generated flows per utility batch.
The current focused report-only probe used `target/benchmarks/pfm5-multitarget-heralded-flow-baseline/baseline.json` and `target/benchmarks/pfm5-multitarget-heralded-flow-compare/compare.json`.
It measured `stab_pf5_flow_generators_measurement_cases` at `3.128e5 cases/s` and `stab_pf5_flow_generators_measurement_flows` at `1.037e6 flows/s`.
The Python multi-target generator row measures the pinned Python `flow_generators()` examples through the same Rust helper.
It reports `stab_pf5_flow_generators_measurement_python_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_python_flows`, normalized as flows per second.
The Python generator corpus covers 4 cases and 32 generated flows per utility batch.
The current focused report-only Python generator probe used `target/benchmarks/rpf5-flow-generator-python-probe/baseline.json` and `target/benchmarks/rpf5-flow-generator-python-compare/compare.json`.
It measured `stab_pf5_flow_generators_measurement_python_cases` at `1.837e5 cases/s` and `stab_pf5_flow_generators_measurement_python_flows` at `1.461e6 flows/s`.
The C++ solver row measures the promoted C++ `solve_for_flow_measurements` examples through the Rust public helper.
It reports `stab_pf5_flow_solve_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_solve_measurement_queries`, normalized as queries per second.
The refreshed C++ solver corpus covers 2 cases and 15 queried flows per utility batch.
The current focused report-only C++ solver probe used `target/benchmarks/rpf5-flow-solve-cpp-probe/baseline.json` and `target/benchmarks/rpf5-flow-solve-cpp-compare/compare.json`.
It measured `stab_pf5_flow_solve_measurement_cases` at `8.502e4 cases/s` and `stab_pf5_flow_solve_measurement_queries` at `6.340e5 queries/s`.
The Python solver row measures the promoted Python `solve_flow_measurements` examples through the same Rust helper.
It reports `stab_pf5_flow_solve_measurement_python_cases`, normalized as cases per second, and `stab_pf5_flow_solve_measurement_python_queries`, normalized as queries per second.
The refreshed Python solver corpus covers 8 cases and 15 queried flows per utility batch.
The current focused report-only Python solver probe used `target/benchmarks/rpf5-flow-solve-python-probe/baseline.json` and `target/benchmarks/rpf5-flow-solve-python-compare/compare.json`.
It measured `stab_pf5_flow_solve_measurement_python_cases` at `1.175e5 cases/s` and `stab_pf5_flow_solve_measurement_python_queries` at `2.204e5 queries/s`.
These rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface, and they are not part of the 1.25x primary threshold file.
The `pf2-time-reverse-flow` row measures the scoped unitary flow-transform bridge from the RPF2 side and remains report-only for the same reason.
The `pf2-time-reverse-flow-measurement` row measures the selected measurement-rich flow-transform bridge from the RPF2 side, now including the selected `MZZ` plus plain-qubit unitary suffix packet, the exact pinned `flow_flip` packet, selected measurement-ordering, selected plain reset-to-measurement over one or more unique qubit targets, selected measurement-to-reset including the selected `dont_turn_measurements_into_resets` option case, and measure-reset cases over one or more unique qubit targets including inverted result targets, and remains report-only for the same reason.
The current focused report-only probe used `target/benchmarks/rpf2-time-reverse-flow-probe/baseline.json` and `target/benchmarks/rpf2-time-reverse-flow-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_unitary` at `4.097e5 flows/s`.
The focused selected measurement-rich time-reversal probe used `target/benchmarks/pf2-dont-turn-probe-baseline/baseline.json` and `target/benchmarks/pf2-dont-turn-probe-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_measurement` at `1.826e6 flows/s` with the refreshed corpus including the selected `dont_turn_measurements_into_resets` option case, selected `MZZ` plus plain-qubit unitary suffix packet, inverted result-target measure-reset cases, and exact pinned `flow_flip` packet.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core --test circuit_flows solve_for_flow_measurements --quiet
cargo test -p stab-core --test circuit_flow_generators --quiet
cargo test -p stab-core --test circuit_flow_generators python_multi_target --quiet
cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet
cargo test -p stab-core --test circuit_time_reverse_flow_mzz_suffix --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5:: --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just oracle::run --implemented-only
just bench::smoke
just bench::baseline --only pf5-flow-generators-measurement-rich --out target/benchmarks/pfm5-multitarget-heralded-flow-baseline
just bench::compare --only pf5-flow-generators-measurement-rich --baseline target/benchmarks/pfm5-multitarget-heralded-flow-baseline/baseline.json --report target/benchmarks/pfm5-multitarget-heralded-flow-compare
just bench::baseline --only pf5-flow-generators-measurement-python --out target/benchmarks/rpf5-flow-generator-python-probe
just bench::compare --only pf5-flow-generators-measurement-python --baseline target/benchmarks/rpf5-flow-generator-python-probe/baseline.json --report target/benchmarks/rpf5-flow-generator-python-compare
just bench::baseline --only pf5-flow-solve-measurement-rich --out target/benchmarks/rpf5-flow-solve-cpp-probe
just bench::compare --only pf5-flow-solve-measurement-rich --baseline target/benchmarks/rpf5-flow-solve-cpp-probe/baseline.json --report target/benchmarks/rpf5-flow-solve-cpp-compare
just bench::baseline --only pf5-flow-solve-measurement-python --out target/benchmarks/rpf5-flow-solve-python-probe
just bench::compare --only pf5-flow-solve-measurement-python --baseline target/benchmarks/rpf5-flow-solve-python-probe/baseline.json --report target/benchmarks/rpf5-flow-solve-python-compare
just bench::baseline --only pf2-time-reverse-flow-measurement --out target/benchmarks/pf2-dont-turn-probe-baseline
just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/pf2-dont-turn-probe-baseline/baseline.json --report target/benchmarks/pf2-dont-turn-probe-compare
```

## Audit And Review

Milestone-audit for the unsigned has-all helper slice found the promoted scope complete against the current PFM5 text: the Rust API is additive, deterministic, unsigned-only, fail-closed for unsupported sparse-tracker gates, covered by direct tests, represented by oracle row `pf5-has-all-flows-rust`, and measured by report-only row `pf5-has-all-flows-batch`.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P1 fail-open risk where unsupported `SPP` could be treated as identity by sparse-tracker fallback; the initial fix made unsupported non-noise and non-annotation sparse-tracker instructions return an error, and the promoted follow-up added unsigned `SPP`/`SPP_DAG` propagation plus false identity-flow and anti-Hermitian regression coverage.
The docs and benchmark reviewer found two P2 alignment gaps: this audit block still described the earlier composed-measurement slice, and the historical RPF oracle-row rollup omitted `pf5-has-all-flows-rust`; both were corrected.
The benchmark row remains report-only and outside primary thresholds because there is no faithful pinned-Stim CLI timing ratio for this Rust utility helper.
Milestone-audit for the selected measurement-rich time-reversal slice found the promoted scope complete against the current PFM5 flow-transform text while keeping broader measurement-rich transform integration open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment; both sidecars reported no confirmed findings for this slice.
The selected time-reversal evidence is PF2-owned in oracle row `pf2-time-reverse-flow-measurement-rust`, so `just oracle::run --milestone PF5 --structural` does not directly run that row even though PF5 docs cross-reference it.
The selected measure-reset slice originally promoted only the single-target measure-reset shape plus selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion in the PF2-owned time-reversal row, refreshed the report-only benchmark corpus, and kept broader reset-only operations, duplicate or inverted measure-reset groups, detector, feedback, noise, repeat, and multi-instruction QEC inverse shapes open before later slices expanded it first to plain unique-target groups and then to inverted result targets.

The current selected reset-to-measurement and measurement-ordering slice promotes one noiseless plain `R`, `RX`, or `RY` instruction over one or more unique qubit targets, selected multi-record `M` and `MZZ` measurement ordering, and `MR`, `MRX`, and `MRY` measure-reset flow reversal over one or more unique qubit targets in the same PF2-owned time-reversal row, rejects duplicate measurement targets, duplicate reset targets, duplicate measure-reset targets, rejects observable terms for selected measurement-rich flow reversals, rejects measurement-record terms for selected reset-only paths, refreshes the report-only benchmark corpus, and keeps duplicate reset-only and duplicate measure-reset behavior fail-closed under the boundary locked in `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md` while parser-rejected inverted reset targets, detector, feedback, noise, repeat, and multi-instruction QEC inverse shapes remain open.
Full-code-review for this PF2-owned measurement-ordering and measure-reset bridge used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found and the implementation fixed a P1 ordering bug where multi-record selected measurement and measure-reset inverses kept original target order instead of Stim-style reversed target groups, plus a P2 large-group duplicate-detection issue now addressed with set-backed uniqueness checks.
The docs/evidence sidecar reported no confirmed findings and confirmed this report stays scoped to the PF2-owned oracle row and report-only benchmark row.
The current multi-target reset follow-up used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar reported no confirmed findings.
The docs/evidence sidecar found an under-specified arity evidence question because the implementation accepts arbitrary plain unique-target reset groups while the initial new positive test only exercised two targets.
The PF2-owned test now also covers a three-target `R 0 1 2` reset reversal, closing that evidence gap without expanding beyond the selected single-instruction reset scope.
The duplicate reset-only scope-reconciliation pass probed Stim v1.16.0 with `uv run --with stim==1.16.0 python` and found malformed inverse flows for duplicate reset targets, such as `R 0 0` producing `M 0 0` with `Z -> rec[-4] xor rec[-3]`. Stab therefore keeps `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets` as the source-owned fail-closed behavior until `docs/plans/milestone-spec-gaps.md` resolves the compatibility decision; the current boundary is locked in `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`.
The inverted measure-reset slice probed Stim v1.16.0 and found coherent self-validating inverse flows for inverted result targets such as `MR !0 1`, while duplicate measure-reset targets such as `MR 0 0` produced malformed out-of-range inverse flows. Stab now implements `time_reversed_for_flows_measurement_rich_subset_reverses_inverted_measure_resets`, keeps `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets` fail-closed, updates the report-only benchmark corpus, logs the duplicate measure-reset compatibility choice in `docs/plans/milestone-spec-gaps.md`, and locks the current boundary in `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`.
The selected `MZZ` plus plain-qubit unitary suffix slice promotes the pinned `flow_through_mzz_h_cx_s` packet through the PF2-owned time-reversal row, adds `circuit_time_reverse_flow_mzz_suffix`, updates the PF2 oracle row and report-only benchmark corpus, and keeps noisy, multi-record, feedback, detector, observable-aware, noise-suffix, repeat-suffix, and broader multi-instruction measurement-rich shapes fail-closed.
Milestone-audit for this selected suffix slice found no blocking gaps after the explicit scope note limited the packet to one noiseless plain single-record `MZZ` group followed by plain-qubit unitary instructions and kept broader observable-aware rewrites open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found a P2 fail-open gap where observable terms on the selected suffix flow could be preserved even though observable-aware rewrites are out of scope; selected measurement-rich flow reversal now rejects observable terms before sparse validation returns a reversed flow, and `mzz_unitary_suffix_rejects_observable_terms` covers the regression.
The docs/evidence sidecar found a P2 stale benchmark-evidence gap where this report still cited `pf2-inverted-measure-reset-*` artifacts after the `pf2-time-reverse-flow-measurement` corpus added the selected suffix packet and exact `flow_flip` packet; this report now cites the later refreshed `target/benchmarks/pf2-dont-turn-probe-baseline/baseline.json` and `target/benchmarks/pf2-dont-turn-probe-compare/compare.json` artifacts.
The exact pinned `flow_flip` slice is PF2-owned but cross-referenced here because it is part of the selected measurement-rich flow-transform bridge. The slice adds positive and fail-closed evidence under `measurement_rich_subset`, refreshes the report-only benchmark corpus, and keeps broader transform integration outside the PF5 claim.
The selected `dont_turn_measurements_into_resets` option slice is also PF2-owned but cross-referenced here because it is part of the selected measurement-rich flow-transform bridge. It adds Rust-native options API coverage for the pinned single-measurement example, refreshes the report-only benchmark corpus, keeps Python binding parity plus broader option shapes outside the PF5 claim, and locks the exact scope in `docs/plans/pfm2-time-reverse-dont-turn-measurements-scope.md`.
Milestone-audit for the selected `dont_turn_measurements_into_resets` slice found the packet complete against the scope note: the option is additive, the default API remains unchanged, the exact pinned circuit and flow are tested through free-function and method forms, oracle and report-only benchmark metadata include the option case, and broader option parity remains explicitly outside the claim.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment. The Rust/API sidecar found no confirmed findings and noted only the public-options API growth risk that matches the existing `InverseQecOptions` style; the docs/evidence sidecar found a P3 discoverability gap for the new scope note, fixed by linking it from the progress reports and oracle provenance.
Milestone-audit for the bounded repeat-contained flow-generator slice found the promoted scope complete against the current PFM5 text: it promotes repeat-contained measurement-rich instruction sequences only through the existing flattened-operation cap plus a new 4096-row flow-generator cap, has repeat-versus-expanded equivalence tests, has a compact-repeat resource rejection test, updates oracle and benchmark metadata, and keeps full folded repeat traversal open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P1 resource issue where compact repeats at the generic flattened-operation limit could still enter the expensive flow-row canonicalization path; the implementation now validates measurement-rich flow-generator rows before flattening or allocating solver rows, and the regression uses `REPEAT 1000000 { M 0 }`.
The docs and benchmark reviewer for the earlier repeat-contained slice found a stale verification-command path for that refreshed benchmark probe; the report was corrected in that slice before the later unitary-mixed probe replaced the current benchmark artifact names.
Milestone-audit for the selected unitary-mixed flow-generator slice found the promoted scope complete against the then-current PFM5 text: it promoted tableau-backed plain-qubit unitary groups inside composed measurement-rich flow generators, kept sweep-controlled and mixed classical-control shapes fail-closed at that point, updated oracle and benchmark metadata, refreshed report-only benchmark evidence, and kept broader all-operation generator synthesis open.
At that historical checkpoint, the selected sweep-controlled Pauli slice promoted only instruction groups made of one sweep bit plus one qubit as sign-only no-ops in `circuit_flow_generators`; mixed sweep-plus-quantum groups and measurement-record-to-sweep controls remained fail-closed until the later PFM-B4 pairwise transition work.
Milestone-audit for the selected sweep-controlled Pauli slice found no blocking findings after adding source-owned target-order and multi-group evidence.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P3 evidence gap because the accepted `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` sweep/qubit matrix was broader than the positive tests; the slice now has table-driven identity-row generator tests for both target orders and table-driven `solve_for_flow_measurements` tests proving those groups are sign-only no-ops for all accepted gates and orders.
The docs and benchmark reviewer found stale focused benchmark evidence for the expanded 46-case and 148-flow corpus; the current report now cites the refreshed `target/benchmarks/rpf5-sweep-flow-generator-probe` and `target/benchmarks/rpf5-sweep-flow-generator-compare` artifacts.
Full-code-review for the earlier selected unitary-mixed slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer reported no confirmed core correctness findings for the tableau-undo path and recommended additional sign, repeat-equivalence, and mixed controlled-Pauli boundary tests; the slice now includes exact canonical sign assertions for `S; MX` and `S_DAG; MX`, repeat-versus-expanded equivalence for `REPEAT { M; H }`, and fail-closed tests for mixed feedback/plain `CX` groups.
The docs and benchmark reviewer found stale roadmap and benchmark-manifest wording plus an underclaimed feature-checklist row; those were corrected to describe selected unitary-mixed composed measurement-rich support and keep broader all-operation solving open.
The full-code-review large-file pass found `crates/stab-core/src/circuit_flow/generators.rs` had crossed the 1200-line threshold after this slice; helper ownership for target parsing, local tableau application, record-index conversion, and flow-generator error mapping moved to `crates/stab-core/src/circuit_flow/generators/helpers.rs`, leaving `generators.rs` at 1177 lines and the helper module at 117 lines.
The current SPP generator slice promotes pinned Stim `circuit_flow_generators` examples for `SPP Z0`, `SPP X0 Z0`, `SPP X0*X1`, and `SPP_DAG Z0` through the existing public decomposition path, adds anti-Hermitian `SPP X0*Z0` rejection coverage, refreshes the report-only generator benchmark corpus to include those variable-target unitary cases, and keeps broader all-operation flow-generator synthesis open.
Milestone-audit for the current SPP generator slice found the promoted scope complete against the current PFM5 text while keeping broader all-operation flow-generator synthesis open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer reported no confirmed findings and noted residual non-blocking opportunities for broader inverted, multi-product, repeat-contained, and checker-satisfaction coverage; the slice now includes checker-satisfaction coverage for the promoted SPP generator outputs.
The docs and benchmark reviewer found a P2 oracle-filter gap where `pf5-flow-generators-measurement-rust` claimed SPP evidence but ran only tests matching `measurement`, plus a P3 stale roadmap summary at `docs/plans/rust-stim-drop-in-rewrite.md`.
The positive SPP test was renamed into the oracle filter, `cargo test -p stab-core --test circuit_flow_generators measurement -- --list` now includes it, and the stale roadmap summary was corrected.
The large-file pass records `crates/stab-core/src/circuit_flow/generators.rs` at 1193 lines after this slice, still under the 1200-line threshold but on the watch list for the next flow-generator change.
The current solver-scope slice promotes pinned Python `solve_flow_measurements` examples for measured-idle collapse, multi-target measurement and `MPP`, and fewer-measurements heuristic cases into source-specific Rust `solve_for_flow_measurements` tests and report-only benchmark corpus.
Milestone-audit for the historical solver-scope slice found its then-promoted examples complete while leaving general generator-table solving open; PFM-B4 later supersedes that limitation with independently selected evidence and general GF(2) reduction.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P3 benchmark-work guard gap where the new test asserted hardcoded work units instead of deriving expected work from the actual corpus; `expected_flow_solve_measurement_work_for_test` now derives those counts from the selected row corpus and compares them to `measurement_work`.
The docs and benchmark reviewer found P2 provenance and reproducibility gaps: Python-derived solver coverage was machine-recorded as C++-only, and the verification block initially omitted the focused Python solver benchmark commands. The oracle and benchmark rows are now split by upstream source, the PFM5 source list includes `circuit_flow_generators_test.py`, and the verification block names the focused Python solver benchmark commands.
The current Python multi-target generator slice promotes pinned Python `flow_generators()` examples for offset `M`, `MX`, `MYY`, and `MPP` targets. It fixes single-qubit measurement flow generation to retain idle-qubit identity rows and canonicalize through the same row-elimination path used by pair-measurement and Pauli-product generators, while keeping broader all-operation generator synthesis open.
The current all-operation generator slice promotes no-op annotation and ordinary-noise traversal plus composed `SPP` or `SPP_DAG` unitary decomposition inside the measurement-rich flow-generator solver. It adds generator equivalence tests for annotations and ordinary noise, decomposition equivalence and checker-satisfaction tests for selected composed `SPP` or `SPP_DAG` cases, anti-Hermitian composed `SPP` rejection coverage, refreshed oracle metadata, refreshed report-only generator benchmark work units, and a fresh focused report-only benchmark probe while keeping broad all-operation generated circuits and noisy-flow checker semantics outside the claim.
Milestone-audit for this slice found the promoted scope complete against the current PFM5 text after replacing stale benchmark evidence with `target/benchmarks/rpf5-flow-generator-all-op-noop-probe/baseline.json` and `target/benchmarks/rpf5-flow-generator-all-op-noop-compare/compare.json`; no new under-specification was found.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment. The docs and benchmark sidecar found the same P2 stale benchmark-evidence issue and it was fixed by the fresh report-only probe; the Rust/API sidecar reported no confirmed findings and recorded residual risks that composed `SPP` tests compare against Stab decomposition plus the unsigned checker instead of independently pinning every signed Stim output, repeat-contained `SPP` still uses the existing materialized flattened-operation path, and `generators.rs` remains close to the 1200-line watch threshold.
The signed sampled flow-checking slice promotes a scoped Rust `sample_if_circuit_has_stabilizer_flows` helper with sign-sensitive unitary, measurement-record, record-backed observable, Pauli-backed observable, inverted Pauli-backed observable, inverted record-backed observable, malformed-record, and 256-shot effective-count rounding evidence. Milestone-audit found the promoted scope complete against the current PFM5 text with no new under-specification after keeping Python-style `has_flow` and `has_all_flows` binding parity, signed diagnostics, and throughput benchmarking outside this slice. Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment; the Rust/API sidecar found a P2 probabilistic-parity gap where Stab previously sampled the exact caller count instead of Stim's rounded word-width count, and the fix now rounds to 256 with `sampled_flow_counts_round_to_stim_word_width` coverage. The docs and benchmark sidecar reported no confirmed findings, and the residual PF5 oracle concern was closed by `just oracle::run --milestone PF5 --structural` passing with `pf5-signed-sampled-flows-rust`.
The inverted record-backed observable follow-up locks the pinned Stim v1.16.0 `sample_if_circuit_has_stabilizer_flows_inverted_obs_rec` subcase in `docs/plans/pfm5-signed-sampled-flow-inverted-record-observable-scope.md` and `sample_if_circuit_has_stabilizer_flows_checks_inverted_record_observables`.
Milestone-audit found the follow-up complete against the selected PF5 signed sampled-flow contract because it proves the exact upstream circuit, accepted signed flow, rejected opposite-sign flow, oracle row filter, no-benchmark rationale, and non-goals without expanding Python binding parity, exact random streams, diagnostics, or benchmark scope.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
Both sidecars reported no confirmed findings; residual risks are intentionally scoped to broader repeats, multiple record terms, multiple observable includes, Python bindings, and future flow-test file splitting as the test file is now on the large-file watch list.
The multi-target heralded flow-generator evidence slice is locked by `docs/plans/pfm5-flow-generators-multitarget-heralded-scope.md`. Milestone-audit found the selected single- and multi-target heralded-noise MPP cases complete against the narrowed scope after exact pinned Stim v1.16.0 generator strings, checker-satisfaction coverage for all three selected multi-target cases, oracle metadata, report-only benchmark corpus work units, and focused benchmark artifacts were refreshed. Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment; the docs sidecar found that checker-satisfaction coverage and the audit trail were initially narrower than the scope note, and the slice now covers all selected checker cases, names the new test in the implemented-test inventory, and logs broader heralded-noise generator synthesis as under-specified in `docs/plans/milestone-spec-gaps.md`.

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for broader all-operation composed measurement-rich circuits beyond the promoted pinned generated all-operations fixture, no-op annotation, ordinary-noise, tableau, measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, heralded-record, and `SPP` or `SPP_DAG` decomposition subcases, invalid or otherwise unsupported feedback shapes outside the selected pairwise contract, broader heralded-noise synthesis beyond the selected MPP cases which is under-specified in `docs/plans/milestone-spec-gaps.md`, folded repeat traversal beyond the current flow-row and materialized flattened-operation caps, noisy-flow checker semantics, and broader generated all-operation generator checks.
- General generator-table `solve_for_flow_measurements` no longer has a matrix-size or exhaustive-subset gap after PFM-B4; richer diagnostic wording and generator families outside the finite ledger require a new explicit plan.
- Broader measurement-rich `time_reversed_for_flows` and broader transform-integration checks beyond the selected single-instruction unique-target measurement group, selected `dont_turn_measurements_into_resets` single-measurement option, selected plain reset group over one or more unique qubit targets, selected measure-reset group over one or more unique qubit targets with inverted result-target support, selected empty-flow plus Pauli-only, measurement-record, and observable MPAD record-tail reversal, selected duplicate MPAD observable-id record parity, selected `MZZ` unitary-suffix packet, and exact pinned `flow_flip` packet, especially MPAD observable flow terms outside selected record-only tails and non-selected MPAD shapes.
- Python-style signed sampled `has_flow` and `has_all_flows` semantics remain absent until binding work chooses the exact API shape.
- Broader diagnostics for generator-table solving, signed sampled flow checking, and unpromoted flow-generator synthesis.
- Python binding ergonomics remain deferred.
