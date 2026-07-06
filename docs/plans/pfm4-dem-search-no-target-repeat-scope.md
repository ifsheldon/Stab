# PFM4 DEM Search No-Target Repeat Scope

## Objective

Promote one selected PFM4 search traversal subcase: graphlike and hypergraph logical-error search over large flat zero-shift DEM repeat bodies containing nonzero-probability `error` instructions with no detector or logical-observable targets.

The search semantics of those no-target errors are no-ops because they cannot change detector parity or logical-observable parity.
For selected flat zero-shift repeat bodies, Stab should skip no-target errors when deciding whether the body can be folded for graphlike or hypergraph search instead of rejecting the repeat solely because a nonzero no-target error would exceed the materialized repeat cap.

## Positive Scope

- Graphlike search accepts large flat zero-shift repeat bodies whose instructions are all nonzero-probability `error` instructions, where each error has either at least one detector or logical-observable target, or no targets at all.
- Hypergraph search accepts the same selected repeat body shape.
- No-target error instructions in those selected bodies count as zero search work and are replayed once through the existing compact body traversal, where they have no effect on the search graph.
- Mixed bodies such as `error(0.1)` plus `error(0.2) L0` compare against the compact one-body model.
- Pure no-target bodies followed by active search errors compare against the model with the no-target repeat removed.

## Explicit Non-Scope

- SAT/WCNF generation is unchanged in this slice.
- Analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, and simulator-product APIs are unchanged.
- Shifted, nested, non-flat, mixed-instruction, numeric-target, separator-only, and detectorless no-target shapes outside flat `error`-only zero-shift bodies remain capped, rejected, or owned by other PFM4 slices.
- No public CLI behavior changes.

## Comparator Class

Comparator class: structural Rust parity.
The compact comparator is a DEM where no-target repeated errors are removed or retained once in the compact body, because they have no detector or logical-observable effect on graphlike and hypergraph search.

## Tests

Owned tests:

- `pf4_dem_search_skips_flat_nonzero_no_target_repeat_bodies` for graphlike search.
- `pf4_hypergraph_no_target_repeat_skips_by_compact_model` for hypergraph search.

The tests must prove:

- pure no-target repeated bodies do not force graphlike or hypergraph repeat expansion;
- mixed no-target plus detectorless logical-only bodies fold to compact graphlike and hypergraph models;
- numeric-target repeated bodies remain outside the selected fold and still reject before unbounded expansion.

## Oracle Rows

- Update `pf4-dem-search-sat-repeat-resource-rust` only for the graphlike no-target repeat test that already matches the `pf4_dem_search_` filter.
- Add `pf4-dem-hypergraph-no-target-repeat-rust` for the hypergraph-specific no-target repeat test.

## Benchmark Rows

- Extend `pf4-dem-folded-graphlike-traversal` with a graphlike no-target repeat skip submeasurement.
- Add `pf4-dem-hypergraph-no-target-repeat` as a non-primary report-only contract-only row for the hypergraph counterpart.

Both rows remain report-only because they are Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search_skips_flat_nonzero_no_target_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf4_hypergraph_no_target_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
```

Fresh focused benchmark probes:

```sh
just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-no-target-graphlike-baseline
just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-no-target-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-no-target-graphlike-compare
just bench::baseline --only pf4-dem-hypergraph-no-target-repeat --out target/benchmarks/pfm4-dem-search-no-target-hypergraph-baseline
just bench::compare --only pf4-dem-hypergraph-no-target-repeat --baseline target/benchmarks/pfm4-dem-search-no-target-hypergraph-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-no-target-hypergraph-compare
```
