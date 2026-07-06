# PFM4 DEM SAT Nested Repeat Scope

## Objective

Promote one selected PFM4 SAT/WCNF traversal subcase: unweighted shortest-error SAT and weighted likeliest-error WCNF generation over large nested DEM repeat bodies whose nested bodies have total detector shift zero and are semantically equivalent to a compact one-body SAT error list.

The previous SAT/WCNF fold covered large flat zero-shift repeat bodies.
This slice removes the avoidable cap for selected nested zero-shift repeat bodies without broadening shifted, non-flat, analyzer, ErrorMatcher, sampler, graphlike, or hypergraph behavior.

## Positive Scope

- `shortest_error_sat_problem` accepts a large outer repeat containing a large inner zero-shift body when every promoted repeated body has total detector shift zero.
- `likeliest_error_sat_problem` accepts the same selected nested repeat shape.
- The selected nested bodies may contain nonzero-probability `error` instructions, zero-probability `error` instructions for unweighted SAT, no-target errors, zero-detector-shift `shift_detectors`, `detector` declarations, standalone `logical_observable` declarations, and nested selected repeat blocks.
- Unweighted SAT folds selected nested zero-shift bodies structurally, including zero-probability mechanisms.
- Weighted WCNF omits zero-probability mechanisms and folds selected nested nonzero-probability bodies by concrete MAP parity cost, preserving the previous flat-repeat probability semantics.
- Nested no-target errors are represented once in the compact SAT error list, matching the existing no-target objective semantics.

## Explicit Non-Scope

- Graphlike and hypergraph search nested zero-shift repeat folding is already covered by `pfm4-dem-search-nested-repeat-progress-report.md`.
- Nonzero detector shifts remain capped or rejected by the existing SAT repeat-expansion resource boundary.
- Nested repeats whose repeated body has a nonzero total detector shift are not promoted.
- High-index dense-target structural SAT repeats, shifted repeats, non-flat repeats outside the selected zero-shift shape, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, and simulator-product APIs are unchanged.
- This slice does not claim folded traversal for every nested DEM operation.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where the selected nested zero-shift repeated body is represented once, except weighted WCNF maps repeated probabilities through the existing concrete MAP parity-cost fold.

## Tests

Owned tests:

- `sat_problem_shortest_folds_large_nested_zero_shift_repeats` for unweighted SAT.
- `sat_problem_likeliest_folds_large_nested_zero_shift_repeats_by_map_cost` for weighted WCNF.

The tests must prove:

- unweighted SAT folds selected nested zero-shift detector-touching bodies without hitting the repeat cap;
- unweighted SAT folds selected nested zero-shift zero-probability bodies structurally without hitting the repeat cap;
- weighted WCNF folds selected nested zero-shift detector-touching bodies by concrete MAP parity cost without hitting the repeat cap;
- weighted WCNF omits selected nested zero-probability bodies without hitting the repeat cap;
- nested no-target bodies compare to the compact model with one folded no-target mechanism;
- nested nonzero detector shifts remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Add `pf4-dem-sat-nested-repeat-fold-rust` for the selected nested zero-shift SAT/WCNF test filter.

## Benchmark Rows

- Extend `pf4-dem-sat-flat-repeat-fold` with nested zero-shift SAT/WCNF submeasurements.
- Record work units as folded nested SAT error occurrences per second.

The row remains report-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core sat_problem_shortest_folds_large_nested_zero_shift_repeats --quiet
cargo test -p stab-core sat_problem_likeliest_folds_large_nested_zero_shift_repeats_by_map_cost --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-dem-sat-flat-repeat-fold --out target/benchmarks/pfm4-dem-sat-nested-repeat-baseline
just bench::compare --only pf4-dem-sat-flat-repeat-fold --baseline target/benchmarks/pfm4-dem-sat-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-sat-nested-repeat-compare
```
