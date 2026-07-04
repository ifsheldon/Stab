# RPF6 Analyzer Progress Report

## Summary

This RPF6 slice makes the generated-QEC analyzer subset explicit in the source-owned evidence.
It does not complete RPF6 because broad generated-loop handling, loop-folded decomposition, graphlike and hypergraph generated search, sparse reverse tracker work beyond supported-Clifford unitary repeats, and active matched-error hardening remain open.

## Implemented Evidence Slice

- `circuit_to_detector_error_model` is covered for noisy generated repetition-code and rotated-surface-code circuits through semantic DEM comparison against pinned Stim v1.16.0 expected output.
- The semantic comparator normalizes detector shifts, graphlike decomposition separators, repeat traversal, and floating probability drift within tolerance.
- The promoted evidence is Rust-core analyzer evidence, not public `stab analyze_errors` CLI parity.

## Tests

Implemented Rust tests:

- `generated_qec_dem_repetition_code_semantics_match_pinned_stim`
- `generated_qec_dem_rotated_surface_code_semantics_match_pinned_stim`
- `semantic_dem_treats_graphlike_decomposition_as_equivalent`

These tests run under `cargo test -p stab-core generated_qec_dem`.

## Oracle Rows

Implemented row:

- `pf6-analyzer-generated-qec-rust`

Still broad and manifest-only:

- `pf6-analyzer-generated-looping`

The existing M10 row `coverage-simulators-generated-qec-dem` remains historical M10 evidence for the same generated-QEC analyzer subset.
The PF6 row exists so RPF6 has explicit source-owned evidence instead of relying on broad roadmap wording or nearby M10 coverage.

## Benchmark Rows

Report-only runner coverage:

- `pf6-analyze-errors-generated-surface`

The row measures the Rust core generated d3/r3 rotated-memory-z analyzer workload through `circuit_to_detector_error_model`.
It reports `stab_pf6_analyze_errors_generated_surface`, normalized as detectors per second.
It remains `non-primary-report-only` because this Rust core path does not have a faithful pinned Stim CLI timing ratio, and it is not part of the 1.25x primary threshold file.

Still placeholder:

- `pf6-error-decomp-loop-folded`
- `pf6-graphlike-search-generated`
- `pf6-hypergraph-search-generated`
- `pf6-sparse-rev-frame-loop` now has report-only runner coverage for the supported-Clifford unitary-repeat sparse-tracker slice; broader sparse-tracker work remains active.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core generated_qec_dem --quiet
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF6
just bench::smoke
```

## Remaining RPF6 Work

- Generated-loop analyzer behavior beyond the promoted generated-QEC semantic subset.
- Loop-folded error decomposition evidence and benchmark coverage.
- Generated-circuit graphlike, hypergraph, shortest-error, SAT, and WCNF search evidence.
- Sparse reverse detector-frame tracker behavior beyond supported-Clifford unitary-repeat folding, including broader all-unitary fuzz coverage and analyzer/search-specific consumption.
- Active matched-error value-object hardening required by analyzer or search outputs.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain deferred unless the roadmap deliberately promotes them.
