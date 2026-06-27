# M11 Progress Report

## Milestone

M11: Detector Error Model Sampling

## Status

Partial progress, not milestone-complete.
This slice implements the first deterministic `sample_dem` path, the first one-bit noisy statistical `sample_dem` row, the M11-owned structural subset of `src/stim/simulators/dem_sampler.test.cc`, and initial M11 benchmark comparison runners.

## Tests Ported Or Created

- `cargo test -p stab-core --test dem_sampler` covers the initial `CompiledDemSampler` subset ported from `src/stim/simulators/dem_sampler.test.cc`, including empty and sparse sizing, high detector and observable ids, `error(1)` detector toggling, `error(0)` no-op behavior, p=0.25, p=0.5, and p=0.75 probability bands, separator handling, detector-observable correlation, correlated detector-combination parity, detector shifts, repeat blocks, logical observable flips, dense bit-packed detector and observable output, fixed-seed noisy sampling reproducibility, and one-bit p=0.25 statistical behavior.
- `cargo test -p stab-cli sample_dem` covers the existing `m11-sample-dem-deterministic` oracle row for `stab sample_dem --shots 3` against pinned Stim v1.16.0 output, the `m11-sample-dem-noisy-statistical` one-bit seeded distribution row, and the upstream `--obs_out` detector/observable split behavior.
- `just oracle::run --milestone M11 --exact` covers the implemented deterministic exact-output row after the manifest status is promoted from `red` to `implemented`.
- `just oracle::run --milestone M11 --statistical` covers the implemented noisy one-bit statistical row.
- `just oracle::run --milestone M11 --structural` covers the implemented `coverage-simulators-dem-sampler` structural row.
- `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners` covers Stab comparison runners for the M11 DEM sampler row, `sample_dem` CLI row, and sparse, dense, repeated, and high-detector contract rows.

## Implementation Areas

- Added `CompiledDemSampler` in `stab-core` with reusable compiled DEM operations, seeded sampling, detector-shift handling, repeat-block unrolling with a bounded initial limit, and shared `DetectionConversionOutput` records.
- Added `stab sample_dem` in `stab-cli` with `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--append_observables`, hidden `--prepend_observables`, `--obs_out`, and `--obs_out_format` arguments.
- Reused the existing detection-event and observable record writers so `sample_dem` uses the same output format behavior as `detect` and `m2d`.
- Promoted `m11-sample-dem-deterministic`, `m11-sample-dem-noisy-statistical`, and `coverage-simulators-dem-sampler` in `oracle/fixtures/manifest.csv` to `implemented`.
- Added Stab benchmark comparison runners for `m11-dem-sampler`, `m11-sample-dem-cli`, `m11-sample-dem-sparse-contract`, `m11-sample-dem-dense-contract`, `m11-sample-dem-repeated-contract`, and `m11-sample-dem-high-detector-contract`.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `CompiledDemSampler` reusable sampling state | Partial | `CompiledDemSampler::compile`, `CompiledDemSampler::sample_detection_events_with_seed`, `cargo test -p stab-core --test dem_sampler` including dense `b8` output coverage |
| `stim sample_dem` deterministic CLI output | Partial | `m11-sample-dem-deterministic`, `cargo test -p stab-cli sample_dem_deterministic`, `cargo test -p stab-cli sample_dem_writes_observables`, `just oracle::run --milestone M11 --exact` |
| `stim sample_dem` one-bit noisy statistical CLI output | Partial | `m11-sample-dem-noisy-statistical`, `cargo test -p stab-cli sample_dem_noisy`, `just oracle::run --milestone M11 --statistical` |
| Sparse, dense, repeated, high-detector-count, and correlated-error fixture groups | Partial | `coverage-simulators-dem-sampler`, `cargo test -p stab-core --test dem_sampler`, `just oracle::run --milestone M11 --structural`; benchmark-scale fixture groups remain future work |
| M11 benchmark reporting | Partial | `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners`; `just bench::compare --milestone M11` now reports all M11 Stab-side benchmark rows, while strict Stab-vs-Stim comparison still depends on the selected baseline artifact containing measured M11 pinned-Stim rows |

## Verification Commands

- `cargo test -p stab-core --test dem_sampler --quiet`
- `cargo test -p stab-cli sample_dem --quiet`
- `just oracle::run --milestone M11 --exact`
- `just oracle::run --milestone M11 --statistical`
- `just oracle::run --milestone M11 --structural`
- `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners --quiet`
- `just bench::compare --milestone M11`
