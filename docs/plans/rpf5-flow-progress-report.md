# RPF5 Flow Progress Report

## Summary

This RPF5 report now covers the promoted unsigned `has_flow` subset for measurement-record and observable dependencies plus the scoped measurement-rich `circuit_flow_generators` subset.
It does not complete the flow milestone because broader composed measurement-rich flow-generator synthesis, MPP generators, solve-for-flow-measurements, failure explanations, `time_reversed_for_flows`, transform integration, variable-target gate flow metadata, and Python flow binding ergonomics remain open.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.
- `circuit_flow_generators` supports exact single-instruction generators for `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, and `MPAD`, plus the scoped measurement-record feedback examples `M; CX rec`, `M; XCZ rec`, and `M; CY rec`.
- Unpromoted measurement-rich generator shapes such as MPP, duplicate measure-reset targets, unsupported sweep feedback, mixed measured/unitary instruction sequences, and repeat-contained measurements fail closed with an explicit unsupported generator error.

## Tests

Implemented Rust tests:

- `check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies`
- `circuit_flow_generators_promotes_single_instruction_measurement_subset`
- `circuit_flow_generators_measurement_subset_flows_satisfy_checker`
- `circuit_flow_generators_rejects_unpromoted_measurement_rich_shapes`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, exact measurement/reset/pair-measurement/feedback/MPAD generators, generated-flow satisfaction checks for the supported checker subset, and negative cases ported from pinned Stim v1.16.0 `has_flow` and `circuit_flow_generators` tests.

## Oracle Rows

Implemented row:

- `pf5-has-flow-record-observable-rust`
- `pf5-flow-generators-measurement-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`
- `pf5-flow-generators-measurement-rich`

The row measures the promoted unsigned has-flow corpus through the Rust public flow checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
The generator row measures the promoted measurement/reset/pair-measurement/feedback/MPAD generator corpus through the Rust public `circuit_flow_generators` helper.
It reports `stab_pf5_flow_generators_measurement_cases`, normalized as cases per second, and `stab_pf5_flow_generators_measurement_flows`, normalized as flows per second.
Both rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface, and they are not part of the 1.25x primary threshold file.

Still placeholder:

- `pf5-flow-solve-measurement-rich`

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core --test circuit_flow_generators --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5_detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
```

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for broader composed measurement-rich circuits, MPP, unsupported feedback shapes, heralded-noise, and all-operation generator checks.
- `solve_for_flow_measurements` and associated measurement-set diagnostics.
- `time_reversed_for_flows` and transform-integration checks.
- Variable-target or measurement-rich gate flow metadata decisions.
- Flow failure explanations beyond boolean unsigned checking.
- Python binding ergonomics remain deferred.
