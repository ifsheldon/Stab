# PFM4 DEM Search Annotation-Only Repeat Scope

## Objective

Promote one small PFM4 traversal evidence slice: oversized zero-detector-shift DEM repeat bodies containing only search-neutral annotations should not force graphlike search, hypergraph search, or SAT/WCNF generation to materialize every repeat iteration.

`detector`, standalone `logical_observable`, and zero-detector-shift `shift_detectors` instructions do not create graphlike edges, hypergraph edges, or SAT error variables.
For selected repeat bodies containing only those instructions, Stab should avoid repeat-count-scaled search-graph edge work, avoid SAT/WCNF error-variable work, and avoid SAT/WCNF dense target caps caused only by annotation ids for the affected consumers.

## Positive Scope

- Graphlike search skips oversized flat or nested annotation-only repeat bodies when followed by active errors.
- Hypergraph search skips the same selected annotation-only repeat bodies.
- Unweighted SAT and weighted WCNF generation skip the same selected annotation-only repeat bodies.
- Selected bodies may contain `detector` declarations, standalone `logical_observable` declarations, and `shift_detectors` instructions whose detector shift is exactly zero, including coordinate-only shifts.
- Selected bodies may use high sparse detector and logical-observable ids in those annotations, including ids beyond the dense SAT/WCNF target cap, when active error mechanisms still touch only a compact target set.
- The compact comparator is the same model with the annotation-only repeated body removed.

## Explicit Non-Scope

- Repeats with nonzero detector shifts remain capped or rejected.
- Repeats containing active error mechanisms remain governed by the existing selected graphlike, hypergraph, SAT/WCNF, and ErrorMatcher repeat-folding slices.
- High sparse detector or logical-observable ids appearing on actual `error` targets remain governed by the existing search sparse-indexing behavior and SAT/WCNF dense target caps.
- ErrorMatcher filter DEM annotation-only repeats remain capped as already locked by `pf4_error_matcher_filter_rejects_annotation_only_repeat`.
- Analyzer traversal, DEM sampler sampled-error output, replay behavior, Python, diagrams, CLI behavior, public graph/vector simulator APIs, and full folded traversal across all consumers are unchanged.

## Comparator Class

Comparator class: structural Rust parity.
The selected annotation-only repeated model must produce the same graphlike search output, hypergraph search output, unweighted SAT string, and weighted WCNF string as the compact model that omits the repeated body.
This is a source-owned resource-hardening comparator for annotation-only DEM structures, not a byte-for-byte pinned-Stim WCNF sizing claim for annotation-only high-id models.

## Tests

Owned tests:

- Add `pf4_dem_search_skips_annotation_only_repeat_bodies`.
- Add `pf4_dem_search_skips_high_id_annotation_only_repeat_bodies`.

The tests should prove graphlike, hypergraph, unweighted SAT, and weighted WCNF parity against the compact model, and should include a nested annotation-only repeat plus a coordinate-only zero-detector shift.
The high-id test should prove the same compact parity when annotation-only declarations contain sparse detector and logical-observable ids that would exceed dense SAT/WCNF target caps if treated as SAT targets.

## Oracle Rows

- Update `pf4-dem-search-sat-repeat-resource-rust` to describe selected annotation-only graphlike, hypergraph, SAT, and WCNF repeat skipping.
- No new oracle row is needed because the existing PF4 Rust row already runs `cargo test -p stab-core --test dem_search pf4_dem_search_`, and the new test is in that filtered subset.

## Benchmark Rows

- Do not add a new benchmark row.
- This is an admission/resource evidence slice for avoiding repeat-count-scaled work, not a distinct throughput path.
- Existing report-only folded-traversal rows still cover active search and SAT/WCNF folding workloads.

## Verification

Focused verification:

```sh
cargo test -p stab-core --test dem_search pf4_dem_search_skips_annotation_only_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf4_dem_search_skips_high_id_annotation_only_repeat_bodies --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
```

Standard pre-commit verification remains required before committing.
