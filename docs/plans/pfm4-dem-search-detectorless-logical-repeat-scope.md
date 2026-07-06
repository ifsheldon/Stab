# PFM4 DEM Search Detectorless Logical Repeat Scope

## Summary

This slice promotes one additional PFM4 search-traversal subcase: graphlike and hypergraph search over large flat zero-shift DEM repeats whose nonzero-probability error mechanisms are detectorless logical-only mechanisms.
It also hardens the hypergraph direct-search distance-1 comparator for multiple detectorless logical error mechanisms, because that compact-model behavior defines the promoted repeat fold.

## Owned Subcases

- Fold selected large flat zero-shift repeat bodies for graphlike and hypergraph search when every body item is a nonzero-probability `error` instruction, no target is numeric, and each error has at least one detector or logical-observable target.
- Preserve compact-model semantic parity for detectorless logical-only repeated errors instead of expanding every occurrence through the generic repeat cap.
- Preserve existing selected flat detector-touching repeat folding.
- Match pinned Stim v1.16.0 hypergraph distance-1 behavior for multiple detectorless logical error mechanisms by using the latest detectorless logical mechanism as the distance-1 candidate.

## Explicit Rejections And Deferrals

- Keep shifted, nested, non-flat, zero-probability, numeric-target, detectorless no-target, analyzer, ErrorMatcher, SAT/WCNF, sampled-error, replay, Python, and diagram behavior outside this slice unless already implemented by earlier slices.
- Keep dense detector and observable caps in force.
- Keep the broad `pf4-dem-folded-traversal` row manifest-only until every remaining traversal consumer is folded, capped with evidence, or explicitly deferred.

## Comparator And Evidence

Comparator class: structural Rust parity for selected graphlike and hypergraph search behavior.
Small direct hypergraph search uses pinned Stim v1.16.0 source behavior for detectorless distance-1 overwrites.
Large selected repeats compare against compact single-body DEMs because pinned Stim would reach the same semantic candidate by flattening, but Stab must avoid materializing the oversized repeat.

## Oracle And Benchmark Policy

- Oracle rows: update `pf4-dem-search-sat-repeat-resource-rust` for the graphlike selected repeat evidence and add `pf4-dem-hypergraph-logical-repeat-rust` for the hypergraph detectorless logical-only comparator and repeat fold.
- Benchmark rows: extend report-only row `pf4-dem-folded-graphlike-traversal` with graphlike detectorless logical-only flat-repeat work and add report-only row `pf4-dem-hypergraph-logical-repeat` for the hypergraph counterpart.
- Keep both benchmark rows non-primary and contract-only because they are Rust API semantic and resource workloads without faithful pinned-Stim timing ratios for oversized folded repeats.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-logical-only-graphlike-baseline
just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-logical-only-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-logical-only-graphlike-compare
just bench::baseline --only pf4-dem-hypergraph-logical-repeat --out target/benchmarks/pfm4-dem-search-logical-only-hypergraph-baseline
just bench::compare --only pf4-dem-hypergraph-logical-repeat --baseline target/benchmarks/pfm4-dem-search-logical-only-hypergraph-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-logical-only-hypergraph-compare
```

Broader pre-commit verification follows the active `GOAL.md` work loop before commit.
