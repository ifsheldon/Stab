# RPF4 DEM Transform Progress Report

## Summary

This RPF4 slice adds the current Rust public materialized `DetectorErrorModel::flattened` and `DetectorErrorModel::rounded` transform subset.
It is not an RPF4 completion report because folded traversal across every DEM consumer, late selected-coordinate traversal optimization, diagram APIs, and Python binding shape remain outside this slice or still active.

## Implemented Surfaces

- Added `DetectorErrorModel::flattened`, which materializes the existing lazy adjusted flattened instruction iterator after validating the DEM flattening budget.
- `flattened` preserves instruction tags, drops repeat tags, drops materialized `shift_detectors` instructions, shifts detector ids, applies coordinate shifts, preserves decomposition separators, and preserves logical observable targets.
- `flattened` rejects excessive materialized repeat expansion using the existing DEM flattening cap instead of trying to allocate unbounded output.
- Added `DetectorErrorModel::rounded`, which recursively rounds only `error` instruction probability arguments and preserves non-error coordinate arguments, targets, tags, repeat structure, and zero-probability errors.

## Tests

Implemented Rust tests:

- `pf4_dem_materialized_flattened_matches_pinned_stim_cases`
- `pf4_dem_materialized_flattened_rejects_excessive_repeat`
- `pf4_dem_materialized_rounded_matches_pinned_stim_probability_cases`
- `pf4_dem_materialized_rounded_keeps_zero_probability_errors`

These tests cover empty models, detector shifts, coordinate shifts, repeat blocks, instruction tags, repeat-tag dropping, logical observables, probability rounding, unchanged non-error coordinate arguments, zero-probability rounded errors, and materialized repeat rejection.

## Oracle Rows

Implemented row:

- `pf4-dem-materialized-transforms-rust`

Still broad and manifest-only:

- `pf4-dem-introspection-transforms`
- `pf4-dem-coordinate-api`
- `pf4-dem-folded-traversal`

## Benchmark Rows

Report-only runner coverage:

- `pf4-dem-flatten-repeat`
- `pf4-dem-rounded`
- `pf4-dem-coordinate-map`, tracked in `docs/plans/rpf4-dem-coordinate-progress-report.md`

Still placeholder rows:

- `pf4-dem-folded-traversal`
- `pf4-dem-folded-graphlike-traversal`
- `pf4-dem-sampler-folded-repeat`

The implemented rows remain `non-primary-report-only` because they measure Rust public APIs and pinned Stim does not provide a faithful Rust direct timing baseline in this harness.
They are not part of the 1.25x primary threshold file.

## Verification Evidence

Target checks for this slice:

```sh
cargo test -p stab-core --test dem_api pf4_dem_materialized_ --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF4
just bench::smoke
```

## Remaining RPF4 Work

- Finish folded coordinate-map and final-shift resource policy where current APIs still require caps or do not prove large nested repeat behavior; the all-coordinate map cap and selected-query fallback are tracked separately in `docs/plans/rpf4-dem-coordinate-progress-report.md`.
- Finish folded or capped traversal evidence for graphlike search, hypergraph search, SAT or WCNF encoding, matcher-adjacent operations, sampler-adjacent operations, and analyzer-adjacent operations.
- Decide whether any Rust-specific copy, concat, repetition, or mutation helpers beyond existing `Clone`, `push_instruction`, `push_repeat_block`, and `append_from_dem_text` are still worth adding.
- Add remaining malformed-input and resource-boundary cases for high detector shifts, high observable counts, invalid separator use, invalid coordinate values, and unsupported transform shapes.
