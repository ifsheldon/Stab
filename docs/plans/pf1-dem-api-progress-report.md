# PF1 DEM API Progress Report

## Summary

This PF1 slice implements a bounded Rust `DetectorErrorModel` basic and introspection API subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim full Python `stim.DetectorErrorModel` API parity.
Public materialized `flattened`, `rounded`, exact Python API shape, and transform resource-boundary closure remain active PF4 work.

## Implemented Surfaces

- Added public `DetectorErrorModel::len`, `DetectorErrorModel::is_empty`, and `DetectorErrorModel::clear`.
- Added public `DetectorErrorModel::append_from_dem_text`, which parses a full DEM snippet before mutating the receiver so parse failures are atomic.
- Covered public programmatic construction through `DemInstruction` convenience constructors, `DemRepeatBlock::new`, `DetectorErrorModel::push_instruction`, `DetectorErrorModel::push_repeat_block`, typed `Probability`, typed `RepeatCount`, and derived `Clone`.
- Added public `DetectorErrorModel::without_tags`, recursively removing instruction and repeat-block tags.
- Added public `DetectorErrorModel::final_coordinate_shift`, folding nested `shift_detectors` coordinate shifts through repeat blocks.
- Added public `DetectorErrorModel::count_errors`, folding nested repeat blocks without materializing repeated instructions.
- Added public `DetectorErrorModel::detector_coordinates`, `DetectorErrorModel::detector_coordinates_for`, and `DetectorErrorModel::coordinates_of_detector` using `DemDetectorId`, including empty-coordinate defaults for valid undeclared detectors and shifted coordinates through repeats.
- Added public `DetectorErrorModel::iter_items`, `DetectorErrorModel::item_range`, `DetectorErrorModel::instruction_range`, `DetectorErrorModel::iter_flattened_instructions`, `DemItem::as_instruction`, and `DemItem::as_repeat_block`.
- Item and instruction range views validate top-level ranges, instruction-only ranges reject repeat blocks instead of silently skipping them, and the flattened iterator yields adjusted owned instructions through repeat blocks without materializing all yielded instructions.
- Added non-finite folded-coordinate rejection instead of silently returning infinity.
- Tightened `detector` and `logical_observable` validation to reject multiple targets like Stim v1.16.0.

## Oracle Rows

Selected closure row:

- `pf1-dem-rust-api`

Implemented supporting rows:

- `pf1-dem-basic-rust-api`
- `pf1-dem-counts-coordinates`
- `pf1-dem-iterators`

The selected closure row runs `cargo test -p stab-core --test dem_api pf1_dem_` and is intentionally scoped to the current Rust DEM API surface. It does not claim materialized transform parity, folded traversal across every DEM consumer, Python operator ergonomics, Python list operations, or exact Python API shape.

## Benchmark Rows

Non-primary report-only rows:

- `pf1-dem-counts-repeat`
- `pf1-dem-without-tags`

Recorded probe reports from the original PF1 DEM API slice:

- `target/benchmarks/pf1-dem-counts-probe/baseline.json`
- `target/benchmarks/pf1-dem-counts-compare/compare.json`
- `target/benchmarks/pf1-dem-without-tags-probe/baseline.json`
- `target/benchmarks/pf1-dem-without-tags-compare/compare.json`

Recorded probe rates from the original PF1 DEM API slice:

- `stab_dem_counts_nested_repeat`: `1.042e7 queries/s`.
- `stab_dem_final_coordinate_shift_nested_repeat`: `1.695e7 queries/s`.
- `stab_dem_detector_coordinates_nested_repeat`: `2.178e5 queries/s`.
- `stab_dem_without_tags_nested_repeat`: `2.809e6 queries/s`.

These benchmarks remain `non-primary-report-only` because pinned Stim exposes comparable behavior through C++ and Python APIs but not through a faithful Rust direct baseline.
They were not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api --quiet
cargo test -p stab-core --test dem_api pf1_dem_ --quiet
cargo test -p stab-core --test dem_api pf1_dem_counts_ --quiet
cargo test -p stab-core --test dem_api pf1_dem_iterators_ --quiet
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

- Public `flattened`, `rounded`, and complete transform APIs.
- Resource-boundary tests and folded traversal for materialized transform operations and coordinate-map cases that can produce very large outputs.

## PFM0 Refresh

The PFM0 refresh promoted `pf1-dem-rust-api` from a manifest-only extraction row to executable structural evidence and synchronized the checklist row for DEM construction and mutation to `Done for current Rust API surface`.
No additional non-Python mutation ergonomics are selected beyond the existing constructors, push helpers, append helper, clear, and derived `Clone`; Python-style operators and list ergonomics remain deferred.
