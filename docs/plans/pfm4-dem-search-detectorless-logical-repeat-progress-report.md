# PFM4 DEM Search Detectorless Logical Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large flat zero-shift DEM repeat bodies whose nonzero-probability error mechanisms are detectorless logical-only mechanisms.
It also hardens direct hypergraph search distance-1 behavior for multiple detectorless logical mechanisms so the compact-model comparator matches pinned Stim v1.16.0 source behavior.

## Explicit Non-Scope

This slice did not change shifted, nested, non-flat, zero-probability, numeric-target, detectorless no-target, SAT/WCNF, analyzer, ErrorMatcher, sampled-error, replay, Python, diagram, or deferred simulator-product behavior at the time it landed.
The later no-target follow-up promotes selected flat zero-shift no-target graphlike and hypergraph search repeats without widening this detectorless logical-only slice.
Dense detector and observable caps remain in force.
The broad `pf4-dem-folded-traversal` row remains manifest-only because other traversal consumers and repeat shapes still need folded behavior, precise caps, or explicit deferral.

## Comparator And Evidence

Comparator class: structural Rust parity.
For small direct hypergraph search, the comparator is pinned Stim v1.16.0 source behavior: separate detectorless logical error mechanisms overwrite the hypergraph distance-1 candidate instead of XORing across mechanisms.
For large selected repeats, graphlike and hypergraph search compare against compact single-body DEMs because the repeated independent logical-only mechanisms expose the same shortest-error candidate while Stab must avoid materializing the oversized repeat.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_flat_repeat_error_count` now accepts flat nonzero error bodies where every error has at least one detector or logical-observable target and no numeric target.
This keeps the existing selected detector-touching graphlike and hypergraph repeat fold and additionally promotes detectorless logical-only flat zero-shift bodies.

Hypergraph `Graph::add_edge_from_dem_targets` now matches Stim's distance-1 overwrite behavior for detectorless logical mechanisms.
That fixes the compact-model comparator for multiple detectorless logical-only error rows and prevents a repeated logical-only body from depending on parity of the repeat expansion.

## Tests

Updated tests:

- `pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies`
- `pf4_hypergraph_logical_only_repeat_folds_by_compact_model`

The test now proves:

- selected detector-touching graphlike and hypergraph repeat folding still matches the compact model;
- selected detectorless logical-only graphlike repeat folding matches the compact model;
- selected multiple detectorless logical-only graphlike body errors fold to the compact model;
- direct graphlike search returns the first detectorless logical mechanism as the distance-1 candidate, matching pinned Stim source behavior;
- selected detectorless logical-only hypergraph repeat folding matches the compact model;
- selected multiple detectorless logical-only hypergraph body errors fold to the compact model;
- direct hypergraph search returns the latest detectorless logical mechanism as the distance-1 candidate, matching pinned Stim source behavior.

Related direct-search check:

- `pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases`

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-sat-repeat-resource-rust`
- `pf4-dem-hypergraph-logical-repeat-rust`

Updated benchmark rows:

- `pf4-dem-folded-graphlike-traversal` adds `stab_pf4_dem_graphlike_logical_only_flat_repeat_fold` with `folded-detectorless-logical-errors/s` measurement work.
- `pf4-dem-hypergraph-logical-repeat` adds `stab_pf4_dem_hyper_logical_only_flat_repeat_fold` with `folded-detectorless-logical-errors/s` measurement work.

Both benchmark rows remain non-primary report-only and contract-only because they measure Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compares:

```text
stab_pf4_dem_graphlike_logical_only_flat_repeat_fold=0.000000162s, rate=6.173e12 folded-detectorless-logical-errors/s
stab_pf4_dem_hyper_logical_only_flat_repeat_fold=0.000000328s, rate=3.049e12 folded-detectorless-logical-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-logical-only-graphlike-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-logical-only-graphlike-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-logical-only-hypergraph-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-logical-only-hypergraph-compare/compare.json`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf4_hypergraph_logical_only_repeat --quiet
cargo test -p stab-core --test dem_search pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases --quiet
cargo test -p stab-core --test dem_search pf4_dem_search_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-logical-only-graphlike-baseline
just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-logical-only-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-logical-only-graphlike-compare
just bench::baseline --only pf4-dem-hypergraph-logical-repeat --out target/benchmarks/pfm4-dem-search-logical-only-hypergraph-baseline
just bench::compare --only pf4-dem-hypergraph-logical-repeat --baseline target/benchmarks/pfm4-dem-search-logical-only-hypergraph-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-logical-only-hypergraph-compare
```

## Audit And Review

Milestone-audit status: complete for this selected slice.
The implemented evidence covers compact-model semantic parity for selected detectorless logical-only zero-shift graphlike and hypergraph repeated error bodies, preserves the existing selected detector-touching repeat fold, records the pinned Stim hypergraph detectorless distance-1 overwrite comparator, and keeps shifted, nested, non-flat, numeric-target, SAT/WCNF, analyzer, ErrorMatcher, sampled-error, and replay behavior explicitly outside the slice.
The audit found one provenance loophole during review: the hypergraph detectorless evidence was initially grouped under graphlike-sourced oracle and benchmark rows.
That loophole is fixed by splitting hypergraph evidence into `pf4-dem-hypergraph-logical-repeat-rust`, `pf4-dem-hypergraph-logical-repeat`, and `pf4_hypergraph_logical_only_repeat_folds_by_compact_model`.

Full-code-review status: complete with GPT-5.5/xhigh sidecars.
The Rust/compatibility reviewer found no blocking issue and recommended an explicit graphlike direct-search assertion for multiple detectorless logical mechanisms; that regression is now covered in `pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies`.
The docs/oracle/benchmark reviewer found the provenance split issue noted above, then re-checked the final manifests and reports with no remaining findings.
