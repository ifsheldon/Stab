# RPF4 DEM Coordinate Progress Report

## Summary

This RPF4 slice hardens Rust DEM coordinate-map resource behavior and adds report-only benchmark coverage for the current coordinate API subset.
It is not an RPF4 completion report because folded traversal across every DEM consumer, broad ambiguous-overlap coordinate resource policy, diagram APIs, and Python binding shape remain outside this slice or still active.

## Implemented Surfaces

- Added a 1,000,000 detector cap to `DetectorErrorModel::detector_coordinates`, the all-detector coordinate-map convenience API.
- The cap rejects huge materialized coordinate maps before constructing the list of every detector id.
- The error points callers to `DetectorErrorModel::detector_coordinates_for`, which accepts typed `DemDetectorId` values for selected lookups.
- Selected-detector and single-detector coordinate lookups remain available for huge-repeat models when the requested detectors are reachable without materializing the full coordinate map.
- Selected lookups now use folded repeat indexing for non-overlapping repeat detector declarations, so late detectors in huge repeats do not require flattened linear iteration.
- Bounded overlapping repeat declarations preserve first-declaration semantics when several repeat iterations could declare the same detector id.
- PF4 transform evidence now separately covers final detector shifts, final coordinate shifts, detector counts, observable counts, error counts, and selected coordinate lookups through shifted repeats.

## Tests

Implemented Rust tests:

- `pf4_dem_coordinates_reject_huge_all_map_but_allow_selected_queries`
- `pf4_dem_coordinates_fold_late_selected_detector_lookup`
- `pf4_dem_coordinates_preserve_first_overlapping_repeat_declaration`

These tests cover huge all-map rejection, selected coordinate lookup through a huge repeat, single-detector lookup through the same huge-repeat model, folded late selected lookup through a billion-record non-overlapping repeat, and first-declaration behavior for bounded overlapping repeats.

## Oracle Rows

Implemented row:

- `pf4-dem-coordinate-resource-rust`

Still broad and manifest-only:

- `pf4-dem-introspection-transforms`
- `pf4-dem-coordinate-api`
- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-coordinate-map`

The row measures a bounded all-coordinate map and a folded selected-coordinate lookup through a huge-repeat model.
It remains `non-primary-report-only` because it measures Rust public APIs and pinned Stim does not provide a faithful Rust direct timing baseline in this harness.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api pf4_dem_coordinates_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF4
just bench::smoke
```

## Remaining RPF4 Work

- Finish folded traversal or explicit caps for graphlike search, hypergraph search, SAT or WCNF encoding, matcher-adjacent operations, sampler-adjacent operations, and analyzer-adjacent operations.
- Finish or explicitly cap any later-promoted ambiguous overlapping selected-coordinate ranges that need more than the current capped candidate search.
- Decide whether any Rust-specific copy, concat, repetition, or mutation helpers beyond existing `Clone`, `push_instruction`, `push_repeat_block`, and `append_from_dem_text` are still worth adding.
- Add remaining malformed-input or resource-boundary cases only if later RPF4 work promotes behavior beyond the current validation, introspection, and coordinate-resource subsets.
