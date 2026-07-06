# PFM4 DEM Search Annotation Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large flat DEM repeat bodies whose instructions are nonzero-probability `error` instructions, zero-detector-shift `shift_detectors` instructions, `detector` annotations, or standalone `logical_observable` annotations.
For graphlike and hypergraph search, detector and logical-observable annotations are search-neutral because they do not create error mechanisms and do not change detector target offsets.

## Explicit Non-Scope

This slice does not change SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, nonzero detector shifts, nested repeats, non-flat repeats, mixed non-annotation instructions, separator-only error target lists, numeric raw error targets, Python, diagrams, CLI behavior, or simulator-product APIs.
The broad `pf4-dem-folded-traversal` row remains manifest-only because other traversal consumers and repeat shapes still need folded behavior, precise caps, or explicit deferral.

## Comparator And Evidence

Comparator class: structural Rust parity.
For selected annotation-bearing repeated bodies, graphlike and hypergraph search compare against the compact one-body model because annotation instructions have no detector-parity or logical-observable error effect.
Nonzero detector shifts remain rejected through the existing repeat-expansion cap.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_flat_repeat_error_count` now treats `detector` and `logical_observable` instructions as zero search work within otherwise selected flat repeat bodies.
It still requires a flat body of error, zero-detector-shift, detector, or logical-observable instructions, still rejects zero-probability error bodies to preserve the existing zero-probability skip path, still rejects numeric raw targets, still rejects separator-only target lists, still rejects nonzero detector shifts, and still counts only relative-detector or logical-observable target-bearing error instructions as folded search work.

## Tests

Added tests:

- `pf4_dem_search_folds_flat_annotation_repeat_bodies`
- `pf4_hypergraph_annotation_repeat_folds_by_compact_model`

The tests prove:

- graphlike search folds detector-touching repeated bodies containing `detector` and standalone `logical_observable` annotations to the compact model;
- graphlike search folds detectorless logical-only repeated bodies containing `detector` and standalone `logical_observable` annotations to the compact model;
- hypergraph search folds detector-touching repeated bodies containing `detector` and standalone `logical_observable` annotations to the compact model;
- hypergraph search folds detectorless logical-only repeated bodies containing `detector` and standalone `logical_observable` annotations to the compact model;
- nonzero detector shifts remain outside the selected graphlike and hypergraph fold and still reject before unbounded expansion.

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-sat-repeat-resource-rust`
- `pf4-dem-hypergraph-annotation-repeat-rust`

Updated benchmark rows:

- `pf4-dem-search-annotation-repeat` adds `stab_pf4_dem_graphlike_annotation_repeat_fold` and `stab_pf4_dem_hyper_annotation_repeat_fold` with `folded-annotated-target-errors/s` measurement work.

The benchmark row remains non-primary report-only and contract-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compare:

```text
stab_pf4_dem_graphlike_annotation_repeat_fold=0.000002112s, rate=9.470e11 folded-annotated-target-errors/s
stab_pf4_dem_hyper_annotation_repeat_fold=0.000002022s, rate=9.891e11 folded-annotated-target-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-annotation-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-annotation-repeat-baseline/report.md`
- `target/benchmarks/pfm4-dem-search-annotation-repeat-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-annotation-repeat-compare/report.md`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search annotation_repeat --quiet
cargo test -p stab-core --test dem_search zero_detector_shift --quiet
cargo test -p stab-core --test dem_search no_target_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench runner_smoke --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
just bench::baseline --only pf4-dem-search-annotation-repeat --out target/benchmarks/pfm4-dem-search-annotation-repeat-baseline
just bench::compare --only pf4-dem-search-annotation-repeat --baseline target/benchmarks/pfm4-dem-search-annotation-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-annotation-repeat-compare
```

## Audit And Review Closure

`milestone-audit` status is complete for this slice. The implemented evidence covers the owned graphlike and hypergraph selected flat annotation-bearing repeat shapes, compact-model semantic parity, detector-touching and detectorless logical-only error combinations, explicit nonzero-shift cap preservation, oracle metadata, report-only benchmark metadata, and synchronized roadmap/checklist updates, while leaving broader folded traversal under the active PF4 umbrella.

`full-code-review` used two GPT-5.5/xhigh sidecars. The Rust/compatibility reviewer found no blocking issues and confirmed the selected annotation-bearing fold is consistent with graphlike and hypergraph search semantics because `detector` and standalone `logical_observable` annotations affect metadata but not error graph edges. The docs/oracle/benchmark reviewer found a P2 closure gap in this report and the RPF4 rollup; this section and the RPF4 audit log now record the audit and review closure. Both reviewers noted the relevant file-size watch list: `ops/bench/src/baseline/pf4.rs` and `crates/stab-core/tests/dem_search.rs` are near, but still below, the 1200-line hard threshold.
