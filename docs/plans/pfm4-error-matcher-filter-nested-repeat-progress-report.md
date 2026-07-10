# PFM4 ErrorMatcher Filter Nested Repeat Progress Report

## Scope

This PFM4 slice owns one matcher-adjacent DEM traversal subcase: `explain_errors_from_circuit` filter DEMs whose oversized repeated body is selected by a recursive compact filter-key rule.

The selected recursive rule accepts detector-touching `error` instructions, optional logical-observable and separator targets, nested repeat blocks with zero total detector shift, and `shift_detectors` instructions whose detector shift is exactly zero.
For this historical slice, the selected recursive rule rejected or declined raw numeric targets, detectorless logical-only filter repeats, nonzero detector shifts, annotation-only or broader mixed-instruction bodies, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior. PFM-B3 later promotes detectorless logical-only keys and corrects annotation-only neutral bodies to skip.
The later PFM4 logical-only slice promotes selected detectorless logical-observable-only filter repeats.

## Comparator And Evidence

Comparator class: structural Rust parity for selected large nested zero-detector-shift ErrorMatcher filter DEM repeats.
The compact comparator is a DEM filter containing each effective detector-touching key once at the same detector offset, because the filter is a set of requested DEM error keys and repeated identical zero-shift keys do not change matching semantics.

## Implemented Slice

`error_keys_from_dem` now uses a recursive compact-repeat selector for filter DEM repeats.
Budget validation recognizes selected nested zero-shift detector-touching filter bodies before applying the generic repeat-count cap.
Filter-key collection recognizes the same selected body and traverses it once at the current detector offset.

At this slice boundary, the implementation kept the old fail-closed behavior for shifted filter DEM repeats and detectorless logical-only filter repeats.
The later PFM4 logical-only slice preserves shifted rejection while promoting selected zero-shift logical-observable-only filter repeats.
Repeat-contained circuit noise remains rejected by the circuit-side ErrorMatcher budget until recursive provenance is deliberately selected.

## Tests

New test:

- `pf4_error_matcher_filter_folds_nested_detector_repeat`

Existing retained tests:

- `pf4_error_matcher_filter_rejects_shifted_repeat`
- `pf4_error_matcher_filter_folds_flat_detector_repeat`
- `pf4_error_matcher_filter_folds_rich_flat_detector_repeat`

The nested test compares a large nested filter with duplicate detector cancellation, logical terms, separator terms, zero-detector-shift nested shifts, and a nonzero outer detector offset against the compact filter output.

## Oracle And Benchmarks

Metadata changes:

- Updated `pf4-error-matcher-filter-repeat-rust` to cover selected flat and nested detector-touching zero-detector-shift filter DEM repeat folding.
- Added `pf4-error-matcher-filter-nested-repeat` with `stab_pf4_error_matcher_filter_nested_repeat_fold` and `folded-nested-filter-keys/s` work units.
- Kept the benchmark row out of primary timing gates because the compact large-repeat behavior is source-owned semantic evidence, not a faithful pinned-Stim timing ratio.

Fresh focused compare:

```text
stab_pf4_error_matcher_filter_nested_repeat_fold=0.000006666s, rate=5.401e12 folded-nested-filter-keys/s
```

Artifacts:

- `target/benchmarks/pfm4-error-matcher-filter-nested-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-error-matcher-filter-nested-repeat-compare/compare.json`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_traversal_resource pf4_error_matcher_filter_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
cargo test -p stab-oracle fixtures --quiet
just bench::baseline --only pf4-error-matcher-filter-nested-repeat --out target/benchmarks/pfm4-error-matcher-filter-nested-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-nested-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-nested-repeat-compare
```

Milestone-audit status: complete for this selected slice. No blocking implementation defects or milestone loopholes were found; the slice satisfies the scope note by folding selected nested detector-touching zero-detector-shift filter bodies, preserving shifted filter DEM rejection, updating oracle metadata, adding report-only benchmark evidence, and keeping detectorless logical-only filter repeats, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior explicitly out of scope for that slice.
Full-code-review status: complete with two GPT-5.5/xhigh sidecars. The Rust and compatibility reviewer found no blocking issues and confirmed the set-like filter-key semantics are sound for this scope, with residual nonblocking risk for additional nested negative shapes. The docs, oracle, and benchmark reviewer found two P2 alignment issues: stale flat-only wording in the PF4 rollup and missing ErrorMatcher filter rows plus work units in the PFM4 benchmark contract. Both were fixed in `docs/plans/rpf4-dem-search-sat-progress-report.md` and `docs/plans/non-deferred-partial-feature-milestones.md`.
