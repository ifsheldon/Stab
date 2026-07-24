# Goal: Close Qualification Simplification

## Status

Q0 through Q8 are implemented. Clean source revision `68d107a42f655254f31628f0cbedc55479f6c0f3` passed repaired correctness qualification and all 36 controlled AArch64 DEM parity reports. Eighteen reviewed AArch64 self-regression identities are now seeded for future runs.

The original completion correctly reports self-regression as `unseeded`; the baseline was created afterward and cannot retroactively pass the first run. Exact evidence lives in [qualification-economy-regression-progress-report.md](qualification-economy-regression-progress-report.md).

## Sources Of Truth

- Correctness contract: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md)
- Performance contract: [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Execution plan: [qualification-economy-regression-plan.md](qualification-economy-regression-plan.md)
- Generated state: [../qualification-status.md](../qualification-status.md)
- Evidence report: [qualification-economy-regression-progress-report.md](qualification-economy-regression-progress-report.md)
- Project lessons: [lessons-learned.md](lessons-learned.md)

Stop if these sources disagree. Fix the source and regenerate derived state instead of choosing the easiest interpretation.

## Completion State

1. The reviewed AArch64 baselines, completion checkpoint, generated dashboard, and synchronized evidence report are committed.
2. The post-evidence milestone audit and full code review are complete. Their stale published-state test fixtures were fixed in `f465b6f6c4b2ef9ab543bca76e6d9b399f600032`; no blocking implementation or contract finding remains.
3. The complete verification set passed against the closure source state, including the live 62-case pinned-Stim corpus, checked inventory regeneration, generated status, benchmark smoke, and staged pre-commit policy.

x86-64 controlled evidence and all intentionally deferred Stim surfaces remain future work, not blockers for this goal.

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

Run milestone audit and full code review against the completed evidence and generated status. Log only genuine under-specification in `milestone-spec-gaps.md`; fix implementation, test, benchmark, and documentation defects directly.

Then run:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just oracle::result-formats --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```

## Completion

This goal is complete. The post-evidence audits have no unresolved confirmed findings, the verification set passes, the dashboard agrees with checked evidence, swap remains restored, and no qualification process remains.
