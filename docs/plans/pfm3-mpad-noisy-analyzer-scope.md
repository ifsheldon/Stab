# PFM3 Noisy MPAD Analyzer Scope

## Summary

This note selects one exact PFM3 legal non-tableau execution subcase: `MPAD(p)` measurement-pad flip probabilities in `circuit_to_detector_error_model` and the public `stab analyze_errors` command.
It extends the existing selected `MPAD` execution boundary only for analyzer propagation of pad-flip noise into detector and observable terms.

## Selected Positive Cases

- `MPAD(0.25) 0 1` followed by detector declarations emits one independent error per pad measurement, matching pinned Stim v1.16.0.
- `MPAD(0.25) 0` followed by an observable declaration emits an observable-only error, matching pinned Stim v1.16.0.
- `MPAD(0.25) 0 1` with detector and observable declarations emits the combined detector and observable target for the matching pad record, matching pinned Stim v1.16.0.
- `MPAD(0)` remains deterministic and does not add an error mechanism.

## Comparator

The core comparator is exact DEM text for the selected Rust analyzer cases.
The CLI comparator is exact-output oracle parity against pinned Stim v1.16.0 for `stab analyze_errors`.

## Negative Scope

- This slice does not claim exact random-stream parity for stochastic pad sampling.
- This slice does not promote broader stochastic `MPAD(p)` behavior outside the analyzer. The selected sampler and detection-sampling record-flip behavior is owned separately by `docs/plans/pfm3-stochastic-mpad-sampler-detection-scope.md`.
- This slice does not select another legal non-tableau gate family.

## Resource Behavior

The selected cases add one completed analyzer error per `MPAD` target when the probability is nonzero.
They do not change analyzer repeat expansion, parser limits, CLI input caps, or output streaming behavior.

## Oracle And Benchmark Policy

Oracle row `pf3-analyze-errors-mpad-noisy-cli` records the selected exact CLI behavior.
No benchmark row is added because this is a small semantic bookkeeping correction on the analyzer measurement-record path, not a new hot throughput path or public performance claim.

## Verification Commands

- `cargo test -p stab-core --test dem_analyzer_measurements mpad --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF3 --exact`
- `just oracle::run --milestone PF3 --structural`
- `just bench::smoke`
