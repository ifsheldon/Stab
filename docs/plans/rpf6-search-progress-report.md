# RPF6 Search Progress Report

## Summary

This report records the promoted PF6 generated-QEC graphlike and hypergraph search subset.
It adds executable Rust evidence and report-only benchmark runners for generated rotated-surface-code and repetition-code logical-error search without claiming full PF6 search parity.

## Implemented Surfaces

- Generated rotated-surface-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 5.
- Generated repetition-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 7.
- Generated rotated-surface-code graphlike search with ungraphlike DEMs and `ignore_ungraphlike_errors=true`, matching the pinned Stim v1.16.0 instruction count of 5.
- Generated rotated-surface-code graphlike search rejection for ungraphlike DEMs when `ignore_ungraphlike_errors=false`.
- Generated rotated-surface-code and repetition-code hypergraph search with the pinned Stim v1.16.0 instruction counts of 5 and 7.

## Tests

Implemented Rust tests:

- `pf6_generated_qec_graphlike_search_matches_upstream_instruction_counts`
- `pf6_generated_qec_hypergraph_search_matches_upstream_instruction_counts`

The tests live in `crates/stab-core/tests/dem_search.rs` and are derived from `vendor/stim/src/stim/search/graphlike/algo.test.cc` and `vendor/stim/src/stim/search/hyper/algo.test.cc`.
They assert the generated-code search result sizes that pinned Stim v1.16.0 asserts, require error-only DEM output, require at least one logical observable in the returned logical error, and cover the ungraphlike generated surface-code rejection path.

## Oracle Rows

Implemented row:

- `pf6-search-generated-qec-rust`

The broad row `pf6-search-generated` remains manifest-only because full generated-circuit search parity still includes broader generated-code families, ordering-insensitive search outputs, SAT or WCNF generated cases, additional resource behavior, and sparse reverse tracker integration.

## Benchmark Rows

Rows with new report-only runner coverage:

- `pf6-graphlike-search-generated`, measured as `stab_pf6_graphlike_search_generated_surface`.
- `pf6-hypergraph-search-generated`, measured as `stab_pf6_hypergraph_search_generated_surface`.

Both rows measure generated rotated-surface-code DEM search after source-owned Rust analysis and decomposition.
They remain `non-primary-report-only` and `contract-only` because pinned Stim exposes these search APIs through C++ API and perf surfaces, not a faithful public CLI baseline for Stab.
They were not added to `benchmarks/m12-primary-thresholds.json`.

## Remaining PF6 Search Work

- Broader generated-circuit search families beyond the promoted rotated-surface-code and repetition-code cases.
- Exact or structural comparators for ordering-insensitive search outputs beyond result instruction counts.
- Generated SAT or WCNF encoding evidence.
- Loop-folded generated search behavior.
- Sparse reverse detector-frame tracker optimization and analyzer/search integration beyond the existing staged subset.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.
