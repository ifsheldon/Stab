# PFM4 DEM Search No-Target Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large flat zero-shift DEM repeat bodies whose nonzero-probability `error` instructions either have no targets or have graphlike or hypergraph search targets already accepted by the selected flat-repeat search fold.
No-target errors are search no-ops because they affect neither detector parity nor logical-observable parity.

## Explicit Non-Scope

This slice does not change SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, shifted repeats, nested repeats, non-flat repeats, mixed-instruction repeats, numeric raw error targets, Python, diagrams, CLI behavior, or simulator-product APIs.
At the time of this slice, the broad `pf4-dem-folded-traversal` row remained manifest-only. PFM-B3 later promotes it to an implemented umbrella after migrating the selected consumers and documenting inherent caps.

## Comparator And Evidence

Comparator class: structural Rust parity.
For pure no-target repeated bodies, graphlike and hypergraph search compare against the same DEM with the no-target repeat removed.
For mixed no-target plus detectorless logical-only repeated bodies, graphlike and hypergraph search compare against the compact one-body model because no-target errors have no search effect.
Numeric raw error targets remain rejected by the typed `DemInstruction::error` constructor before any repeat traversal can claim support for them.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_flat_repeat_error_count` now treats empty-target nonzero error instructions as zero search work within otherwise selected flat zero-shift repeat bodies.
It still requires a flat body of error instructions, still rejects zero-probability bodies to preserve the existing zero-probability skip path, still rejects numeric raw targets, still rejects separator-only target lists, and still counts only detector or logical-observable target-bearing errors as folded search work.

## Tests

Added tests:

- `pf4_dem_search_skips_flat_nonzero_no_target_repeat_bodies`
- `pf4_hypergraph_no_target_repeat_skips_by_compact_model`

The tests prove:

- graphlike search skips pure no-target repeated bodies before the repeat cap;
- graphlike search folds mixed no-target plus detectorless logical-only repeated bodies to the compact model;
- hypergraph search skips pure no-target repeated bodies before the repeat cap;
- hypergraph search folds mixed no-target plus detectorless logical-only repeated bodies to the compact model;
- numeric raw error targets remain rejected at the typed constructor boundary.

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-sat-repeat-resource-rust`
- `pf4-dem-hypergraph-no-target-repeat-rust`

Updated benchmark rows:

- `pf4-dem-folded-graphlike-traversal` adds `stab_pf4_dem_graphlike_no_target_repeat_skip` with `skipped-no-target-errors/s` measurement work.
- `pf4-dem-hypergraph-no-target-repeat` adds `stab_pf4_dem_hyper_no_target_repeat_skip` with `skipped-no-target-errors/s` measurement work.

Both benchmark rows remain non-primary report-only and contract-only because they measure Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compares:

```text
stab_pf4_dem_graphlike_no_target_repeat_skip=0.000001016s, rate=9.843e11 skipped-no-target-errors/s
stab_pf4_dem_hyper_no_target_repeat_skip=0.000001054s, rate=9.488e11 skipped-no-target-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-no-target-graphlike-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-no-target-graphlike-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-no-target-hypergraph-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-no-target-hypergraph-compare/compare.json`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search_skips_flat_nonzero_no_target_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf4_hypergraph_no_target_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-no-target-graphlike-baseline
just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-no-target-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-no-target-graphlike-compare
just bench::baseline --only pf4-dem-hypergraph-no-target-repeat --out target/benchmarks/pfm4-dem-search-no-target-hypergraph-baseline
just bench::compare --only pf4-dem-hypergraph-no-target-repeat --baseline target/benchmarks/pfm4-dem-search-no-target-hypergraph-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-no-target-hypergraph-compare
```

## Audit And Review Closure

`milestone-audit` status for this selected flat no-target zero-shift graphlike and hypergraph search repeat slice is complete. The implementation satisfies the scoped contract: graphlike and hypergraph search accept the selected flat `error`-only repeat shape, no-target errors count as zero search work, mixed no-target plus detectorless logical-only bodies compare to the compact one-body model, numeric raw error targets remain rejected at the typed constructor boundary, and non-selected shifted, nested, non-flat, mixed-instruction, separator-only, SAT/WCNF, analyzer, ErrorMatcher, sampler, CLI, Python, and diagram surfaces remain outside this slice.

`full-code-review` used two GPT-5.5/xhigh sidecars. The Rust and compatibility reviewer found no blocking findings and noted two scoped residual risks at the time this slice landed: separator-only, non-flat, and zero-probability selector exits were covered mostly by existing PF4 or typed-boundary tests, and semantic no-op instructions such as `shift_detectors 0` remained outside this flat `error`-only fast path. A later PFM4 follow-up promotes the selected flat `shift_detectors 0` graphlike and hypergraph search subcase. The docs, oracle, and benchmark reviewer found one P2 provenance issue where `pf4-dem-hypergraph-no-target-repeat-rust` claimed numeric-target rejection without the filtered hypergraph test executing that assertion; this was fixed by adding `assert_numeric_error_target_rejected()` to `pf4_hypergraph_no_target_repeat_skips_by_compact_model`.
