# RPF6 Analyzer Progress Report

Historical note: PFM-B5 supersedes this staged analyzer report. Retired period-specific benchmark IDs below are historical evidence only; current ownership is in `docs/plans/pfm-b5-analyzer-search-progress-report.md`.

## Summary

This RPF6 report makes the generated-QEC analyzer subset, the selected prefix/repeat/tail true-folded analyzer subset, the selected loop-carried observable true-folded analyzer subset, the selected period-8 observable true-folded analyzer subset, the selected period-127 observable true-folded analyzer subset, the selected loop-folded error-decomposition subset, the selected folded observable-dependency rejection guard, the bounded mixed-top-level `fold_loops=true` fallback, and the selected matched-error canonicalization hardening explicit in the source-owned evidence.
It does not complete RPF6 because broad true folded generated-loop output beyond the selected detector-chain, loop-carried observable, period-8 observable, and period-127 observable slices, graphlike and hypergraph generated search beyond the promoted QEC subset, and sparse reverse tracker work beyond supported-Clifford unitary repeats remain open; additional matched-error value-object hardening belongs to future analyzer or search slices only when those outputs require it.

## Implemented Evidence Slice

- `circuit_to_detector_error_model` is covered for noisy generated repetition-code and rotated-surface-code circuits through semantic DEM comparison against pinned Stim v1.16.0 expected output.
- The semantic comparator normalizes detector shifts, graphlike decomposition separators, repeat traversal, and floating probability drift within tolerance.
- `circuit_to_detector_error_model` is covered for the selected `fold_loops + decompose_errors + block_decomposition_from_introducing_remnant_edges` cases where repeated composite errors decompose into graphlike components inside a folded DEM repeat and remnant-edge blocking is enforced inside a folded repeat.
- `circuit_to_detector_error_model` now emits true compact folded DEM output for the selected top-level prefix, repeat, and tail detector-chain shape after validating the compact candidate against a measurement-record-lookback-sized non-folded expansion, including selected tail-error output and repeat counts above the bounded fallback unroll cap.
- `circuit_to_detector_error_model` now emits true compact folded DEM output for the selected pinned loop-carried observable case where a huge odd repeat has a tail `OBSERVABLE_INCLUDE` that annotates the repeated body errors, after validating the compact candidate against a measurement-record-lookback-sized non-folded expansion.
- `circuit_to_detector_error_model` now emits the exact Stim-style compact folded DEM output for the selected pinned period-8 logical-observable oscillation case after validating the candidate against a non-folded nine-iteration expansion.
- `circuit_to_detector_error_model` now emits the exact Stim-style compact folded DEM output for the selected pinned period-127 logical-observable oscillation case with a deterministic tail detector after validating the candidate against a non-folded 338-iteration expansion.
- `circuit_to_detector_error_model` is covered for the selected upstream `fold_loops + decompose_errors + approximate_disjoint_errors` rejection case where `OBSERVABLE_INCLUDE` omits loop-carried measurement dependencies across folded iterations and must fail with nondeterministic-observable evidence instead of producing a folded DEM.
- `circuit_to_detector_error_model` handles unsafe or still-unsupported mixed top-level `fold_loops=true` shapes through the existing capped non-folded analyzer. This selected bounded fallback covers the pinned generated surface-code prefix, repeat, and tail coordinate case without claiming true folded output for broader generated-loop families.
- `ExplainedError::canonicalize` and `CircuitErrorLocation::canonicalize` now sort DEM terms, circuit error locations, flipped Pauli products, and flipped measured observables like pinned Stim, while `ErrorMatcher` preserves upstream-like returned location order by avoiding implicit location canonicalization.
- The promoted evidence is primarily Rust-core analyzer evidence. The selected loop-carried, period-8, and period-127 observable slices also have narrow exact-output `stab analyze_errors --fold_loops` CLI fixtures, but this report does not claim full public `stab analyze_errors` CLI parity.

## Tests

Implemented Rust tests:

- `generated_qec_dem_repetition_code_semantics_match_pinned_stim`
- `generated_qec_dem_rotated_surface_code_semantics_match_pinned_stim`
- `semantic_dem_treats_graphlike_decomposition_as_equivalent`
- `pf6_dem_analyzer_fold_loops_decomposes_repeat_errors`
- `pf6_dem_analyzer_fold_loops_respects_remnant_edge_blocking`
- `pf6_dem_analyzer_prefix_repeat_tail_folds_detector_chain`
- `pf6_dem_analyzer_prefix_repeat_tail_folds_tail_error`
- `pf6_dem_analyzer_prefix_repeat_tail_folds_large_detector_chain`
- `pf6_dem_analyzer_loop_carried_observable_folds_like_upstream`
- `pf6_dem_analyzer_period8_observable_folds_like_upstream`
- `pf6_dem_analyzer_period127_observable_folds_like_upstream`
- `pf6_dem_analyzer_period127_observable_folds_minimum_compact_shape_like_upstream`
- `pf6_dem_analyzer_period127_observable_keeps_single_middle_repeat_unfolded`
- `pf6_dem_analyzer_period127_observable_keeps_adjacent_residue_unfolded`
- `pf6_dem_analyzer_period127_observable_rejects_huge_adjacent_residue`
- `pf6_dem_analyzer_rejects_folded_observables_crossing_iterations`
- `pf6_dem_analyzer_fallback_uses_bounded_unfolded_for_unsafe_tail_dependency`
- `pf6_dem_analyzer_fallback_preserves_delayed_rec_dependency`
- `pf6_dem_analyzer_fallback_does_not_mask_prefixed_repeat_errors`
- `pf6_dem_analyzer_fallback_preserves_repeat_count_cap`
- `pf6_dem_analyzer_fallback_preserves_repeat_count_cap_for_delayed_rec_dependency`
- `pf6_dem_analyzer_fallback_preserves_repeat_iteration_cap`
- `pf6_dem_analyzer_fallback_preserves_expanded_instruction_cap`
- `pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit`
- `matched_error_canonicalize_sorts_terms_like_upstream`

These tests run under `cargo test -p stab-core generated_qec_dem`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fold_loops_`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_prefix_repeat_tail_`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_loop_carried_observable`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_period8_observable`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_period127_observable`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_rejects_folded_observables_crossing_iterations`, `cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fallback_`, `cargo test -p stab-core --test dem_api pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit`, and `cargo test -p stab-core matched_error_canonicalize_sorts_terms_like_upstream`.

## Oracle Rows

Implemented rows:

- `pf6-analyzer-generated-qec-rust`
- `pf6-analyzer-prefix-repeat-tail-folding-rust`
- `pf6-analyzer-loop-carried-observable-rust`
- `pf6-analyze-errors-loop-carried-observable-cli`
- `pf6-analyzer-period8-observable-rust`
- `pf6-analyze-errors-period8-observable-cli`
- `pf6-analyzer-period127-observable-rust`
- `pf6-analyze-errors-period127-observable-cli`
- `pf6-error-decomp-loop-folded-rust`

Implemented bounded fallback row:

- `pf6-analyzer-mixed-top-level-fallback-rust`
- `pf6-analyzer-generated-fold-loop-fallback-rust`

Implemented folded-observable rejection row:

- `pf6-analyzer-folded-observable-guard-rust`

Implemented matched-error value-object row:

- `pf6-matched-error-canonicalize-rust`

Still broad and manifest-only:

- `pf6-analyzer-generated-looping`

The existing M10 row `coverage-simulators-generated-qec-dem` remains historical M10 evidence for the generated-QEC analyzer subset.
The PF6 rows exist so RPF6 has explicit source-owned evidence instead of relying on broad roadmap wording or nearby M10 coverage.

## Benchmark Rows

Report-only runner coverage:

- `pf6-analyze-errors-generated-surface`
- `pf6-error-decomp-loop-folded`
- `pf6-analyzer-loop-observable-folded`
- `pf6-analyzer-period8-observable-folded`
- `pf6-analyzer-period127-observable-folded`

The row measures the Rust core generated d3/r3 rotated-memory-z analyzer workload through `circuit_to_detector_error_model`.
It reports `stab_pf6_analyze_errors_generated_surface`, normalized as detectors per second.
It remains `non-primary-report-only` because this Rust core path does not have a faithful pinned Stim CLI timing ratio, and it is not part of the 1.25x primary threshold file.
The loop-folded decomposition row measures the Rust core analyzer over a repeated composite-error fixture with `fold_loops`, `decompose_errors`, and remnant-edge blocking enabled.
It reports `stab_pf6_error_decomp_loop_folded`, normalized as folded rounds per second.
It remains `non-primary-report-only` because it is a Rust core contract workload, not a faithful pinned Stim public CLI timing ratio.
The loop-carried observable row measures the Rust core analyzer over the selected pinned giant-repeat shape with `fold_loops` enabled.
It reports `stab_pf6_analyzer_loop_observable_folded`, normalized as folded rounds per second.
It remains `non-primary-report-only` because it is a selected Rust core contract workload without a faithful pinned Stim direct Rust timing ratio in the current harness.
The period-8 observable row measures the Rust core analyzer over the selected pinned period-8 logical-observable oscillation shape with `fold_loops` enabled.
It reports `stab_pf6_analyzer_period8_observable_folded`, normalized as folded rounds per second.
It remains `non-primary-report-only` because it is a selected Rust core contract workload without a faithful pinned Stim direct Rust timing ratio in the current harness.
The period-127 observable row measures the Rust core analyzer over the selected pinned period-127 logical-observable oscillation shape with `fold_loops` enabled.
It reports `stab_pf6_analyzer_period127_observable_folded`, normalized as folded rounds per second.
It remains `non-primary-report-only` because it is a selected Rust core contract workload without a faithful pinned Stim direct Rust timing ratio in the current harness.

Related PF6 report-only runner coverage tracked in search and sparse-tracker reports:

- `pf6-graphlike-search-generated`
- `pf6-hypergraph-search-generated`
- `pf6-generated-sat-wcnf`
- `pf6-sparse-rev-frame-loop`

No analyzer benchmark placeholder remains in this report after promoting `pf6-error-decomp-loop-folded`.
No separate benchmark row is added for matched-error canonicalization because this is a value-object ordering contract and `ErrorMatcher` deliberately avoids implicit canonicalization on its returned hot path.
No separate benchmark row is added for the selected prefix/repeat/tail detector-chain folding slice because it is a small structural compact-output and resource-boundary contract, while generated analyzer throughput remains covered by `pf6-analyze-errors-generated-surface`.
No separate benchmark row is added for the folded-observable rejection guard because it is negative correctness evidence for an unsupported folded-output shape, not a throughput path.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core generated_qec_dem --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fold_loops_ --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_prefix_repeat_tail_ --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_loop_carried_observable --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_period8_observable --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_period127_observable --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_rejects_folded_observables_crossing_iterations --quiet
cargo test -p stab-core --test dem_analyzer_loop_folding pf6_dem_analyzer_fallback_ --quiet
cargo test -p stab-core --test dem_api pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit --quiet
cargo test -p stab-core matched_error_canonicalize_sorts_terms_like_upstream --quiet
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF6
just bench::smoke
just bench::baseline --only pf6-error-decomp-loop-folded --out target/benchmarks/pf6-error-decomp-loop-folded-probe
just bench::compare --only pf6-error-decomp-loop-folded --baseline target/benchmarks/pf6-error-decomp-loop-folded-probe/baseline.json --report target/benchmarks/pf6-error-decomp-loop-folded-compare
just bench::baseline --only pf6-analyzer-loop-observable-folded --out target/benchmarks/pf6-analyzer-loop-observable-folded-probe
just bench::compare --only pf6-analyzer-loop-observable-folded --baseline target/benchmarks/pf6-analyzer-loop-observable-folded-probe/baseline.json --report target/benchmarks/pf6-analyzer-loop-observable-folded-compare
just bench::baseline --only pf6-analyzer-period8-observable-folded --out target/benchmarks/pf6-analyzer-period8-observable-folded-probe
just bench::compare --only pf6-analyzer-period8-observable-folded --baseline target/benchmarks/pf6-analyzer-period8-observable-folded-probe/baseline.json --report target/benchmarks/pf6-analyzer-period8-observable-folded-compare
just bench::baseline --only pf6-analyzer-period127-observable-folded --out target/benchmarks/pf6-analyzer-period127-observable-folded-probe
just bench::compare --only pf6-analyzer-period127-observable-folded --baseline target/benchmarks/pf6-analyzer-period127-observable-folded-probe/baseline.json --report target/benchmarks/pf6-analyzer-period127-observable-folded-compare
```

## Remaining RPF6 Work

- True folded generated-loop analyzer behavior beyond the promoted prefix/repeat/tail detector-chain slice, selected loop-carried observable slice, selected period-8 observable slice, selected period-127 observable slice, generated-QEC semantic subset, and bounded mixed-top-level fallback.
- Broader loop-folded error decomposition subcases beyond the promoted repeated composite-error and remnant-edge blocking fixtures.
- Broader generated-circuit graphlike, hypergraph, shortest-error, SAT, and WCNF search evidence beyond the promoted generated-QEC search rows.
- Sparse reverse detector-frame tracker analyzer/search-specific consumption beyond the supported-Clifford generated repeat-folding evidence.
- Future matched-error value-object hardening required by newly promoted analyzer or search outputs beyond the selected canonicalization slice.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain deferred unless the roadmap deliberately promotes them.
