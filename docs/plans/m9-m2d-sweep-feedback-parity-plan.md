# M9 M2D Sweep And Feedback Parity Plan

## Summary

This plan is the next roughly two-day implementation wave for Stab.
It finishes the visible `stab m2d` parity gaps for sweep-conditioned conversion and `--ran_without_feedback`, while keeping deprecated `--detector_hypergraph` out of Stab CLI parity.
The work deliberately stays inside the Rust core and CLI surfaces and does not reopen Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, public graph/vector simulator APIs, or broader detector-analysis APIs.

Use `docs/plans/lessons-learned.md` as the execution guardrail.
Do not mark a milestone done based on broad upstream file references, dirty benchmark evidence, non-strict compare reports, or docs that still describe implemented behavior as deferred.

## Starting State

`stab m2d` already streams measurement input for `01`, `b8`, `r8`, `hits`, `dets`, and accepted `ptb64` input formats, and it streams detector and observable outputs in the supported non-`ptb64` output formats.
`crates/stab-cli/src/detection.rs` already parses `--ran_without_feedback` but rejects it with `UnsupportedRanWithoutFeedback`.
`stab m2d` does not yet accept `--sweep` or `--sweep_format`.
`crates/stab-core/src/detection.rs` rejects every sweep target before detection conversion because the current converter uses one circuit-level reference sample instead of a per-shot reference sample derived from sweep bits.
The oracle manifest still has `coverage-util-top-transform-without-feedback` as a manifest-only M9 row, and the feature checklist names sweep-conditioned conversion and `m2d --ran_without_feedback` as deferred gaps.

## Public Surface Changes

- Add `stab m2d --sweep <path>` and `stab m2d --sweep_format <format>`, where `--sweep_format` defaults to `01`.
- Match Stim v1.16.0 defaulting: if `--sweep` is omitted, all sweep bits are false for every shot.
- Treat `--sweep_format` as meaningful only when `--sweep` is provided, and accept the same result formats as measurement input where the reader can stream bounded records.
- Keep `--out_format=ptb64` and `--obs_out_format=ptb64` rejected for `m2d`, matching pinned Stim v1.16.0.
- Make `--ran_without_feedback` apply a feedback-inlining transform before detection conversion instead of rejecting the command.
- Add Stab-native help text for the new `m2d` flags without claiming byte-for-byte Stim help output.

## Milestone 0: Extract Exact Acceptance Subcases

Goal: turn the upstream references into owned subcases before changing behavior.

Tasks:

- Split `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc` into owned sweep subcases and deferred semantic-mining notes.
- Own the `single_detector_no_sweep_data`, `sweep_data`, `with_error_propagation`, `many_shots`, `many_measurements_and_detectors`, `file_01_to_01_yes_obs`, `empty_input_01_empty_sweep_b8`, `some_input_01_empty_sweep_b8`, and `empty_input_b8_empty_sweep_b8` behaviors where they map to the Rust CLI or core converter.
- Split `vendor/stim/src/stim/cmd/command_m2d.test.cc` into current implemented rows, new `--ran_without_feedback` rows, and unchanged observable-routing rows.
- Split `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc` into transform rows for `basic`, `demolition_feedback`, `loop`, `mpp`, and `interleaved_feedback_does_not_reorder_operations`.
- Decide the exact unsupported sweep shapes that remain explicit rejections after this wave, then record them in the plan progress note and feature checklist.

Tests to add or update:

- Add oracle manifest rows for the exact CLI cases that will be implemented in later milestones.
- Add structural manifest rows for core-only transform tests that are not meaningful as CLI exact-output fixtures.
- Replace the current manifest-only `coverage-util-top-transform-without-feedback` description with implemented subcase rows when the transform tests land.

Acceptance:

- Every upstream file cited by this plan has owned subcases, deferred subcases, and comparator modes.
- No implementation milestone depends on "the whole upstream file" as its acceptance criterion.

## Milestone 1: Add Core Sweep-Aware Detection Conversion

Goal: make the core converter compute detector and observable events using per-shot sweep records.

Tasks:

- Add a typed sweep-bit count to the detection conversion plan by scanning accepted sweep-bit targets in the circuit.
- Replace the single static reference-sample assumption with a reusable per-shot reference buffer.
- Add an additive API such as `CompiledDetectionConverter::convert_record_with_sweep_into(measurement_record, sweep_record, record)` and keep the existing `convert_record` and `try_for_each_detection_event` APIs as all-false-sweep compatibility wrappers.
- Add a materialized helper such as `convert_measurements_to_detection_events_with_sweep` for tests and small in-memory callers, while keeping the CLI on streaming visitors.
- Implement the sweep reference evaluator for the Stab-supported deterministic detector-conversion subset, including sweep-controlled `CX`, `CY`, and `CZ` Pauli feedback groups, measurement-record feedback groups, repeats within existing conversion limits, measurement aliases, detector declarations, observable declarations, and `skip_reference_sample`.
- Preserve explicit errors for unsupported sweep target placements, unsupported gates with sweep targets, invalid sweep indices, unsupported frame-path Pauli observable targets, repeat expansion above the existing limit, and record-width overflow.
- Make `skip_reference_sample` mean "do not subtract the sweep-dependent reference sample", so measurements are interpreted directly even when sweep input is present.

Core tests:

- Port `single_detector_no_sweep_data` to prove omitted sweep input defaults to all-false behavior.
- Port `sweep_data` to prove multiple sweep bits affect detector events shot by shot.
- Port `with_error_propagation` to prove sweep-controlled `CX` and `CZ` effects are reflected in detector parity.
- Add `skip_reference_sample` with sweep input to prove sweep-dependent reference subtraction is disabled.
- Add repeat coverage where sweep-controlled operations appear inside a supported repeat.
- Add observable coverage where sweep-dependent references affect `OBSERVABLE_INCLUDE`.
- Add negative tests for too-short and too-wide sweep records, unknown sweep target placements, unsupported sweep gates, sweep-index overflow, and zero-width packed sweep input where the format cannot infer records.

Acceptance:

- Existing non-sweep detection conversion tests keep passing without behavior changes.
- The old test that expected sweep-conditioned conversion rejection is replaced with positive tests plus narrower unsupported-shape rejection tests.
- The converter reuses buffers per shot and does not materialize all sweep or measurement records in the streaming path.

## Milestone 2: Add CLI `m2d --sweep` Streaming

Goal: expose sweep-aware conversion through the public `stab m2d` command.

Tasks:

- Add `sweep: Option<PathBuf>` and `sweep_format: RecordFormatArg` to `M2dArgs`.
- Reuse the existing m2d record readers for sweep input, but parameterize error messages so measurement and sweep failures name the right input stream.
- Stream measurement and sweep records in lockstep.
- When `--sweep` is omitted, feed the core converter a reusable all-false sweep record of `converter.sweep_bit_count()` bits.
- When `--sweep` is provided, require exactly one sweep record for each measurement record and fail if either stream ends early or has trailing records.
- Preserve path-boundary behavior for `--in`, `--out`, `--obs_out`, `--circuit`, and new `--sweep` paths.
- Keep the existing writer behavior for detector output, observable side output, and writer-error propagation.

CLI tests:

- `m2d --sweep --sweep_format=01 --in_format=01 --out_format=dets` matches a pinned-Stim exact-output oracle fixture.
- `m2d --sweep --sweep_format=b8 --in_format=b8 --out_format=b8` is covered by a structural CLI test and source-owned benchmark fixture against the same pinned-Stim command shape.
- `m2d --sweep --append_observables --out_format=dets` matches pinned Stim.
- `m2d --sweep --obs_out --obs_out_format=b8` writes primary and observable side outputs matching pinned Stim.
- Omitted `--sweep` on a sweep-conditioned circuit matches pinned Stim all-false sweep behavior.
- Missing sweep path, invalid sweep format, sweep width mismatch, measurement stream longer than sweep stream, sweep stream longer than measurement stream, and malformed packed sweep records fail nonzero with nonempty stderr.
- Existing `m2d` `ptb64` input tests still pass, and `ptb64` detector output remains rejected.

Oracle rows:

- `m9-m2d-sweep-01-dets`
- `m9-m2d-sweep-default-false`

Additional structural CLI tests cover packed `b8`, observable append or side-output routing, width rejection, and sweep/measurement count mismatch rejection where exact stdout fixture rows are not the clearest evidence.

Acceptance:

- `just oracle::run --milestone M9` passes for the new sweep rows while continuing to report intentionally manifest-only M9 detector-analysis utility rows.
- Large measurement and sweep inputs stream until completion or writer failure, without total-shot pre-materialization.

## Milestone 3: Implement `--ran_without_feedback`

Goal: make `stab m2d --ran_without_feedback` match Stim’s feedback-inlining behavior for the supported M9 transform subset.

Tasks:

- Add a core transform named `circuit_with_inlined_feedback` or an equivalently clear Rust API.
- Reuse the existing sparse reverse frame tracker where possible instead of adding a duplicate detector-sensitivity engine.
- Remove measurement-record controlled Pauli feedback from the transformed circuit and rewrite affected `DETECTOR` and `OBSERVABLE_INCLUDE` declarations so detector-event meaning is preserved.
- Preserve sweep-controlled operations as sweep-controlled operations; `--ran_without_feedback` should not erase sweep controls.
- Preserve operation ordering for interleaved non-feedback operations and merge adjacent identical repeat blocks only if the local circuit representation already has a safe helper for that.
- In `run_m2d`, apply the transform after parsing the circuit and before compiling the detection converter when `args.ran_without_feedback` is set.
- Keep unsupported feedback shapes as explicit transform errors with actionable messages.

Core tests:

- Port `transform_without_feedback.basic` and assert canonical transformed circuit text.
- Port `transform_without_feedback.demolition_feedback` and assert canonical transformed circuit text.
- Port `transform_without_feedback.loop` at least structurally; if exact loop refolding is not implemented in this wave, assert semantic detector-conversion parity and log exact refolding as a spec gap.
- Port `transform_without_feedback.mpp` if the existing Stab MPP and reverse-frame support can prove it faithfully; otherwise keep it as a documented deferred transform subcase.
- Port `interleaved_feedback_does_not_reorder_operations` for the supported subset.
- Add property-style parity tests comparing `m2d(circuit, original_measurements)` to `m2d(circuit_with_inlined_feedback(circuit), transformed_measurements)` for small deterministic cases.

CLI tests:

- Replace `m2d_rejects_ran_without_feedback_until_feedback_removal_is_implemented` with a positive exact-output test from `command_m2d.m2d_without_feedback`.
- Add a CLI test where `--ran_without_feedback` and `--sweep` are both present and sweep controls are preserved.
- Add unsupported-transform negative tests for feedback shapes outside the supported subset.

Oracle rows:

- `m9-m2d-ran-without-feedback`
- `m9-m2d-ran-without-feedback-sweep-preserved`
- `coverage-util-top-transform-without-feedback-basic`
- `coverage-util-top-transform-without-feedback-demolition`
- `coverage-util-top-transform-without-feedback-loop`
- `coverage-util-top-transform-without-feedback-interleaved`

Acceptance:

- `stab m2d --ran_without_feedback` no longer rejects the supported upstream command case.
- Transform tests prove detector and observable meaning is preserved, not merely that feedback instructions disappeared.
- Any transform subcase not finished in the two-day window is logged in `docs/plans/milestone-spec-gaps.md` with the exact missing behavior.

## Milestone 4: Add Source-Owned Benchmarks

Goal: add performance evidence for the newly implemented public `m2d` surfaces without prematurely turning noisy or unproven rows into release gates.

Benchmark rows to add:

| Row | Comparability | Command shape | Fixture intent | Initial gate policy |
| --- | --- | --- | --- | --- |
| `m9-m2d-sweep-01-cli` | `stim-cli` / `cli-baseline` | `m2d --in_format=01 --out_format=dets --sweep=<fixture> --sweep_format=01 --circuit=<fixture>` | Dense text measurement plus dense text sweep input | Report-only until probe evidence is stable. |
| `m9-m2d-sweep-b8-cli` | `stim-cli` / `cli-baseline` | `m2d --in_format=b8 --out_format=b8 --sweep=<fixture> --sweep_format=b8 --circuit=<fixture>` | Packed measurement and packed sweep input | Report-only until probe evidence is stable. |
| `m9-m2d-sweep-obs-out-cli` | `stim-cli` / `cli-baseline` | `m2d --in_format=01 --out_format=dets --obs_out=<scratch> --obs_out_format=b8 --sweep=<fixture> --circuit=<fixture>` | Observable side-output overhead with sweep reference updates | Report-only until probe evidence is stable. |
| `m9-m2d-ran-without-feedback-cli` | `stim-cli` / `cli-baseline` | `m2d --in_format=01 --out_format=b8 --ran_without_feedback --circuit=<fixture>` | Public feedback-inlining conversion path | Report-only until probe evidence is stable. |

Fixture requirements:

- Use deterministic source-owned fixtures under `benchmarks/fixtures/`.
- Use enough records to avoid tiny-timer noise; start with 4096 records for text and packed rows.
- Keep sweep patterns readable and deterministic, such as alternating single-bit controls, all-zero controls, all-one controls, and sparse high-index controls.
- Include detector and observable declarations so the benchmark exercises both primary output and side output paths.
- Use scratch output paths under `target/benchmarks/cli-scratch/` for side-output rows.

Benchmark harness tasks:

- Add Stab runners in `ops/bench/src/baseline/m9.rs`.
- Prefer `stab_cli::run_from(["stab", "m2d", ...])` for public CLI path coverage if the benchmark harness already supports in-process CLI rows; otherwise document any temporary core-runner limitation in the compare note.
- Add `measurement_work` entries using `records/s` for text and side-output rows, and `input-bytes/s` or `bits/s` for packed rows.
- Add `compare_note` entries explaining that the rows are public CLI baselines against pinned Stim.
- Extend `ops/bench/src/baseline/tests.rs` and manifest tests so each row has a runner, expected measurement names, measurement work, and compare notes.

Benchmark commands:

```sh
just bench::smoke
just bench::baseline --only m9-m2d-sweep-01-cli --out target/benchmarks/m9-m2d-sweep-01-baseline
just bench::compare --only m9-m2d-sweep-01-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-sweep-01-baseline/baseline.json --report target/benchmarks/m9-m2d-sweep-01-compare
just bench::baseline --only m9-m2d-sweep-b8-cli --out target/benchmarks/m9-m2d-sweep-b8-baseline
just bench::compare --only m9-m2d-sweep-b8-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-sweep-b8-baseline/baseline.json --report target/benchmarks/m9-m2d-sweep-b8-compare
just bench::baseline --only m9-m2d-ran-without-feedback-cli --out target/benchmarks/m9-m2d-ran-without-feedback-baseline
just bench::compare --only m9-m2d-ran-without-feedback-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-ran-without-feedback-baseline/baseline.json --report target/benchmarks/m9-m2d-ran-without-feedback-compare
```

Acceptance:

- `just bench::smoke` sees the new rows and all benchmark source validators pass.
- Each new row has fresh probe evidence against pinned Stim or a source-owned explanation if exact public comparison is blocked.
- Do not add these rows to `benchmarks/m12-primary-thresholds.json` in the same change unless the probe reports are stable, comparable, and below the `1.25x` threshold with clean repeated evidence.

## Milestone 5: Documentation, Audit, And Review Closure

Goal: make the repo’s documentation and acceptance evidence agree with the implemented behavior.

Tasks:

- Update `docs/stab-feature-checklist.md` so `m2d --sweep`, `m2d --sweep_format`, and supported `m2d --ran_without_feedback` move from deferred to implemented or explicitly partial with exact remaining subcases.
- Update `docs/plans/rust-stim-drop-in-rewrite.md` only where milestone scope, CLI behavior, or benchmark acceptance changed.
- Update CLI help docs and README content if they mention `m2d` flags.
- Add a completion report under `docs/plans/` after implementation, including exact oracle rows, test commands, benchmark rows, probe report paths, and remaining exclusions.
- Run milestone-audit against this plan and fix implementation defects.
- Run full-code-review against the touched Rust, CLI, oracle, benchmark, and docs surfaces and fix findings.
- Log under-specified upstream transform or sweep cases in `docs/plans/milestone-spec-gaps.md` instead of silently treating them as complete.

Required final verification:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core detection --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench m9 --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

Acceptance:

- The final completion report cites committed-code evidence when completion is claimed.
- Docs no longer say the implemented sweep and feedback surfaces are deferred.
- The only remaining exclusions are explicitly named and consistent across the checklist, roadmap, oracle manifest, and completion report.

## Two-Day Execution Schedule

Day 1 morning: finish Milestone 0, add oracle fixture rows, and implement the core sweep reference-buffer API.
Day 1 afternoon: finish core sweep conversion and CLI `--sweep` streaming for `01` and `b8`, then add the first exact oracle rows.
Day 2 morning: implement `--ran_without_feedback` using the sparse reverse frame tracker and port the command-level feedback oracle case.
Day 2 afternoon: add benchmark rows, run smoke and targeted probes, synchronize docs, run targeted verification, and write the completion report or spec-gap notes.

If the feedback transform proves larger than expected, finish sweep-conditioned `m2d` first, log the exact transform blockers in `milestone-spec-gaps.md`, and leave `--ran_without_feedback` rejecting only the unsupported transform shapes that remain.

## Non-Negotiable Rules

- Do not implement or document deprecated `--detector_hypergraph` as a supported Stab alias.
- Do not materialize all measurement or sweep records in the CLI path.
- Do not weaken existing `m2d` format validation, `ptb64` output rejection, observable routing, writer-error propagation, or path-boundary errors.
- Do not claim full transform parity from partial `--ran_without_feedback` cases; name every deferred transform subcase.
- Do not add performance thresholds from one noisy probe run.
- Do not treat dirty-worktree benchmark reports as completion evidence.
