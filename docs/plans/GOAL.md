# Post-Beta Threshold Completion Goal

## Position

The active goal is to finish `docs/plans/post-beta-threshold-completion-plan.md`.
The previous `GOAL.md` was useful for the broader post-beta timing hardening pass, but it is now too broad for the remaining work.
This file is the execution checklist for closing the two current items: remove ambiguous timing-regression `not-configured` rows, and finish the real timing work for `m4-gate-lookup`, `m5-sparse-xor`, and the `m8-measure-reader-*` family.

Do not reopen the completed parts of the previous timing-hardening pass unless evidence shows they regressed.
Do not reopen intentionally deferred Stim parity or ecosystem surfaces such as Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.

## Sources Of Truth

- Active plan: `docs/plans/post-beta-threshold-completion-plan.md`
- Lessons for avoiding repeated planning failures: `docs/plans/lessons-learned.md`
- Prior timing plan: `docs/plans/post-beta-timing-hardening-plan.md`
- Current post-beta status report: `docs/plans/post-beta-fix-report.md`
- Historical M12 report: `docs/plans/m12-progress-report.md`
- Under-specification log: `docs/plans/milestone-spec-gaps.md`
- Threshold source: `benchmarks/m12-primary-thresholds.json`
- Beta waiver source: `benchmarks/m12-primary-beta-waivers.json`
- Timing-regression waiver source: `benchmarks/m12-primary-regression-waivers.json`, once added by the active plan
- Benchmark manifest: `benchmarks/manifest.csv`
- Profiler notes: `benchmarks/profiler-notes/m12/`

If these sources disagree, fix the stale source in the same change set.
Do not leave contradictory row counts, waiver counts, threshold counts, report paths, commit ids, or status labels for the next agent.

## Objective

Finish the remaining post-beta threshold work so the final timing-regression report has no ambiguous `not-configured` rows.

The expected final states are:

- `pass` for comparable implemented rows whose row-level or schema-version-2 submeasurement thresholds pass at `1.25x`.
- `waived-not-thresholdable` or an equivalent explicit status for true no-ratio contract-only rows whose regression waivers are source-owned and machine-checked.
- zero rows left as plain `not-configured`.

The active rows are:

1. `m4-circuit-canonical-print`
2. `m4-gate-lookup`
3. `m5-sparse-xor`
4. `m7-convert-stim-canonical`
5. `m8-measure-reader-01`
6. `m8-measure-reader-b8`
7. `m8-measure-reader-r8`
8. `m8-measure-reader-hits`
9. `m8-measure-reader-dets`
10. `m8-measure-reader-ptb64-contract`
11. `m10-dem-print-contract`

## Non-Negotiable Rules

- Do not weaken a threshold to make a row pass.
- Do not add a waiver for a row that can be made comparable with better benchmark shape.
- Do not add fake thresholds to contract-only rows with no faithful pinned-Stim ratio.
- Do not hide slow direct submeasurements behind a passing row median.
- Do not add strict thresholds for tiny or noisy evidence until warmup and repeated clean runs prove the measurement is stable.
- Do not optimize before benchmark evidence identifies the slow comparable surface.
- Do not cite dirty-worktree benchmark reports as final acceptance evidence.
- Do not mark a row complete while tests, thresholds, waivers, profiler notes, reports, and plan docs disagree.
- Preserve Stim v1.16.0 compatibility for implemented public behavior.
- Keep public API changes additive unless the plan is explicitly amended.

## Work Loop

Complete one row group at a time.
Each row group must finish tests, benchmark shape, implementation, threshold or waiver decision, documentation, milestone-audit, full-code-review, and final evidence.

### 1. Establish Current Evidence

Run the Milestone 0 commands from `docs/plans/post-beta-threshold-completion-plan.md`.
Record the starting row statuses and confirm whether the run was dirty or clean.
Use exploratory dirty reports only for iteration, never for final acceptance.

### 2. Fix Contract-Only Waiver Handling

Implement timing-regression waiver support before touching performance code.
The contract-only rows are measured evidence rows that cannot produce a faithful Stim-relative ratio.
They should be checked and explicit, not hidden under `not-configured`.

Required rows:

- `m4-circuit-canonical-print`
- `m7-convert-stim-canonical`
- `m8-measure-reader-ptb64-contract`
- `m10-dem-print-contract`

Done means the timing-regression command rejects stale or misapplied waivers and reports these rows with an explicit no-ratio waiver status.

### 3. Finish Reader Threshold Ownership

Complete `m8-measure-reader-01`, `m8-measure-reader-b8`, `m8-measure-reader-r8`, `m8-measure-reader-hits`, and `m8-measure-reader-dets`.
Split dense and sparse submeasurements so each faithful pinned-Stim reader filter has matching Stab evidence.
Guard stable direct pairs below `1.25x` with schema-version-2 thresholds.
Document any pair that remains outside strict threshold ownership with a source-owned reason in `benchmarks/profiler-notes/m12/m8-measure-reader.md`.

### 4. Finish Sparse XOR Threshold Ownership

Complete `m5-sparse-xor`.
Keep table row-XOR and item-XOR separate.
Make the item-XOR workload large and faithful enough to escape timer noise, profile the real cost, optimize only the measured bottleneck, and preserve sorted-unique invariants.
Guard both direct pairs only after repeated clean evidence is stable below `1.25x`.

### 5. Finish Gate Lookup Threshold Ownership

Complete `m4-gate-lookup`.
Stabilize the benchmark by using larger repeated canonical lookup sets before changing lookup implementation.
Pair faithful canonical lookup evidence with pinned Stim `gate_data_hash_all_gate_names`.
Keep alias, lowercase, and invalid lookup measurements as Stab-only contract extras unless faithful Stim pairs exist.
If optimization is needed, derive any table-driven path from canonical gate metadata instead of hand-maintaining duplicate definitions.

### 6. Synchronize Documentation

Update every source that would otherwise mislead a future agent.
At minimum, check:

- `docs/plans/post-beta-threshold-completion-plan.md`
- `docs/plans/post-beta-timing-hardening-plan.md`
- `docs/plans/post-beta-fix-report.md`
- `docs/plans/m12-progress-report.md`
- `docs/plans/milestone-spec-gaps.md`
- `benchmarks/README.md`
- `benchmarks/m12-primary-thresholds.json`
- `benchmarks/m12-primary-regression-waivers.json`
- `benchmarks/m12-primary-beta-waivers.json`
- `benchmarks/profiler-notes/m12/m4-gate-lookup.md`
- `benchmarks/profiler-notes/m12/m5-sparse-xor.md`
- `benchmarks/profiler-notes/m12/m8-measure-reader.md`

Only log a new entry in `docs/plans/milestone-spec-gaps.md` for a true under-specification discovered during implementation.
Implementation defects, missing tests, stale docs, and benchmark failures should be fixed in the active work.

### 7. Run Audit And Review

Run milestone-audit after each row group or after a coherent batch, using names from the active plan.
Run full-code-review after milestone-audit is clean or has only accepted under-specification follow-ups.
Fix all findings unless the issue is a true future-scope specification gap and is logged in `docs/plans/milestone-spec-gaps.md`.

## Test And Benchmark Requirements

Add or update focused tests before or alongside implementation changes.
Tests must protect meaningful contracts, not just labels or constants.

Required focused areas:

- threshold parsing, regression waiver parsing, stale waiver rejection, stale submeasurement rejection, missing evidence rejection, and failing ratio rejection;
- reader exact round trips, dense and sparse fixture decoding, malformed input rejection, and benchmark submeasurement presence;
- sparse XOR sorted-unique invariants, symmetric-difference behavior, repeated item toggling, duplicate input handling, and reference-model equivalence;
- gate lookup canonical names, aliases, lowercase variants, invalid names, metadata single-sourcing, and benchmark submeasurement presence.

Use targeted tests during iteration.
Run the full required verification before declaring completion.

## Final Completion Criteria

The goal is complete only when all of the following are true:

- `docs/plans/post-beta-threshold-completion-plan.md` has been implemented.
- Every active comparable row owns a strict row-level or schema-version-2 submeasurement threshold.
- Every active no-ratio contract-only row has a source-owned checked regression waiver.
- The final timing-regression report has zero ambiguous `not-configured` rows.
- Every behavior changed for performance has meaningful tests.
- Every benchmark shape change has benchmark-runner tests.
- Profiler notes explain the final threshold or waiver decision for each active row group.
- Documentation and machine-readable sources agree on final row counts, threshold counts, waiver counts, commands, and report paths.
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
just bench::baseline --primary --out target/benchmarks/post-beta-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-compare
just bench::primary-beta --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json --report target/benchmarks/post-beta-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/post-beta-primary-baseline/baseline.json
just maintenance::pre-commit
```

If any command is skipped, blocked, or replaced by a narrower check, record the reason in the durable completion evidence and do not mark the goal complete until the gap is resolved or explicitly accepted as follow-up scope.
