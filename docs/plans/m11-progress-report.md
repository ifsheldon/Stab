# M11 Progress Report

## Milestone

M11: Detector Error Model Sampling

## Status

Partial progress, not milestone-complete.
This slice implements the first deterministic `sample_dem` path, the first one-bit noisy statistical `sample_dem` row, the M11-owned structural subset of `src/stim/simulators/dem_sampler.test.cc`, initial M11 benchmark comparison runners, `sample_dem` error-record output and replay for Stim result formats including `ptb64`, oracle side-output comparisons for observable, error, and replay streams, explicit buffered-sampling limits, and two audit fixes for `sample_dem` detector/observable routing and hostile nested DEM repeats.

## Tests Ported Or Created

- `cargo test -p stab-core --test dem_sampler` covers the initial `CompiledDemSampler` subset ported from `src/stim/simulators/dem_sampler.test.cc`, including empty and sparse sizing, high detector and observable ids, observables-only errors, `error(1)` detector toggling, `error(0)` no-op behavior, p=0.25, p=0.5, and p=0.75 probability bands, separator handling, detector-observable correlation, correlated detector-combination parity, detector shifts, repeat blocks, bounded repeat-expansion rejection, logical observable flips, sampled-error recording and replay width validation, dense bit-packed detector and observable output, excessive buffered-output and replay-buffer rejection, fixed-seed noisy sampling reproducibility, and one-bit p=0.25 statistical behavior.
- `cargo test -p stab-core result_formats` and `cargo test -p stab-core detection_record_writers_cover_text_and_bit_packed_formats` cover `ptb64` byte layout, replay decoding, detector stream output, and observable stream output helpers.
- `cargo test -p stab-cli sample_dem` covers the existing `m11-sample-dem-deterministic` oracle row for `stab sample_dem --shots 3` against pinned Stim v1.16.0 output, the `m11-sample-dem-noisy-statistical` one-bit seeded distribution row, the upstream `--obs_out` detector/observable split behavior, `--out_format=dets` detector-only stdout with separate observable output, `--err_out` sampled-error output, `--replay_err_in` replay into detector and observable streams, replayed error copying through `--err_out`, `ptb64` detector, observable, error, and replay streams, excessive buffered-output rejection, bounded replay-prefix parsing, and replay shot-count validation.
- `just oracle::run --milestone M11 --exact` covers the implemented deterministic exact-output rows for the basic `sample_dem` CLI path, sparse detector ids, dense detector targets, repeated detector shifts, high detector ids with `b8` output, correlated detector combinations, observable side output, `dets` detector-only stdout with observable side output, sampled-error side output, and replayed error detector, observable, and error-copy side streams.
- `just oracle::run --milestone M11 --statistical` covers the implemented noisy one-bit statistical row.
- `just oracle::run --milestone M11 --structural` covers the implemented `coverage-simulators-dem-sampler` structural row.
- `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners` covers Stab comparison runners for the M11 DEM sampler row, `sample_dem` CLI row, and sparse, dense, repeated, and high-detector contract rows.

## Implementation Areas

- Added `CompiledDemSampler` in `stab-core` with reusable compiled DEM operations, seeded sampling, detector-shift handling, repeat-block unrolling with a bounded initial limit, and shared `DetectionConversionOutput` records.
- Added `stab sample_dem` in `stab-cli` with `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--append_observables`, hidden `--prepend_observables`, `--obs_out`, `--obs_out_format`, `--err_out`, `--err_out_format`, `--replay_err_in`, and `--replay_err_in_format` arguments.
- Reused the existing detection-event and observable record writers so `sample_dem` uses the same output format behavior as `detect` and `m2d`.
- Reused the existing result-format readers and writers so `sample_dem` can write sampled-error records and replay error records in `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` formats.
- Fixed `sample_dem --out_format=dets` so detector output remains detector-only and `--obs_out` can be used for observables, matching Stim's independent detector and observable stream semantics for the covered subset.
- Added a pre-count DEM sampler compilation budget so oversized and nested repeat expansion is rejected before detector counting can perform unbounded work.
- Added a DEM sampler output budget so excessive shots, high detector or observable widths, and optional error-record buffers fail before sampling materializes records. The current materialized sampler limit is 64,000,000 buffered units, where each requested shot counts `detectors + observables` output units, sampled or replayed error records add one unit per compiled DEM error operation, and zero-width output still counts as one unit per shot so huge empty outputs are rejected.
- Bounded `sample_dem --replay_err_in` reads to the requested replay prefix: `ptb64` and `b8` read only the required byte window, `r8` decodes only the requested records, and text replay formats read at most 1,048,576 bytes per requested record.
- Promoted `m11-sample-dem-deterministic`, `m11-sample-dem-noisy-statistical`, and `coverage-simulators-dem-sampler` in `oracle/fixtures/manifest.csv` to `implemented`.
- Added implemented M11 exact oracle rows for deterministic sparse, dense, repeated, high-detector, and correlated detector-combination DEM fixture groups, with expected stdout recorded from pinned Stim v1.16.0.
- Extended the oracle fixture runner with manifest-declared `{fixture_input:...}` and `{fixture_output:...}` placeholders so exact-output rows can validate side-input fixture paths and compare pinned-Stim and Stab side-output files in addition to stdout, then added M11 rows for `--obs_out`, `--out_format=dets` plus `--obs_out`, `--err_out`, and `--replay_err_in` with copied error and observable outputs.
- Hardened oracle fixture-output scratch paths so fresh side-output files are created under per-run directories below `target/oracle/fixture-outputs` and pre-existing symlink components in the scratch parent are rejected.
- Added Stab benchmark comparison runners for `m11-dem-sampler`, `m11-sample-dem-cli`, `m11-sample-dem-sparse-contract`, `m11-sample-dem-dense-contract`, `m11-sample-dem-repeated-contract`, and `m11-sample-dem-high-detector-contract`.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `CompiledDemSampler` reusable sampling state | Partial | `CompiledDemSampler::compile`, `CompiledDemSampler::sample_detection_events_with_seed`, `CompiledDemSampler::sample_detection_events_and_errors_with_seed`, `CompiledDemSampler::sample_detection_events_from_error_records`, bounded repeat-expansion rejection, and `cargo test -p stab-core --test dem_sampler` including dense `b8` output coverage |
| `stim sample_dem` deterministic CLI output | Partial | `m11-sample-dem-deterministic`, `m11-sample-dem-sparse-exact`, `m11-sample-dem-dense-exact`, `m11-sample-dem-repeated-exact`, `m11-sample-dem-high-detector-b8-exact`, `m11-sample-dem-correlated-exact`, `m11-sample-dem-observable-output-exact`, `m11-sample-dem-dets-observable-output-exact`, `m11-sample-dem-error-output-exact`, `m11-sample-dem-replay-side-outputs-exact`, `cargo test -p stab-cli sample_dem_deterministic`, `cargo test -p stab-cli sample_dem_writes_observables`, `cargo test -p stab-cli sample_dem_dets_output`, `cargo test -p stab-cli sample_dem_writes_error_records`, `cargo test -p stab-cli sample_dem_replays_error_records`, `just oracle::run --milestone M11 --exact` |
| `stim sample_dem` one-bit noisy statistical CLI output | Partial | `m11-sample-dem-noisy-statistical`, `cargo test -p stab-cli sample_dem_noisy`, `just oracle::run --milestone M11 --statistical` |
| `stim sample_dem` sampled-error output and replay | Partial | `m11-sample-dem-error-output-exact`, `m11-sample-dem-replay-side-outputs-exact`, `cargo test -p stab-core --test dem_sampler`, `cargo test -p stab-core result_formats`, `cargo test -p stab-cli sample_dem_writes_error_records`, `cargo test -p stab-cli sample_dem_writes_ptb64_detector_observable_and_error_streams`, `cargo test -p stab-cli sample_dem_replays_error_records`, `cargo test -p stab-cli sample_dem_replays_ptb64_error_records`, `cargo test -p stab-cli sample_dem_rejects_replay_record_count_mismatch`, `just oracle::run --milestone M11 --exact` |
| `ptb64` result-format helpers for `sample_dem` streams | Satisfied | `cargo test -p stab-core result_formats`, `cargo test -p stab-core detection_record_writers_cover_text_and_bit_packed_formats`, `cargo test -p stab-cli sample_dem_writes_ptb64_detector_observable_and_error_streams`, `cargo test -p stab-cli sample_dem_replays_ptb64_error_records`, `cargo test -p stab-cli sample_dem_rejects_truncated_ptb64_replay_input`, `cargo test -p stab-cli sample_dem_rejects_ptb64_shots_that_are_not_multiple_of_64` |
| M11 buffered sampling scale limits | Partial | `CompiledDemSampler::validate_sample_buffer_units`, `cargo test -p stab-core --test dem_sampler dem_sampler_rejects_excessive_buffered_outputs_before_sampling`, `cargo test -p stab-cli sample_dem_rejects_excessive_buffered_output_before_sampling`, `cargo test -p stab-cli sample_dem_replay_ignores_malformed_extra_text_records_after_requested_shots`, `cargo test -p stab-cli sample_dem_replay_ignores_partial_extra_b8_records_after_requested_shots`, `cargo test -p stab-cli sample_dem_rejects_excessive_replay_buffers_before_reading_replay_path`; true streaming, DEM input-size policy, exact output-byte limits, and full text replay scale policy remain open |
| Sparse, dense, repeated, high-detector-count, and correlated-error fixture groups | Partial | `m11-sample-dem-sparse-exact`, `m11-sample-dem-dense-exact`, `m11-sample-dem-repeated-exact`, `m11-sample-dem-high-detector-b8-exact`, `m11-sample-dem-correlated-exact`, `coverage-simulators-dem-sampler`, `cargo test -p stab-core --test dem_sampler`, `just oracle::run --milestone M11 --exact`, `just oracle::run --milestone M11 --structural`; noisy sparse/dense/repeated/high-detector statistical acceptance remains future scope unless the milestone matrix is amended |
| M11 benchmark reporting | Partial | `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners`; `just bench::compare --milestone M11` now reports all M11 Stab-side benchmark rows, while strict Stab-vs-Stim comparison still depends on the selected baseline artifact containing measured M11 pinned-Stim rows |

## Audit And Review Notes

- Milestone audit and full-code-review found and this slice fixed the `sample_dem --out_format=dets` observable-routing incompatibility.
- Milestone audit and full-code-review found and this slice fixed the nested-repeat denial-of-service path by validating sampler repeat expansion before detector counting.
- This slice further addresses the open `sample_dem` flag-scope gap by adding exact oracle side-output comparisons for observable streams, detector-only `dets` stdout with observable streams, sampled-error streams, and replayed error detector, observable, and error-copy streams.
- This slice partially addresses the open streaming and scale-limit gap by making the current materialized sampler reject excessive output and error-record buffers before sampling.
- Remaining under-specified M11 scope is logged in `docs/plans/milestone-spec-gaps.md`: full `sample_dem` flag and format scope, fixture-group acceptance, streaming and scale limits, and benchmark baseline comparability.

## Verification Commands

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `cargo test -p stab-core result_formats --quiet`
- `cargo test -p stab-core --test dem_sampler --quiet`
- `cargo test -p stab-cli sample_dem --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone M11 --exact`
- `just oracle::run --milestone M11 --statistical`
- `just oracle::run --milestone M11 --structural`
- `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners --quiet`
- `just bench::compare --milestone M11`
- `just maintenance::pre-commit`
