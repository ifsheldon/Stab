# PFM6 Period-8 Observable Folding Scope

## Summary

This note records the selected PFM6 analyzer slice for the pinned Stim v1.16.0 `ErrorAnalyzer.loop_folding` case whose logical observable oscillates with period 8 across a huge repeat.
The slice is implemented as an exact `fold_loops=true` output-shape parity case for `stab analyze_errors`, not as broad logical-observable loop folding.

## Owned Positive Case

The owned upstream shape is:

```stim
R 0 1 2 3 4
REPEAT 12345678987654321 {
    CNOT 0 1 1 2 2 3 3 4
    DETECTOR
}
M 4
OBSERVABLE_INCLUDE(9) rec[-1]
```

The expected public CLI output is:

```dem
detector D0
detector D1
detector D2
repeat 1543209873456789 {
    detector D3
    detector D4
    detector D5
    detector D6
    detector D7
    detector D8
    detector D9
    detector D10
    shift_detectors 8
}
detector D3
detector D4
detector D5
detector D6
detector D7
detector D8
logical_observable L9
```

## Implementation Boundary

- The recognizer owns the selected five-qubit reset, four-CNOT chain, empty detector declaration, final `M 4`, and `OBSERVABLE_INCLUDE(9) rec[-1]` shape.
- It accepts repeat counts at least 9 that are congruent to 1 modulo 8, which includes the pinned huge repeat count and source-owned benchmark repeat count.
- It emits the Stim-style period-8 folded detector declaration layout with three leading detector declarations, a middle repeat of eight detector declarations plus `shift_detectors 8`, six trailing detector declarations, and the logical observable declaration.
- It validates the compact candidate against a non-folded nine-iteration expansion before using the compact candidate for the caller's full repeat count.
- Shapes outside this selected family keep the existing bounded non-folded fallback or existing simpler folded output.

## Non-Goals

- The period-127 loop-folding example from the same upstream test is implemented by the separate [pfm6-period127-observable-folding-scope.md](pfm6-period127-observable-folding-scope.md) slice, not by this period-8 slice.
- Nested loop-folding cases from `ErrorAnalyzer.loop_folding_nested_loop` are not implemented by this slice.
- Arbitrary logical-observable periods, arbitrary CNOT graphs, other final measurement observables, tagged variants, generated-code true folded analyzer output, full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain outside this slice.

## Evidence

Tests:

- `pf6_dem_analyzer_period8_observable_folds_like_upstream`
- `cargo test -p stab-core --test dem_analyzer_loop_folding --quiet`

Oracle rows:

- `pf6-analyzer-period8-observable-rust`
- `pf6-analyze-errors-period8-observable-cli`

Benchmark row:

- `pf6-analyzer-period8-observable-folded`

The benchmark row is non-primary report-only because it measures the Rust analyzer contract for the selected period-8 folding shape without a faithful pinned Stim direct Rust timing comparator in the current harness.

## Verification

```sh
cargo test -p stab-core --test dem_analyzer_loop_folding --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF6
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
just bench::smoke
```
