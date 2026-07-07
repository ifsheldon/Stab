# PFM4 DEM Search Annotation-Only Repeat Progress Report

## Scope

This report records the selected annotation-only DEM repeat resource slice described in [pfm4-dem-search-annotation-only-repeat-scope.md](pfm4-dem-search-annotation-only-repeat-scope.md).
The promoted behavior is intentionally narrow: graphlike search, hypergraph search, unweighted SAT, and weighted WCNF treat oversized flat or nested zero-detector-shift repeat bodies containing only `detector`, standalone `logical_observable`, and zero-detector-shift `shift_detectors` declarations as contributing no repeat-count-scaled search edges, no SAT/WCNF error variables, and no SAT/WCNF dense target cap pressure when high sparse ids appear only in annotations.

This is not a full folded-traversal implementation.
Repeats with nonzero detector shifts, repeats containing active errors, high sparse ids that appear on actual `error` targets, ErrorMatcher filter annotation-only repeats, analyzer traversal, DEM sampler sampled-error output, replay behavior, CLI behavior, Python bindings, diagrams, and broader public graph/vector simulator APIs remain governed by their existing plans and caps.

## Implementation And Tests

- Added `pf4_dem_search_skips_annotation_only_repeat_bodies` and `pf4_dem_search_skips_high_id_annotation_only_repeat_bodies` in [dem_search.rs](../../crates/stab-core/tests/dem_search.rs).
- The tests construct oversized outer repeats with `detector`, standalone `logical_observable`, a coordinate-only `shift_detectors(5, 7) 0`, and oversized nested annotation-only repeats, including a high-id case with sparse detector and logical-observable annotations beyond the SAT/WCNF dense target cap.
- The comparator is the compact DEM with the repeated annotation-only body removed.
- The assertions compare graphlike search output, hypergraph search output, unweighted SAT output, and weighted WCNF output against the compact model.
- Updated [sat.rs](../../crates/stab-core/src/dem/sat.rs) so SAT/WCNF dense target counts are derived from flattened error targets instead of full DEM annotation counts.

The existing selected repeat predicates already classify detector declarations, standalone logical-observable declarations, and zero-detector-shift shifts as search/SAT neutral instructions.
The production change closes the high-id loophole where annotation-only declarations could still affect SAT/WCNF dense target limits despite not creating error variables.

## Oracle And Benchmarks

- Updated `pf4-dem-search-sat-repeat-resource-rust` in [manifest.csv](../../oracle/fixtures/manifest.csv) so the existing PF4 structural oracle row owns the new test.
- No new oracle row was added because the existing row runs `cargo test -p stab-core --test dem_search pf4_dem_search_`, which includes the new test.
- No new benchmark row was added because this slice is admission/resource evidence for avoiding repeat-count-scaled work, not a throughput path.
- The active throughput-style folded search/SAT rows remain the existing report-only PF4 rows for zero-probability, no-target, zero-shift, annotation-bearing, mixed zero-probability, nested, and SAT/WCNF repeat families.

## Documentation

Updated the PFM4 scope and rollup docs to name selected annotation-only zero-shift repeat skipping for graphlike search, hypergraph search, unweighted SAT, and weighted WCNF, including high sparse annotation ids, while preserving ErrorMatcher filter annotation-only repeats as capped:

- [rpf4-dem-search-sat-progress-report.md](rpf4-dem-search-sat-progress-report.md)
- [non-deferred-partial-feature-milestones.md](non-deferred-partial-feature-milestones.md)
- [partial-feature-inventory.md](partial-feature-inventory.md)
- [remaining-partial-feature-milestones.md](remaining-partial-feature-milestones.md)
- [rust-stim-drop-in-rewrite.md](rust-stim-drop-in-rewrite.md)
- [../stab-feature-checklist.md](../stab-feature-checklist.md)

## Verification

Passed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test dem_search pf4_dem_search_skips_annotation_only_repeat_bodies --quiet
cargo test -p stab-core --test dem_search pf4_dem_search_skips_high_id_annotation_only_repeat_bodies --quiet
cargo test -p stab-core --lib sat_problem --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
just maintenance::pre-commit
```

## Audit And Review

Local `milestone-audit` status is complete for this selected slice.
The evidence satisfies the scope note by testing the promised graphlike, hypergraph, unweighted SAT, and weighted WCNF compact-model parity, including the high sparse annotation-id boundary; updating the owning PF4 oracle row; documenting the no-benchmark rationale; and preserving explicit non-goals for high ids on actual error targets, ErrorMatcher filter annotation-only repeats, and broader traversal consumers.

`full-code-review` used two GPT-5.5/xhigh sidecars.
The Rust/core sidecar found no blocking issues and confirmed that deriving SAT/WCNF dense target counts from flattened error targets is correct for this source-owned behavior, that actual error-target dense caps remain covered, and that the high-id test meaningfully proves the annotation-only boundary.
The docs/oracle/benchmark sidecar found no blocking issues and confirmed the PF4 oracle row, checklist, roadmap, inventory, and no-benchmark rationale consistently distinguish high sparse annotation ids from high ids on actual error targets.

The final pre-commit large-file check recorded [dem_search.rs](../../crates/stab-core/tests/dem_search.rs) at 1196 lines, below the 1200-line source-file threshold but still on the watch list for the next PF4 test addition.
