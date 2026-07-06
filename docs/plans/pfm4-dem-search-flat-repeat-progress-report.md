# PFM4 DEM Search Flat Repeat Progress Report

## Scope

This PFM4 slice owns one graphlike and hypergraph search traversal subcase: large DEM repeat bodies whose body has zero detector shift, contains only nonzero-probability `error(p)` instructions, and whose repeated error instructions each include at least one detector target.

The selected behavior is compact search-graph construction for repeated identical detector-touching error mechanisms:

- `shortest_graphlike_undetectable_logical_error` represents each selected repeated flat body error once at the current detector offset instead of unrolling all repeat iterations.
- `find_undetectable_logical_error` uses the same selected folded traversal for the hypergraph search graph.
- Small selected repeats may also share the folded graph-construction path because graph nodes already deduplicate identical edges and the search objective is unweighted shortest error count.

This slice did not change SAT or WCNF generation, analyzer traversal, ErrorMatcher traversal, sampled-error output, replayed-error output, detectorless logical-only repeated errors, zero-probability structural repeats, shifted active repeats, repeat bodies containing non-error instructions, nested repeat bodies beyond capped outer traversal, dense detector or observable caps, Python APIs, diagrams, or deferred simulator-product surfaces.
The later detectorless logical-only follow-up in `docs/plans/pfm4-dem-search-detectorless-logical-repeat-progress-report.md` promotes the selected flat detectorless logical-only graphlike and hypergraph search case.

## Comparator And Evidence

Comparator class: structural Rust parity for selected large flat zero-shift graphlike and hypergraph search repeat bodies.
Pinned Stim v1.16.0 would materialize these repeated error mechanisms, so large selected repeats use semantic equivalence against a compact folded model instead of exact upstream text.

Existing exact-output tests remain the comparator for ordinary small direct DEM search cases.
Existing rejection tests remain the comparator for shifted active repeats and non-selected shapes.

## Implemented Slice

`Graph::from_dem` for graphlike and hypergraph search now uses a search-graph-specific DEM traversal budget and nonzero-target counter.
That traversal recognizes selected flat detector-touching zero-shift repeat bodies before applying the generic repeat-count cap.
The graph builders then add the selected body once at the current detector offset, relying on the existing graph edge deduplication and unweighted shortest-search objective.

Detectorless logical-only repeated errors were intentionally excluded at the time of this slice because hypergraph search folded detectorless logical rows through a separate distance-1 mask behavior that needed its own compatibility decision.
The later detectorless logical-only follow-up resolves the selected flat graphlike and hypergraph search case.
Shifted, nested, non-flat, mixed-instruction, zero-probability, analyzer, matcher, sampled-error, and replay paths keep the previous caps or explicit rejections.

## Tests

Targeted tests prove:

- A selected large flat zero-shift repeated graphlike body produces the same graphlike shortest-error output as the compact single-body model.
- A selected large flat zero-shift repeated hypergraph body produces the same hypergraph shortest-error output as the compact single-body model.
- Detectorless logical-only repeated errors were outside this fold at the time of this slice; the later detectorless logical-only follow-up promotes the selected flat graphlike and hypergraph search case.
- Existing zero-probability repeat skipping and shifted zero-probability dense-node rejection remain unchanged.

New test:

- `pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies`.

Existing tests retained:

- `pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned`.
- `pf4_dem_search_skips_zero_probability_repeat_bodies`.
- `pf4_dem_search_rejects_shifted_zero_probability_repeat_node_explosion`.

## Oracle And Benchmarks

Metadata changes:

- Extended `pf4-dem-search-sat-repeat-resource-rust` to name the selected graphlike and hypergraph search flat-repeat folding evidence.
- Extended `pf4-dem-folded-traversal` with `stab_pf4_dem_hyper_flat_repeat_fold` and `folded-errors/s` work units.
- Extended `pf4-dem-folded-graphlike-traversal` with `stab_pf4_dem_graphlike_flat_repeat_fold` and `folded-errors/s` work units.
- Kept these rows out of primary timing gates because the compact large-repeat behavior is source-owned semantic evidence, not a faithful pinned-Stim timing ratio.

Fresh focused compares:

```text
stab_pf4_dem_graphlike_flat_repeat_fold=0.000001024s, rate=1.953e12 folded-errors/s
stab_pf4_dem_hyper_flat_repeat_fold=0.000001064s, rate=1.880e12 folded-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-compare/compare.json`
- `target/benchmarks/pfm4-dem-search-flat-repeat-hyper-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-search-flat-repeat-hyper-compare/compare.json`

## Verification

Focused commands run after implementation:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::smoke
just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-baseline
just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-compare
just bench::baseline --only pf4-dem-folded-traversal --out target/benchmarks/pfm4-dem-search-flat-repeat-hyper-baseline
just bench::compare --only pf4-dem-folded-traversal --baseline target/benchmarks/pfm4-dem-search-flat-repeat-hyper-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-flat-repeat-hyper-compare
```

Milestone-audit status: complete for this selected slice.
No blocking findings were found; the audit verified the scoped graphlike and hypergraph search fold, direct tests, PF4 oracle metadata, report-only benchmark runner coverage, measurement work units, compare notes, focused benchmark reports, and explicit exclusions for detectorless logical-only, shifted, nested, non-flat, analyzer, matcher, sampled-error, and replay paths at the time of that slice.
The later detectorless logical-only follow-up promotes the previously excluded flat detectorless logical-only search case without changing the remaining shifted, nested, non-flat, analyzer, matcher, sampled-error, or replay exclusions.
Full-code-review status: complete for this selected slice.
The core GPT-5.5/xhigh sidecar and the docs/oracle/benchmark GPT-5.5/xhigh sidecar found no evidence-backed blocking issues; the local review also found no blocker, with only the existing PF4/PF6 large-file watch list remaining below the 1200-line source threshold.
