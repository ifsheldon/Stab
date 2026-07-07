# PFM4 ErrorMatcher Filter Flat Repeat Progress Report

## Scope

This PFM4 slice owns one matcher-adjacent DEM traversal subcase: `explain_errors_from_circuit` filter DEMs whose large repeat body contains only `error(p)` instructions, each repeated error instruction touches at least one detector target, and the body has zero detector shift by construction.

The selected behavior is compact filter-key collection for repeated identical detector-touching error keys:

- `explain_errors_from_circuit(circuit, Some(filter), ...)` collects each selected repeated flat filter body error once at the current detector offset instead of rejecting on the generic filter DEM repeat-count cap.
- The filter remains a set of allowed DEM error keys, so repeated identical keys do not change the public matching result.
- Shifted, nested, mixed-instruction, detectorless logical-only, analyzer, circuit-repeat noise, stack-frame provenance, and `explain_errors` CLI surfaces stay outside this slice.

## Comparator And Evidence

Comparator class: structural Rust parity for selected large flat zero-shift ErrorMatcher filter DEM repeats.
Pinned Stim v1.16.0 would materialize repeated filter keys, so large selected repeats use semantic equivalence against a compact folded filter model instead of exact upstream text.

Existing rejection tests remain the comparator for shifted filter DEM repeats and repeat-contained circuit noise.

## Implemented Slice

`explain_errors_from_circuit` now validates filter DEMs with an ErrorMatcher-specific DEM budget instead of the generic materialized flattening budget.
That budget recognizes selected flat detector-touching repeat bodies before applying the generic repeat-count cap.
Filter-key collection also recognizes the same selected body and visits it once at the current detector offset.

Filter probabilities remain ignored, matching the existing ErrorMatcher filter-key semantics.
The selected body must contain only error instructions with at least one detector target and no numeric targets.
At this slice boundary, shifted, nested, mixed-instruction, detectorless logical-only, analyzer, circuit-repeat noise, stack-frame provenance, and `explain_errors` CLI paths kept the previous caps or explicit rejections.
Later PFM4 slices promote selected nested detector-touching and detectorless logical-observable-only filter repeats.

## Tests

Targeted tests prove:

- A selected large flat zero-shift filter DEM repeat produces the same `ExplainedError` output as the compact single-body filter model.
- Non-selected shifted filter DEM repeats remain capped and rejected before materializing repeated keys.
- Repeat-contained circuit noise remains rejected until recursive ErrorMatcher provenance is deliberately selected.

New test:

- `pf4_error_matcher_filter_folds_flat_detector_repeat`.

Existing tests retained:

- `pf4_error_matcher_repeat_resource_policy_is_source_owned`.

## Oracle And Benchmarks

Metadata changes:

- Added `pf4-error-matcher-filter-repeat-rust` with `src/stim/simulators/error_matcher.test.cc` provenance for selected ErrorMatcher filter DEM repeat behavior.
- Added `pf4-error-matcher-filter-flat-repeat` with `stab_pf4_error_matcher_filter_flat_repeat_fold` and `folded-filter-keys/s` work units.
- Kept the benchmark row out of primary timing gates because the compact large-repeat behavior is source-owned semantic evidence, not a faithful pinned-Stim timing ratio.

Fresh focused compare:

```text
stab_pf4_error_matcher_filter_flat_repeat_fold=0.000001834s, rate=5.453e11 folded-filter-keys/s
```

Artifacts:

- `target/benchmarks/pfm4-error-matcher-filter-flat-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-error-matcher-filter-flat-repeat-compare/compare.json`

## Verification

Focused commands run after implementation:

```sh
cargo test -p stab-core --test dem_traversal_resource pf4_error_matcher --quiet
cargo test -p stab-core --test dem_traversal_resource pf4_error_matcher_filter --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::smoke
just bench::baseline --only pf4-error-matcher-filter-flat-repeat --out target/benchmarks/pfm4-error-matcher-filter-flat-repeat-baseline
just bench::compare --only pf4-error-matcher-filter-flat-repeat --baseline target/benchmarks/pfm4-error-matcher-filter-flat-repeat-baseline/baseline.json --report target/benchmarks/pfm4-error-matcher-filter-flat-repeat-compare
```

Milestone-audit status: complete for this selected slice. No blocking implementation defects or milestone loopholes were found; broader shifted, nested, mixed-instruction, detectorless logical-only, analyzer traversal, ErrorMatcher circuit-repeat provenance, and `explain_errors` CLI work remained explicitly outside this slice.
Full-code-review status: complete with two GPT-5.5/xhigh sidecars. The core reviewer found no blocking issues and noted residual future coverage for richer filter bodies; this slice added a richer folded filter regression covering multiple body errors, logical terms, duplicate detector cancellation, separators, and nonzero outer detector offsets. The docs and benchmark reviewer found a P2 provenance issue where the filter evidence was attached to broad analyzer and graphlike rows; that was fixed by splitting focused `pf4-error-matcher-filter-repeat-rust` oracle evidence and `pf4-error-matcher-filter-flat-repeat` benchmark evidence sourced to `src/stim/simulators/error_matcher.test.cc`.
