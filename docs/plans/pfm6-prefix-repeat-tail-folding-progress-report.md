# PFM6 Prefix-Repeat-Tail Folding Progress Report

## Scope

This slice promotes a narrow true-folded `circuit_to_detector_error_model(..., fold_loops=true)` analyzer path for top-level circuits shaped as plain instructions, one `REPEAT` block, and plain tail instructions.
The owned positive cases are loop-carried detector chains where the first iteration, loop body, and tail can be composed into a compact DEM candidate and the candidate validates against a measurement-record-lookback-sized non-folded expansion before it is used for the caller's repeat count.
The owned resource case is the same validated shape with repeat counts above the current bounded fallback unroll cap.
Unsafe prefix/repeat/tail shapes, including tails whose detector declarations cause prior loop-body errors to reach different targets, continue to use the bounded non-folded fallback and preserve the analyzer expansion caps.

## Explicit Non-Goals

- This slice does not claim broad generated surface-code folded output.
- This slice does not change decomposition, approximate-disjoint, gauge-detector, ignored-failure, or matched-error provenance behavior.
- This slice does not add `stim explain_errors` CLI support.
- This slice does not add a new public simulator, Python API, JS/WASM API, or diagram surface.

## Tests

The focused Rust tests live in `crates/stab-core/tests/dem_analyzer_loop_folding.rs`.
They must cover exact compact output for the selected prefix/repeat/tail detector-chain shape, exact compact output when the tail has its own error mechanism, large repeat-count folding for the selected shape, fallback preservation for an unsafe tail dependency that must remain bounded, and fallback preservation for delayed measurement-record dependencies that are not proven by the selected compact fold.

## Oracle And Benchmark Policy

The source-owned oracle row for this slice should supplement the broad `pf6-analyzer-generated-looping` manifest row instead of replacing it.
The existing report-only analyzer benchmark row `pf6-analyze-errors-generated-surface` remains the benchmark evidence for generated analyzer throughput.
This slice does not add a separate benchmark row because the new owned cases are small structural folding contracts plus one compact large-repeat resource case, not a representative generated-code throughput workload.

## Acceptance Criteria

- Selected prefix/repeat/tail detector chains produce compact folded DEM output rather than bounded non-folded output.
- The compact output validates against a measurement-record-lookback-sized non-folded expansion before being returned.
- Unsafe prefix/repeat/tail shapes continue to fall back to the bounded non-folded analyzer and preserve repeat-count, repeat-iteration, and expanded-instruction caps.
- Documentation, oracle metadata, and checklist text identify this as a selected folding slice, not broad generated-loop parity.
