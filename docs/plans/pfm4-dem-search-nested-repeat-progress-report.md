# PFM4 DEM Search Nested Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large nested DEM repeat bodies whose nested bodies contain only nonzero-probability `error` instructions, zero-detector-shift `shift_detectors` instructions, `detector` annotations, standalone `logical_observable` annotations, and nested selected repeat blocks with total detector shift zero.
For graphlike and hypergraph search, the selected zero-shift repeat hierarchy only duplicates the same search edges, so it can be represented by the compact one-body search model.

## Explicit Non-Scope

This slice does not change SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, shifted nested repeats, non-flat repeats, numeric raw error targets, Python, diagrams, CLI behavior, or simulator-product APIs.
The broad `pf4-dem-folded-traversal` row remains manifest-only because other traversal consumers and repeat shapes still need folded behavior, precise caps, or explicit deferral.

## Comparator And Evidence

Comparator class: structural Rust parity.
For selected nested zero-shift repeated bodies, graphlike and hypergraph search compare against the compact model where the effective nested body appears once.
Nested nonzero detector shifts remain rejected through the existing repeat-expansion cap.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_compact_repeat_error_count` now recurses through nested selected repeat blocks instead of accepting only top-level flat bodies.
It requires each promoted repeat body to have total detector shift zero, counts only relative-detector or logical-observable target-bearing nonzero-probability error instructions as folded search work, treats no-target errors and search-neutral annotations as zero search work, preserves the zero-probability skip path, and still rejects nonzero detector shifts, numeric raw targets, and unselected repeat shapes.

## Tests

Added tests:

- `pf4_dem_search_folds_nested_zero_shift_repeat_bodies`
- `pf4_hypergraph_nested_zero_shift_repeat_folds_by_compact_model`

The tests prove:

- graphlike search folds a large outer repeat containing a large inner zero-shift detector-touching body to the compact model;
- graphlike search folds a large outer repeat containing a large inner zero-shift detectorless logical-only body to the compact model;
- graphlike search folds a large outer repeat containing a large inner zero-shift no-target body to the compact model;
- hypergraph search folds the same detector-touching, detectorless logical-only, and no-target nested shapes to the compact model;
- nested nonzero detector shifts remain outside the selected graphlike and hypergraph fold and still reject before unbounded expansion.

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-nested-repeat-rust`
- `pf4-dem-hypergraph-nested-repeat-rust`

Updated benchmark rows:

- `pf4-dem-search-nested-repeat` adds `stab_pf4_dem_graphlike_nested_repeat_fold` and `stab_pf4_dem_hyper_nested_repeat_fold` with `folded-nested-target-errors/s` measurement work.

The benchmark row remains non-primary report-only and contract-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compare:

```text
stab_pf4_dem_graphlike_nested_repeat_fold=0.000002224s, rate=8.993e17 folded-nested-target-errors/s
stab_pf4_dem_hyper_nested_repeat_fold=0.000002084s, rate=9.597e17 folded-nested-target-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-nested-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-nested-repeat-baseline/report.md`
- `target/benchmarks/pfm4-dem-search-nested-repeat-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-nested-repeat-compare/report.md`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search_nested_repeat nested_zero_shift --quiet
cargo test -p stab-core --test dem_search pf4_dem_search --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::baseline --only pf4-dem-search-nested-repeat --out target/benchmarks/pfm4-dem-search-nested-repeat-baseline
just bench::compare --only pf4-dem-search-nested-repeat --baseline target/benchmarks/pfm4-dem-search-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-nested-repeat-compare
```

## Audit And Review Closure

`milestone-audit` status is complete for this slice. The implemented evidence covers the owned graphlike and hypergraph selected nested zero-shift repeat shapes, compact-model semantic parity, detector-touching, detectorless logical-only, and no-target nested bodies, explicit nested nonzero-shift cap preservation, oracle metadata, report-only benchmark metadata, and synchronized roadmap/checklist updates, while leaving broader folded traversal under the active PF4 umbrella.

`full-code-review` used two GPT-5.5/xhigh sidecars. The Rust/compatibility reviewer found no blocking issues, confirmed the selected compact-repeat predicate preserves the existing nonzero-shift rejection and flat-repeat behavior, and noted only residual risks around full workspace and oracle coverage that were covered by the local verification pass. The docs/oracle/benchmark reviewer found three P2 documentation and evidence issues: the scope note used a stale zero-test command, the no-target nested subcase was claimed without recorded evidence, and this report still had a closure placeholder. The scope note command now points at `dem_search_nested_repeat`, the nested no-target graphlike and hypergraph regression is implemented and recorded, and this section records the audit and review closure.
