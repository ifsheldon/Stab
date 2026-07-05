# RPF5 Flow Progress Report

## Summary

This RPF5 report now covers the promoted unsigned `has_flow` and `has_all_flows` Rust helper subset for measurement-record and observable dependencies, the scoped `circuit_flow_generators` subset including pinned variable-target `SPP` and `SPP_DAG` unitary examples, Python multi-target measurement and `MPP` examples with idle-qubit identity rows, nonconstant and constant single-instruction `MPP`, selected unitary-mixed composed measurement-rich instruction sequences, bounded repeat-contained measurement-rich instruction sequences, and the pinned heralded-noise MPP fixture, the pinned Stim `solve_for_flow_measurements` empty, `MX`, measured-idle, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code examples, the scoped unitary `time_reversed_for_flows` transform bridge, and the selected single-instruction measurement-rich `time_reversed_for_flows` bridge for pinned `M`, `R`, and `MZZ` examples plus selected multi-record measurement-ordering evidence, selected plain unique-target `R`/`RX`/`RY` reset-to-measurement conversion, selected single-target `M`/`MX`/`MY` measurement-to-reset conversion, and plain unique-target `MR`/`MRX`/`MRY` measure-reset flow reversal.
It does not complete the flow milestone because broader all-operation composed measurement-rich flow-generator synthesis, broader heralded-noise generator synthesis, full generator-table measurement solving, signed sampled flow checking, failure explanations, broader measurement-rich `time_reversed_for_flows`, broader transform integration, and Python flow binding ergonomics remain open.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- `circuit_has_unsigned_stabilizer_flow` and `circuit_has_all_unsigned_stabilizer_flows` are additive Rust helpers over the same deterministic unsigned checker semantics and intentionally do not implement Stim's randomized signed Python `has_flow` or `has_all_flows` mode.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.
- `circuit_flow_generators` supports exact single-instruction generators for `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, nonconstant and constant `MPP`, Python multi-target `M`, `MX`, `MYY`, and `MPP` examples with idle-qubit identity rows, pinned variable-target `SPP` and `SPP_DAG` unitary examples through existing decomposition, and `MPAD`, plus selected unitary-mixed composed measurement-rich instruction sequences with tableau-backed plain-qubit Clifford gates, bounded repeat-contained measurement-rich instruction sequences through the 4096-row flow-generator cap and `Circuit::flattened_operations` materialization limit, the scoped measurement-record feedback examples `M; CX rec`, `MPP; CX rec`, `M; XCZ rec`, `M; CY rec`, and the pinned `HERALDED_ERASE`; `HERALDED_PAULI_CHANNEL_1`; `TICK`; `MPP` generator fixture.
- Unpromoted measurement-rich generator shapes such as duplicate measure-reset targets, unsupported sweep feedback, unsupported classical-control shapes, excessive flow-generator rows or repeat expansion beyond current caps, and broader heralded-noise composition fail closed with an explicit unsupported generator or resource-limit error.
- `solve_for_flow_measurements` is exposed as a Rust helper for the promoted unsigned scope, uses generator rows when the current `circuit_flow_generators` subset applies, and falls back to a bounded checker search for small composed measurement-rich circuits.
- The promoted solver scope covers empty input, simple `MX`, measured-idle identity and collapse flows, multi-target measurement and `MPP` products, fewer-measurements heuristic cases, repetition-code measurement solving, empty-Pauli query rejection, and fallback resource-limit hardening.
- `Circuit::time_reversed_for_flows` is exposed for the scoped unitary flow-transform subset, validates unsigned Pauli-only flows against the original unitary circuit with bounded tableau validation or folded sparse validation for supported large repeats including the owned `H`, `SQRT_X`, and `CY` regression cases, and supports idle flow qubits beyond the circuit width.
- The selected measurement-rich flow-transform subset validates flows through the sparse tracker and reverses flow endpoints for one noiseless plain unique-target `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, with pinned Stim `M` and `MZZ` evidence, source-owned basis coverage for `MX`, `MY`, `MXX`, and `MYY`, selected multi-record measurement-ordering evidence, selected plain unique-target `R`, `RX`, and `RY` reset-to-measurement conversion, and selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion when the full flow batch has record dependence but no future Pauli dependence on the measured qubit; the selected plain unique-target measure-reset subset covers `MR`, `MRX`, and `MRY` by mapping reset-effect output terms into reversed measurement records and measurement-record dependencies back into reset effects.

## Composed-Measurement Scope

The current PFM5 composed-measurement slice promotes composed measurement-rich flow generators for top-level or bounded repeat-contained instruction sequences handled by the existing reverse measurement-flow solver: measurement, reset, measure-reset, pair-measurement, Pauli-product measurement, `MPAD`, `TICK`, supported heralded record-producing instructions, supported measurement-record feedback gates, and selected tableau-backed unitary gates with plain qubit targets.
It explicitly keeps unsupported sweep feedback, unsupported classical-control shapes, excessive repeat expansion, and broad all-operation generated circuits fail-closed until their exact flow semantics and resource rules are specified.
Completion evidence for this slice includes exact generator tests for repeated same-qubit measurements across instructions, reset/measurement ordering, and selected unitary-mixed compositions, repeat-versus-expanded equivalence tests for bounded repeat-contained measurement-rich instruction sequences, generated-flow checker satisfaction for independent composed measurements and mixed composed measurement families, oracle metadata updates, refreshed `pf5-flow-generators-measurement-rich` benchmark corpus work units, milestone-audit, full-code-review, and targeted verification.

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
- `circuit_flow_generators_promotes_python_multi_target_examples`
- `circuit_flow_generators_promotes_spp_measurement_row_unitary_examples`
- `circuit_flow_generators_promotes_composed_measurement_subset`
- `circuit_flow_generators_promotes_unitary_mixed_measurement_subset`
- `circuit_flow_generators_promotes_bounded_repeat_measurement_subset`
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker`
- `circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes`
- `circuit_flow_generators_rejects_excessive_repeat_measurement_expansion`
- `circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products`
- `solve_for_flow_measurements_cpp_empty_and_simple_examples`
- `solve_for_flow_measurements_python_measured_idle_examples`
- `solve_for_flow_measurements_python_multi_target_examples`
- `solve_for_flow_measurements_python_fewer_measurements_heuristic_examples`
- `solve_for_flow_measurements_cpp_repetition_code_example`
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
- `time_reversed_for_flows_measurement_rich_subset_preserves_measurement_ordering`
- `time_reversed_for_flows_measurement_rich_subset_turns_measurements_into_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_measure_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_measure_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_resets`
- `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_resets`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unsatisfied_flows`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_terms`
- `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measurement_targets`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_reset_shapes`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unpromoted_measure_reset_shapes`
- `time_reversed_for_flows_measurement_rich_subset_rejects_unscoped_reset_terms`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, unsigned single-flow and all-flow helper success and failure cases, empty all-flow batches, folded-repeat measurement references, unsigned `SPP`/`SPP_DAG` product propagation with false identity-flow rejection, exact measurement, reset, pair-measurement, nonconstant and constant `MPP`, Python multi-target measurement and `MPP` flow-generator examples with idle-qubit identity rows, pinned variable-target `SPP` and `SPP_DAG` unitary flow-generator examples with anti-Hermitian rejection, selected unitary-mixed composed measurement-rich instruction sequences, bounded repeat-contained measurement-rich instruction sequences, feedback, `MPAD`, and promoted heralded-noise MPP generators, generated-flow satisfaction checks for the supported checker subset, pinned Stim measurement-solver examples for empty input, simple `MX`, measured-idle collapse, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code solving, scoped unitary flow time reversal, selected single-instruction measurement-rich flow time reversal for all promoted measurement bases with selected multi-record ordering, selected plain unique-target reset-to-measurement conversion for `R`, `RX`, and `RY`, selected single-target measurement-to-reset conversion for `M`, `MX`, and `MY`, selected plain unique-target measure-reset flow time reversal for `MR`, `MRX`, and `MRY`, duplicate measurement, reset, and measure-reset target rejection, unscoped reset observable-term and measurement-record-term rejection, and negative cases ported from pinned Stim v1.16.0 `has_flow`, `has_all_flows`, `circuit_flow_generators`, and `circuit_inverse_qec` tests.

## Oracle Rows

Implemented row:

- `pf5-has-flow-record-observable-rust`
- `pf5-has-all-flows-rust`
- `pf5-flow-generators-measurement-rust`
- `pf5-flow-generators-measurement-python-rust`
- `pf5-flow-solve-measurement-rust`
- `pf5-flow-solve-measurement-python-rust`
- `pf5-flow-solve-measurement-resource-rust`
- `pf2-time-reverse-flow-unitary-rust`
- `pf2-time-reverse-flow-measurement-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`
- `pf5-flow-generators-measurement-rich`
- `pf5-flow-generators-measurement-python`
- `pf5-flow-solve-measurement-rich`
- `pf5-flow-solve-measurement-python`
- `pf2-time-reverse-flow`
- `pf2-time-reverse-flow-measurement`

The row measures the promoted unsigned has-all-flow corpus through the Rust public batch helper while validating per-flow expected results through the vector checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
The generator row measures the promoted measurement, reset, pair-measurement, nonconstant and constant `MPP`, pinned variable-target `SPP` and `SPP_DAG` unitary, selected unitary-mixed composed measurement-rich, bounded repeat-contained measurement-rich, feedback, `MPAD`, and heralded-noise MPP generator corpus through the Rust public `circuit_flow_generators` helper.
It reports `stab_pf5_flow_generators_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_flows`, normalized as flows per second.
The refreshed corpus covers 36 cases and 120 generated flows per utility batch.
The current focused report-only probe used `target/benchmarks/rpf5-spp-flow-generator-probe/baseline.json` and `target/benchmarks/rpf5-spp-flow-generator-compare/compare.json`.
It measured `stab_pf5_flow_generators_measurement_cases` at `3.829e5 cases/s` and `stab_pf5_flow_generators_measurement_flows` at `1.279e6 flows/s`.
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
The `pf2-time-reverse-flow-measurement` row measures the selected single-instruction measurement-rich flow-transform bridge from the RPF2 side, now including selected measurement-ordering, selected plain unique-target reset-to-measurement, selected measurement-to-reset, and plain unique-target measure-reset cases, and remains report-only for the same reason.
The current focused report-only probe used `target/benchmarks/rpf2-time-reverse-flow-probe/baseline.json` and `target/benchmarks/rpf2-time-reverse-flow-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_unitary` at `4.097e5 flows/s`.
The focused selected measurement-rich time-reversal probe used `target/benchmarks/pf2-time-reverse-reset-multitarget-probe/baseline.json` and `target/benchmarks/pf2-time-reverse-reset-multitarget-compare/compare.json`.
It measured `stab_circuit_time_reversed_for_flows_measurement` at `1.093e6 flows/s`.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core --test circuit_flows solve_for_flow_measurements --quiet
cargo test -p stab-core --test circuit_flow_generators --quiet
cargo test -p stab-core --test circuit_flow_generators python_multi_target --quiet
cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5:: --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just oracle::run --implemented-only
just bench::smoke
just bench::baseline --only pf5-flow-generators-measurement-rich --out target/benchmarks/rpf5-spp-flow-generator-probe
just bench::compare --only pf5-flow-generators-measurement-rich --baseline target/benchmarks/rpf5-spp-flow-generator-probe/baseline.json --report target/benchmarks/rpf5-spp-flow-generator-compare
just bench::baseline --only pf5-flow-generators-measurement-python --out target/benchmarks/rpf5-flow-generator-python-probe
just bench::compare --only pf5-flow-generators-measurement-python --baseline target/benchmarks/rpf5-flow-generator-python-probe/baseline.json --report target/benchmarks/rpf5-flow-generator-python-compare
just bench::baseline --only pf5-flow-solve-measurement-rich --out target/benchmarks/rpf5-flow-solve-cpp-probe
just bench::compare --only pf5-flow-solve-measurement-rich --baseline target/benchmarks/rpf5-flow-solve-cpp-probe/baseline.json --report target/benchmarks/rpf5-flow-solve-cpp-compare
just bench::baseline --only pf5-flow-solve-measurement-python --out target/benchmarks/rpf5-flow-solve-python-probe
just bench::compare --only pf5-flow-solve-measurement-python --baseline target/benchmarks/rpf5-flow-solve-python-probe/baseline.json --report target/benchmarks/rpf5-flow-solve-python-compare
just bench::baseline --only pf2-time-reverse-flow-measurement --out target/benchmarks/pf2-time-reverse-reset-multitarget-probe
just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/pf2-time-reverse-reset-multitarget-probe/baseline.json --report target/benchmarks/pf2-time-reverse-reset-multitarget-compare
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
The selected measure-reset slice originally promoted only the single-target measure-reset shape plus selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion in the PF2-owned time-reversal row, refreshed the report-only benchmark corpus, and kept broader reset-only operations, duplicate or inverted measure-reset groups, detector, feedback, noise, repeat, and multi-instruction QEC inverse shapes open before this slice expanded it to plain unique-target groups.

The current selected reset-to-measurement and measurement-ordering slice promotes one noiseless plain unique-target `R`, `RX`, or `RY` instruction, selected multi-record `M` and `MZZ` measurement ordering, and plain unique-target `MR`, `MRX`, and `MRY` measure-reset flow reversal in the same PF2-owned time-reversal row, rejects duplicate measurement targets, duplicate reset targets, duplicate or inverted measure-reset targets, and unscoped observable or measurement-record terms for selected reset paths, refreshes the report-only benchmark corpus, and keeps duplicate reset-only operations, parser-rejected inverted reset targets, duplicate or inverted measure-reset groups, detector, feedback, noise, repeat, and multi-instruction QEC inverse shapes open.
Full-code-review for this PF2-owned measurement-ordering and unique-target measure-reset bridge used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found and the implementation fixed a P1 ordering bug where multi-record selected measurement and measure-reset inverses kept original target order instead of Stim-style reversed target groups, plus a P2 large-group duplicate-detection issue now addressed with set-backed uniqueness checks.
The docs/evidence sidecar reported no confirmed findings and confirmed this report stays scoped to the PF2-owned oracle row and report-only benchmark row.
The current multi-target reset follow-up used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar reported no confirmed findings.
The docs/evidence sidecar found an under-specified arity evidence question because the implementation accepts arbitrary plain unique-target reset groups while the initial new positive test only exercised two targets.
The PF2-owned test now also covers a three-target `R 0 1 2` reset reversal, closing that evidence gap without expanding beyond the selected single-instruction reset scope.
Milestone-audit for the bounded repeat-contained flow-generator slice found the promoted scope complete against the current PFM5 text: it promotes repeat-contained measurement-rich instruction sequences only through the existing flattened-operation cap plus a new 4096-row flow-generator cap, has repeat-versus-expanded equivalence tests, has a compact-repeat resource rejection test, updates oracle and benchmark metadata, and keeps full folded repeat traversal open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P1 resource issue where compact repeats at the generic flattened-operation limit could still enter the expensive flow-row canonicalization path; the implementation now validates measurement-rich flow-generator rows before flattening or allocating solver rows, and the regression uses `REPEAT 1000000 { M 0 }`.
The docs and benchmark reviewer for the earlier repeat-contained slice found a stale verification-command path for that refreshed benchmark probe; the report was corrected in that slice before the later unitary-mixed probe replaced the current benchmark artifact names.
Milestone-audit for the selected unitary-mixed flow-generator slice found the promoted scope complete against the current PFM5 text: it promotes tableau-backed plain-qubit unitary groups inside composed measurement-rich flow generators, keeps unsupported sweep and mixed classical-control shapes fail-closed, updates oracle and benchmark metadata, refreshes report-only benchmark evidence, and keeps broader all-operation generator synthesis open.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
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
Milestone-audit for the current solver-scope slice found the promoted scope complete against the current PFM5 text: exact pinned examples are tested, oracle rows run source-specific filters, benchmark rows separate C++ and Python provenance, docs keep full generator-table solving open, and no new under-specification was found.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API reviewer found a P3 benchmark-work guard gap where the new test asserted hardcoded work units instead of deriving expected work from the actual corpus; `expected_flow_solve_measurement_work_for_test` now derives those counts from the selected row corpus and compares them to `measurement_work`.
The docs and benchmark reviewer found P2 provenance and reproducibility gaps: Python-derived solver coverage was machine-recorded as C++-only, and the verification block initially omitted the focused Python solver benchmark commands. The oracle and benchmark rows are now split by upstream source, the PFM5 source list includes `circuit_flow_generators_test.py`, and the verification block names the focused Python solver benchmark commands.
The current Python multi-target generator slice promotes pinned Python `flow_generators()` examples for offset `M`, `MX`, `MYY`, and `MPP` targets. It fixes single-qubit measurement flow generation to retain idle-qubit identity rows and canonicalize through the same row-elimination path used by pair-measurement and Pauli-product generators, while keeping broader all-operation generator synthesis open.

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for broader all-operation composed measurement-rich circuits, unsupported feedback shapes, broader heralded-noise synthesis, folded repeat traversal beyond the current flow-row and materialized flattened-operation caps, and all-operation generator checks.
- Full generator-table `solve_for_flow_measurements` parity for larger composed measurement-rich circuits and richer measurement-set diagnostics.
- Broader measurement-rich `time_reversed_for_flows` and broader transform-integration checks beyond the selected single-instruction unique-target measurement group, selected plain unique-target reset group, and selected plain unique-target measure-reset group.
- Signed sampled `has_flow` and `has_all_flows` semantics remain absent until a Rust API plan chooses whether to expose randomized evidence.
- Flow failure explanations beyond boolean unsigned checking.
- Python binding ergonomics remain deferred.
