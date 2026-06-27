# M11 Progress Report

## Milestone

M11: Detector Error Model Sampling

## Status

Partial progress, not milestone-complete.
This slice implements the first deterministic `sample_dem` path, M11 noisy statistical oracle rows for one-bit, sparse, dense, repeated, high-detector, correlated, and observable-only side-output DEM sampling, the M11-owned structural subset of `src/stim/simulators/dem_sampler.test.cc`, initial M11 benchmark comparison runners, `sample_dem` error-record output and replay for Stim result formats including `ptb64`, oracle side-output comparisons for observable, error, and replay streams, explicit DEM input and buffered-sampling limits, and two audit fixes for `sample_dem` detector/observable routing and hostile nested DEM repeats.

## Tests Ported Or Created

- `cargo test -p stab-core --test dem_sampler` covers the initial `CompiledDemSampler` subset ported from `src/stim/simulators/dem_sampler.test.cc`, including empty and sparse sizing, high detector and observable ids, observables-only errors, `error(1)` detector toggling, `error(0)` no-op behavior, p=0.25, p=0.5, and p=0.75 probability bands, separator handling, detector-observable correlation, correlated detector-combination parity, detector shifts, repeat blocks, bounded repeat-expansion rejection, logical observable flips, sampled-error recording and replay width validation, dense bit-packed detector and observable output, excessive detector-width, observable-width, buffered-output, and replay-buffer rejection, fixed-seed noisy sampling reproducibility, and one-bit p=0.25 statistical behavior.
- `cargo test -p stab-core result_formats` and `cargo test -p stab-core detection_record_writers_cover_text_and_bit_packed_formats` cover `ptb64` byte layout, replay decoding, Stim-compatible CRLF text record reading, detector stream output, and observable stream output helpers.
- `cargo test -p stab-cli sample_dem` covers the existing `m11-sample-dem-deterministic` oracle row for `stab sample_dem --shots 3` against pinned Stim v1.16.0 output, the `m11-sample-dem-noisy-statistical` one-bit seeded distribution row, the upstream `--obs_out` detector/observable split behavior, `--out_format=dets` detector-only stdout with separate observable output, conflict rejection for `--append_observables`, hidden `--prepend_observables`, and `--obs_out`, `--err_out` sampled-error output, `--replay_err_in` replay into detector and observable streams, replayed error copying through `--err_out`, `ptb64` and `r8` detector, observable, error, and replay streams, Stim-compatible CRLF replay text records, zero-shot validation of declared input and replay paths, oversized DEM input rejection, excessive buffered-output rejection, bounded replay-prefix parsing including blank `dets` prefix rejection, and replay shot-count validation.
- `just oracle::run --milestone M11 --exact` covers the implemented deterministic exact-output rows for the basic `sample_dem` CLI path, sparse detector ids, dense detector targets, repeated detector shifts, high detector ids with `b8` output, correlated detector combinations, observable side output, observable-only side output, `dets` detector-only stdout with observable side output, sampled-error side output, and replayed error detector, observable, and error-copy side streams.
- `just oracle::run --milestone M11 --statistical` covers implemented noisy statistical rows for one-bit sampling, sparse detector ids, dense detector targets, repeated detector shifts, high detector ids, correlated detector combinations, and observable-only side output.
- `just oracle::run --milestone M11 --structural` covers the implemented `coverage-simulators-dem-sampler` structural row.
- `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners` covers Stab comparison runners for the M11 DEM sampler row, `sample_dem` CLI row, and sparse, dense, repeated, and high-detector contract rows.

## Implementation Areas

- Added `CompiledDemSampler` in `stab-core` with reusable compiled DEM operations, seeded sampling, detector-shift handling, repeat-block unrolling with a bounded initial limit, and shared `DetectionConversionOutput` records.
- Added `stab sample_dem` in `stab-cli` with `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--append_observables`, hidden `--prepend_observables`, `--obs_out`, `--obs_out_format`, `--err_out`, `--err_out_format`, `--replay_err_in`, and `--replay_err_in_format` arguments.
- Reused the existing detection-event and observable record writers so `sample_dem` uses the same output format behavior as `detect` and `m2d`.
- Reused the existing result-format readers and writers so `sample_dem` can write sampled-error records and replay error records in `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` formats.
- Updated text result readers so Stim-compatible CRLF records are accepted for `01`, `hits`, and `dets`, and blank or whitespace-only `dets` lines are ignored before a `shot` record.
- Fixed `sample_dem --out_format=dets` so detector output remains detector-only and `--obs_out` can be used for observables, matching Stim's independent detector and observable stream semantics for the covered subset.
- Added a pre-count DEM sampler compilation budget so oversized and nested repeat expansion is rejected before detector counting can perform unbounded work.
- Added a DEM sampler output budget so excessive shots, high detector or observable widths, and optional error-record buffers fail before sampling materializes records. The current materialized sampler limit is 64,000,000 buffered units, where each requested shot counts `detectors + observables` output units, sampled or replayed error records add one unit per compiled DEM error operation, and zero-width output still counts as one unit per shot so huge empty outputs are rejected.
- Added a 64 MiB `sample_dem` DEM input cap by reusing the shared CLI bounded reader, so oversized file inputs are rejected before the parser reads them.
- Matched Stim's zero-shot path validation more closely by opening declared `--in` and `--replay_err_in` paths before returning empty output for `--shots 0`.
- Bounded `sample_dem --replay_err_in` reads to the requested replay prefix: `ptb64` and `b8` read only the required byte window, `r8` decodes only the requested records, and text replay formats read at most 1,048,576 bytes per requested record.
- Updated the M11 roadmap section with the required flag and format surface, fixture acceptance matrix, bounded materialization policy, and report-only benchmark policy revealed by implementation and review.
- Promoted `m11-sample-dem-deterministic`, `m11-sample-dem-noisy-statistical`, and `coverage-simulators-dem-sampler` in `oracle/fixtures/manifest.csv` to `implemented`.
- Added implemented M11 exact oracle rows for deterministic sparse, dense, repeated, high-detector, and correlated detector-combination DEM fixture groups, with expected stdout recorded from pinned Stim v1.16.0.
- Extended the oracle fixture runner with manifest-declared `{fixture_input:...}` and `{fixture_output:...}` placeholders so exact-output rows can validate side-input fixture paths and compare pinned-Stim and Stab side-output files in addition to stdout, then added M11 rows for `--obs_out`, `--out_format=dets` plus `--obs_out`, `--err_out`, and `--replay_err_in` with copied error and observable outputs.
- Extended statistical oracle plans with `source=fixture_output` so one validated side-output stream can be checked statistically against both pinned Stim and Stab without committing random golden side-output files, while stdout remains exact-compared for that row.
- Hardened oracle fixture-output scratch paths so fresh side-output files are created under per-run directories below `target/oracle/fixture-outputs` and pre-existing symlink components in the scratch parent are rejected.
- Added bucketed M11 statistical oracle rows for noisy sparse, dense, repeated, high-detector, correlated detector-combination, and observable-only side-output DEM fixture groups.
- Added Stab benchmark comparison runners for `m11-dem-sampler`, `m11-sample-dem-cli`, `m11-sample-dem-sparse-contract`, `m11-sample-dem-dense-contract`, `m11-sample-dem-repeated-contract`, and `m11-sample-dem-high-detector-contract`.

## Current Evidence

| Requirement | Status | Evidence |
| --- | --- | --- |
| `CompiledDemSampler` reusable sampling state | Partial | `CompiledDemSampler::compile`, `CompiledDemSampler::sample_detection_events_with_seed`, `CompiledDemSampler::sample_detection_events_and_errors_with_seed`, `CompiledDemSampler::sample_detection_events_from_error_records`, bounded repeat-expansion rejection, and `cargo test -p stab-core --test dem_sampler` including dense `b8` output coverage |
| `stim sample_dem` deterministic CLI output | Partial | `m11-sample-dem-deterministic`, `m11-sample-dem-sparse-exact`, `m11-sample-dem-dense-exact`, `m11-sample-dem-repeated-exact`, `m11-sample-dem-high-detector-b8-exact`, `m11-sample-dem-correlated-exact`, `m11-sample-dem-observable-output-exact`, `m11-sample-dem-observable-only-exact`, `m11-sample-dem-dets-observable-output-exact`, `m11-sample-dem-error-output-exact`, `m11-sample-dem-replay-side-outputs-exact`, `cargo test -p stab-cli sample_dem_deterministic`, `cargo test -p stab-cli sample_dem_writes_observables`, `cargo test -p stab-cli sample_dem_dets_output`, `cargo test -p stab-cli sample_dem_rejects_conflicting_observable_routes`, `cargo test -p stab-cli sample_dem_round_trips_r8_detector_observable_error_and_replay_streams`, `cargo test -p stab-cli sample_dem_replays_stim_compatible_crlf_text_records`, `cargo test -p stab-cli sample_dem_zero_shots_validates_declared_input_paths_like_stim`, `cargo test -p stab-cli sample_dem_writes_error_records`, `cargo test -p stab-cli sample_dem_replays_error_records`, `just oracle::run --milestone M11 --exact` |
| `stim sample_dem` noisy statistical CLI output | Partial | `m11-sample-dem-noisy-statistical`, `m11-sample-dem-sparse-statistical`, `m11-sample-dem-dense-statistical`, `m11-sample-dem-repeated-statistical`, `m11-sample-dem-high-detector-statistical`, `m11-sample-dem-correlated-statistical`, `m11-sample-dem-observable-only-statistical`, `cargo test -p stab-cli sample_dem_noisy`, `just oracle::run --milestone M11 --statistical` |
| `stim sample_dem` sampled-error output and replay | Partial | `m11-sample-dem-error-output-exact`, `m11-sample-dem-replay-side-outputs-exact`, `cargo test -p stab-core --test dem_sampler`, `cargo test -p stab-core result_formats`, `cargo test -p stab-cli sample_dem_writes_error_records`, `cargo test -p stab-cli sample_dem_writes_ptb64_detector_observable_and_error_streams`, `cargo test -p stab-cli sample_dem_round_trips_r8_detector_observable_error_and_replay_streams`, `cargo test -p stab-cli sample_dem_replays_stim_compatible_crlf_text_records`, `cargo test -p stab-cli sample_dem_rejects_excessive_blank_dets_replay_prefix`, `cargo test -p stab-cli sample_dem_replays_error_records`, `cargo test -p stab-cli sample_dem_replays_ptb64_error_records`, `cargo test -p stab-cli sample_dem_rejects_replay_record_count_mismatch`, `just oracle::run --milestone M11 --exact` |
| `ptb64` result-format helpers for `sample_dem` streams | Satisfied | `cargo test -p stab-core result_formats`, `cargo test -p stab-core detection_record_writers_cover_text_and_bit_packed_formats`, `cargo test -p stab-cli sample_dem_writes_ptb64_detector_observable_and_error_streams`, `cargo test -p stab-cli sample_dem_replays_ptb64_error_records`, `cargo test -p stab-cli sample_dem_rejects_truncated_ptb64_replay_input`, `cargo test -p stab-cli sample_dem_rejects_ptb64_shots_that_are_not_multiple_of_64` |
| M11 bounded materialized sampling limits | Partial | `CompiledDemSampler::validate_sample_buffer_units`, `cargo test -p stab-core --test dem_sampler dem_sampler_rejects_excessive_buffered_outputs_before_sampling`, including excessive detector-width and observable-width cases, `cargo test -p stab-cli sample_dem_rejects_oversized_input_file_before_reading`, `cargo test -p stab-cli sample_dem_rejects_excessive_buffered_output_before_sampling`, `cargo test -p stab-cli sample_dem_replay_ignores_malformed_extra_text_records_after_requested_shots`, `cargo test -p stab-cli sample_dem_rejects_excessive_blank_dets_replay_prefix`, `cargo test -p stab-cli sample_dem_replay_ignores_partial_extra_b8_records_after_requested_shots`, `cargo test -p stab-cli sample_dem_rejects_excessive_replay_buffers_before_reading_replay_path`; true streaming, folded repeat sampling without bounded unrolling, and exact output-byte limits are deferred to M12 by the roadmap |
| Sparse, dense, repeated, high-detector-count, observable-only, and correlated-error fixture groups | Partial | `m11-sample-dem-sparse-exact`, `m11-sample-dem-dense-exact`, `m11-sample-dem-repeated-exact`, `m11-sample-dem-high-detector-b8-exact`, `m11-sample-dem-correlated-exact`, `m11-sample-dem-observable-only-exact`, `m11-sample-dem-sparse-statistical`, `m11-sample-dem-dense-statistical`, `m11-sample-dem-repeated-statistical`, `m11-sample-dem-high-detector-statistical`, `m11-sample-dem-correlated-statistical`, `m11-sample-dem-observable-only-statistical`, `coverage-simulators-dem-sampler`, `cargo test -p stab-core --test dem_sampler`, `just oracle::run --milestone M11 --exact`, `just oracle::run --milestone M11 --statistical`, `just oracle::run --milestone M11 --structural` |
| M11 benchmark reporting | Satisfied | `cargo test -p stab-bench m11_benchmark_rows_have_stab_compare_runners`; `just bench::compare --milestone M11` reports all M11 Stab-side benchmark rows under the roadmap's report-only M11 benchmark policy; strict Stab-vs-Stim baseline completeness is deferred to M12 |

## Audit And Review Notes

- Milestone audit and full-code-review found and this slice fixed the `sample_dem --out_format=dets` observable-routing incompatibility.
- Milestone audit and full-code-review found and this slice fixed the nested-repeat denial-of-service path by validating sampler repeat expansion before detector counting.
- This slice further addresses the open `sample_dem` flag-scope gap by adding exact oracle side-output comparisons for observable streams, detector-only `dets` stdout with observable streams, sampled-error streams, and replayed error detector, observable, and error-copy streams.
- This slice resolves the M11 under-specification around `sample_dem` flag and format scope, fixture-group acceptance, bounded materialized sampling limits, and report-only benchmark comparability by updating `docs/plans/rust-stim-drop-in-rewrite.md`.
- This slice fixes follow-up milestone-audit and full-code-review findings by adding explicit `r8` stream proof, observable-width rejection proof, Stim-compatible CRLF text replay parsing, bounded blank `dets` replay prefix scanning, and zero-shot declared-path validation.
- This slice's milestone-audit and full-code-review findings were fixed; M11 remains partial pending a completion audit of the whole milestone against the clarified roadmap.

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
