# RPF5 Flow Progress Report

## Summary

This RPF5 report now covers the promoted unsigned `has_flow` subset for measurement-record and observable dependencies, the scoped measurement-rich `circuit_flow_generators` subset including nonconstant and constant single-instruction `MPP` plus the pinned heralded-noise MPP fixture, and the pinned Stim `solve_for_flow_measurements` empty, `MX`, idle-extra-qubit, and repetition-code examples.
It does not complete the flow milestone because broader composed measurement-rich flow-generator synthesis, broader heralded-noise generator synthesis, full generator-table measurement solving, failure explanations, `time_reversed_for_flows`, transform integration, and Python flow binding ergonomics remain open.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.
- `circuit_flow_generators` supports exact single-instruction generators for `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, nonconstant and constant `MPP`, and `MPAD`, plus the scoped measurement-record feedback examples `M; CX rec`, `MPP; CX rec`, `M; XCZ rec`, `M; CY rec`, and the pinned `HERALDED_ERASE`; `HERALDED_PAULI_CHANNEL_1`; `TICK`; `MPP` generator fixture.
- Unpromoted measurement-rich generator shapes such as duplicate measure-reset targets, unsupported sweep feedback, mixed measured/unitary instruction sequences, repeat-contained measurements, and broader heralded-noise composition fail closed with an explicit unsupported generator error.
- `solve_for_flow_measurements` is exposed as a Rust helper for the promoted unsigned scope, uses generator rows when the current `circuit_flow_generators` subset applies, and falls back to a bounded checker search for small composed measurement-rich circuits.
- The promoted solver scope covers empty input, simple `MX`, idle extra-qubit identity flows, repetition-code measurement solving, empty-Pauli query rejection, and fallback resource-limit hardening.

## Tests

Implemented Rust tests:

- `check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies`
- `circuit_flow_generators_promotes_single_instruction_measurement_subset`
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker`
- `circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes`
- `circuit_flow_generators_measurement_subset_rejects_anti_hermitian_mpp_products`
- `solve_for_flow_measurements_matches_stim_empty_and_simple_examples`
- `solve_for_flow_measurements_matches_stim_repetition_code_example`
- `solve_for_flow_measurements_has_documented_fallback_resource_limit`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, exact measurement, reset, pair-measurement, nonconstant and constant `MPP`, feedback, `MPAD`, and promoted heralded-noise MPP generators, generated-flow satisfaction checks for the supported checker subset, pinned Stim measurement-solver examples, and negative cases ported from pinned Stim v1.16.0 `has_flow` and `circuit_flow_generators` tests.

## Oracle Rows

Implemented row:

- `pf5-has-flow-record-observable-rust`
- `pf5-flow-generators-measurement-rust`
- `pf5-flow-solve-measurement-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`
- `pf5-flow-generators-measurement-rich`
- `pf5-flow-solve-measurement-rich`

The row measures the promoted unsigned has-flow corpus through the Rust public flow checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
The generator row measures the promoted measurement, reset, pair-measurement, nonconstant and constant `MPP`, feedback, `MPAD`, and heralded-noise MPP generator corpus through the Rust public `circuit_flow_generators` helper.
It reports `stab_pf5_flow_generators_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_flows`, normalized as flows per second.
The current generator benchmark corpus contains 21 circuits and 77 expected flows.
The current focused report-only probe used `target/benchmarks/rpf5-heralded-flow-generator-probe/baseline.json` and `target/benchmarks/rpf5-heralded-flow-generator-compare/compare.json`.
It measured `stab_pf5_flow_generators_measurement_cases` at `5.545e5 cases/s` and `stab_pf5_flow_generators_measurement_flows` at `2.032e6 flows/s`.
The solver row measures the promoted `solve_for_flow_measurements` corpus through the Rust public helper.
It reports `stab_pf5_flow_solve_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_solve_measurement_queries`, normalized as queries per second.
These rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface, and they are not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core --test circuit_flow_generators --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
just bench::baseline --only pf5-flow-generators-measurement-rich --out target/benchmarks/rpf5-heralded-flow-generator-probe
just bench::compare --only pf5-flow-generators-measurement-rich --baseline target/benchmarks/rpf5-heralded-flow-generator-probe/baseline.json --report target/benchmarks/rpf5-heralded-flow-generator-compare
```

## Audit And Review

Milestone-audit for the heralded-noise MPP generator slice found no implementation or specification blockers.
The promoted scope is the pinned Stim v1.16.0 `HERALDED_ERASE`; `HERALDED_PAULI_CHANNEL_1`; `TICK`; `MPP` fixture plus generated-flow checker satisfaction, while broader heralded-noise synthesis remains active follow-up work.
Full-code-review used GPT-5.5/xhigh sidecars for Rust compatibility and docs or benchmark alignment.
The Rust compatibility reviewer found no confirmed issues and confirmed the heralded record-generator path matches pinned Stim v1.16.0 behavior.
The docs and benchmark reviewer found one stale M6 oracle-manifest note that still said measurement-rich generator flows remained deferred; that note now points to the PF5 evidence row and keeps only broader composed or noise-derived synthesis as follow-up work.
`crates/stab-core/src/circuit_flow.rs` is now on the large-file watch list at 1169 lines, below the 1200-line finding threshold but close enough that the next flow-generator expansion should split generator ownership into a submodule before adding much more code.

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for broader composed measurement-rich circuits, unsupported feedback shapes, broader heralded-noise synthesis, and all-operation generator checks.
- Full generator-table `solve_for_flow_measurements` parity for larger composed measurement-rich circuits and richer measurement-set diagnostics.
- `time_reversed_for_flows` and transform-integration checks.
- Flow failure explanations beyond boolean unsigned checking.
- Python binding ergonomics remain deferred.
