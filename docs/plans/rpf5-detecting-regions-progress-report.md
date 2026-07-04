# RPF5 Detecting Regions Progress Report

## Summary

This RPF5 slice adds bounded repeat traversal to the Rust `circuit_detecting_regions` utility for the currently supported simple gate subset.
It is not an RPF5 completion report because broader Clifford gates, target shapes, detector filtering, multi-detector regions, anticommutation modes, gauge behavior, missing-detector families, and measurement-rich flows remain active work.

## Implemented Surfaces

- `circuit_detecting_regions` now validates supported instructions recursively through repeat blocks instead of rejecting every repeat block.
- Detector and tick counts are computed through repeat blocks with checked arithmetic.
- Reverse traversal snapshots repeat bodies in reverse execution order, preserving global detector, measurement-record, and tick numbering for bounded repeat workloads.
- Detecting-region extraction rejects excessive repeat expansion before unbounded unrolling.

## Tests

Implemented Rust tests:

- `detecting_regions_repeat_supports_bounded_ticks`
- `detecting_regions_repeat_rejects_excessive_expansion`

These tests cover bounded repeat tick traversal, expected tick-indexed detecting regions, and resource rejection for repeat expansion beyond the current cap.

## Oracle Rows

Implemented row:

- `pf5-detecting-regions-repeat-rust`

Still broad and manifest-only:

- `pf5-detecting-regions-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-detecting-regions-repeat`

The row measures the bounded repeat-tick detecting-region workload through the Rust public utility API.
It remains `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core detecting_regions_repeat_ --quiet
cargo test -p stab-bench pf5_detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
```

## Remaining RPF5 Work

- Broader detecting-region Clifford gate support, target-shape support, detector filtering, multi-detector regions, anticommutation modes, and gauge behavior.
- Missing-detector generated-code suffix analysis for honeycomb and broader toric cases beyond the promoted global-stabilizer suffix case, plus broader flow-dependent utility behavior.
- Measurement-rich flows, `has_flow`, `has_all_flows`, `flow_generators`, diagnostics, and transform integration.
