# PFM4 ErrorMatcher Filter Logical-Only Repeat Progress Report

## Scope

This PFM4 slice owns one matcher-adjacent DEM traversal subcase: `explain_errors_from_circuit` filter DEMs whose oversized repeated body contains detectorless logical-observable-only `error` keys and zero total detector shift.

Comparator class: structural Rust parity by compact filter-key equivalence.
The repeated filter must produce the same `ExplainedError` strings as a compact filter containing each effective logical-observable filter key once.

The selected rule accepts flat and nested zero-shift bodies containing `error` instructions with at least one detector or logical-observable target, optional separators, and zero-shift `shift_detectors`.
It still declines raw numeric targets, empty or separator-only target lists, nonzero detector shifts, annotation-only or broader mixed-instruction bodies, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior.

## Implementation

`error_keys_from_dem` now treats logical-observable targets as filter-key targets when deciding whether a repeat body can use compact filter-key traversal.
This extends the existing detector-touching compact-repeat selector without changing canonical filter-key construction or shifted-repeat fallback behavior.

## Tests

Added:

- `pf4_error_matcher_filter_folds_logical_only_repeat`.

The new regression constructs a circuit with two observable-only noisy measurements, compares a compact `L0`/`L1` filter against an oversized filter containing a direct logical-only key and a nested zero-shift logical-only key, and asserts that the compact comparator is nonempty.

Existing coverage still proves:

- `pf4_error_matcher_filter_rejects_shifted_repeat`.
- `pf4_error_matcher_filter_folds_flat_detector_repeat`.
- `pf4_error_matcher_filter_folds_rich_flat_detector_repeat`.
- `pf4_error_matcher_filter_folds_nested_detector_repeat`.

## Oracle And Benchmarks

Oracle metadata:

- Updated `pf4-error-matcher-filter-repeat-rust` to include selected flat and nested detector-touching plus detectorless logical-observable-only zero-shift filter DEM repeat folding.

Benchmark metadata:

- Added `pf4-error-matcher-filter-logical-repeat` as a non-primary report-only contract-only row.
- Added Stab runner `stab_pf4_error_matcher_filter_logical_repeat_fold`.
- Added `folded-logical-filter-keys/s` measurement work.
- Kept the row out of the primary 1.25x gate because it has no faithful pinned-Stim timing ratio.

Focused benchmark probe:

```text
stab_pf4_error_matcher_filter_logical_repeat_fold=0.000005948s, rate=3.026e12 folded-logical-filter-keys/s
```

Artifacts:

- `target/benchmarks/pfm4-error-matcher-filter-logical-repeat-baseline/baseline.json`.
- `target/benchmarks/pfm4-error-matcher-filter-logical-repeat-compare/compare.json`.

## Documentation

Updated:

- `docs/stab-feature-checklist.md`.
- `docs/plans/non-deferred-partial-feature-milestones.md`.
- `docs/plans/partial-feature-inventory.md`.
- `docs/plans/remaining-partial-feature-milestones.md`.
- `docs/plans/rpf4-dem-search-sat-progress-report.md`.
- `docs/plans/rust-stim-drop-in-rewrite.md`.
- Historical flat and nested ErrorMatcher filter progress reports, so their detectorless logical-only wording reads as slice-local history instead of current behavior.

## Audit And Review

Milestone-audit status: complete for this selected slice.
No blocking implementation defects or milestone loopholes were found.
The slice satisfies the scope note by folding selected flat and nested detectorless logical-observable-only zero-shift filter keys, preserving shifted filter DEM rejection, updating oracle metadata, adding report-only benchmark evidence, and keeping annotation-only or broader mixed-instruction filters, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior explicitly out of scope.

Full-code-review status: complete with two GPT-5.5/xhigh sidecars.
The Rust and compatibility reviewer found no confirmed findings, confirmed the selector change is semantically sound against upstream Stim's filter-key xor behavior, and noted residual risk that no direct C++ oracle run exists for the exact oversized logical-only repeat filter.
The docs, oracle, and benchmark reviewer found no confirmed findings, confirmed runner dispatch, measurement work, compare note, manifest entry, and report-only status, and noted that `ops/bench/src/baseline/pf4.rs` is 1109 lines and should be split before future PF4 benchmark growth crosses the 1200-line source-file threshold.

## Verification

Completed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test dem_traversal_resource pf4_error_matcher_filter_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just oracle::record --check-clean
just bench::smoke
just bench::baseline --only pf4-error-matcher-filter-logical-repeat --out target/benchmarks/pfm4-error-matcher-filter-logical-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-logical-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-logical-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-logical-repeat-compare
git diff --check
```

Subagent-only verification also passed:

```sh
cargo test -p stab-core --test error_matcher_basic error_matcher_rejects_filter_dem_repeat_expansion_budget --quiet
cargo test -p stab-core --test dem_api pf4_dem_public_validation_rejects_malformed_inputs --quiet
```
