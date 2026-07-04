# RPF5 Missing Detectors Progress Report

## Summary

This RPF5 slice promotes the Rust `missing_detectors` utility beyond the M9 basic reset and single-record subset.
It adds Gaussian row reduction over detector and observable rows plus a scoped internal stabilizer-invariant tracker for deterministic reset, measurement, MPP, and pair-measurement cases.
It is not an RPF5 completion report because honeycomb suffixes, toric suffixes, generated-code workloads, broader gate support, public measurement-rich flow solving, and transform integration remain active work.

## Implemented Surfaces

- Existing `DETECTOR` rows now participate in Gaussian elimination instead of being limited to single-record coverage.
- Repeated deterministic MPP and pair-measurement stabilizer-product measurements produce missing-detector suggestions compatible with the pinned Stim v1.16.0 subcases ported in this slice.
- Record-only `OBSERVABLE_INCLUDE` rows participate as known rows.
- `OBSERVABLE_INCLUDE` rows with Pauli targets mark that observable row ignored, matching the pinned Stim behavior used by the promoted tests.
- Ordinary noise gates are ignored by this diagnostic utility for the promoted cases, while repeat blocks and unsupported gates still fail closed.

## Tests

Implemented Rust tests:

- `missing_detectors_reduces_multi_record_detector_rows`
- `missing_detectors_supports_mpp_stabilizer_products`
- `missing_detectors_supports_observable_interactions`
- `missing_detectors_rejects_unpromoted_control_flow_and_cliffords`

These tests cover Gaussian cleanup for multi-record detector rows, repeated MPP stabilizer-product constraints, unknown-input behavior, record-only observable rows, ignored Pauli observable rows, and fail-closed behavior for repeat blocks and unsupported Clifford gates.

## Oracle Rows

Implemented rows:

- `pf5-missing-detectors-row-reduction-rust`
- `pf5-missing-detectors-mpp-observable-rust`

Still broad and manifest-only:

- `pf5-missing-detectors-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-missing-detectors-mpp`

The row measures the promoted MPP and observable-row workload through the Rust public utility API.
It remains `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-bench pf5_detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
```

## Remaining RPF5 Work

- Generated-code missing-detector suffix analysis for honeycomb and toric cases.
- Broader gate support in the invariant tracker if generated-code cases require it.
- Public measurement-rich flow semantics beyond the promoted unsigned `has_flow` and single-instruction generator subsets, including `has_all_flows`, composed `flow_generators`, diagnostics, and transform integration.
- A split of `ops/bench/src/baseline/tests.rs` before adding more benchmark harness cases, because the file is now close to the project’s 1200-line threshold.
