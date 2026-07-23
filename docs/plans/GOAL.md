# Goal: Simplify And Complete Qualification

## Status

Active execution contract as of 2026-07-23.

Follow [qualification-economy-regression-plan.md](qualification-economy-regression-plan.md). The R0 through R5 compatibility repair is committed. Its old R6 evidence procedure was superseded before formal repaired-contract evidence began.

Formal evidence must wait until Q0 through Q7 are implemented, reviewed, committed, and regenerated from a clean unchanged revision.

## Sources Of Truth

- Correctness contract: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md)
- Performance contract: [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Execution plan: [qualification-economy-regression-plan.md](qualification-economy-regression-plan.md)
- Generated state: [../qualification-status.md](../qualification-status.md), once Q7 creates it
- Project lessons: [lessons-learned.md](lessons-learned.md)

Stop if these sources disagree. Fix the source and regenerate derived state instead of choosing the easiest interpretation.

## Current Work

1. Q0: freeze the replacement program and documentation hierarchy.
2. Q1: centralize the result-format corpus and remove low-value test assertions.
3. Q2: curate the finite release and diagnostic performance matrices.
4. Q3: separate Stim parity from Stab self-regression.
5. Q4: reduce the global worker preflight to source-derived representative receipts.
6. Q5: add three representative DEM workload families.
7. Q6: replace the active completion ceremony with one revision manifest and replay.
8. Q7: add contract CI and generated qualification status.
9. Q8: freeze the source revision and produce formal correctness, timing, memory, rollup, completion, and baseline evidence.

Do not begin Q8 while any Q0 through Q7 contract is unsettled.

## Non-Negotiable Rules

- Target Stim v1.16.0.
- Preserve repaired result-format, path-alias, process-supervision, and `raw-work-v2` behavior.
- Keep Stim parity at median and confidence upper bound no greater than `1.25x`.
- Treat missing self-regression baselines as unseeded, never passing.
- Preserve failed and historical artifacts with their original source and schema identities.
- Never reuse an artifact path.
- Promotable evidence requires `local_modifications=false`.
- Disable swap only for controlled formal timing and restore the exact prior configuration on every exit.
- Intentionally deferred Stim and ecosystem surfaces remain deferred.

## Acceptance Loop

For Q0 through Q7, add meaningful focused tests, implement the complete contract, run targeted checks, and keep documentation and generated state synchronized. Before Q8, run milestone audit and full code review once across the complete source contract and fix every confirmed finding.

After Q8 evidence, repeat milestone audit and full code review against the reports and generated status. Log only genuine under-specification in `milestone-spec-gaps.md`; fix implementation, test, benchmark, and documentation defects directly.

## Next Commands

During source work, use targeted Cargo tests plus:

```text
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just oracle::result-formats --check
```

Before the Q8 source freeze, run formatting, workspace Clippy, all workspace tests, generated status checks, benchmark smoke, and pre-commit.

## Completion

The goal completes only when Q0 through Q8 meet their acceptance criteria, the generated dashboard agrees with checked inventories and evidence, controlled AArch64 parity passes without weakened gates, the first reviewed self-regression baseline is seeded, host state is restored, no qualification process remains, and the worktree is clean.
