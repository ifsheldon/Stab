# M9 Completion Report

## Milestone

M9: Detection Event Workflows.

Objective: implement measurement-to-detection conversion and the CLI workflows that decoder pipelines depend on.

## Status

Complete with spec follow-ups.

The public M9 workflow slice is implemented and verified for measurement-record detector conversion, `detect`, `m2d`, observable routing, Pauli-target observable flips in `detect`, product-measurement frame updates, text formats, `b8`, `detect` `ptb64` outputs, `m2d` `ptb64` input, `m2d` `ptb64` output rejection, structural gauge behavior, and report-only benchmark runners.
The remaining work is explicitly logged as milestone under-specification because the current roadmap does not yet define whether it belongs in M9, a detector-analysis submilestone, or M12 performance hardening.

## Tests Ported Or Created

- Added `crates/stab-core/src/detection.rs` tests for reference-sample subtraction, `--skip_reference_sample`, repeats, coordinate annotations, empty detectors, empty-detector circuits, invalid `rec` references, observable ids, `dets`/`hits`/`01`/`b8` writers, `ptb64` detection writers, gauge-detector structural behavior, Pauli-target observable flips in `detect`, product-measurement frame updates, frame-path reference-sample measurement-bit cancellation, invalid frame-path feedback references, and bounded record-shape validation.
- Added `crates/stab-core/src/result_formats.rs` `ptb64` tests for measurement-major encoding, full-input decoding, truncated input rejection, zero-width input rejection, and inferred record counts.
- Added `crates/stab-cli/src/tests.rs` coverage for `detect`, `--detect`, `m2d`, `--m2d`, `--append_observables`, deprecated `--prepend_observables`, `--obs_out`, Pauli-target observable flips in `detect`, product-measurement Pauli-observable flips in `detect`, `m2d` Pauli-target conversion ignore behavior, `b8` input/output, `detect` `ptb64` detector and observable outputs, `m2d` `ptb64` measurement input, `m2d` `ptb64` output rejection, `dets` default observable placement, zero-shot `detect`, zero-width and oversized `ptb64` input rejection, observable-route conflicts, and measurement-width errors.
- Added M9 exact oracle fixtures for public `detect`, public `m2d`, `m2d` `ptb64` input, `m2d` `ptb64` output rejection, and reference-sample parity from `measurements_to_detection_events`.
- Added M9 structural oracle rows for detection-event sampling, Pauli-target observable frame-simulator parity, and measurement-to-detection conversion subsets.
- Added M9 benchmark compare runners for `detect`, `m2d`, `b8`, `dets`, and generated repetition-code representative workloads.

## Implementation Areas

- `crates/stab-core/src/detection.rs` owns the M9 conversion plan, reference-sample subtraction, detector and observable record construction, output writers, explicit observable placement modes, detection-sampling validation, and temporary resource limits.
- `crates/stab-core/src/detection/frame.rs` owns the M9 scalar detector-frame path for Pauli-target observable flips in `detect`, including basis measurements, product measurements, Pauli noise, selected Clifford propagation, and explicit unsupported-instruction errors.
- `crates/stab-core/src/result_formats.rs` now has a measurement-only `dets` reader for `m2d` so `D` and `L` tokens are not misread as measurement tokens, plus complete-file `ptb64` decoding and record-count inference for `m2d` measurement-major input.
- `crates/stab-cli/src/detection.rs` implements `detect` and `m2d`, zero-shot `detect` behavior, observable-output routing validation, `ptb64` dispatch for supported M9 detection workflows, Stim-compatible `m2d` `ptb64` output rejection, decoded `ptb64` input bounds, limited `m2d` input reads, and clear unsupported-scope errors for `--ran_without_feedback` and unsupported detection formats; `crates/stab-cli/src/lib.rs` retains command dispatch and Stim legacy aliases.
- `ops/oracle` now supports `--structural` fixture filtering.
- `ops/bench/src/baseline/m9.rs` provides M9 Stab-side benchmark runners and mirrors the `detect --out_format=dets` observable placement behavior.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Deterministic detection oracle rows pass | Satisfied | `just oracle::run --milestone M9 --exact` |
| Gauge-detector and Pauli-observable structural rows pass | Satisfied with manifest-only utility reports for utility rows | `just oracle::run --milestone M9 --structural` |
| Core detection tests cover coordinate shifts, repeats, observables, Pauli-target observable flips, empty-detector circuits, and invalid measurement references | Satisfied | `cargo test -p stab-core detection` |
| CLI tests cover public `detect` and `m2d` workflows | Satisfied | `cargo test -p stab-cli m9`; `cargo test -p stab-cli detect`; `cargo test -p stab-cli m2d` |
| Benchmarks report `detect` and `m2d` throughput for text and bit-packed formats | Satisfied as report-only | `just bench::compare --milestone M9` |

## Audit Outcome

Milestone audit found implementation issues in structural oracle filtering, gauge-detector evidence, exact `m2d` reference parity, and public benchmark runners.
Those were fixed by adding `--structural` support to `stab-oracle run`, expanding core detection tests, adding exact oracle fixtures, and adding M9 benchmark compare runners.

Milestone audit also found under-specified scope around feedback-removal conversion, sweep-conditioned conversion, detector-analysis utility rows, generated fixture round trips, pinned benchmark baseline completeness, and bit-packed format scope.
Those items are logged in `docs/plans/milestone-spec-gaps.md`.

## Full Code Review Outcome

Full code review found compatibility and resource issues in `detect` observable placement, `m2d --in_format=dets`, zero-shot `detect`, deprecated `--prepend_observables`, top-level `--m2d`, Pauli-target observable detection, unbounded detection planning, whole-workload materialization, and duplicate sampler analysis.
The concrete implementation issues were fixed with explicit observable output modes, measurement-only `dets` parsing, zero-shot early return, legacy alias support, observable-route validation, scalar frame-simulator Pauli-target observable support for `detect`, product-measurement frame updates, bounded planning and buffering limits, limited `m2d` reads, and reference-sample reuse from the compiled sampler.

Open M9 spec follow-ups:

- `2026-06-27 - M9: Feedback-Removal Conversion Scope`
- `2026-06-27 - M9: Detector Analysis Utility Row Ownership`
- `2026-06-27 - M9: Sweep-Conditioned Detection Conversion Scope`
- `2026-06-27 - M9: Benchmark Baseline Completeness`
- `2026-06-27 - M9: Generated Fixture Round-Trip Coverage`
- `2026-06-27 - M9: Detection Conversion Streaming And Scale Limits`

Resolved M9 spec entry:

- `2026-06-27 - M9: Structural Oracle Flag Mismatch`
- `2026-06-27 - M9: Pauli-Target Observable Detection Scope`
- `2026-06-27 - M9: Detection Bit-Packed Format Scope`

## Verification Commands

- `cargo fmt --check --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p stab-core detection`
- `cargo test -p stab-core detection_sampling`
- `cargo test -p stab-core result_formats::tests::ptb64_records_are_measurement_major_over_64_shot_groups`
- `cargo test -p stab-core measure_record_reader_handles_multiple_records`
- `cargo test -p stab-cli m9`
- `cargo test -p stab-cli detect`
- `cargo test -p stab-cli detect_supports_pauli_target_observable_flips`
- `cargo test -p stab-cli detect_supports_product_measurements_with_pauli_observable_flips`
- `cargo test -p stab-cli m2d`
- `cargo test -p stab-cli m2d_ignores_pauli_target_observables_like_stim_conversion`
- `cargo test -p stab-bench m9_benchmark_rows_have_stab_compare_runners`
- `just oracle::matrix --check`
- `just oracle::run --milestone M9 --exact`
- `just oracle::run --milestone M9 --structural`
- `just oracle::run --milestone M9`
- `just bench::compare --milestone M9`
