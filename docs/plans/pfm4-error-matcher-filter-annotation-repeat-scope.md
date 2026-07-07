# PFM4 ErrorMatcher Filter Annotation Repeat Scope

## Objective

Promote one selected PFM4 matcher-adjacent traversal subcase: `explain_errors_from_circuit` filter DEM traversal over large zero-detector-shift repeat bodies that contain ignored DEM annotations plus selected effective filter-key `error` instructions.

ErrorMatcher filter DEMs are a set of requested DEM error keys.
`detector` and `logical_observable` instructions do not add filter keys in the current ErrorMatcher filter-key collector.
When a repeated filter body has zero total detector shift and at least one selected `error` key, repeating ignored annotations does not change matching semantics.
Stab should therefore fold this selected annotation-bearing shape instead of rejecting solely because the repeat exceeds the materialized expansion cap.

## Positive Scope

- Filter DEM traversal accepts oversized flat or nested `repeat` bodies selected by the recursive compact filter-key rule.
- The recursive rule accepts `error` instructions with at least one relative detector or logical-observable target, optional separators, and no raw numeric targets.
- The recursive rule accepts `detector` and standalone `logical_observable` instructions as ignored annotations when the selected body also contains at least one effective `error` key.
- The recursive rule accepts nested repeat blocks only when each nested body has zero total detector shift and is itself selected by the same rule.
- The recursive rule accepts `shift_detectors` instructions only when their detector shift is exactly zero.
- Accepted repeated filters compare against the compact filter DEM that lists the effective filter keys once and may include equivalent ignored annotations.

## Explicit Non-Scope

- Nonzero detector shifts in filter DEM repeats remain capped or rejected by the existing repeat-expansion boundary.
- Annotation-only repeat bodies with no effective `error` keys remain capped or rejected.
- Empty or separator-only error target lists remain outside the selected compact-repeat rule.
- Raw numeric error targets remain rejected or declined before traversal, as already locked by DEM validation and existing PFM4 invalid-target evidence.
- Broader mixed-instruction filter bodies beyond `error`, zero-shift `shift_detectors`, `detector`, and standalone `logical_observable` remain capped or rejected.
- Circuit-repeat provenance, repeat-contained circuit noise, full ErrorMatcher provenance, `explain_errors` CLI behavior, Python, diagrams, and simulator-product APIs are unchanged.

## Comparator Class

Comparator class: structural Rust parity.
The comparator is compact filter-key equivalence: `explain_errors_from_circuit(circuit, Some(repeated_filter), false)` must produce the same `ExplainedError` strings as the corresponding compact filter for selected annotation-bearing zero-shift bodies.

## Tests

Owned tests:

- Extend `pf4_error_matcher_filter_` coverage with an oversized annotation-bearing flat body and nested annotation-bearing body.
- Include ignored `detector` and standalone `logical_observable` annotations, selected detector-touching and logical-observable-bearing error keys, zero-shift nested bodies, and a nonzero outer detector offset.
- Compare the repeated filter output against a compact filter with the same effective keys.
- Preserve existing negative coverage proving shifted filter DEM repeats still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-error-matcher-filter-repeat-rust` to describe selected flat and nested detector-touching, detectorless logical-observable-only, and annotation-bearing zero-detector-shift filter DEM repeat folding.
- Keep this as structural `cargo test` evidence because pinned Stim does not provide a separate public CLI timing surface for this filter-key extraction helper.

## Benchmark Rows

- Add `pf4-error-matcher-filter-annotation-repeat` as a non-primary report-only contract-only row with one Stab Rust runner submeasurement.
- Record work units as folded annotated filter-key occurrences per second.
- Keep the row out of the primary 1.25x gate because it is a Stab resource-boundary contract without a faithful pinned-Stim timing ratio.
- Move ErrorMatcher filter benchmark helpers into a PF4 benchmark submodule if needed to keep `ops/bench/src/baseline/pf4.rs` below the project source-file size guardrail.

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
just bench::baseline --only pf4-error-matcher-filter-annotation-repeat --out target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-annotation-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-compare
```
