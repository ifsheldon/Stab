# RPF4 DEM Transform Progress Report

## Summary

This RPF4 slice adds the current Rust public materialized `DetectorErrorModel::flattened` and `DetectorErrorModel::rounded` transform subset, plus source-owned evidence for recursive tag stripping and final count or shift introspection.
It is not an RPF4 completion report because folded traversal across every DEM consumer, non-flat selected-coordinate traversal above the bounded flattened-declaration scan, diagram APIs, and Python binding shape remain outside this slice or still active.

## Implemented Surfaces

- Added `DetectorErrorModel::flattened`, which materializes the existing lazy adjusted flattened instruction iterator after validating the DEM flattening budget.
- `flattened` preserves instruction tags, drops repeat tags, drops materialized `shift_detectors` instructions, shifts detector ids, applies coordinate shifts, preserves decomposition separators, and preserves logical observable targets.
- `flattened` rejects excessive materialized repeat expansion using the existing DEM flattening cap instead of trying to allocate unbounded output.
- Added `DetectorErrorModel::rounded`, which recursively rounds only `error` instruction probability arguments and preserves non-error coordinate arguments, targets, tags, repeat structure, and zero-probability errors.
- Added PF4 evidence for the existing `DetectorErrorModel::without_tags`, `count_errors`, `count_detectors`, `count_observables`, `total_detector_shift`, `final_coordinate_shift`, and selected coordinate-query behavior through shifted repeats.

## Tests

Implemented Rust tests:

- `pf4_dem_materialized_flattened_matches_pinned_stim_cases`
- `pf4_dem_materialized_flattened_rejects_excessive_repeat`
- `pf4_dem_materialized_rounded_matches_pinned_stim_probability_cases`
- `pf4_dem_materialized_rounded_keeps_zero_probability_errors`
- `pf4_dem_introspection_transform_queries_cover_without_tags_and_final_counts`
- `pf4_dem_public_validation_rejects_malformed_inputs`
- `pf4_dem_public_validation_rejects_high_ids_and_unsupported_ranges`

These tests cover empty models, detector shifts, coordinate shifts, repeat blocks, instruction tags, repeat-tag dropping, logical observables, recursive `without_tags`, final detector shifts, final coordinate shifts, error counts, detector counts, observable counts, selected coordinates through shifted repeats, probability rounding, unchanged non-error coordinate arguments, zero-probability rounded errors, materialized repeat rejection, malformed DEM text, invalid probabilities, invalid separators, invalid targets, invalid repeat counts, invalid tags, high detector ids, high observable ids, detector-shift overflow, programmatic non-finite coordinate rejection, and repeat-block rejection from instruction-only ranges.

## Oracle Rows

Implemented row:

- `pf4-dem-materialized-transforms-rust`
- `pf4-dem-introspection-query-rust`
- `pf4-dem-validation-negative-rust`

Still broad and manifest-only:

- `pf4-dem-introspection-transforms`
- `pf4-dem-coordinate-api`
- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-flatten-repeat`
- `pf4-dem-rounded`
- `pf4-dem-coordinate-map`, tracked in `docs/plans/rpf4-dem-coordinate-progress-report.md`
- `pf4-dem-sampler-folded-repeat`, tracking folded sampler compilation, stochastic direct sampling, zero-probability repeat skipping, and sampled-error cap evidence in `docs/plans/rpf4-dem-sampler-progress-report.md` plus `docs/plans/pfm4-dem-sampler-error-bit-cap-evidence-lock.md`
- `pf4-dem-folded-traversal`, tracking current capped search/analyzer traversal, graphlike and hypergraph zero-probability repeat skipping, and weighted SAT/WCNF zero-probability handling in `docs/plans/rpf4-dem-search-sat-progress-report.md`
- `pf4-dem-folded-graphlike-traversal`, tracking current capped graphlike traversal and graphlike zero-probability repeat skipping in `docs/plans/rpf4-dem-search-sat-progress-report.md`
- `pf4-dem-sat-flat-repeat-fold`, tracking selected flat and nested zero-shift SAT/WCNF repeat folding in `docs/plans/rpf4-dem-search-sat-progress-report.md`

The implemented rows remain `non-primary-report-only` because they measure Rust public APIs and pinned Stim does not provide a faithful Rust direct timing baseline in this harness.
They are not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api pf4_dem_materialized_ --quiet
cargo test -p stab-core --test dem_api pf4_dem_introspection_transform_ --quiet
cargo test -p stab-core --test dem_api pf4_dem_public_validation_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF4
just bench::smoke
```

## Remaining RPF4 Work

- Finish folded coordinate-map resource policy where current APIs still require caps or do not prove non-flat selected-coordinate lookup through very large repeats; the all-coordinate map cap, folded selected-query behavior, flat sparse-overlap fast path, bounded nested sparse-overlap fast path, valid flat sparse-hole behavior, and many-selected flat-overlap scan are tracked separately in `docs/plans/rpf4-dem-coordinate-progress-report.md`.
- Finish folded or capped traversal evidence for graphlike search, hypergraph search, SAT or WCNF encoding beyond selected flat zero-shift repeat folding, matcher-adjacent operations, analyzer-adjacent operations, and nested or shifted repeated stochastic direct DEM sampling beyond the selected sampler folds.
- Decide whether any Rust-specific copy, concat, repetition, or mutation helpers beyond existing `Clone`, `push_instruction`, `push_repeat_block`, and `append_from_dem_text` are still worth adding.
- Add remaining resource-boundary cases only if later RPF4 work promotes high-detector or high-observable behavior beyond the current public validation and coordinate-resource subsets.
