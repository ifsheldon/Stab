# PFM6 Period-127 Observable Folding Scope

## Summary

This note records the selected PFM6 analyzer slice for the pinned Stim v1.16.0 `ErrorAnalyzer.loop_folding` case whose logical observable oscillates with period 127 across a huge repeat.
The slice is implemented as an exact `fold_loops=true` output-shape parity case for `stab analyze_errors`, not as broad logical-observable period solving.

## Owned Positive Case

The owned upstream shape is:

```stim
R 0 1 2 3 4 5 6
REPEAT 12345678987654321 {
    CNOT 0 1 1 2 2 3 3 4 4 5 5 6 6 0
    DETECTOR
}
M 6
OBSERVABLE_INCLUDE(9) rec[-1]
R 7
X_ERROR(1) 7
M 7
DETECTOR rec[-1]
```

The expected public CLI output starts with detector declarations `D0` through `D85`, contains a middle repeat count of `97210070768930` with detector declarations `D86` through `D212` followed by `shift_detectors 127`, then emits `error(1) D211`, detector declarations `D86` through `D210`, and `logical_observable L9`.
The full byte-for-byte output is tracked by `oracle/fixtures/expected/pf6_analyze_errors_period127_observable.stdout`.

## Implementation Boundary

- The recognizer owns the selected seven-qubit reset, seven-CNOT cycle, empty detector declaration, final `M 6`, `OBSERVABLE_INCLUDE(9) rec[-1]`, and deterministic `R 7; X_ERROR(1) 7; M 7; DETECTOR rec[-1]` tail shape.
- It emits compact output for repeat counts at least 465 that are congruent to 84 modulo 127, which includes the pinned huge repeat count and source-owned benchmark repeat count.
- It emits the Stim-style period-127 folded detector declaration layout with 86 leading detector declarations, a middle repeat of 127 detector declarations plus `shift_detectors 127`, a deterministic error on `D211`, 125 trailing detector declarations, and the logical observable declaration.
- It validates the compact candidate against a non-folded 338-iteration expansion before using the compact candidate for the caller's full repeat count.
- The 338-iteration single-middle-repeat boundary intentionally stays on the bounded non-folded path because pinned Stim v1.16.0 does not print the compact `repeat 1` shape for that input.
- The same circuit shape with non-owned repeat-count residues uses the bounded non-folded analyzer when it is within the repeat cap and reports the existing analyzer repeat-count cap instead of falling through to the generic compact folder when it is too large.
- Shapes outside this selected family keep the existing bounded non-folded fallback or existing simpler folded output.

## Non-Goals

- Nested loop-folding cases from `ErrorAnalyzer.loop_folding_nested_loop` are not implemented by this slice.
- Arbitrary logical-observable periods, arbitrary CNOT graphs, other final measurement observables, tagged variants, other deterministic tail checks, generated-code true folded analyzer output, full ErrorMatcher provenance, heralded matching, repeat-contained noise provenance, and `stim explain_errors` remain outside this slice.

## Evidence

Tests:

- `pf6_dem_analyzer_period127_observable_folds_like_upstream`
- `pf6_dem_analyzer_period127_observable_folds_minimum_compact_shape_like_upstream`
- `pf6_dem_analyzer_period127_observable_keeps_single_middle_repeat_unfolded`
- `pf6_dem_analyzer_period127_observable_keeps_adjacent_residue_unfolded`
- `pf6_dem_analyzer_period127_observable_rejects_huge_adjacent_residue`
- `cargo test -p stab-core --test dem_analyzer_loop_folding --quiet`

Oracle rows:

- `pf6-analyzer-period127-observable-rust`
- `pf6-analyze-errors-period127-observable-cli`

Benchmark row:

- `pf6-analyzer-period127-observable-folded`

The benchmark row is non-primary report-only because it measures the Rust analyzer contract for the selected period-127 folding shape without a faithful pinned Stim direct Rust timing comparator in the current harness.

## Verification

```sh
cargo test -p stab-core --test dem_analyzer_loop_folding --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF6
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
just bench::smoke
```
