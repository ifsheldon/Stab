# PFM4 DEM Search Nested Repeat Scope

## Objective

Promote one selected PFM4 search traversal subcase: graphlike and hypergraph logical-error search over large nested DEM repeat bodies whose nested bodies are still flat, zero-detector-shift, and search-equivalent to a compact one-body model.

The previous PFM4 search folds covered large flat repeat bodies containing nonzero-probability errors, no-target errors, `shift_detectors 0`, and search-neutral annotations.
Those folds still rejected a nested repeat even when every nested level had zero detector shift and therefore produced only duplicate copies of the same compact search edges.
This slice removes that avoidable cap for the selected graphlike and hypergraph search shape.

## Positive Scope

- Graphlike search accepts a large outer repeat containing a large inner repeat when every selected repeated body has total detector shift zero.
- Hypergraph search accepts the same selected nested repeat shape.
- The selected nested bodies may contain nonzero-probability `error` instructions, no-target errors, zero-detector-shift `shift_detectors`, `detector` declarations, and standalone `logical_observable` declarations.
- The selected nested bodies fold to compact-model semantic parity by replaying the effective search body once, not by materializing outer-count times inner-count copies.
- Detector-touching and detectorless logical-only nested bodies are both covered.

## Explicit Non-Scope

- Nonzero detector shifts remain capped or rejected by the existing repeat-expansion resource boundary.
- Nested repeats whose repeated body has a nonzero total detector shift are not promoted.
- Non-flat search bodies outside the selected zero-shift nested shape remain capped or excluded.
- Numeric raw error targets remain rejected at the typed constructor boundary.
- SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, and simulator-product APIs are unchanged.
- This slice does not claim folded traversal for every nested DEM operation; it is graphlike and hypergraph search only.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where the nested selected repeated body is represented once, because the selected zero-shift repeat hierarchy only duplicates the same search edges.

## Tests

Owned tests:

- `pf4_dem_search_folds_nested_zero_shift_repeat_bodies` for graphlike search.
- `pf4_hypergraph_nested_zero_shift_repeat_folds_by_compact_model` for hypergraph search.

The tests must prove:

- a large outer repeat containing a large inner zero-shift detector-touching body folds without hitting the repeat cap;
- a large outer repeat containing a large inner zero-shift detectorless logical-only body folds without hitting the repeat cap;
- a large outer repeat containing a large inner zero-shift no-target body folds without hitting the repeat cap;
- search output matches the compact graphlike or hypergraph model for the promoted shapes;
- nonzero detector shifts remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Add `pf4-dem-search-nested-repeat-rust` for the graphlike nested zero-shift repeat test.
- Add `pf4-dem-hypergraph-nested-repeat-rust` for the hypergraph nested zero-shift repeat test.

## Benchmark Rows

- Add `pf4-dem-search-nested-repeat` as a non-primary report-only contract-only row with graphlike and hypergraph submeasurements.
- Record work units as folded nested target-error occurrences per second.

The row remains report-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search_nested_repeat nested_zero_shift --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-dem-search-nested-repeat --out target/benchmarks/pfm4-dem-search-nested-repeat-baseline
just bench::compare --only pf4-dem-search-nested-repeat --baseline target/benchmarks/pfm4-dem-search-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-nested-repeat-compare
```
