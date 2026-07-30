# Goal: Close A6 Before Decoder Extraction

## Objective

Finish milestone A6 of [the agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) from one clean reviewed revision before restoring the parked A7 decoder work.

## Current State

- A0 through A5 are complete, and the physical workspace split is committed: `stab-bits`, `stab-records`, `stab-algebra`, `stab-model`, `stab-analysis`, `stab-engine`, `stab-kernels-simd`, the `stab-core` facade, and `stab-cli`.
- Historical `3a93719b` correctness, SIMD, matrix, and focused artifacts remain review-rejected diagnostics.
- The checked A6 measurement contract owns exact executable preflights for 65 policy-gated rows plus the selected equal-width M6 row. Other report-only rows prove workload continuity only.
- The replacement publication contract uses a checked predecessor registry, typed Linux-perf receipts, and append-only content-addressed evidence objects. No source-current object has been published.
- `benchmarks/a6-predecessors.json` is intentionally empty while predecessor backports are pending, so publication must fail closed.
- A7 changes remain parked in the named `a7-decoder-benchmark-wip` and `a7-decoder-interoperability-wip` stashes until A6 closes.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), milestone A6
- [A6 extraction map](../architecture/a6-component-extraction-map.md)
- [Architecture graph](../architecture/README.md)
- [Append-only progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owning source when Cargo metadata, architecture checks, generated inventories, tests, benchmarks, or these documents disagree.

## Execution Sequence

1. Commit the predecessor-registry, typed-profile, append-only-publication, tests, and documentation changes in focused commits.
2. Run milestone-audit and full-code-review before timing. Fix every confirmed product, test, benchmark-contract, and documentation finding, then regenerate affected inventories.
3. For every non-seed report-only phase, identify the reviewed historical product commit and create one clean schema-version-4 instrumentation backport as its direct child. The tree delta may contain only the evidence harness needed to execute the unchanged historical workload.
4. Tag every backport at `a6-predecessors/<backport-commit>`, record its product commit, backport commit, raw-tree-delta SHA-256, and exact phases in `benchmarks/a6-predecessors.json`, and commit the complete registry before current evidence.
5. On the same host, produce one warmed strict create-new predecessor report per registered backport with one outer run. Never reuse a failed path.
6. From the resulting clean current revision, regenerate full-tier M5 and M6 correctness, the scalar-versus-portable SIMD diagnostic, one schema-version-3 baseline, and the complete schema-version-4 warmed three-run comparison under unchanged `1.25x` policies.
7. Run one warmed focused report for every greater-than-`1.15x` predecessor crossing. For a reproduced crossing, create a typed receipt with `just bench::a6-profile-receipt`; do not lower kernel policy to manufacture profiler access.
8. Publish request-schema-version-2 evidence with `just bench::a6-focused-evidence --publish-from <request>`. The request selects reports, optional profile receipts, and owner actions only.
9. Review and commit the resulting `benchmarks/a6-focused-evidence-<source-revision>-<sha256>.json` object. Validation discovers only tracked objects and accepts only that object plus `GOAL.md` and the progress report after its source revision.
10. Append the source-current checkpoint, rerun local verification, milestone-audit, full-code-review, and exact-revision GitHub CI. Fixing compiled source, policy, inventory, fixture, or workflow code invalidates the evidence and restarts steps 3 through 10.

## Nonnegotiable Contracts

- Stable 1.97.1 owns model, bits, records, scalar algebra, pure analysis, and scalar engine code.
- Only `stab-kernels-simd` contains direct portable-SIMD code, and it has no Stab dependency.
- `stab-core` remains the compatibility facade; canonical implementation and qualification ownership remain in component crates.
- No comparator, `1.25x` threshold, waiver, preflight, feature choice, or timing boundary is relaxed to obtain closure.
- Predecessor phase ownership comes only from the checked registry; report requests cannot assign phases.
- Profile availability comes only from a typed receipt; prose cannot claim capture or unavailability.
- Evidence objects are append-only and content-addressed. The fixed legacy path is never a current publication target.
- Host-unverified timing is diagnostic only. A9 owns promotable controlled-host parity.
- Historical, failed, and rejected artifacts remain immutable and are never reused.

## Done

A6 is complete only when the component graph, direct owner-package evidence, source-current correctness, SIMD selection, complete matrix, predecessor provenance, focused evidence object, selected scalar checks, inventories, audits, local verification, and exact-revision CI all pass with no open finding. Then mark A6 complete and restore the two named A7 stashes.
