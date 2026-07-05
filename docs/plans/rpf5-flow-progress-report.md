# RPF5 Flow Progress Report

## Summary

This RPF5 report now covers the promoted unsigned `has_flow` and `has_all_flows` Rust helper subset for measurement-record and observable dependencies, the scoped measurement-rich `circuit_flow_generators` subset including nonconstant and constant single-instruction `MPP`, composed measurement-rich instruction sequences without ordinary unitary mixing, and the pinned heralded-noise MPP fixture, the pinned Stim `solve_for_flow_measurements` empty, `MX`, idle-extra-qubit, and repetition-code examples, the scoped unitary `time_reversed_for_flows` transform bridge, and the selected single-instruction measurement-rich `time_reversed_for_flows` bridge for pinned `M` and `MZZ` examples.
It does not complete the flow milestone because broader all-operation composed measurement-rich flow-generator synthesis, broader heralded-noise generator synthesis, full generator-table measurement solving, signed sampled flow checking, failure explanations, broader measurement-rich `time_reversed_for_flows`, broader transform integration, and Python flow binding ergonomics remain open.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- `circuit_has_unsigned_stabilizer_flow` and `circuit_has_all_unsigned_stabilizer_flows` are additive Rust helpers over the same deterministic unsigned checker semantics and intentionally do not implement Stim's randomized signed Python `has_flow` or `has_all_flows` mode.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.
- `circuit_flow_generators` supports exact single-instruction generators for `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, nonconstant and constant `MPP`, and `MPAD`, plus composed measurement-rich instruction sequences without ordinary unitary mixing, the scoped measurement-record feedback examples `M; CX rec`, `MPP; CX rec`, `M; XCZ rec`, `M; CY rec`, and the pinned `HERALDED_ERASE`; `HERALDED_PAULI_CHANNEL_1`; `TICK`; `MPP` generator fixture.
- Unpromoted measurement-rich generator shapes such as duplicate measure-reset targets, unsupported sweep feedback, mixed measured/unitary instruction sequences, repeat-contained measurements, and broader heralded-noise composition fail closed with an explicit unsupported generator error.
- `solve_for_flow_measurements` is exposed as a Rust helper for the promoted unsigned scope, uses generator rows when the current `circuit_flow_generators` subset applies, and falls back to a bounded checker search for small composed measurement-rich circuits.
- The promoted solver scope covers empty input, simple `MX`, idle extra-qubit identity flows, repetition-code measurement solving, empty-Pauli query rejection, and fallback resource-limit hardening.
- `Circuit::time_reversed_for_flows` is exposed for the scoped unitary flow-transform subset, validates unsigned Pauli-only flows against the original unitary circuit with bounded tableau validation or folded sparse validation for supported large repeats including the owned `H`, `SQRT_X`, and `CY` regression cases, and supports idle flow qubits beyond the circuit width.
- The selected measurement-rich flow-transform subset validates flows through the sparse tracker and reverses flow endpoints while preserving record and observable terms for one noiseless plain `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, with pinned Stim `M` and `MZZ` evidence plus source-owned basis coverage for `MX`, `MY`, `MXX`, and `MYY`; larger QEC inverse shapes still fail closed.

## Composed-Measurement Scope

The current PFM5 composed-measurement slice promotes composed measurement-rich flow generators only for top-level instruction sequences handled by the existing reverse measurement-flow solver: measurement, reset, measure-reset, pair-measurement, Pauli-product measurement, `MPAD`, `TICK`, supported heralded record-producing instructions, and supported measurement-record feedback gates.
It explicitly keeps ordinary mixed unitary instructions, sweep feedback, repeat-contained measurements, unsupported classical-control shapes, and broad all-operation generated circuits fail-closed until their exact flow semantics and resource rules are specified.
Completion evidence for this slice includes exact generator tests for repeated same-qubit measurements across instructions and reset/measurement ordering, generated-flow checker satisfaction for independent composed measurements and mixed composed measurement families, oracle metadata updates, refreshed `pf5-flow-generators-measurement-rich` benchmark corpus work units, milestone-audit, full-code-review, and targeted verification.

## Unsigned Has-All Scope

The current PFM5 has-all slice promotes only deterministic unsigned Rust helpers over the existing supported flow checker: `circuit_has_unsigned_stabilizer_flow` for one flow and `circuit_has_all_unsigned_stabilizer_flows` for an empty or non-empty batch.
Owned positive and negative subcases are unitary sign-insensitive flows, false unitary flows, unsigned `SPP`/`SPP_DAG` product propagation, anti-Hermitian `SPP` failure, measurement-record and observable dependencies, and folded-repeat measurement references.
The helpers return booleans and deliberately preserve the existing fail-closed checker behavior for sparse-tracker shapes that remain unsupported outside the promoted unsigned path.
They do not expose Stim's signed randomized `has_flow` or `has_all_flows` behavior and do not claim Python binding parity.
The report-only `pf5-has-all-flows-batch` benchmark now calls the public batch helper while still validating the vector checker's per-flow expected results inside the benchmark closure.

## Tests

Implemented Rust tests:

- `check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies`
- `circuit_has_unsigned_stabilizer_flow_helpers_match_supported_batch_semantics`
- `pf6_sparse_rev_spp_circuit_has_unsigned_stabilizer_flow_helpers_support_unsigned_semantics`
- `circuit_flow_generators_promotes_single_instruction_measurement_subset`
- `circuit_flow_generators_promotes_composed_measurement_subset`
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker`
- `circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes`
- `circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products`
- `solve_for_flow_measurements_matches_stim_empty_and_simple_examples`
- `solve_for_flow_measurements_matches_stim_repetition_code_example`
- `solve_for_flow_measurements_has_documented_fallback_resource_limit`
- `time_reversed_for_flows_unitary_subset_matches_qec_inverse`
- `time_reversed_for_flows_unitary_subset_supports_flow_past_end`
- `time_reversed_for_flows_unitary_subset_supports_extra_idle_qubits`
- `time_reversed_for_flows_unitary_subset_folds_large_repeats`
- `time_reversed_for_flows_unitary_subset_folds_large_cy_repeats`
- `time_reversed_for_flows_unitary_subset_rejects_unsatisfied_flow`
- `time_reversed_for_flows_measurement_rich_subset_reverses_single_measurement`
- `time_reversed_for_flows_measurement_rich_subset_reverses_pair_measurement`
- `time_reversed_for_flows_measurement_rich_subset_covers_selected_bases`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unsatisfied_flows`
- `time_reversed_for_flows_rejects_unpromoted_measurement_rich_terms`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, unsigned single-flow and all-flow helper success and failure cases, empty all-flow batches, folded-repeat measurement references, unsigned `SPP`/`SPP_DAG` product propagation with false identity-flow rejection, exact measurement, reset, pair-measurement, nonconstant and constant `MPP`, composed measurement-rich instruction sequences without ordinary unitary mixing, feedback, `MPAD`, and promoted heralded-noise MPP generators, generated-flow satisfaction checks for the supported checker subset, pinned Stim measurement-solver examples, scoped unitary flow time reversal, selected single-instruction measurement-rich flow time reversal for all promoted measurement bases, and negative cases ported from pinned Stim v1.16.0 `has_flow`, `has_all_flows`, `circuit_flow_generators`, and `circuit_inverse_qec` tests.

## Oracle Rows

Implemented row:

- `pf5-has-flow-record-observable-rust`
- `pf5-has-all-flows-rust`
- `pf5-flow-generators-measurement-rust`
- `pf5-flow-solve-measurement-rust`
- `pf2-time-reverse-flow-unitary-rust`
- `pf2-time-reverse-flow-measurement-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`
- `pf5-flow-generators-measurement-rich`
- `pf5-flow-solve-measurement-rich`
- `pf2-time-reverse-flow`
- `pf2-time-reverse-flow-measurement`

The row measures the promoted unsigned has-all-flow corpus through the Rust public batch helper while validating per-flow expected results through the vector checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
The generator row measures the promoted measurement, reset, pair-measurement, nonconstant and constant `MPP`, composed measurement-rich, feedback, `MPAD`, and heralded-noise MPP generator corpus through the Rust public `circuit_flow_generators` helper.
It reports `stab_pf5_flow_generators_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_flows`, normalized as flows per second.
The refreshed corpus covers 27 cases and 100 generated flows per utility batch.
The current focused report-only probe used `target/benchmarks/rpf5-composed-flow-generator-probe/baseline.json` and `target/benchmarks/rpf5-composed-flow-generator-compare/compare.json`.
It measured `stab_pf5_flow_generators_measurement_cases` at `5.590e5 cases/s` and `stab_pf5_flow_generators_measurement_flows` at `2.056e6 flows/s`.
The solver row measures the promoted `solve_for_flow_measurements` corpus through the Rust public helper.
It reports `stab_pf5_flow_solve_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_solve_measurement_queries`, normalized as queries per second.
These rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface, and they are not part of the 1.25x primary threshold file.
The `pf2-time-reverse-flow` row measures the scoped unitary flow-transform bridge from the RPF2 side and remains report-only for the same reason.
The `pf2-time-reverse-flow-measurement` row measures the selected single-instruction measurement-rich flow-transform bridge from the RPF2 side and remains report-only for the same reason.
The current focused report-only probe used `target/benchmarks/rpf2-time-reverse-flow-probe/baseline.json` and `target/benchmarks/rpf2-time-reverse-flow-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_unitary` at `4.097e5 flows/s`.
The focused selected measurement-rich time-reversal probe used `target/benchmarks/rpf2-time-reverse-flow-measurement-probe/baseline.json` and `target/benchmarks/rpf2-time-reverse-flow-measurement-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_measurement` at `1.079e6 flows/s`.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core --test circuit_flow_generators --quiet
cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
just bench::baseline --only pf5-flow-generators-measurement-rich --out target/benchmarks/rpf5-composed-flow-generator-probe
just bench::compare --only pf5-flow-generators-measurement-rich --baseline target/benchmarks/rpf5-composed-flow-generator-probe/baseline.json --report target/benchmarks/rpf5-composed-flow-generator-compare
just bench::baseline --only pf2-time-reverse-flow-measurement --out target/benchmarks/rpf2-time-reverse-flow-measurement-probe
just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/rpf2-time-reverse-flow-measurement-probe/baseline.json --report target/benchmarks/rpf2-time-reverse-flow-measurement-compare
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
`crates/stab-core/src/circuit_flow.rs` remains on the large-file watch list below the 1200-line finding threshold, so the next flow-generator expansion should split generator ownership into a submodule before adding much more code.

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for broader all-operation composed measurement-rich circuits, unsupported feedback shapes, broader heralded-noise synthesis, and all-operation generator checks.
- Full generator-table `solve_for_flow_measurements` parity for larger composed measurement-rich circuits and richer measurement-set diagnostics.
- Broader measurement-rich `time_reversed_for_flows` and broader transform-integration checks beyond the selected single-instruction measurement group.
- Signed sampled `has_flow` and `has_all_flows` semantics remain absent until a Rust API plan chooses whether to expose randomized evidence.
- Flow failure explanations beyond boolean unsigned checking.
- Python binding ergonomics remain deferred.
