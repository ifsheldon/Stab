# M9 Sweep And Feedback Progress Report

## Summary

This report records the implemented M9 follow-up slice for `stab m2d --sweep`, `--sweep_format`, and scoped `--ran_without_feedback`.
The slice implements public CLI parity for the owned text and packed sweep paths, all-false omitted sweep behavior, observable side-output routing, record-count mismatch errors, and the pinned Stim `command_m2d.m2d_without_feedback` command case.
It does not claim full `Circuit.with_inlined_feedback` transform API parity.

## Implemented Surfaces

- Core detection conversion now has additive sweep-aware APIs: `CompiledDetectionConverter::convert_record_with_sweep_into`, `try_for_each_detection_event_with_sweep`, `sweep_bit_count`, reusable reference buffers, and `convert_measurements_to_detection_events_with_sweep`.
- Public `CompiledDetectionConverter::compile` accepts the current sweep-conditioned detector-conversion subset and treats omitted sweep records as all false through existing compatibility wrappers.
- `stab m2d` now accepts `--sweep <path>` and `--sweep_format <format>`, streams measurement and sweep records in lockstep, reports clear errors when either stream ends early, and mirrors Stim's zero-width text input plus empty b8 sweep behavior for circuits with no sweep bits.
- `stab m2d --ran_without_feedback` now applies scoped `circuit_with_inlined_feedback` before detection conversion.
- Stab-native help for `m2d` lists `--sweep`, `--sweep_format`, and `--ran_without_feedback`.

## Oracle And Test Evidence

- Exact oracle rows added:
  - `m9-m2d-sweep-01-dets`
  - `m9-m2d-sweep-default-false`
  - `m9-m2d-ran-without-feedback`
- Structural oracle coverage updated:
  - `coverage-simulators-measurements-to-detection-events-rust`
  - `coverage-util-top-transform-without-feedback`
- Targeted checks run during implementation:
  - `cargo test -p stab-core circuit_with_inlined_feedback -- --nocapture`
  - `cargo test -p stab-core detection --quiet`
  - `cargo test -p stab-core sampling --quiet`
  - `cargo test -p stab-cli m2d --quiet`
  - `cargo test -p stab-bench m9 --quiet`
  - `just oracle::run --milestone M9`
  - `just bench::smoke`

## Benchmark Evidence

New report-only benchmark rows were added:

- `m9-m2d-sweep-01-cli`
- `m9-m2d-sweep-b8-cli`
- `m9-m2d-sweep-obs-out-cli`
- `m9-m2d-ran-without-feedback-cli`

These rows use the public `stab_cli::run_from(["stab", "m2d", ...])` path in the Stab-side benchmark runner.
They are not added to `benchmarks/m12-primary-thresholds.json`; threshold ownership requires later repeated probe evidence.
Focused probe reports were generated under:

- `target/benchmarks/m9-m2d-sweep-01-compare/report.md`
- `target/benchmarks/m9-m2d-sweep-b8-compare/report.md`
- `target/benchmarks/m9-m2d-sweep-obs-out-compare/report.md`
- `target/benchmarks/m9-m2d-ran-without-feedback-compare/report.md`

## Audit And Review Notes

- Milestone audit found that the first pass overclaimed binary b8 exact-output oracle rows; the plan and GOAL now distinguish text exact oracle evidence from packed b8 structural CLI and benchmark evidence.
- Full-code-review found that the additive sweep iterator API silently truncated mismatched measurement and sweep iterators; `try_for_each_detection_event_with_sweep` now rejects both short and long sweep iterators.
- Full-code-review found that unsupported sweep target shapes could be accepted as no-op sampler operations; the sampler now rejects unsupported sweep shapes explicitly.
- The review file-size scan pushed test-only bulk into `crates/stab-cli/src/tests/m9/sweep.rs` and `crates/stab-core/src/detection/tests.rs`, and moved sampling execution helpers to `crates/stab-core/src/sampling/execute.rs`.

## Final Verification

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::run --milestone M9`
- `just oracle::run --implemented-only`
- `just bench::smoke`

## Remaining Exclusions

- `detect` sweep-conditioned sampling now accepts the selected omitted-all-false sweep subset documented by [rpf3-sweep-gate-progress-report.md](rpf3-sweep-gate-progress-report.md). Pinned Stim v1.16.0 has no `stim detect --sweep` flag, so public typed sweep files remain an `m2d --sweep` surface instead of a `detect` CLI gap.
- Full `Circuit.with_inlined_feedback` parity is not claimed.
- Broader repeat-contained feedback and unsupported feedback-inlining shapes remain open spec gaps in `docs/plans/milestone-spec-gaps.md`.
- Deprecated `--detector_hypergraph` remains excluded from Stab CLI parity.
- Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, public graph/vector simulator APIs, and broader ecosystem surfaces remain out of this M9 slice.
