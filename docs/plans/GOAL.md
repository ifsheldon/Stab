# Post-Beta Timing Hardening Goal

## Position

The old `GOAL.md` is no longer the right control document.
It was useful when Stab was moving through broad M0 through M12 implementation milestones, but the active work is now narrower and stricter.
The current goal is to finish `docs/plans/post-beta-timing-hardening-plan.md` correctly, with truthful timing evidence, meaningful tests, source-owned thresholds, synchronized documentation, and clean final reports.

Use `docs/plans/post-beta-timing-hardening-plan.md` for row-specific tasks.
Use this file to decide whether that plan is actually complete.

## Objective

Finish the post-beta timing-hardening plan for already implemented Rust and CLI surfaces.
The plan is complete only when each target row or row family has either stable guarded timing evidence or a documented, evidence-backed reason why the remaining surface should not own a strict threshold yet.

Target rows and row families:

1. `m8-measure-reader`, now represented in `benchmarks/manifest.csv` by `m8-measure-reader-01`, `m8-measure-reader-b8`, `m8-measure-reader-r8`, `m8-measure-reader-hits`, `m8-measure-reader-dets`, and `m8-measure-reader-ptb64-contract`
2. `m5-simd-bits`
3. `m5-sparse-xor`
4. `m10-error-decomp`
5. `m4-gate-lookup`

Do not reopen intentionally deferred Stim parity or ecosystem surfaces while completing this goal.
Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, and new public graph/vector simulator APIs remain out of scope unless the plan is explicitly amended.

## Sources Of Truth

- Primary plan: `docs/plans/post-beta-timing-hardening-plan.md`
- Current status report: `docs/plans/post-beta-fix-report.md`
- Historical M12 report: `docs/plans/m12-progress-report.md`
- Lessons for planning and verification: `docs/plans/lessons-learned.md`
- Under-specified follow-ups: `docs/plans/milestone-spec-gaps.md`
- Threshold source: `benchmarks/m12-primary-thresholds.json`
- Waiver source: `benchmarks/m12-primary-beta-waivers.json`
- Benchmark manifest: `benchmarks/manifest.csv`
- Profiler notes: `benchmarks/profiler-notes/m12/`

If these files disagree, fix the stale file in the same change set.
Do not leave contradictory row counts, waiver counts, threshold counts, command results, or evidence paths for the next agent to untangle.

## Non-Negotiable Rules

- Do not weaken a threshold to make a row pass.
- Do not add a waiver for a row that can be made comparable with better benchmark shape.
- Do not hide slow direct submeasurements behind a passing row median.
- Do not add strict thresholds for tiny or noisy evidence until repeated clean runs prove the measurement is stable.
- Do not optimize before the benchmark identifies the slow comparable surface.
- Do not cite dirty-worktree benchmark reports as final acceptance evidence.
- Do not mark a row complete while tests, thresholds, profiler notes, reports, and plan docs disagree.
- Preserve Stim v1.16.0 compatibility for implemented public behavior.
- Keep public API changes additive unless the plan is explicitly amended.

## Row Work Loop

Complete one target row or row family at a time.
Each row or row family is a small milestone with its own tests, benchmark evidence, threshold decision, documentation update, audit, and review closure.

### 1. Read The Contract

Read the row section in `docs/plans/post-beta-timing-hardening-plan.md`, the matching profiler note, the matching `benchmarks/manifest.csv` row or rows, and the current status in `docs/plans/post-beta-fix-report.md`.
Classify each source-owned row as a direct comparable benchmark, a mixed comparable benchmark, a contract-only benchmark, or a benchmark with intentionally unguarded tiny evidence.

### 2. Make The Benchmark Truthful

Split mixed rows before optimizing implementation code.
Each comparable submeasurement needs a paired Stim name, Stab name, ratio, and normalized work unit where a normalized rate is meaningful.
For configured schema-version-2 thresholds, the timing-regression report is the authoritative paired-evidence report because threshold application records explicit configured pairs even when plain compare or beta reports only include automatic exact-name or positional pairs.
Each Stab-only contract extra must be documented as ineligible for Stim-relative timing thresholds.
Tiny operations must be batched or repeated enough that benchmark time is measuring the operation instead of timer noise.

### 3. Add Or Port Tests

Add tests before or alongside implementation changes.
Tests should protect compatibility, public behavior, format validation, numerical invariants, resource boundaries, and data-structure invariants.
Avoid tests that only restate constants, assert freshly constructed struct fields, check generic serialization mechanics, or verify labels without exercising behavior.

### 4. Profile Stable Evidence

Profile only after the benchmark surface is stable enough to be meaningful.
Update the matching profiler note with the dominant cost, optimization decision, evidence quality, and any reason a submeasurement remains outside strict threshold ownership.

### 5. Optimize Conservatively

Optimize the measured hot path behind existing abstractions.
Keep changes local to the bottleneck.
Preserve typed boundaries, validation semantics, file-format behavior, numerical stability, memory safety, and data-structure invariants.
For bit and sparse-XOR work, prove equivalence against scalar or reference models.
For probability arithmetic, preserve correctness and validation over speed.
For result readers, preserve exact format validation and bounded-memory behavior.

### 6. Decide Threshold Ownership

After implementation, rerun repeated evidence.
If the whole row is comparable, stable, and below `1.25x`, guard it with a row-level threshold.
If only selected submeasurements are comparable, stable, and below `1.25x`, guard those pairs with schema-version-2 submeasurement thresholds.
If a submeasurement is noisy, contract-only, intentionally deferred, or still too small to threshold honestly, keep it out of `benchmarks/m12-primary-thresholds.json` and document the reason in the profiler note.

Any threshold-system behavior change must include tests for compatible schema-version-1 parsing, schema-version-2 parsing, stale submeasurement ids, missing evidence, and failing ratios.

### 7. Synchronize Documentation

Update every document that would otherwise mislead a future agent.
At minimum, check `docs/plans/post-beta-timing-hardening-plan.md`, `docs/plans/post-beta-fix-report.md`, `docs/plans/m12-progress-report.md`, `docs/plans/milestone-spec-gaps.md`, `benchmarks/README.md`, and the matching profiler note under `benchmarks/profiler-notes/m12/`.

### 8. Run Audit And Review

Run milestone-audit for each completed row using the milestone name `post-beta-timing-hardening: row-id`.
Fix implementation, test, benchmark, documentation, compatibility, workflow, and verification findings.
Log only true under-specification issues in `docs/plans/milestone-spec-gaps.md`.

Run full-code-review after milestone-audit is clean or has only accepted under-specification follow-ups.
Fix correctness, compatibility, security, performance, architecture, test-quality, and documentation findings.
Re-run affected tests, benchmarks, audits, or review slices after fixes.

## Row Ledger

| Row | Required finish state | Threshold expectation |
| --- | --- | --- |
| `m8-measure-reader-*` | Reader evidence is split into `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` source-owned rows; tests prove exact format parity, malformed-input rejection, and bounded streaming behavior where used. | Guard stable direct format pairs only; document mixed dense/sparse comparisons and contract-only `ptb64` surfaces. |
| `m5-simd-bits` | Direct bit operations are split from Stab-only extras; tests prove scalar-reference equivalence across offsets, masks, lengths, overlaps, and dirty tails. | Guard stable direct comparable operations only. |
| `m5-sparse-xor` | Table row-XOR and item-XOR are measured separately; tests prove sorted-unique invariants and symmetric-difference behavior. | Guard row-XOR and item-XOR only after each has stable meaningful evidence. |
| `m10-error-decomp` | Approximate, exact, and independent-to-disjoint conversion families are measured with case-diverse batched evidence; tests prove numerical stability and boundary behavior. | Keep stable approximate thresholds; add exact or independent-to-disjoint thresholds only when clean repeated evidence supports them. |
| `m4-gate-lookup` | Lookup evidence uses larger repeated sets and separates canonical names, aliases, lowercase normalization, and invalid names; tests prove metadata remains single-sourced. | Guard stable lookup submeasurements only; document sub-100ns noisy surfaces. |

## Evidence Requirements

Dirty-worktree benchmark reports are useful during iteration, but they are not final evidence.
Final acceptance evidence must be generated from committed code and must report `local_modifications=false`.

Every completed row needs durable evidence for:

- Stab commit id
- Pinned Stim commit id
- Benchmark command
- Report path
- Warmup status
- Measurement-run count
- Local-modification status
- Comparable Stim and Stab measurements
- Threshold decision
- Reason for every comparable-looking submeasurement left outside strict thresholds

## Final Completion Criteria

The post-beta timing-hardening plan is complete only when all of the following are true:

- Every target row or row family has completed the row work loop.
- Every behavior changed for performance has meaningful tests.
- Every mixed row has explicit paired submeasurement evidence.
- Every stable comparable row or submeasurement below `1.25x` is guarded in `benchmarks/m12-primary-thresholds.json`.
- Every unthresholded row or submeasurement has a source-owned explanation in its profiler note.
- `docs/plans/post-beta-timing-hardening-plan.md`, `docs/plans/post-beta-fix-report.md`, `docs/plans/m12-progress-report.md`, profiler notes, benchmark sources, threshold files, and waiver files match the final behavior.
- Milestone-audit and full-code-review have been run, and their findings are fixed or logged as accepted specification follow-ups.
- Final benchmark evidence was regenerated from committed code with `local_modifications=false`.

If the worktree still contains uncommitted changes, the plan may have progress but it is not complete under this goal.

## Required Final Verification

Run these commands before declaring the plan complete:

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

If any command is skipped, blocked, or replaced by a narrower check, record the reason in the durable completion evidence and do not mark the plan complete until the gap is resolved or explicitly accepted as follow-up scope.
