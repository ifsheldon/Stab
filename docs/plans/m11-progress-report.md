# M11 Progress Report

## Milestone

M11: Detector Error Model Sampling

## Status

Partial progress, not milestone-complete.
This slice implements the first deterministic `sample_dem` path and the first one-bit noisy statistical `sample_dem` row, while broad format parity, sparse and dense fixture groups, high-detector-count fixture groups, and M11 benchmark reporting remain pending.

## Tests Ported Or Created

- `cargo test -p stab-core --test dem_sampler` covers the initial `CompiledDemSampler` subset, including `error(1)` detector toggling, `error(0)` no-op behavior, detector shifts, repeat blocks, logical observable flips, fixed-seed noisy sampling reproducibility, and one-bit p=0.25 statistical behavior.
- `cargo test -p stab-cli sample_dem` covers the existing `m11-sample-dem-deterministic` oracle row for `stab sample_dem --shots 3` against pinned Stim v1.16.0 output, the `m11-sample-dem-noisy-statistical` one-bit seeded distribution row, and the upstream `--obs_out` detector/observable split behavior.
- `just oracle::run --milestone M11 --exact` covers the implemented deterministic exact-output row after the manifest status is promoted from `red` to `implemented`.
- `just oracle::run --milestone M11 --statistical` covers the implemented noisy one-bit statistical row and still reports the broader `coverage-simulators-dem-sampler` row as manifest-only.

## Implementation Areas

- Added `CompiledDemSampler` in `stab-core` with reusable compiled DEM operations, seeded sampling, detector-shift handling, repeat-block unrolling with a bounded initial limit, and shared `DetectionConversionOutput` records.
- Added `stab sample_dem` in `stab-cli` with `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--append_observables`, hidden `--prepend_observables`, `--obs_out`, and `--obs_out_format` arguments.
- Reused the existing detection-event and observable record writers so `sample_dem` uses the same output format behavior as `detect` and `m2d`.
- Promoted `m11-sample-dem-deterministic` and `m11-sample-dem-noisy-statistical` in `oracle/fixtures/manifest.csv` to `implemented`.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `CompiledDemSampler` reusable sampling state | Partial | `CompiledDemSampler::compile`, `CompiledDemSampler::sample_detection_events_with_seed`, `cargo test -p stab-core --test dem_sampler` |
| `stim sample_dem` deterministic CLI output | Partial | `m11-sample-dem-deterministic`, `cargo test -p stab-cli sample_dem_deterministic`, `cargo test -p stab-cli sample_dem_writes_observables`, `just oracle::run --milestone M11 --exact` |
| `stim sample_dem` one-bit noisy statistical CLI output | Partial | `m11-sample-dem-noisy-statistical`, `cargo test -p stab-cli sample_dem_noisy`, `just oracle::run --milestone M11 --statistical` |
| Sparse, dense, repeated, and high-detector-count fixture groups | Missing | `coverage-simulators-dem-sampler` remains manifest-only |
| M11 benchmark reporting | Missing | `bench-dem-sampler` has no Stab comparison runner yet |

## Verification Commands

- `cargo test -p stab-core --test dem_sampler --quiet`
- `cargo test -p stab-cli sample_dem --quiet`
- `just oracle::run --milestone M11 --exact`
- `just oracle::run --milestone M11 --statistical`
