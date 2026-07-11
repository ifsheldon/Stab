# PFM6 Loop-Carried Observable Folding Scope

Historical note: this selected period-shaped slice is now subsumed by the generic PFM-B5 recurrence engine. Its retired benchmark ID is historical evidence only; current ownership is in `docs/plans/pfm-b5-analyzer-search-progress-report.md`.

## Summary

This note records the selected PFM6 analyzer slice for the pinned Stim v1.16.0 `ErrorAnalyzer.loop_folding` case where a huge odd repeat count has a tail `OBSERVABLE_INCLUDE` that adds the same logical observable to the repeated body errors.
The slice is implemented as a true compact `fold_loops=true` output path, not as bounded unrolling.

## Owned Positive Case

The owned upstream shape is:

```stim
MR 1
REPEAT 12345678987654321 {
    X_ERROR(0.25) 0
    CX 0 1
    MR 1
    DETECTOR rec[-2] rec[-1]
}
M 0
OBSERVABLE_INCLUDE(9) rec[-1]
```

The expected public CLI output is:

```dem
error(0.25) D0 L9
repeat 6172839493827159 {
    error(0.25) D1 L9
    error(0.25) D2 L9
    shift_detectors 2
}
error(0.25) D1 L9
error(0.25) D2 L9
```

## Implementation Boundary

- The recognizer starts from the existing top-level prefix, repeat, and tail analyzer seam.
- It compares the one-iteration-plus-tail DEM against the prefix DEM and requires the selected body terms to match the repeated body errors with added logical observables.
- It requires the folded body to contain only error instructions, to have a nonzero detector shift, and to use an odd repeat count of at least three.
- It validates the compact candidate against a measurement-record-lookback-sized non-folded expansion before using the compact candidate for the caller's full repeat count.
- Unsupported shapes keep the existing bounded non-folded fallback and its current repeat-count, aggregate-repeat, and expanded-instruction caps.

## Non-Goals

- The period-8 loop-folding example from the same upstream test is implemented by the separate [pfm6-period8-observable-folding-scope.md](pfm6-period8-observable-folding-scope.md) slice, not by this loop-carried-error slice.
- The period-127 loop-folding example from the same upstream test is implemented by the separate [pfm6-period127-observable-folding-scope.md](pfm6-period127-observable-folding-scope.md) slice, not by this loop-carried-error slice.
- Nested loop-folding cases from `ErrorAnalyzer.loop_folding_nested_loop` are not implemented by this slice.
- Generated-code true folded analyzer output remains under-specified outside the selected detector-chain, loop-carried observable, period-8 observable, and period-127 observable shapes.
- Even repeat-count loop-carried observable behavior, arbitrary loop-carried observable periods, full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain outside this slice.

## Evidence

Tests:

- `pf6_dem_analyzer_loop_carried_observable_folds_like_upstream`
- `cargo test -p stab-core --test dem_analyzer_loop_folding --quiet`

Oracle rows:

- `pf6-analyzer-loop-carried-observable-rust`
- `pf6-analyze-errors-loop-carried-observable-cli`

Benchmark row:

- `pf6-analyzer-loop-observable-folded`

The benchmark row is non-primary report-only because it measures the Rust analyzer contract for the selected folding shape without a faithful pinned Stim direct Rust timing comparator in the current harness.

## Verification

```sh
cargo test -p stab-core --test dem_analyzer_loop_folding --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF6
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
just bench::smoke
```
