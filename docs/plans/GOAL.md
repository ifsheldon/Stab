# 1.25x Beta Performance Goal

## Position

The active goal is to finish `docs/plans/beta-125-performance-plan.md`.
The previous `GOAL.md` closed the post-beta threshold-completion work and is no longer the active execution contract.
This file gives agents the operating instructions for tightening the primary beta gate from `2.0x` to `1.25x` and doing the necessary performance, benchmark-shape, documentation, audit, and review work correctly.

Do not reopen completed threshold-completion work unless current evidence proves it regressed.
Do not reopen intentionally deferred Stim parity or ecosystem surfaces such as Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.

## Sources Of Truth

- Active plan: `docs/plans/beta-125-performance-plan.md`
- Lessons for avoiding repeated planning failures: `docs/plans/lessons-learned.md`
- Current roadmap: `docs/plans/rust-stim-drop-in-rewrite.md`
- Historical M12 report: `docs/plans/m12-progress-report.md`
- Current post-beta status report: `docs/plans/post-beta-fix-report.md`
- Prior timing-hardening plan: `docs/plans/post-beta-timing-hardening-plan.md`
- Prior threshold-completion plan: `docs/plans/post-beta-threshold-completion-plan.md`
- Under-specification log: `docs/plans/milestone-spec-gaps.md`
- Benchmark manifest: `benchmarks/manifest.csv`
- Beta waiver source: `benchmarks/m12-primary-beta-waivers.json`
- Timing threshold source: `benchmarks/m12-primary-thresholds.json`
- Timing-regression waiver source: `benchmarks/m12-primary-regression-waivers.json`
- Memory baseline source: `benchmarks/m12-primary-memory-baseline.json`
- Profiler notes and optimization log: `benchmarks/profiler-notes/m12/`

If these sources disagree, fix the stale source in the same change set.
Do not leave contradictory beta ratios, row counts, waiver counts, threshold counts, report paths, commit ids, or status labels for the next agent.

## Objective

Make `just bench::primary-beta` enforce a `1.25x` pinned-Stim beta performance gate and make the implemented primary matrix pass that stricter gate without dishonest benchmark comparisons.

The expected final state is:

- every comparable primary row passes beta at `<=1.25x` using the worse of row median and paired submeasurement ratios;
- no comparable row receives a beta waiver;
- measured no-ratio `contract-only` rows remain accepted only through checked source-owned beta waivers;
- timing-regression still has zero ambiguous `not-configured` rows;
- memory regression still passes;
- all docs and machine-readable sources describe the same active gate.

## Active Row Groups

The original hard blocker for a `1.25x` beta gate was:

1. `m10-error-decomp`

The current dirty-worktree beta-125 probe in `docs/plans/beta-125-performance-plan.md` shows no remaining comparable row above `1.25x`.
Treat final clean committed-code evidence as the active blocker unless a clean rerun changes the row set.

The headroom rows to recheck in clean final evidence are:

1. `m8-measure-reader-dets`
2. `m10-error-decomp`

The historical watch rows from the older clean report are:

1. `m5-simd-bits`
2. `m4-circuit-parse`
3. `m5-sparse-xor`
4. `m4-gate-lookup`
5. `m8-sample-primary-unrotated-surface-contract`

The current no-ratio beta-waiver rows are:

1. `m4-circuit-canonical-print`
2. `m7-convert-stim-canonical`
3. `m8-measure-reader-ptb64-contract`
4. `m10-dem-print-contract`

If fresh clean evidence changes these sets, update `docs/plans/beta-125-performance-plan.md` before optimizing more code.

## Non-Negotiable Rules

- Do not weaken benchmark semantics to make a row pass.
- Do not add beta waivers for comparable rows.
- Do not hide slow direct submeasurements behind a passing row median.
- Do not let Stab-only contract extras decide strict Stim-relative beta status.
- Do not treat a dirty-worktree report as final acceptance evidence.
- Do not optimize before benchmark evidence identifies the slow comparable surface.
- Do not add or keep tiny strict-gate evidence if timer overhead dominates and a larger faithful benchmark is needed.
- Do not mark a row complete while tests, thresholds, waivers, profiler notes, reports, and plan docs disagree.
- Preserve Stim v1.16.0 compatibility for implemented public behavior.
- Keep public API changes additive unless the plan is explicitly amended.

## Work Loop

Work one milestone or row group at a time.
Each group must finish tests, benchmark shape, implementation, docs, source-owned evidence, milestone-audit, full-code-review, and final clean reports before the goal is closed.

### 1. Establish Clean Starting Evidence

Run the B0 commands from `docs/plans/beta-125-performance-plan.md`.
Confirm the current failure set and headroom rows.
If the fresh evidence differs from the plan, update the plan first and explain why.

### 2. Change The Gate Deliberately

Implement the `1.25x` beta gate as an expected-failing change before optimizing rows.
Update code, CLI help, tests, docs, and any report text that defines the active beta gate.
Historical `2.0x` evidence may remain only when it is explicitly labeled historical.

### 3. Fix Benchmark Shape Before Performance Code

Check `m5-simd-bits` before optimizing it.
Make faithful direct pairs and Stab-only contract extras impossible to confuse if the strict gate exposes the older row-shape problem again.
Prefer splitting rows over adding special-case prose when a split makes the evidence clearer.
Do not optimize SIMD code until the benchmark proves a faithful direct pair is slow.

### 4. Optimize Real Hot Paths

For `m4-circuit-parse`, use focused evidence to identify the remaining sparse-parser cost before changing parser internals if the row returns above or near the strict gate.
For `m10-error-decomp`, the current accepted implementation keeps the tiny direct-match filters and optimizes the real overhead exposed by focused evidence.
Do not add larger case-array benchmark variants unless clean evidence shows the current tiny filters are still too unstable to support honest gate evidence.

### 5. Add Headroom

Recheck `m8-measure-reader-dets`, `m5-sparse-xor`, `m4-gate-lookup`, and `m8-sample-primary-unrotated-surface-contract` after the hard blockers are fixed.
Optimize only where focused evidence shows real risk or repeated clean runs drift toward `1.25x`.
Do not overfit nanosecond-scale rows.

### 6. Synchronize Documentation

Update all docs and machine-readable sources affected by the gate or benchmark-shape change.
At minimum, check:

- `docs/plans/beta-125-performance-plan.md`
- `docs/plans/GOAL.md`
- `docs/plans/rust-stim-drop-in-rewrite.md`
- `docs/plans/m12-progress-report.md`
- `docs/plans/post-beta-fix-report.md`
- `docs/plans/milestone-spec-gaps.md`
- `benchmarks/README.md`
- `benchmarks/m12-primary-beta-waivers.json`
- `benchmarks/m12-primary-thresholds.json`
- `benchmarks/m12-primary-regression-waivers.json`
- `benchmarks/profiler-notes/m12/optimization-log.json`
- relevant `benchmarks/profiler-notes/m12/*.md` files

Only log a new entry in `docs/plans/milestone-spec-gaps.md` for a true under-specification discovered during implementation.
Implementation defects, missing tests, stale docs, and benchmark failures should be fixed in the active work.

### 7. Run Audit And Review

Run milestone-audit after the row groups are implemented and docs are synchronized.
Run full-code-review after milestone-audit is clean or has only accepted under-specification follow-ups.
Fix all findings unless the issue is a true future-scope specification gap and is logged in `docs/plans/milestone-spec-gaps.md`.

## Required Focused Tests And Evidence

Add or update focused tests before or alongside implementation changes.
Tests must protect meaningful behavior, benchmark contracts, compatibility, or regression risk.

Required areas:

- beta gate parsing and failure messages at `1.25x`;
- beta waiver rejection for stale, misapplied, comparable, pending, invalid-baseline, or above-gate rows;
- paired submeasurement worst-ratio beta behavior;
- benchmark runner tests for any split M5 SIMD rows or changed M10 error-decomposition rows;
- parser tests for any changed `.stim` parse fast path;
- arithmetic tests for any changed probability or error-decomposition fast path;
- sparse XOR invariant tests if the headroom work changes sparse XOR code;
- sampler or oracle tests if the M8 unrotated-surface row requires code changes.

Use targeted tests during iteration.
Run the full required verification before declaring completion.

## Final Completion Criteria

The goal is complete only when all of the following are true:

- `docs/plans/beta-125-performance-plan.md` has been implemented.
- `--require-beta-gate` enforces `1.25x`.
- Every comparable primary row passes the `1.25x` beta gate.
- Every measured no-ratio `contract-only` beta waiver is source-owned and machine-checked.
- No comparable beta row is waived.
- Timing-regression still has zero ambiguous `not-configured` rows.
- Memory regression passes all primary rows.
- Every behavior changed for performance has meaningful tests.
- Every benchmark-shape change has benchmark-runner tests.
- Profiler notes and optimization-log entries explain final decisions for changed or close rows.
- Documentation and machine-readable sources agree on final gate ratio, row counts, threshold counts, waiver counts, commands, and report paths.
- Milestone-audit and full-code-review have been run, and their findings are fixed or logged as accepted specification follow-ups.
- Final benchmark evidence was regenerated from committed code with `local_modifications=false`.
- The worktree is clean unless the user explicitly accepts uncommitted follow-up work.

## Required Final Verification

Run these commands before declaring the goal complete:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::baseline --primary --out target/benchmarks/beta-125-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/beta-125-primary-baseline/baseline.json --report target/benchmarks/beta-125-primary-compare
just bench::primary-beta --baseline target/benchmarks/beta-125-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/beta-125-primary-baseline/baseline.json --report target/benchmarks/beta-125-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/beta-125-primary-baseline/baseline.json
just maintenance::pre-commit
```

If any command is skipped, blocked, or replaced by a narrower check, record the reason in durable completion evidence and do not mark the goal complete until the gap is resolved or explicitly accepted as follow-up scope.
