# RPF5 Detecting Regions Progress Report

## Summary

This RPF5 report covers bounded repeat traversal and additive detector or logical-observable target filters in the Rust `circuit_detecting_regions` utility for the currently supported gate subset.
The target-filter slice adds a `DemTarget`-based detecting-region API that can query detector and logical-observable targets, default-like helpers for all detector and logical-observable targets and all ticks, and the pinned Stim `MX` and `MZZ` detecting-region examples.
It is not an RPF5 completion report because broader Clifford gates, target shapes beyond the promoted measurement families, multi-detector generated-code cases, anticommutation ignored mode, gauge behavior, missing-detector families, and measurement-rich flow-transform integration remain active work.

## Implemented Surfaces

- `circuit_detecting_regions` now validates supported instructions recursively through repeat blocks instead of rejecting every repeat block.
- Detector and tick counts are computed through repeat blocks with checked arithmetic.
- Reverse traversal snapshots repeat bodies in reverse execution order, preserving global detector, measurement-record, and tick numbering for bounded repeat workloads.
- Detecting-region extraction rejects excessive repeat expansion before unbounded unrolling.
- `circuit_detecting_regions_for_targets` returns detecting regions keyed by `DemTarget` and supports detector and logical-observable target filters while preserving the original detector-id `circuit_detecting_regions` API as a wrapper.
- `all_detecting_region_targets` returns the currently declared detector and logical-observable targets within the dense helper materialization cap, and `all_detecting_region_ticks` returns all tick indices within the documented helper cap.
- The supported validation set now includes `R`/`RX`/`RY`, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, `H`, `CX`, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.

## Target-Filter Scope

The target-filter slice promotes a new Rust API that returns regions keyed by `DemTarget` instead of only `DemDetectorId`.
The owned positive subcases are detector targets, logical-observable targets from measurement records or Pauli targets, duplicate target deduplication, default all-detector/all-observable target selection, `M`/`MX`/`MY`, `MXX`/`MYY`/`MZZ`, `H`, `CX`, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`.
The owned negative subcases are out-of-range detector targets, out-of-range observable targets, separator or numeric DEM targets, dense all-target helper requests beyond the materialization cap or representable logical-observable target range, unsupported gates, feedback or sweep-controlled targets, excessive repeat expansion, and unsupported ignored-anticommutation mode.
The comparator class is structural Rust API parity against pinned Stim v1.16.0 Python examples from `circuit_pybind_test.py` and utility failure examples from `circuit_to_detecting_regions_test.py`.
The existing `circuit_detecting_regions` detector-id API remains as a compatibility wrapper and keeps its current output type.

## Tests

Implemented Rust tests:

- `detecting_regions_repeat_supports_bounded_ticks`
- `detecting_regions_repeat_rejects_excessive_expansion`
- `detecting_regions_target_api_matches_mx_python_example`
- `detecting_regions_target_api_supports_mzz_example`
- `detecting_regions_target_api_supports_logical_observable_targets`
- `detecting_regions_target_api_rejects_invalid_targets`
- `detecting_regions_target_api_rejects_dense_helper_expansion`

These tests cover bounded repeat tick traversal, expected tick-indexed detecting regions, resource rejection for repeat expansion beyond the current cap, pinned `MX` and `MZZ` detecting-region examples, detector and logical-observable target filters, default-like all-target and all-tick helpers, duplicate target deduplication, invalid target rejection, and dense helper rejection before large allocation.

## Oracle Rows

Implemented row:

- `pf5-detecting-regions-repeat-rust`
- `pf5-detecting-regions-targets-rust`

Still broad and manifest-only:

- `pf5-detecting-regions-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-detecting-regions-repeat`
- `pf5-detecting-regions-targets`

The repeat row measures the bounded repeat-tick detecting-region workload through the Rust public utility API.
The target row measures detector and logical-observable target filters through the additive `DemTarget` API and default-like helper functions.
Both rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
They are not part of the 1.25x primary threshold file.
The target row is coverage for the promoted helper path, not a claim that all-target/all-tick scaling is representative for large generated-code workloads.

## Verification Evidence

Completed target checks for this slice:

```sh
cargo test -p stab-core detecting_regions_repeat_ --quiet
cargo test -p stab-core detecting_regions_target_api --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
just bench::baseline --only pf5-detecting-regions-targets --out target/benchmarks/rpf5-detecting-region-targets-probe
just bench::compare --only pf5-detecting-regions-targets --baseline target/benchmarks/rpf5-detecting-region-targets-probe/baseline.json --report target/benchmarks/rpf5-detecting-region-targets-compare
```

The target-filter benchmark probe reported `stab_pf5_detecting_regions_target_filters=0.006348216s` and `6.452e5 cases/s`, with output written to `target/benchmarks/rpf5-detecting-region-targets-compare`.
The row remains report-only with the documented note that this Rust utility workload has no faithful pinned Stim CLI timing ratio.

## Audit And Review

Milestone audit status is complete for this target-filter slice and incomplete for broader RPF5.
Full-code-review sidecars found one P1 issue in the dense all-target helper, where huge observable ids or detector counts could cause excessive allocation before failure.
The slice now rejects all-target helper requests beyond the dense materialization cap or representable logical-observable target range before allocation, with `detecting_regions_target_api_rejects_dense_helper_expansion` covering the regression.
The remaining review risk is that the report-only benchmark row exercises the promoted helper path on a small fixture and should not be used as representative scaling evidence for large generated-code workloads.

## Remaining RPF5 Work

- Broader detecting-region Clifford gate support, target-shape support beyond the promoted measurement families, multi-detector generated-code regions, ignored anticommutation mode, and gauge behavior.
- Missing-detector generated-code suffix analysis beyond the promoted honeycomb and toric cases, plus broader flow-dependent utility behavior.
- Measurement-rich flows, `has_flow`, `has_all_flows`, `flow_generators`, diagnostics, and transform integration.
