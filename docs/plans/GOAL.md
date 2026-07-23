# Goal: Simplify And Complete Qualification

## Status

Active execution contract as of 2026-07-23.

Follow [qualification-economy-regression-plan.md](qualification-economy-regression-plan.md). The R0 through R5 compatibility repair is committed. Its old R6 evidence procedure was superseded before formal repaired-contract evidence began.

Q0 through Q7 are implemented, reviewed, and committed. Formal repaired-contract evidence has not started and must be produced from the clean documentation-synchronized revision that closes this source freeze.

## Sources Of Truth

- Correctness contract: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md)
- Performance contract: [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Execution plan: [qualification-economy-regression-plan.md](qualification-economy-regression-plan.md)
- Generated state: [../qualification-status.md](../qualification-status.md)
- Project lessons: [lessons-learned.md](lessons-learned.md)

Stop if these sources disagree. Fix the source and regenerate derived state instead of choosing the easiest interpretation.

## Current Work

1. Completed source work: Q0 documentation freeze, Q1 corpus/test economy, Q2 curated matrix, Q3 parity and self-regression separation, Q4 representative worker preflight, Q5 DEM families, Q6 revision manifest, and Q7 contract CI plus generated status.
2. The pre-evidence milestone audit and full code review repaired parity-ceiling, stale-regression-target, semantic workload-identity, exact rollup-parity, completion-boundary, generated-status, accepted-maximum memory-publication, dead-test, and source-file-size findings.
3. No Q0 through Q7 implementation or specification blocker remains.
4. Next: Q8 reopened correctness evidence, diagnostic legacy benchmarks, controlled AArch64 DEM timing and memory evidence, two accepted-maximum memory receipts, four rollups, one completion manifest and replay, then a separately reviewed self-regression baseline candidate.

Do not begin Q8 from a dirty tree or reuse any prior artifact path.

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
