# PF1 DEM API Progress Report

## Summary

This PF1 slice implements a bounded Rust `DetectorErrorModel` basic API subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim full Python `stim.DetectorErrorModel` API parity.
Detector-coordinate maps, flattened iterators, rounded transforms, error counts, copy ergonomics beyond `Clone`, and transform resource-boundary closure remain active PF1/PF4 work.

## Implemented Surfaces

- Added public `DetectorErrorModel::len`, `DetectorErrorModel::is_empty`, and `DetectorErrorModel::clear`.
- Added public `DetectorErrorModel::append_from_dem_text`, which parses a full DEM snippet before mutating the receiver so parse failures are atomic.
- Added public `DetectorErrorModel::without_tags`, recursively removing instruction and repeat-block tags.
- Added public `DetectorErrorModel::final_coordinate_shift`, folding nested `shift_detectors` coordinate shifts through repeat blocks.
- Added non-finite folded-coordinate rejection instead of silently returning infinity.
- Tightened `detector` and `logical_observable` validation to reject multiple targets like Stim v1.16.0.

## Oracle Rows

Implemented row:

- `pf1-dem-basic-rust-api`

Still broad and manifest-only:

- `pf1-dem-rust-api`

## Benchmark Rows

Non-primary report-only rows:

- `pf1-dem-counts-repeat`
- `pf1-dem-without-tags`

Probe reports:

- `target/benchmarks/pf1-dem-counts-probe/baseline.json`
- `target/benchmarks/pf1-dem-counts-compare/compare.json`
- `target/benchmarks/pf1-dem-without-tags-probe/baseline.json`
- `target/benchmarks/pf1-dem-without-tags-compare/compare.json`

Fresh probe rates from the current worktree:

- `stab_dem_counts_nested_repeat`: `1.099e7 queries/s`.
- `stab_dem_final_coordinate_shift_nested_repeat`: `1.724e7 queries/s`.
- `stab_dem_without_tags_nested_repeat`: `2.801e6 queries/s`.

These benchmarks remain `non-primary-report-only` because pinned Stim exposes comparable behavior through C++ and Python APIs but not through a faithful Rust direct baseline.
They were not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api --quiet
cargo test -p stab-bench pf1_dem_counts --quiet
cargo test -p stab-bench manifest --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF1
just bench::smoke
just bench::baseline --only pf1-dem-counts-repeat --out target/benchmarks/pf1-dem-counts-probe
just bench::compare --only pf1-dem-counts-repeat --baseline target/benchmarks/pf1-dem-counts-probe/baseline.json --report target/benchmarks/pf1-dem-counts-compare
just bench::baseline --only pf1-dem-without-tags --out target/benchmarks/pf1-dem-without-tags-probe
just bench::compare --only pf1-dem-without-tags --baseline target/benchmarks/pf1-dem-without-tags-probe/baseline.json --report target/benchmarks/pf1-dem-without-tags-compare
```

## Remaining PF1/PF4 DEM API Work

- Detector-coordinate maps and single-detector coordinate lookup.
- Error counts and flattened instruction or iterator views.
- Public `flattened`, `rounded`, and complete transform APIs.
- Copy ergonomics beyond the existing `Clone` implementation if a Rust-specific helper is still useful.
- Resource-boundary tests and folded traversal for transform and coordinate-map operations that can produce large outputs.
