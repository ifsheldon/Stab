# PFM4 DEM Search Mixed Zero-Probability Repeat Progress Report

## Scope

This PFM4 slice owns graphlike and hypergraph search over selected large zero-detector-shift DEM repeat bodies whose zero-probability `error` instructions are adjacent to active nonzero-probability `error` instructions.
Zero-probability error instructions are search-neutral for graphlike and hypergraph search because they do not create search edges.

## Explicit Non-Scope

This slice does not change SAT/WCNF generation, analyzer traversal, ErrorMatcher traversal, DEM sampler sampled-error output, replay behavior, nonzero detector shifts, shifted nested repeats, non-flat repeats, numeric raw error targets, Python, diagrams, CLI behavior, or simulator-product APIs.
SAT/WCNF remains governed by its separate probability policy: weighted SAT omits zero-probability variables, while unweighted SAT preserves selected zero-probability structural mechanisms.
The broad `pf4-dem-folded-traversal` row remains partial because other traversal consumers and repeat shapes still need folded behavior, precise caps, or explicit deferral.

## Comparator And Evidence

Comparator class: structural Rust parity.
Graphlike and hypergraph search compare against compact DEMs containing only the active nonzero-probability error mechanisms.
High-index zero-probability targets inside the selected repeated body are ignored by search target counting and must not force dense graph allocation.

## Implemented Surface

`DetectorErrorModel::selected_search_graph_compact_repeat_error_count` now treats zero-probability error instructions as zero search work within otherwise selected zero-shift compact repeat bodies.
It still rejects nonzero detector shifts, shifted nested repeats, non-flat shapes outside the selected instruction set, numeric raw targets, and broader active repeats through the existing caps.

## Tests

Added tests:

- `pf4_dem_search_mixed_zero_probability_repeat_folds_by_compact_model`
- `pf4_hypergraph_mixed_zero_probability_repeat_folds_by_compact_model`

The tests prove:

- graphlike search folds selected mixed zero-probability plus active detector-touching repeated bodies before the repeat cap;
- hypergraph search folds the same selected shape before the repeat cap;
- zero-probability high-index detector and observable targets do not force dense graph allocation;
- detectorless logical-only active errors remain folded when adjacent zero-probability errors are present;
- nested zero-detector-shift repeats containing mixed zero-probability plus active errors compare to the compact model;
- nonzero detector shifts remain outside the selected graphlike and hypergraph fold and still reject before unbounded expansion.

## Oracle And Benchmark Evidence

Updated oracle rows:

- `pf4-dem-search-mixed-zero-probability-repeat-rust`
- `pf4-dem-hypergraph-mixed-zero-probability-repeat-rust`

Updated benchmark rows:

- `pf4-dem-search-mixed-zero-probability-repeat` adds `stab_pf4_dem_graphlike_mixed_zero_probability_repeat_fold` and `stab_pf4_dem_hyper_mixed_zero_probability_repeat_fold` with `folded-active-target-errors/s` measurement work.

The benchmark row remains non-primary report-only and contract-only because it measures Stab Rust API resource behavior without a faithful pinned-Stim timing ratio for oversized folded repeats.

Fresh focused compare:

```text
stab_pf4_dem_graphlike_mixed_zero_probability_repeat_fold=0.000002116s, rate=9.452e11 folded-active-target-errors/s
stab_pf4_dem_hyper_mixed_zero_probability_repeat_fold=0.000002158s, rate=9.268e11 folded-active-target-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline/report.md`
- `target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-compare/report.md`

## Verification

Focused commands run during implementation:

```sh
cargo test -p stab-core --test dem_search_mixed_zero_probability_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
just oracle::run --milestone PF4 --structural
cargo test -p stab-bench runner_smoke --quiet
just bench::smoke
just bench::baseline --only pf4-dem-search-mixed-zero-probability-repeat --out target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline
just bench::compare --only pf4-dem-search-mixed-zero-probability-repeat --baseline target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-mixed-zero-probability-repeat-compare
```

Final verification before commit:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just maintenance::pre-commit
```

## Audit And Review Closure

`milestone-audit` status for this selected slice is complete. The implementation satisfies the scope by treating zero-probability error instructions as zero search work in selected graphlike and hypergraph compact repeat bodies, preserving caps for nonzero detector shifts and other non-selected shapes, adding source-owned graphlike and hypergraph tests, adding implemented oracle rows, adding a report-only benchmark row with measurement work, and updating the roadmap, checklist, inventory, and progress reports.
The audit found one evidence gap before closure: nested mixed zero-probability repeats were in positive scope but not explicitly tested. The gap was fixed by adding nested compact-model assertions to `pf4_dem_search_mixed_zero_probability_repeat_folds_by_compact_model` and `pf4_hypergraph_mixed_zero_probability_repeat_folds_by_compact_model`.

`full-code-review` used two GPT-5.5/xhigh sidecars. The Rust/compatibility sidecar found no confirmed implementation issues and confirmed that SAT semantics remain on their separate path. The docs/oracle/benchmark sidecar found the nested evidence gap plus stale rollup summaries; the nested assertions and rollup updates address those findings.

Residual risk: `ops/bench/src/baseline/pf4.rs` is close to the 1200-line large-file threshold and should be split before the next PF4 benchmark dispatch expansion.
