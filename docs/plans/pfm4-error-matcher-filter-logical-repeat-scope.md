# PFM4 ErrorMatcher Filter Logical-Only Repeat Scope

## Objective

Promote one selected PFM4 matcher-adjacent traversal subcase: `explain_errors_from_circuit` filter DEM traversal over large zero-detector-shift repeat bodies whose effective filter keys are detectorless logical-observable-only `error` instructions.

ErrorMatcher filter DEMs are a set of requested DEM error keys, not a probabilistic model.
When a repeated filter body has zero total detector shift, repeating the same logical-observable-only keys does not add new requested keys.
Stab should therefore fold this selected shape to one compact traversal of the body instead of rejecting solely because the repeat exceeds the materialized expansion cap.

## Positive Scope

- Filter DEM traversal accepts oversized flat or nested `repeat` bodies selected by the recursive compact filter-key rule.
- The recursive rule accepts `error` instructions with at least one relative detector target or at least one logical-observable target, optional separators, and no raw numeric targets.
- The recursive rule accepts nested repeat blocks only when each nested body has zero total detector shift and is itself selected by the same rule.
- The recursive rule accepts `shift_detectors` instructions only when their detector shift is exactly zero.
- Accepted repeated filters compare against the compact filter DEM that lists the effective detector or logical-observable filter keys once.

## Explicit Non-Scope

- Nonzero detector shifts in filter DEM repeats remain capped or rejected by the existing repeat-expansion boundary.
- Empty or separator-only error target lists remain outside the selected compact-repeat rule.
- Raw numeric error targets remain rejected or declined before traversal, as already locked by DEM validation and existing PFM4 invalid-target evidence.
- Annotation-only or broader mixed-instruction filter bodies remain capped or rejected unless they are already covered by the selected zero-shift `shift_detectors` rule.
- Circuit-repeat provenance, repeat-contained circuit noise, full ErrorMatcher provenance, `explain_errors` CLI behavior, Python, diagrams, and simulator-product APIs are unchanged.

## Comparator Class

Comparator class: structural Rust parity.
The comparator is compact filter-key equivalence: `explain_errors_from_circuit(circuit, Some(repeated_filter), false)` must produce the same `ExplainedError` strings as the corresponding compact filter for selected detectorless logical-observable-only zero-shift bodies.

## Tests

Owned tests:

- Extend `pf4_error_matcher_filter_` coverage with an oversized flat logical-observable-only filter repeat and a nested logical-observable-only filter repeat.
- Compare each repeated filter output against a compact filter with the same effective logical-observable keys.
- Preserve the existing negative test proving shifted filter DEM repeats still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-error-matcher-filter-repeat-rust` to describe selected flat and nested detector-touching and detectorless logical-observable-only zero-detector-shift filter DEM repeat folding.
- Keep this as structural `cargo test` evidence because pinned Stim does not provide a separate public CLI timing surface for this filter-key extraction helper.

## Benchmark Rows

- Add `pf4-error-matcher-filter-logical-repeat` as a non-primary report-only contract-only row with one Stab Rust runner submeasurement.
- Record work units as folded logical filter-key occurrences per second.
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
just bench::baseline --only pf4-error-matcher-filter-logical-repeat --out target/benchmarks/pfm4-error-matcher-filter-logical-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-logical-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-logical-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-logical-repeat-compare
```
