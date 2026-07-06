# PFM4 DEM Search Zero-Shift Repeat Scope

## Objective

Promote one selected PFM4 search traversal subcase: graphlike and hypergraph logical-error search over large flat DEM repeat bodies that contain nonzero-probability `error` instructions plus `shift_detectors` instructions whose detector shift is zero.

For graphlike and hypergraph search, a zero detector shift inside the repeated body is a semantic no-op because it does not change detector target offsets.
Stab should treat zero-detector-shift `shift_detectors` instructions as zero search work inside otherwise selected flat repeat bodies instead of rejecting the repeat solely because the body is no longer `error`-only.

## Positive Scope

- Graphlike search accepts large flat repeat bodies whose instructions are nonzero-probability `error` instructions or `shift_detectors` instructions with detector shift exactly zero, including coordinate-only shifts such as `shift_detectors(4, 5) 0`.
- Hypergraph search accepts the same selected repeat body shape.
- Zero-detector-shift `shift_detectors` instructions in those selected bodies count as zero search work and are replayed once through the existing compact body traversal.
- Mixed bodies such as `error(0.1)`, `shift_detectors 0`, and `error(0.2) L0` compare against the compact one-body model.
- Pure no-target bodies with zero detector shifts followed by active search errors compare against the model with the repeated body removed.

## Explicit Non-Scope

- Nonzero detector shifts remain capped or rejected by the existing repeat-expansion resource boundary.
- `detector`, `logical_observable`, nested repeats, non-flat bodies, mixed non-shift instructions, numeric raw error targets, separator-only error target lists, SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, and simulator-product APIs are unchanged.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where a selected repeated body is represented once, or where a pure no-target repeated body is removed, because zero detector shifts and no-target errors have no detector-parity or logical-observable effect on graphlike and hypergraph search.

## Tests

Owned tests:

- `pf4_dem_search_folds_flat_zero_detector_shift_repeat_bodies` for graphlike search.
- `pf4_hypergraph_zero_detector_shift_repeat_folds_by_compact_model` for hypergraph search.

The tests must prove:

- pure no-target repeated bodies with `shift_detectors 0` do not force graphlike or hypergraph repeat expansion;
- mixed no-target plus detectorless logical-only bodies with zero detector shifts fold to compact graphlike and hypergraph models;
- detector-touching bodies with coordinate-only zero detector shifts fold to compact graphlike and hypergraph models;
- nonzero detector shifts remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-dem-search-sat-repeat-resource-rust` for the graphlike zero-shift repeat test that matches the `pf4_dem_search_` filter.
- Add `pf4-dem-hypergraph-zero-shift-repeat-rust` for the hypergraph-specific zero-shift repeat test.

## Benchmark Rows

- Add `pf4-dem-search-zero-shift-repeat` as a non-primary report-only contract-only row with graphlike and hypergraph submeasurements.
- Record work units as folded zero-shift target-error occurrences per second.

The row remains report-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search zero_detector_shift --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-dem-search-zero-shift-repeat --out target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline
just bench::compare --only pf4-dem-search-zero-shift-repeat --baseline target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-zero-shift-repeat-compare
```
