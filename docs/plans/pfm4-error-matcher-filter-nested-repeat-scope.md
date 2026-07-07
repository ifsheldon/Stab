# PFM4 ErrorMatcher Filter Nested Repeat Scope

## Objective

Promote one selected PFM4 matcher-adjacent traversal subcase: `explain_errors_from_circuit` filter DEM traversal over large nested zero-detector-shift repeat bodies whose effective filter keys are detector-touching `error` instructions.

ErrorMatcher filter DEMs are used as a set of requested DEM error keys.
When a repeated filter body has zero total detector shift, repeating the same detector-touching keys does not create new requested keys.
Stab should therefore fold this selected nested shape to one compact traversal of the body instead of rejecting solely because the outer repeat exceeds the materialized expansion cap.

## Positive Scope

- Filter DEM traversal accepts an oversized outer `repeat` whose body is selected by the recursive compact filter-key rule.
- The recursive rule accepts `error` instructions with at least one relative detector target, optional logical-observable and separator targets, and no raw numeric targets.
- The recursive rule accepts nested repeat blocks only when each nested body has zero total detector shift and is itself selected by the same rule.
- The recursive rule accepts `shift_detectors` instructions only when their detector shift is exactly zero.
- Accepted nested repeated filters compare against the compact filter DEM that lists the effective detector-touching error keys once.

## Explicit Non-Scope

- Nonzero detector shifts in filter DEM repeats remain capped or rejected by the existing repeat-expansion boundary.
- Detectorless logical-only filter repeats, mixed non-error instruction families beyond zero-detector-shift `shift_detectors`, raw numeric error targets, circuit-repeat provenance, repeat-contained circuit noise, full ErrorMatcher provenance, `explain_errors` CLI behavior, Python, diagrams, and simulator-product APIs are unchanged.

## Comparator Class

Comparator class: structural Rust parity.
The comparator is compact filter-key equivalence: `explain_errors_from_circuit(circuit, Some(nested_filter), false)` must produce the same `ExplainedError` strings as the corresponding compact filter for the selected nested zero-shift detector-touching body.

## Tests

Owned tests:

- Extend `pf4_error_matcher_filter_` coverage with a nested zero-shift repeat whose outer count exceeds the current materialized repeat cap.
- Compare the nested filter output against a compact filter with the same detector-touching keys.
- Include duplicate detector cancellation, logical-observable terms, separator terms, a zero-detector-shift nested `shift_detectors`, and a nonzero detector offset before the oversized repeat.
- Preserve a negative test proving shifted filter DEM repeats still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-error-matcher-filter-repeat-rust` to describe selected flat and nested zero-detector-shift filter DEM repeat folding.
- Keep this as structural `cargo test` evidence because pinned Stim does not provide a separate public CLI timing surface for this filter-key extraction helper.

## Benchmark Rows

- Add `pf4-error-matcher-filter-nested-repeat` as a non-primary report-only contract-only row with one Stab Rust runner submeasurement.
- Record work units as folded nested filter-key occurrences per second.
- Keep the row out of the primary 1.25x gate because it is a Stab resource-boundary contract without a faithful pinned-Stim timing ratio.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_traversal_resource pf4_error_matcher_filter_ --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-error-matcher-filter-nested-repeat --out target/benchmarks/pfm4-error-matcher-filter-nested-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-nested-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-nested-repeat-compare
```
