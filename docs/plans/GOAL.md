# Goal: Repair And Close A6 Before Decoder Extraction

## Objective

Finish milestone A6 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) from one clean reviewed revision before admitting the parked `stab-decoder` A7 work.

## Current State

- A0 through A5 are complete, and the physical component split is committed.
- The `3a93719b` correctness, SIMD, matrix, and focused artifacts are preserved but review-rejected. They do not close A6 because direct owner-package coverage was incomplete, several benchmark witnesses were not independently fixed, focused timing counts did not follow one valid contract, and no checked ledger bound the focused artifacts.
- Product review also requires checked `ReferenceSampleTree` construction and fallible legacy sampler materialization before evidence can be regenerated.
- `stab-decoder` source and benchmark work remains preserved in named stashes and must stay parked until A6 closes.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), milestone A6
- [Component graph](../architecture/README.md)
- [A6 extraction map](../architecture/a6-component-extraction-map.md)
- [Progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owner when Cargo metadata, architecture checks, generated inventories, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Commit the audit-corrected contract before replacement evidence.
2. Make `ReferenceSampleTree` construction, traversal, and materialization resource-bounded and fallible; add fallible legacy sampler materializers while preserving documented compatibility shims.
3. Enforce direct canonical package evidence for all six Stable components. Retarget wholly owned selectors and record only narrow, reviewed cross-component or facade exceptions.
4. Replace self-derived or width-only benchmark witnesses with independently fixed semantic expectations.
5. Commit source and regenerated inventories, then run milestone-audit and full-code-review before timing.
6. From that clean commit, regenerate exact full-tier M5 and M6 correctness, scalar-versus-portable SIMD diagnostics, and the complete 166-row warmed comparison with unchanged comparator and threshold policies.
7. Recompute every greater-than-15-percent crossing. Run one warmed outer diagnostic per crossing with the source-owned internal timing count of at least eight. Profile reproductions when host policy permits and record restrictions otherwise.
8. Generate and validate the checked focused-evidence ledger, including report and predecessor digests, exact measurements, sample counts, ratios, and dispositions.
9. Run the complete local verification matrix, repeat milestone-audit and full-code-review, and obtain green GitHub CI for the exact closure revision.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and scalar engine code.
- Only `stab-kernels-simd` contains direct portable-SIMD code, and it has no Stab dependency.
- `stab-core` remains the compatibility facade; canonical implementation and qualification ownership stays with component crates.
- No comparator class, `1.25x` threshold, waiver, semantic witness, or feature-selection policy is relaxed to obtain closure.
- Host-unverified timing is diagnostic only. A9 owns promotable controlled-host parity.
- Historical and failed artifact paths remain immutable and are never reused.
- The staged pre-commit hook counts only when run against the actual staged files of each focused commit.

## Done

A6 is complete only when the repaired APIs and witnesses, direct owner-package evidence, source-current correctness, SIMD, complete-matrix, focused ledger, selected scalar, architecture, inventories, audits, local verification, and exact-revision CI all pass with no open finding. Then mark A6 complete and restore the named A7 decoder stashes.
