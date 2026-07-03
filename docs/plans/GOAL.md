# Goal: Finish M9 M2D Sweep And Feedback Parity

## Purpose

This document is the active execution contract for finishing `docs/plans/m9-m2d-sweep-feedback-parity-plan.md`.
The goal is to implement, test, benchmark, document, audit, and review the scoped `stab m2d` parity work for sweep-conditioned conversion and `--ran_without_feedback`.
Completion means the implemented Rust core and CLI behavior matches pinned Stim v1.16.0 for the owned subcases, while every unsupported or under-specified subcase is explicitly logged instead of being silently treated as complete.

Use `docs/plans/lessons-learned.md` throughout this work.
The main lesson for this goal is that broad upstream references are not acceptance criteria.
Every implemented claim must be tied to exact subcases, executable tests, oracle rows, benchmark rows, docs, and audit evidence.

## Scope

Included:

- Add `stab m2d --sweep` and `stab m2d --sweep_format` with streaming measurement and sweep input.
- Make omitted `--sweep` mean all-false sweep bits for every shot.
- Extend core detection conversion so detector and observable events can depend on per-shot sweep bits.
- Implement the supported `stab m2d --ran_without_feedback` surface by applying a feedback-inlining transform before detection conversion.
- Preserve existing `m2d` behavior for non-sweep inputs, observable routing, `ptb64` input, `ptb64` output rejection, path errors, format errors, and writer-error propagation.
- Add source-owned oracle, unit, CLI, and benchmark evidence for the new surfaces.
- Update docs and completion reports so they agree with implemented behavior.
- Run milestone-audit and full-code-review before claiming completion.

Excluded:

- Do not implement or document deprecated `--detector_hypergraph` as a supported Stab alias.
- Do not reopen Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, public graph/vector simulator APIs, or broader detector-analysis APIs.
- Do not claim full `stim.Circuit.with_inlined_feedback` or full transform API parity unless every upstream transform subcase is ported, tested, and documented.
- Do not add primary timing thresholds for the new benchmark rows unless repeated clean probe evidence proves they are stable and comparable.

## Sources Of Truth

- Active plan: `docs/plans/m9-m2d-sweep-feedback-parity-plan.md`.
- Planning lessons: `docs/plans/lessons-learned.md`.
- Roadmap and milestone policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Feature status: `docs/stab-feature-checklist.md`.
- Upstream baseline: pinned Stim v1.16.0 under `vendor/stim`.
- Upstream m2d CLI tests: `vendor/stim/src/stim/cmd/command_m2d.test.cc`.
- Upstream sweep conversion tests: `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`.
- Upstream feedback transform tests: `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`.
- Current Stab CLI path: `crates/stab-cli/src/detection.rs`.
- Current Stab core converter: `crates/stab-core/src/detection.rs`.
- Existing reverse-frame support: `crates/stab-core/src/sparse_rev_frame_tracker.rs`.
- Oracle manifest: `oracle/fixtures/manifest.csv`.
- Benchmark manifest and runner: `benchmarks/manifest.csv` and `ops/bench/src/baseline/m9.rs`.

If these sources disagree about scope, row status, supported flags, benchmark class, report paths, or deferrals, fix the stale source before claiming progress.

## Success State

The goal is complete only when:

- The owned upstream subcases are split into exact implemented rows, structural tests, or explicitly deferred notes.
- `stab m2d --sweep` and `--sweep_format` work for the planned `01` exact-output oracle row, packed `b8` CLI path, and observable-routing cases.
- Omitted `--sweep` on a sweep-conditioned circuit matches Stim all-false sweep behavior.
- Measurement and sweep input streams are processed in bounded memory and fail clearly on width or record-count mismatches.
- `stab m2d --ran_without_feedback` passes the owned command-level feedback parity case.
- The feedback-inlining transform proves detector and observable meaning is preserved for the supported subset.
- Unsupported sweep or feedback-transform shapes fail with precise domain errors and are documented.
- New oracle rows pass under `just oracle::run --implemented-only`.
- New benchmark rows are source-owned, smoke-tested, classified, and supported by probe reports or honest report-only notes.
- Documentation, roadmap text, feature checklist status, oracle manifest, benchmark metadata, and completion report agree with the implemented behavior.
- Milestone-audit and full-code-review findings are fixed, or under-specification findings are logged in `docs/plans/milestone-spec-gaps.md`.

## Execution Rules

- Start each milestone by adding or porting the tests that define the intended behavior.
- Do not treat a broad upstream file name as evidence; extract exact cases and name the comparator for each case.
- Keep the CLI streaming path bounded; do not materialize all measurement records, sweep records, detector records, or observable records.
- Preserve existing public compatibility unless a pinned-Stim oracle row proves the current behavior is wrong.
- Keep all new path-like values typed after parsing and maintain existing hostile-input boundaries.
- Do not weaken validation, resource limits, `ptb64` output rejection, observable routing errors, or writer-error propagation to make implementation easier.
- Do not commit unless the user explicitly asks for commits.
- If final completion is claimed after a requested commit, regenerate final evidence from committed code with `local_modifications=false`.

## Milestone Work Loop

For every milestone in the active plan:

1. Port or create targeted tests first, including negative tests and resource-boundary tests when the milestone touches public input or output.
2. Implement the feature or behavior named by the milestone with the narrowest code changes that fit existing Stab patterns.
3. Run the milestone’s targeted checks and fix failures.
4. Check the milestone acceptance criteria from `m9-m2d-sweep-feedback-parity-plan.md`.
5. Update docs, oracle metadata, benchmark metadata, and progress notes in the same change set as behavior changes.
6. Run milestone-audit for the milestone and fix implementation or evidence issues.
7. Run full-code-review for the touched surfaces and fix findings.
8. If audit or review exposes under-specified scope, log it in `docs/plans/milestone-spec-gaps.md` and keep the corresponding checklist entry partial or deferred.

## Required Milestone Evidence

Milestone 0 evidence:

- Oracle manifest rows exist for every exact CLI case planned for sweep and feedback.
- Structural rows exist for core transform tests that are not meaningful as CLI exact-output fixtures.
- The old manifest-only transform row is either split into implemented rows or kept with an updated, narrower deferred description.

Milestone 1 evidence:

- Core tests prove all-false default sweep behavior, per-shot sweep effects, sweep-controlled error propagation, `skip_reference_sample`, repeat handling, observable handling, and rejection of unsupported sweep shapes.
- Existing non-sweep detection conversion tests still pass.
- The converter exposes additive sweep-aware APIs while preserving existing all-false compatibility APIs.

Milestone 2 evidence:

- CLI tests prove `--sweep` and `--sweep_format` exact-output parity for the text `01` oracle row and structural parity for the packed `b8` path.
- CLI tests prove observable append and `obs_out` behavior with sweep input.
- CLI negative tests cover missing sweep paths, malformed sweep records, width mismatches, measurement/sweep count mismatches, invalid formats, and unchanged `ptb64` output rejection.
- Streaming tests prove large input reaches the writer or the expected validation failure without total-shot materialization.

Milestone 3 evidence:

- Core transform tests prove the supported `basic`, `demolition_feedback`, `loop`, `mpp`, and `interleaved` subcases, or the unfinished subcases are explicitly logged as spec gaps.
- CLI tests replace the old `--ran_without_feedback` rejection with positive parity for the upstream `command_m2d.m2d_without_feedback` case.
- A combined sweep plus `--ran_without_feedback` test proves sweep controls are preserved.
- Negative tests cover feedback shapes that remain unsupported.

Milestone 4 evidence:

- `benchmarks/manifest.csv` has source-owned rows for `m9-m2d-sweep-01-cli`, `m9-m2d-sweep-b8-cli`, `m9-m2d-sweep-obs-out-cli`, and `m9-m2d-ran-without-feedback-cli`.
- `ops/bench/src/baseline/m9.rs` has runners, measurement names, measurement work units, and compare notes for every new row.
- Benchmark tests validate row presence, runner coverage, measurement work, compare notes, and fixture validity.
- `just bench::smoke` passes.
- Focused baseline and compare probes exist for each new row that claims pinned-Stim comparability.

Milestone 5 evidence:

- `docs/stab-feature-checklist.md` reflects the implemented and still-deferred portions exactly.
- `docs/plans/rust-stim-drop-in-rewrite.md` reflects changed CLI behavior, benchmark acceptance, and exclusions.
- A completion report under `docs/plans/` records tests run, oracle rows, benchmark rows, report paths, audit outcome, review outcome, and remaining exclusions.
- `--detector_hypergraph` remains excluded consistently across docs.

## Targeted Test Commands

Run focused checks during implementation rather than waiting for the full suite:

```sh
cargo test -p stab-core detection --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-cli help --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench m9 --quiet
just oracle::run --milestone M9
just bench::smoke
```

Add narrower test filters when a change touches a specific parser, result-format reader, streaming writer, reverse-frame tracker branch, oracle comparator, benchmark manifest validator, or help topic.
Avoid tests that only restate constants or static labels.

## Benchmark Evidence Commands

After the new rows and fixtures exist, collect focused probe evidence before any threshold decision:

```sh
just bench::baseline --only m9-m2d-sweep-01-cli --out target/benchmarks/m9-m2d-sweep-01-baseline
just bench::compare --only m9-m2d-sweep-01-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-sweep-01-baseline/baseline.json --report target/benchmarks/m9-m2d-sweep-01-compare
just bench::baseline --only m9-m2d-sweep-b8-cli --out target/benchmarks/m9-m2d-sweep-b8-baseline
just bench::compare --only m9-m2d-sweep-b8-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-sweep-b8-baseline/baseline.json --report target/benchmarks/m9-m2d-sweep-b8-compare
just bench::baseline --only m9-m2d-sweep-obs-out-cli --out target/benchmarks/m9-m2d-sweep-obs-out-baseline
just bench::compare --only m9-m2d-sweep-obs-out-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-sweep-obs-out-baseline/baseline.json --report target/benchmarks/m9-m2d-sweep-obs-out-compare
just bench::baseline --only m9-m2d-ran-without-feedback-cli --out target/benchmarks/m9-m2d-ran-without-feedback-baseline
just bench::compare --only m9-m2d-ran-without-feedback-cli --warmup --measurement-runs 3 --baseline target/benchmarks/m9-m2d-ran-without-feedback-baseline/baseline.json --report target/benchmarks/m9-m2d-ran-without-feedback-compare
```

Keep the new rows report-only unless repeated evidence proves stable `cli-baseline` comparability.
If a row becomes threshold-owned, update `benchmarks/m12-primary-thresholds.json`, profiler notes, and docs in the same change set.

## Required Final Verification

Before claiming the goal complete, run:

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

If implementation changes broader shared behavior than expected, expand verification to `cargo test --workspace --quiet`.
If the user asks for commits, run the relevant final verification before committing and keep commits focused by purpose.

## Stop And Log Conditions

Stop implementation work and write a spec-gap entry when:

- A sweep target shape is accepted by Stim but cannot be represented by the current Stab sampler or converter without a broader API design.
- A feedback-transform subcase needs full transform API parity beyond the scoped `m2d --ran_without_feedback` surface.
- Exact loop refolding, MPP feedback behavior, or interleaved operation preservation cannot be faithfully completed in the two-day slice.
- Benchmark evidence is too noisy to justify threshold ownership.
- A docs source, oracle row, benchmark manifest, or feature checklist entry would need to overstate the implemented behavior to appear complete.
