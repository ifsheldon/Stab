# Goal: Repair And Close A6 Before Decoder Extraction

## Objective

Finish milestone A6 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) from one clean reviewed revision before admitting the parked `stab-decoder` A7 work.

## Current State

- A0 through A5 are complete, and the physical component split is committed.
- The `3a93719b` correctness, SIMD, matrix, and focused artifacts are preserved but review-rejected and cannot close A6.
- Product/API repair is committed at `59b0f8cf`, independent benchmark witnesses at `0dbc685c`, focused-evidence validation at `66eca557`, direct six-component qualification ownership at `564da37b`, direct component semantic owners at `6dedcfc5`, complete output witnessing at `a53de656`, and immutable source-owned A6 publication at `ba480af8`.
- The source-current correctness inventory has 2,117 evidence parents and digest `589250ea4b300f0768aa5085a6aa8c6584e1c19afbc65e7d4414da390a714012`; the performance inventory has 179 groups and digest `1f709237b11c10d6759e03846568b8a1e13c4a8841ea6f53691ca7b29233f302`.
- Fresh replacement evidence has not started. A6 is waiting on pre-evidence audits, any resulting source repairs, and one clean-revision evidence run.
- `stab-decoder` source and benchmark work remains preserved in named stashes and must stay parked until A6 closes.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), milestone A6
- [Component graph](../architecture/README.md)
- [A6 extraction map](../architecture/a6-component-extraction-map.md)
- [Progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owner when Cargo metadata, architecture checks, generated inventories, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Run milestone-audit and full-code-review against the committed repaired source and qualification contracts before timing.
2. Fix every confirmed source, test, benchmark-contract, and documentation finding in focused commits; regenerate inventories after any identity change.
3. Before current timing, regenerate each comparable historical predecessor from a clean committed schema-version-3 instrumentation backport on the same host. Do not promote schema-version-1 or schema-version-2 reports.
4. From the resulting clean current commit, regenerate exact full-tier M5 and M6 correctness, scalar-versus-portable SIMD diagnostics, a create-new schema-version-2 baseline, and the complete create-new schema-version-3 warmed comparison with unchanged comparator and threshold policies.
5. Recompute every greater-than-15-percent crossing. Run one warmed outer diagnostic per crossing with the source-owned internal timing count of at least eight. Profile reproductions when host policy permits and record restrictions otherwise.
6. Publish `benchmarks/a6-focused-evidence.json` from a selection-only request. Require exact checked measurement and witness ownership, immutable report and predecessor digests, typed measured outcomes, mandatory artifact revalidation, and atomic no-replace publication.
7. Append the source-current evidence checkpoint without rewriting historical rejected artifacts.
8. Run the complete local verification matrix, repeat milestone-audit and full-code-review, and obtain green GitHub CI for the exact closure revision.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and scalar engine code.
- Only `stab-kernels-simd` contains direct portable-SIMD code, and it has no Stab dependency.
- `stab-core` remains the compatibility facade; canonical implementation and qualification ownership stays with component crates.
- No comparator class, `1.25x` threshold, waiver, semantic witness, or feature-selection policy is relaxed to obtain closure.
- Host-unverified timing is diagnostic only. A9 owns promotable controlled-host parity.
- Historical and failed artifact paths remain immutable and are never reused.
- Baseline, matrix, predecessor, focused, profile, and ledger identities are derived and checked; status prose cannot override measured outcomes.
- The staged pre-commit hook counts only when run against the actual staged files of each focused commit.

## Done

A6 is complete only when the repaired APIs and witnesses, direct owner-package evidence, source-current correctness, SIMD, complete-matrix, focused ledger, selected scalar, architecture, inventories, audits, local verification, and exact-revision CI all pass with no open finding. Then mark A6 complete and restore the named A7 decoder stashes.
