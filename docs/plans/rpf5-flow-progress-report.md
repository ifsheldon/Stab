# RPF5 Flow Progress Report

## Summary

This RPF5 slice promotes a real unsigned `has_flow` subset for measurement-record and observable dependencies.
It does not complete the flow milestone because flow-generator synthesis for measurement-rich circuits, solve-for-flow-measurements, failure explanations, `time_reversed_for_flows`, transform integration, variable-target gate flow metadata, and Python flow binding ergonomics remain open.

## Implemented Surfaces

- `check_if_circuit_has_unsigned_stabilizer_flows` still uses tableau comparison for deterministic unitary flows when available.
- For circuits with measurement or observable dependencies, it now uses the sparse reverse tracker to map final Pauli, `rec[...]`, and `obs[...]` terms back to initial Pauli regions.
- Both absolute `rec[0]` and relative `rec[-1]` flow references are supported for the promoted checker subset.
- Sign differences are intentionally ignored, matching the unsigned checker contract.
- Unsupported sparse-tracker shapes fail closed as `false` for individual flows instead of being claimed as full flow parity.

## Tests

Implemented Rust tests:

- `check_if_circuit_has_unsigned_stabilizer_flows_supports_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_pair_measurement_records`
- `check_if_circuit_has_unsigned_stabilizer_flows_supports_observable_dependencies`

These tests cover measurement-record dependencies, pair-measurement records, observable dependencies from records and Pauli targets, sign-insensitive matching, and negative cases ported from pinned Stim v1.16.0 `has_flow` checker tests.

## Oracle Rows

Implemented row:

- `pf5-has-flow-record-observable-rust`

Still broad and manifest-only:

- `pf5-measurement-rich-flows`

## Benchmark Rows

Report-only runner coverage:

- `pf5-has-all-flows-batch`

The row measures the promoted unsigned has-flow corpus through the Rust public flow checker.
It reports `stab_pf5_has_flows_batch_cases`, normalized as cases per second, and `stab_pf5_has_flows_batch_flows`, normalized as flows per second.
It remains `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface, and it is not part of the 1.25x primary threshold file.

Still placeholder:

- `pf5-flow-solve-measurement-rich`

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test circuit_flows --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench pf5_detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
```

## Remaining RPF5 Flow Work

- `circuit_flow_generators` for measurement-rich circuits, including reset, pair-measurement, MPP, MPAD, feedback, heralded-noise, and all-operation generator checks.
- `solve_for_flow_measurements` and associated measurement-set diagnostics.
- `time_reversed_for_flows` and transform-integration checks.
- Variable-target or measurement-rich gate flow metadata decisions.
- Flow failure explanations beyond boolean unsigned checking.
- Python binding ergonomics remain deferred.
