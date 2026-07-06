# RPF4 DEM Coordinate Progress Report

## Summary

This RPF4 slice hardens Rust DEM coordinate-map resource behavior and adds report-only benchmark coverage for the current coordinate API subset.
It is not an RPF4 completion report because folded traversal across every DEM consumer, non-flat ambiguous-overlap coordinate ranges above the bounded flattened-declaration scan, diagram APIs, and Python binding shape remain outside this slice or still active.

## Implemented Surfaces

- Added a 1,000,000 detector cap to `DetectorErrorModel::detector_coordinates`, the all-detector coordinate-map convenience API.
- The cap rejects huge materialized coordinate maps before constructing the list of every detector id.
- The error points callers to `DetectorErrorModel::detector_coordinates_for`, which accepts typed `DemDetectorId` values for selected lookups.
- Selected-detector and single-detector coordinate lookups remain available for huge-repeat models when the requested detectors are reachable without materializing the full coordinate map.
- Selected lookups now use folded repeat indexing for non-overlapping repeat detector declarations, so late detectors in huge repeats do not require flattened linear iteration.
- Bounded overlapping repeat declarations preserve first-declaration semantics when several repeat iterations could declare the same detector id.
- Flat sparse overlapping repeat bodies now compute direct detector-declaration candidates algebraically before falling back to bounded probing, so selected lookups can find first-declaration coordinates beyond the previous one-million candidate cap.
- The flat repeat path scans detector declarations once against the selected detector set, returns empty coordinates for valid sparse holes with no coordinate declaration, and has many-selected all-map benchmark coverage so the sparse fast path does not regress bounded coordinate maps into per-detector body scans.
- Non-flat repeat bodies whose nested structure expands to at most 1,000,000 detector declarations now use the same algebraic selected-lookup scan after a bounded local declaration flattening pass, so nested sparse overlap cases can find coordinates beyond the previous one-million outer-candidate fallback cap.
- The bounded nested scan streams local detector declarations into the arithmetic matcher and delays coordinate-vector materialization until a selected detector candidate wins, avoiding eager allocation for nonmatching declarations.
- The pinned Stim v1.16.0 trivial selected-coordinate examples are now ported exactly for a single declared detector, error-only detector allocation with empty coordinates, shifted detector declarations, and out-of-range selected detector rejection.
- PF4 transform evidence now separately covers final detector shifts, final coordinate shifts, detector counts, observable counts, error counts, and selected coordinate lookups through shifted repeats.
- The pinned Stim generated surface-code coordinate case now succeeds through the PFM6 bounded mixed-top-level analyzer fallback: `fold_loops=true` produces a DEM for the generated prefix, repeat, and tail circuit shape, and the DEM detector coordinates match the circuit detector coordinates for all 168 detectors.

## Tests

Implemented Rust tests:

- `pf4_dem_coordinates_reject_huge_all_map_but_allow_selected_queries`
- `pf4_dem_coordinates_trivial_selected_matches_pinned_stim_examples`
- `pf4_dem_coordinates_fold_late_selected_detector_lookup`
- `pf4_dem_coordinates_nested_loop_matches_pinned_stim_example`
- `pf4_dem_coordinates_fold_nested_sparse_repeat_without_candidate_cap`
- `pf4_dem_coordinates_preserve_first_overlapping_repeat_declaration`
- `pf4_dem_coordinates_fold_many_selected_overlapping_repeat_declarations`
- `pf4_dem_coordinates_fold_sparse_overlapping_repeat_without_candidate_cap`
- `pf4_dem_coordinates_flat_sparse_repeat_hole_returns_empty`
- `pf4_dem_coordinates_huge_flat_repeat_does_not_overvalidate_far_endpoint`

These tests cover pinned Stim trivial selected-coordinate examples, huge all-map rejection, selected coordinate lookup through a huge repeat, single-detector lookup through the same huge-repeat model, folded late selected lookup through a billion-record non-overlapping repeat, the pinned Stim nested-loop coordinate example, nested sparse overlapping repeat lookup whose first matching declaration is beyond the previous one-million outer-candidate cap, first-declaration behavior for bounded overlapping repeats, many-selected flat overlapping repeat declarations, sparse flat overlapping repeat lookup whose first matching declaration is beyond the previous one-million candidate cap, valid sparse flat detector holes that must return empty coordinates instead of a resource-cap error, and huge flat repeats whose far endpoint exceeds the typed detector-id ceiling while the selected detector is still valid.

Related PFM6 generated-loop coordinate evidence:

- `pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit`

This test documents the pinned Stim generated surface-code coordinate comparison, while broader true folded generated-loop output remains PFM6 work.

## Oracle Rows

Implemented row:

- `pf4-dem-coordinate-resource-rust`

Related PFM6 generated-loop coordinate row:

- `pf6-analyzer-generated-fold-loop-fallback-rust`

Still broad and manifest-only:

- `pf4-dem-introspection-transforms`
- `pf4-dem-coordinate-api`
- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-coordinate-map`

The row measures a bounded all-coordinate map, a folded selected-coordinate lookup through a huge-repeat model, the sparse flat overlapping selected-coordinate fast path, the bounded nested sparse overlapping selected-coordinate fast path, and a many-selected flat-overlap all-map path.
It remains `non-primary-report-only` because it measures Rust public APIs and pinned Stim does not provide a faithful Rust direct timing baseline in this harness.
It is not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api pf4_dem_coordinates_ --quiet
cargo test -p stab-core --test dem_api pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
just oracle::run --milestone PF4 --structural
just bench::smoke
just bench::baseline --only pf4-dem-coordinate-map --out target/benchmarks/pf4-coordinate-nested-sparse-probe
just bench::compare --only pf4-dem-coordinate-map --baseline target/benchmarks/pf4-coordinate-nested-sparse-probe/baseline.json --report target/benchmarks/pf4-coordinate-nested-sparse-compare
```

The focused nested-sparse coordinate probe recorded `stab_pf4_dem_coordinate_map_nested_sparse_overlap=0.000000288s`, or `3.472e6 selected-detectors/s`, in `target/benchmarks/pf4-coordinate-nested-sparse-compare/report.md`.

## Audit And Review

Milestone-audit status for the original coordinate-resource slice: complete against the current PFM4 coordinate-resource text, with broader non-flat ambiguous coordinate ranges above the bounded flattened-declaration scan left as documented remaining RPF4 work.
That audit checked that the slice named the supported nested sparse-overlap subcase, kept all-detector materialization capped, preserved selected lookup behavior through typed `DemDetectorId`, updated oracle and benchmark metadata, and kept the benchmark row report-only.

Milestone-audit status for the follow-up trivial-coordinate and generated-loop coordinate slice: complete with broader PFM6 generated-loop folding still active.
The follow-up audit checked that pinned Stim trivial selected-coordinate behavior is direct PFM4 coordinate evidence, while the generated surface-code coordinate case is counted only for the selected bounded analyzer fallback and coordinate-equivalence subcase.

Full-code-review status for the follow-up slice: findings resolved.
Two GPT-5.5/xhigh sidecars found that the generated-loop evidence was easy to miss because it was not covered by the documented PF4 coordinate filter; the generated coordinate case now has a dedicated PF6 oracle row, `pf6-analyzer-generated-fold-loop-fallback-rust`, and the verification commands name it explicitly.
The earlier Rust/resource review finding about eager coordinate-vector materialization remains resolved by streaming local declarations into `FlatRepeatScan` and delaying coordinate materialization until a candidate wins.

## Remaining RPF4 Work

- Finish folded traversal or explicit caps for graphlike search, hypergraph search, SAT or WCNF encoding beyond selected flat zero-shift repeat folding, matcher-adjacent operations, remaining sampled-error sampler work, and analyzer-adjacent operations.
- Broader true folded generated-loop analyzer output remains PFM6 work; the selected pinned `surface_code_coords_dont_infinite_loop` coordinate case is covered through bounded fallback evidence.
- Finish or explicitly cap any later-promoted nested or non-flat ambiguous overlapping selected-coordinate ranges that need more than the current bounded flattened-declaration scan or fallback search.
- Decide whether any Rust-specific copy, concat, repetition, or mutation helpers beyond existing `Clone`, `push_instruction`, `push_repeat_block`, and `append_from_dem_text` are still worth adding.
- Add remaining malformed-input or resource-boundary cases only if later RPF4 work promotes behavior beyond the current validation, introspection, and coordinate-resource subsets.
