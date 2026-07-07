# PFM4 DEM Search Mixed Zero-Probability Repeat Scope

## Objective

Promote one selected PFM4 search traversal subcase: graphlike and hypergraph logical-error search over large zero-detector-shift DEM repeat bodies that contain nonzero-probability error mechanisms plus zero-probability error mechanisms.

For graphlike and hypergraph search, zero-probability error instructions do not create search edges.
They should not force the selected compact repeat traversal to fall back to the materialized repeat cap when the rest of the repeat body is already a selected zero-shift search shape.

## Positive Scope

- Graphlike search accepts large zero-detector-shift repeat bodies whose instructions are nonzero-probability `error` instructions, zero-probability `error` instructions, zero-detector-shift `shift_detectors`, `detector`, `logical_observable`, or nested zero-detector-shift repeat blocks already accepted by the selected compact-repeat traversal.
- Hypergraph search accepts the same selected repeat body shape.
- Zero-probability error instructions count as zero search work and do not contribute detector or logical-observable target counts.
- Mixed zero-probability plus active detector-touching or logical-observable error bodies compare against the compact model with only the active error mechanisms.
- Zero-probability high-index detector or observable targets inside the selected repeat body must not trigger dense search-graph allocation.

## Explicit Non-Scope

- SAT/WCNF generation is unchanged. Weighted SAT already omits zero-probability variables, while unweighted shortest-error SAT intentionally preserves zero-probability structural mechanisms where selected.
- Nonzero detector shifts, shifted nested repeats, non-flat repeats with unselected items, numeric raw error targets, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, and simulator-product APIs are unchanged.
- This slice does not claim folded traversal for every DEM consumer.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where the selected repeat body is represented once with zero-probability error instructions removed, because graphlike and hypergraph search ignore those mechanisms.

## Tests

Owned tests:

- `pf4_dem_search_mixed_zero_probability_repeat_folds_by_compact_model` for graphlike search.
- `pf4_hypergraph_mixed_zero_probability_repeat_folds_by_compact_model` for hypergraph search.

The tests must prove:

- mixed zero-probability plus active detector-touching error repeats fold before the repeat cap;
- zero-probability high-index targets inside the repeated body do not force dense graph allocation;
- detectorless logical-only active errors still compare to the compact model when adjacent zero-probability errors are present;
- nested zero-detector-shift repeats with mixed zero-probability plus active errors compare to the compact model;
- nonzero detector shifts remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Add `pf4-dem-search-mixed-zero-probability-repeat-rust` for the graphlike-specific test.
- Add `pf4-dem-hypergraph-mixed-zero-probability-repeat-rust` for the hypergraph-specific test.

## Benchmark Rows

- Add `pf4-dem-search-mixed-zero-probability-repeat` as a non-primary report-only contract-only row with graphlike and hypergraph submeasurements.
- Record work units as folded active target-error occurrences per second.

The row remains report-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search_mixed_zero_probability_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-dem-search-mixed-zero-probability-repeat --out target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline
just bench::compare --only pf4-dem-search-mixed-zero-probability-repeat --baseline target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-compare
```
