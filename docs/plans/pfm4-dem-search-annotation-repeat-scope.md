# PFM4 DEM Search Annotation Repeat Scope

## Objective

Promote one selected PFM4 search traversal subcase: graphlike and hypergraph logical-error search over large flat DEM repeat bodies that contain nonzero-probability `error` instructions plus search-neutral `detector` or `logical_observable` annotation instructions.

For graphlike and hypergraph search, `detector` coordinate declarations and standalone `logical_observable` declarations do not create error mechanisms and do not change detector target offsets.
Stab should treat these instructions as zero search work inside otherwise selected flat repeat bodies instead of rejecting the repeat solely because the body is no longer `error`-only.

## Positive Scope

- Graphlike search accepts large flat repeat bodies whose instructions are nonzero-probability `error`, zero-detector-shift `shift_detectors`, `detector`, or `logical_observable` instructions.
- Hypergraph search accepts the same selected repeat body shape.
- `detector` and `logical_observable` instructions in those selected bodies count as zero search work and are replayed once through the existing compact body traversal.
- Detector-touching and detectorless logical-only error bodies with annotation instructions compare against the compact one-body model.

## Explicit Non-Scope

- Nonzero detector shifts remain capped or rejected by the existing repeat-expansion resource boundary.
- Nested repeats, non-flat bodies, mixed non-annotation instructions, numeric raw error targets, separator-only error target lists, SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, and simulator-product APIs are unchanged.
- Annotation instructions are promoted only for graphlike and hypergraph search; this slice does not claim folded annotation handling for every DEM traversal consumer.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where the selected repeated body is represented once, because annotation instructions have no detector-parity or logical-observable error effect on graphlike and hypergraph search.

## Tests

Owned tests:

- `pf4_dem_search_folds_flat_annotation_repeat_bodies` for graphlike search.
- `pf4_hypergraph_annotation_repeat_folds_by_compact_model` for hypergraph search.

The tests must prove:

- detector annotations inside selected large flat repeats do not force graphlike or hypergraph repeat expansion;
- standalone logical-observable annotations inside selected large flat repeats do not force graphlike or hypergraph repeat expansion;
- detector-touching and detectorless logical-only error combinations with annotations fold to compact graphlike and hypergraph models;
- nonzero detector shifts remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-dem-search-sat-repeat-resource-rust` for the graphlike annotation-repeat test that matches the `pf4_dem_search_` filter.
- Add `pf4-dem-hypergraph-annotation-repeat-rust` for the hypergraph-specific annotation-repeat test.

## Benchmark Rows

- Add `pf4-dem-search-annotation-repeat` as a non-primary report-only contract-only row with graphlike and hypergraph submeasurements.
- Record work units as folded annotated target-error occurrences per second.

The row remains report-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search annotation_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probe:

```sh
just bench::baseline --only pf4-dem-search-annotation-repeat --out target/benchmarks/pfm4-dem-search-annotation-repeat-baseline
just bench::compare --only pf4-dem-search-annotation-repeat --baseline target/benchmarks/pfm4-dem-search-annotation-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-annotation-repeat-compare
```
