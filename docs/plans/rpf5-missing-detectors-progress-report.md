# RPF5 Missing Detectors Progress Report

## Summary

This RPF5 slice promotes the Rust `missing_detectors` utility beyond the M9 basic reset and single-record subset.
It adds Gaussian row reduction over detector and observable rows plus a scoped internal stabilizer-invariant tracker for deterministic reset, measurement, MPP, and pair-measurement cases.
It also promotes tableau-backed single-qubit and fixed two-qubit Clifford propagation for plain qubit target groups.
It is not an RPF5 completion report because broader generated-code workloads, repeat traversal, public measurement-rich flow solving, and transform integration remain active work.

## Implemented Surfaces

- Existing `DETECTOR` rows now participate in Gaussian elimination instead of being limited to single-record coverage.
- Repeated deterministic MPP and pair-measurement stabilizer-product measurements produce missing-detector suggestions compatible with the pinned Stim v1.16.0 subcases ported in this slice.
- Record-only `OBSERVABLE_INCLUDE` rows participate as known rows.
- `OBSERVABLE_INCLUDE` rows with Pauli targets mark that observable row ignored, matching the pinned Stim behavior used by the promoted tests.
- The pinned Stim big honeycomb-code and toric global-stabilizer generated-code suffix cases are promoted under unknown-input semantics.
- Tableau-backed single-qubit Clifford gates, fixed two-qubit Clifford gates, and SWAP-family gates propagate tracked invariants when their target groups are plain qubit targets.
- Ordinary noise gates are ignored by this diagnostic utility for the promoted cases, while repeat blocks, unsupported gates, and non-plain unitary target groups still fail closed.

## Tests

Implemented Rust tests:

- `missing_detectors_reduces_multi_record_detector_rows`
- `missing_detectors_supports_mpp_stabilizer_products`
- `missing_detectors_supports_observable_interactions`
- `missing_detectors_supports_honeycomb_generated_code_suffix`
- `missing_detectors_supports_toric_global_stabilizer_product`
- `missing_detectors_rejects_unpromoted_control_flow`
- `pf5_missing_detectors_clifford_tracks_single_qubit_basis_changes`
- `pf5_missing_detectors_clifford_covers_all_single_qubit_cliffords`
- `pf5_missing_detectors_clifford_tracks_two_qubit_and_swap_gates`
- `pf5_missing_detectors_clifford_covers_all_fixed_two_qubit_tableau_gates`
- `pf5_missing_detectors_clifford_rejects_non_plain_unitary_targets`

These tests cover Gaussian cleanup for multi-record detector rows, repeated MPP stabilizer-product constraints, unknown-input behavior, record-only observable rows, ignored Pauli observable rows, the promoted honeycomb and toric generated-code suffixes, all single-qubit Clifford gates, every canonical fixed two-qubit tableau gate, hand-pinned non-self-inverse `S`, signed `ISWAP_DAG`, and nondeterministic post-Clifford measurement cases, and fail-closed behavior for repeat blocks and non-plain unitary targets.

## Oracle Rows

Implemented rows:

- `pf5-missing-detectors-row-reduction-rust`
- `pf5-missing-detectors-mpp-observable-rust`
- `pf5-missing-detectors-generated-honeycomb-rust`
- `pf5-missing-detectors-generated-toric-rust`
- `pf5-missing-detectors-clifford-rust`

Still broad and manifest-only:

- `pf5-missing-detectors-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-missing-detectors-mpp`
- `pf5-missing-detectors-generated-code`

The row measures the promoted MPP and observable-row workload through the Rust public utility API.
The generated-code row measures the promoted honeycomb and toric generated-code suffix workloads through the Rust public utility API.
Both rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
They are not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core --test missing_detectors --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_clifford --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
just bench::compare --milestone PF5
```

## Remaining RPF5 Work

- Broader generated-code missing-detector suffix analysis beyond the promoted honeycomb and toric cases.
- Repeat traversal and any generated-code gate families beyond tableau-backed single-qubit and fixed two-qubit Clifford propagation in the invariant tracker.
- Public measurement-rich flow semantics beyond the promoted unsigned `has_flow`, unsigned `has_all_flows` Rust helper, and current generator subsets, including signed sampled checks, broader composed `flow_generators`, diagnostics, and transform integration.
- Continue keeping benchmark harness smoke tests split out of `ops/bench/src/baseline/tests.rs`, because the file is close to the project’s 1200-line threshold.
