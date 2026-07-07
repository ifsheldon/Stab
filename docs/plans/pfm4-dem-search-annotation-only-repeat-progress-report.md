# PFM4 DEM Search Annotation-Only Repeat Progress Report

## Scope

This report records the selected annotation-only DEM repeat resource slice described in [pfm4-dem-search-annotation-only-repeat-scope.md](pfm4-dem-search-annotation-only-repeat-scope.md).
The promoted behavior is intentionally narrow: graphlike search, hypergraph search, unweighted SAT, and weighted WCNF treat oversized flat or nested zero-detector-shift repeat bodies containing only `detector`, standalone `logical_observable`, and zero-detector-shift `shift_detectors` declarations as contributing no repeat-count-scaled search edges or SAT/WCNF error variables.

This is not a full folded-traversal implementation.
Repeats with nonzero detector shifts, repeats containing active errors, ErrorMatcher filter annotation-only repeats, analyzer traversal, DEM sampler sampled-error output, replay behavior, CLI behavior, Python bindings, diagrams, and broader public graph/vector simulator APIs remain governed by their existing plans and caps.

## Implementation And Tests

- Added `pf4_dem_search_skips_annotation_only_repeat_bodies` in [dem_search.rs](../../crates/stab-core/tests/dem_search.rs).
- The test constructs an oversized outer repeat with `detector`, standalone `logical_observable`, a coordinate-only `shift_detectors(5, 7) 0`, and an oversized nested annotation-only repeat.
- The comparator is the compact DEM with the repeated annotation-only body removed.
- The assertions compare graphlike search output, hypergraph search output, unweighted SAT output, and weighted WCNF output against the compact model.

No production code change was needed because the existing selected repeat predicates already classify detector declarations, standalone logical-observable declarations, and zero-detector-shift shifts as search/SAT neutral instructions.

## Oracle And Benchmarks

- Updated `pf4-dem-search-sat-repeat-resource-rust` in [manifest.csv](../../oracle/fixtures/manifest.csv) so the existing PF4 structural oracle row owns the new test.
- No new oracle row was added because the existing row runs `cargo test -p stab-core --test dem_search pf4_dem_search_`, which includes the new test.
- No new benchmark row was added because this slice is admission/resource evidence for avoiding repeat-count-scaled work, not a throughput path.
- The active throughput-style folded search/SAT rows remain the existing report-only PF4 rows for zero-probability, no-target, zero-shift, annotation-bearing, mixed zero-probability, nested, and SAT/WCNF repeat families.

## Documentation

Updated the PFM4 scope and rollup docs to name selected annotation-only zero-shift repeat skipping for graphlike search, hypergraph search, unweighted SAT, and weighted WCNF while preserving ErrorMatcher filter annotation-only repeats as capped:

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
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
just maintenance::pre-commit
```

## Audit And Review

Local `milestone-audit` status is complete for this selected slice.
The evidence satisfies the scope note by testing the promised graphlike, hypergraph, unweighted SAT, and weighted WCNF compact-model parity; updating the owning PF4 oracle row; documenting the no-benchmark rationale; and preserving explicit non-goals for ErrorMatcher filter annotation-only repeats and broader traversal consumers.

`full-code-review` used two GPT-5.5/xhigh sidecars.
The Rust/core sidecar found no blocking issues and confirmed the `100001` repeat count sits above the current unroll cap, so the test meaningfully proves repeat-iteration materialization is avoided for the selected graphlike, hypergraph, unweighted SAT, and weighted WCNF paths.
That review noted a wording risk that "no-op resource work" could overclaim because consumers still perform normal count/setup work and the test uses small annotation ids; the scope and report wording now say the slice avoids repeat-count-scaled search-graph edge work and SAT/WCNF error-variable work instead.
The docs/oracle/benchmark sidecar found no blocking issues and confirmed the PF4 oracle row, checklist, roadmap, and inventory consistently keep ErrorMatcher filter annotation-only bodies capped.

The final pre-commit large-file check recorded [dem_search.rs](../../crates/stab-core/tests/dem_search.rs) at 1181 lines, below the 1200-line source-file threshold but on the watch list for the next PF4 test addition.
