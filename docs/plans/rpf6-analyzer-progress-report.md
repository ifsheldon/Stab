# RPF6 Analyzer Progress Report

## Summary

This RPF6 report makes the generated-QEC analyzer subset, the selected loop-folded error-decomposition subset, and the bounded mixed-top-level `fold_loops=true` fallback explicit in the source-owned evidence.
It does not complete RPF6 because broad true folded generated-loop output, graphlike and hypergraph generated search beyond the promoted QEC subset, sparse reverse tracker work beyond supported-Clifford unitary repeats, and active matched-error hardening remain open.

## Implemented Evidence Slice

- `circuit_to_detector_error_model` is covered for noisy generated repetition-code and rotated-surface-code circuits through semantic DEM comparison against pinned Stim v1.16.0 expected output.
- The semantic comparator normalizes detector shifts, graphlike decomposition separators, repeat traversal, and floating probability drift within tolerance.
- `circuit_to_detector_error_model` is covered for the selected `fold_loops + decompose_errors + block_decomposition_from_introducing_remnant_edges` cases where repeated composite errors decompose into graphlike components inside a folded DEM repeat and remnant-edge blocking is enforced inside a folded repeat.
- `circuit_to_detector_error_model` now handles unsupported mixed top-level `fold_loops=true` shapes through the existing capped non-folded analyzer. This selected bounded fallback covers the pinned generated surface-code prefix, repeat, and tail coordinate case without claiming true folded output for broader generated-loop families.
- The promoted evidence is Rust-core analyzer evidence, not public `stab analyze_errors` CLI parity.

## Tests

Implemented Rust tests:

- `generated_qec_dem_repetition_code_semantics_match_pinned_stim`
- `generated_qec_dem_rotated_surface_code_semantics_match_pinned_stim`
- `semantic_dem_treats_graphlike_decomposition_as_equivalent`
- `pf6_dem_analyzer_fold_loops_decomposes_repeat_errors`
- `pf6_dem_analyzer_fold_loops_respects_remnant_edge_blocking`
- `pf6_dem_analyzer_fallback_uses_bounded_unfolded_for_mixed_top_level`
- `pf6_dem_analyzer_fallback_does_not_mask_prefixed_repeat_errors`
- `pf6_dem_analyzer_fallback_preserves_repeat_count_cap`
- `pf6_dem_analyzer_fallback_preserves_repeat_iteration_cap`
- `pf6_dem_analyzer_fallback_preserves_expanded_instruction_cap`
- `pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit`

These tests run under `cargo test -p stab-core generated_qec_dem`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fold_loops_`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fallback_`, and `cargo test -p stab-core --test dem_api pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit`.

## Oracle Rows

Implemented rows:

- `pf6-analyzer-generated-qec-rust`
- `pf6-error-decomp-loop-folded-rust`

Implemented bounded fallback row:

- `pf6-analyzer-mixed-top-level-fallback-rust`
- `pf6-analyzer-generated-fold-loop-fallback-rust`

Still broad and manifest-only:

- `pf6-analyzer-generated-looping`

The existing M10 row `coverage-simulators-generated-qec-dem` remains historical M10 evidence for the generated-QEC analyzer subset.
The PF6 rows exist so RPF6 has explicit source-owned evidence instead of relying on broad roadmap wording or nearby M10 coverage.

## Benchmark Rows

Report-only runner coverage:

- `pf6-analyze-errors-generated-surface`
- `pf6-error-decomp-loop-folded`

The row measures the Rust core generated d3/r3 rotated-memory-z analyzer workload through `circuit_to_detector_error_model`.
It reports `stab_pf6_analyze_errors_generated_surface`, normalized as detectors per second.
It remains `non-primary-report-only` because this Rust core path does not have a faithful pinned Stim CLI timing ratio, and it is not part of the 1.25x primary threshold file.
The loop-folded decomposition row measures the Rust core analyzer over a repeated composite-error fixture with `fold_loops`, `decompose_errors`, and remnant-edge blocking enabled.
It reports `stab_pf6_error_decomp_loop_folded`, normalized as folded rounds per second.
It remains `non-primary-report-only` because it is a Rust core contract workload, not a faithful pinned Stim public CLI timing ratio.

Related PF6 report-only runner coverage tracked in search and sparse-tracker reports:

- `pf6-graphlike-search-generated`
- `pf6-hypergraph-search-generated`
- `pf6-generated-sat-wcnf`
- `pf6-sparse-rev-frame-loop`

No analyzer benchmark placeholder remains in this report after promoting `pf6-error-decomp-loop-folded`.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core generated_qec_dem --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fold_loops_ --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fallback_ --quiet
cargo test -p stab-core --test dem_api pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit --quiet
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF6
just bench::smoke
just bench::baseline --only pf6-error-decomp-loop-folded --out target/benchmarks/pf6-error-decomp-loop-folded-probe
just bench::compare --only pf6-error-decomp-loop-folded --baseline target/benchmarks/pf6-error-decomp-loop-folded-probe/baseline.json --report target/benchmarks/pf6-error-decomp-loop-folded-compare
```

## Remaining RPF6 Work

- True folded generated-loop analyzer behavior beyond the promoted generated-QEC semantic subset and bounded mixed-top-level fallback.
- Broader loop-folded error decomposition subcases beyond the promoted repeated composite-error and remnant-edge blocking fixtures.
- Generated-circuit graphlike, hypergraph, shortest-error, SAT, and WCNF search evidence.
- Sparse reverse detector-frame tracker analyzer/search-specific consumption beyond the supported-Clifford generated repeat-folding evidence.
- Active matched-error value-object hardening required by analyzer or search outputs.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain deferred unless the roadmap deliberately promotes them.
