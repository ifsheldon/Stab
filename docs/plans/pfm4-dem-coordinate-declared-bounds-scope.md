# PFM4 DEM Coordinate Declared-Bounds Scope

## Summary

This slice tightens selected DEM detector-coordinate lookup through large non-flat repeats.
It is a narrow PFM4 coordinate-resource promotion, not full folded traversal for every ambiguous nested coordinate-map case and not Python API parity.

## Owned Subcases

- Use actual declared-detector bounds inside a repeat body when choosing candidate outer repeat iterations for `DetectorErrorModel::detector_coordinates_for`.
- Preserve first-declaration semantics, coordinate shifts, detector shifts, and typed `DemDetectorId` inputs for selected lookup.
- Return empty coordinates for valid selected detectors that fall inside the dense detector count of a large non-flat repeat but outside the repeat body's declared-detector bounds.
- Keep existing flat and bounded-flattened coordinate scans intact.

## Explicit Rejections And Deferrals

- Keep all-detector coordinate maps capped at the documented materialization limit.
- Keep non-flat ambiguous overlap cases that still require more than the selected candidate cap rejected with a precise resource error.
- Keep true folded generated-loop analyzer output in PFM6.
- Keep diagram APIs and Python binding ergonomics deferred.

## Comparator And Evidence

The comparator class is structural Rust API parity against pinned Stim v1.16.0 coordinate-map semantics for selected detector-coordinate lookups.
The owned regression is Stab-specific resource hardening around a sparse nested repeat hole outside the repeat body's declared-detector bounds, where the correct result is an empty coordinate vector for a valid detector id instead of a candidate-cap error.

## Oracle And Benchmark Policy

- Oracle row: update the existing `pf4-dem-coordinate-resource-rust` row because this behavior belongs to the selected DEM coordinate-resource subset and is covered by the `pf4_dem_coordinates_` test filter.
- Benchmark rows: refresh metadata wording only if needed. No new timing row is required because this narrows candidate selection before the existing selected-lookup traversal and does not introduce a new public throughput workload.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test dem_api pf4_dem_coordinates_ --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
```

Broader pre-commit verification follows the active `GOAL.md` work loop before commit.
