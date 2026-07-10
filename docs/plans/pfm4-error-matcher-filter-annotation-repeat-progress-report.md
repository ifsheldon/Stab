# PFM4 ErrorMatcher Filter Annotation Repeat Progress Report

## Scope

This PFM4 slice owns one matcher-adjacent DEM traversal subcase: `explain_errors_from_circuit` filter DEMs whose oversized repeated body contains ignored `detector` or standalone `logical_observable` annotations plus at least one selected effective filter-key `error` instruction and zero total detector shift.

Comparator class: structural Rust parity by compact filter-key equivalence.
The repeated filter must produce the same `ExplainedError` strings as a compact filter containing each effective filter key once, while ignored annotations do not add filter keys.

The selected rule accepts flat and nested zero-shift bodies containing selected `error` instructions, optional separators, zero-detector-shift `shift_detectors`, and ignored `detector` or standalone `logical_observable` annotations.
It still declines raw numeric targets, empty or separator-only target lists, nonzero detector shifts, annotation-only bodies, broader mixed-instruction bodies, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior.

## Implementation

`selected_filter_compact_repeat_error_count_inner` now treats `detector` and standalone `logical_observable` instructions as ignored annotations when deciding whether a repeated filter body can use compact filter-key traversal.
The selector still requires at least one effective `error` key in the selected body, still rejects nonzero detector shifts, and still delegates all non-selected repeat bodies to the existing materialized expansion cap.

The PF4 benchmark harness now keeps ErrorMatcher filter rows in `ops/bench/src/baseline/pf4/matcher_filter.rs`.
This records the new annotation-bearing row without growing `ops/bench/src/baseline/pf4.rs` toward the 1200-line source-file guardrail.

## Tests

Added:

- `pf4_error_matcher_filter_folds_annotation_repeat`.
- `pf4_error_matcher_filter_skips_annotation_only_repeat`, corrected by PFM-B3 after direct pinned-Stim review showed annotation-only repeats are neutral.

The positive regression constructs a circuit with detector and observable declarations, compares a compact filter against an oversized annotation-bearing filter with a nested zero-shift body, asserts that the compact filter selects errors, and verifies exact `ExplainedError` string parity.
The negative regression proves an oversized body containing only annotations and zero-shift shifts remains capped instead of being silently accepted.

Existing coverage still proves:

- `pf4_error_matcher_filter_rejects_shifted_repeat`.
- `pf4_error_matcher_filter_folds_flat_detector_repeat`.
- `pf4_error_matcher_filter_folds_rich_flat_detector_repeat`.
- `pf4_error_matcher_filter_folds_nested_detector_repeat`.
- `pf4_error_matcher_filter_folds_logical_only_repeat`.

## Oracle And Benchmarks

Oracle metadata:

- Updated `pf4-error-matcher-filter-repeat-rust` to include selected flat and nested detector-touching, detectorless logical-observable-only, and annotation-bearing zero-shift filter DEM repeat folding.

Benchmark metadata:

- Added `pf4-error-matcher-filter-annotation-repeat` as a non-primary report-only contract-only row.
- Added Stab runner `stab_pf4_error_matcher_filter_annotation_repeat_fold`.
- Added `folded-annotated-filter-keys/s` measurement work.
- Kept the row out of the primary 1.25x gate because it has no faithful pinned-Stim timing ratio.

Focused benchmark probe:

```text
stab_pf4_error_matcher_filter_annotation_repeat_fold=0.000006626s, rate=2.717e12 folded-annotated-filter-keys/s
```

Artifacts:

- `target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-baseline/baseline.json`.
- `target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-compare/compare.json`.

## Documentation

Updated:

- `docs/stab-feature-checklist.md`.
- `docs/plans/non-deferred-partial-feature-milestones.md`.
- `docs/plans/partial-feature-inventory.md`.
- `docs/plans/remaining-partial-feature-milestones.md`.
- `docs/plans/rpf4-dem-search-sat-progress-report.md`.
- `docs/plans/rust-stim-drop-in-rewrite.md`.

## Audit And Review

Milestone-audit status: complete for this selected slice.
No blocking implementation defects or milestone loopholes were found.
The slice satisfies its historical annotation-bearing scope by folding selected zero-shift filter keys, preserving shifted active filter rejection, updating oracle metadata, adding report-only benchmark evidence, keeping the row out of the primary gate, and keeping broader active mixed-instruction filters, circuit-repeat provenance, full ErrorMatcher provenance, and `explain_errors` CLI behavior explicitly out of scope. PFM-B3 later corrects neutral annotation-only bodies to skip instead of reject.

Full-code-review status: complete with two GPT-5.5/xhigh sidecars.
The Rust and compatibility reviewer found no confirmed findings, confirmed the selector stays scoped to effective filter-key `error` bodies plus ignored annotations, and checked the vendored Stim v1.16.0 filter-key path where detector and logical-observable annotations are ignored by filter collection.
The docs, oracle, and benchmark reviewer found no confirmed findings after the fresh focused benchmark evidence was recorded.
The main review also ran the required large-file check; the touched source files are below the 1200-line threshold after splitting ErrorMatcher filter benchmark helpers into `ops/bench/src/baseline/pf4/matcher_filter.rs`.

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
just bench::baseline --only pf4-error-matcher-filter-annotation-repeat --out target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-annotation-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-annotation-repeat-compare
git diff --check
just maintenance::pre-commit
```
