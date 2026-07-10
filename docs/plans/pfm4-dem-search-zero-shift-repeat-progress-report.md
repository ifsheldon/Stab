# PFM4 DEM Search Zero-Shift Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large flat DEM repeat bodies whose instructions are nonzero-probability `error` instructions or `shift_detectors` instructions with detector shift exactly zero.
For graphlike and hypergraph search, zero detector shifts are search-neutral because they do not change detector target offsets, even when coordinate-shift arguments are present.

## Explicit Non-Scope

This slice does not change SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, nonzero detector shifts, nested repeats, non-flat repeats, `detector` or `logical_observable` instructions inside selected search repeats, separator-only error target lists, numeric raw error targets, Python, diagrams, CLI behavior, or simulator-product APIs.
At the time of this slice, the broad `pf4-dem-folded-traversal` row remained manifest-only. PFM-B3 later promotes it to an implemented umbrella after migrating the selected consumers and documenting inherent caps.

## Comparator And Evidence

Comparator class: structural Rust parity.
For pure no-target repeated bodies with `shift_detectors 0`, graphlike and hypergraph search compare against the same DEM with the no-target repeat removed.
For mixed no-target plus detectorless logical-only repeated bodies with `shift_detectors 0`, graphlike and hypergraph search compare against the compact one-body model because the zero detector shift and no-target errors have no search effect.
Nonzero detector shifts remain rejected through the existing repeat-expansion cap.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_flat_repeat_error_count` now treats zero-detector-shift `shift_detectors` instructions as zero search work within otherwise selected flat repeat bodies.
It still requires a flat body of error or zero-detector-shift instructions, still rejects zero-probability error bodies to preserve the existing zero-probability skip path, still rejects numeric raw targets, still rejects separator-only target lists, still rejects nonzero detector shifts, and still counts only detector or logical-observable target-bearing errors as folded search work.

## Tests

Added tests:

- `pf4_dem_search_folds_flat_zero_detector_shift_repeat_bodies`
- `pf4_hypergraph_zero_detector_shift_repeat_folds_by_compact_model`

The tests prove:

- graphlike search skips pure no-target repeated bodies containing `shift_detectors 0` before the repeat cap;
- graphlike search folds mixed no-target plus detectorless logical-only repeated bodies containing `shift_detectors 0` to the compact model;
- graphlike search folds detector-touching repeated bodies containing coordinate-only zero detector shifts to the compact model;
- hypergraph search skips pure no-target repeated bodies containing `shift_detectors 0` before the repeat cap;
- hypergraph search folds mixed no-target plus detectorless logical-only repeated bodies containing `shift_detectors 0` to the compact model;
- hypergraph search folds detector-touching repeated bodies containing coordinate-only zero detector shifts to the compact model;
- nonzero detector shifts remain outside the selected graphlike and hypergraph fold and still reject before unbounded expansion.

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-sat-repeat-resource-rust`
- `pf4-dem-hypergraph-zero-shift-repeat-rust`

Updated benchmark rows:

- `pf4-dem-search-zero-shift-repeat` adds `stab_pf4_dem_graphlike_zero_shift_repeat_fold` and `stab_pf4_dem_hyper_zero_shift_repeat_fold` with `folded-zero-shift-target-errors/s` measurement work.

The benchmark row remains non-primary report-only and contract-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compare:

```text
stab_pf4_dem_graphlike_zero_shift_repeat_fold=0.000000400s, rate=2.500e12 folded-zero-shift-target-errors/s
stab_pf4_dem_hyper_zero_shift_repeat_fold=0.000000370s, rate=2.703e12 folded-zero-shift-target-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline/report.md`
- `target/benchmarks/pfm4-dem-search-zero-shift-repeat-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-zero-shift-repeat-compare/report.md`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search zero_detector_shift --quiet
cargo test -p stab-core --test dem_search no_target_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
just bench::baseline --only pf4-dem-search-zero-shift-repeat --out target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline
just bench::compare --only pf4-dem-search-zero-shift-repeat --baseline target/benchmarks/pfm4-dem-search-zero-shift-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-zero-shift-repeat-compare
```

## Audit And Review Closure

`milestone-audit` status is complete for this slice. The implemented evidence covers the owned graphlike and hypergraph selected flat repeat shapes, compact-model semantic parity, detector-touching coordinate-only zero detector shifts, no-target and detectorless logical-only combinations, explicit nonzero-shift cap preservation, oracle metadata, report-only benchmark metadata, and synchronized roadmap/checklist updates, while leaving broader folded traversal under the active PF4 umbrella.

`full-code-review` used two GPT-5.5/xhigh sidecars. The Rust/compatibility reviewer found no blocking issues and noted residual risks around coordinate-only shifts and detector-touching coverage; detector-touching coordinate-only zero detector shift regression coverage was added. The docs/oracle/benchmark reviewer found a P2 closure gap in this report and the RPF4 rollup; this section and the RPF4 audit log now record the audit and review closure.
