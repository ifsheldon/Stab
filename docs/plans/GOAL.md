# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete; A9 must restart after repairing first-failure retention and binding failure ownership for the late-hit bit scan.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): freeze the coordinated 0.2 product, produce source-current correctness and controlled performance evidence, publish the ten product crates from exact reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The active release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Commit `f740fd36442e3561b3cb6acf172a24030352fe80` contains the reviewed seventh DEM parser optimization. Local and remote `main` match it, and exact-head GitHub Actions run `30790983034` passed both `Rust` and `Qualification Contracts`.
- At `f740fd36`, PR correctness passed 700 of 700 selected parents, broad full and soak each passed 933 of 933, all 11 exact full prerequisites passed and replayed, the live 62-case result-format corpus and complete implemented oracle matrix passed, and worker reproducibility produced 46 canonical receipts.
- Formal `f740fd36` timing accepted and replayed 77 reports: all 72 reports for eight complete groups plus five reports for `PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE`. Sixteen full and soak rollups were produced and replayed for the eight complete groups.
- Every accepted report passed the unchanged `1.25x` Stim parity gate. Accepted paired medians ranged from `0.044611x` through `1.168380x`, the worst confidence upper bound was `1.191070x`, and the hottest accepted reading was 92700 millidegrees Celsius under the checked 100000-millidegree evidence ceiling.
- DEM parse and print passed their seeded `1.15x` Stab self-regression checks at all 18 identities. The six completed compact groups correctly reported `unseeded`; no self-regression pass is claimed for them.
- Operator-observed, non-replayable history records that the late-hit soak-large qualification command reached a failed or noisy timing outcome before publication refused the missing failure owner. The artifact absence and unused path are verifiable, but the unpublished outcome class and internal attempt sequence are not machine-replayable, and no ratio is claimed.
- The repair preserves every future first failure as a non-promotable `pending-source-owned-profiler-note` report, binds the observed late-hit note, and refreshes the DEM note without changing any workload, comparator, `1.25x` parity gate, `1.15x` self-regression gate, host policy, or retry rule. These source changes make every `f740fd36` artifact historical for A9 completion.
- The repair milestone audit and full code review found and closed the circular first-failure lifecycle plus three evidence-wording defects. Their final passes report no remaining implementation finding or specification loophole.
- Historical package preflights and all earlier correctness, worker, timing, memory, rollup, and completion artifacts remain immutable diagnostics only. No artifact may be rebound across a source revision or reused as a replacement path.
- The controlled thermal ceiling is 100000 millidegrees Celsius. It is an evidence-admissibility ceiling, not a hardware-safety claim; firmware trips, thermal throttling, affinity, load, memory, swap, and frequency-governor violations remain disqualifying.
- Swap is restored as `/swap.img`, size `137438949376` bytes, priority `-2`, and no qualification process remains.
- There is no current A9 completion checkpoint, `v0.2.0` tag, crates.io publication, draft, or public GitHub release.

## Next Actions

1. Finish the broad verification stack, commit the audited first-failure and profiler-note repair, push it, and require exact-head CI.
2. From that clean verified revision, rerun and replay PR, full, and soak correctness, all 11 exact prerequisites, the live result-format corpus, the implemented oracle matrix, and worker reproducibility.
3. Produce controlled AArch64 full and soak evidence for all 19 release groups from that one revision, using new artifact paths only and preserving every failed, noisy, or host-rejected outcome.
4. Enforce `1.25x` Stim parity, seeded `1.15x` Stab self-regression, honest `unseeded` outcomes, and memory and scaling policy. Produce 38 rollups and the 138-report `a9-release` schema-version-3 completion, then replay it once.
5. Restore swap after every timing session and verify that no qualification process remains.
6. Re-run milestone audit and full code review. Any confirmed executable, test, fixture, workflow, inventory, policy, or normative-contract fix invalidates affected evidence and must precede replacement runs.
7. If evidence remains valid, create the authenticated completion checkpoint and exactly one seven-path status descendant, push it, and require exact-revision CI.
8. From that clean descendant, create and inspect one final immutable schema-version-4 package preflight. Run `release::publish-reviewed`, create and push the annotated tag, dispatch `release::create-draft`, inspect the verified private draft, and publish it manually.

## Release Gates

- Do not relax the `1.25x` parity threshold, `1.15x` self-regression threshold, workload equivalence, memory limits, or controlled-host policy.
- Do not rerun a stable failure, replace a noisy attempt with a favorable sample outside the source-owned rule, reuse an artifact path, or combine evidence from different source revisions.
- Do not publish from a dirty tree, stale preflight, source outside the one-descendant completion contract, or revision without exact-head CI.
- `release::publish-reviewed` and `release::create-draft` must both pass `qualification-status --check --require-release-completion` before reading a credential or making an irreversible request.
- Keep ops and test-support crates unpublished. Deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and a public execution IR remain outside 0.2.0.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially A9
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Correctness contract](comprehensive-correctness-qualification-plan.md)
- [Performance contract](comprehensive-stim-performance-qualification-plan.md)
- [Release procedure](../RELEASING.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Done

A9 is complete only when source-current evidence and audits pass, all ten exact reviewed archives are visible with matching crates.io checksums, the annotated tag and verified GitHub assets bind the release commit, the draft is manually published, swap is restored, no qualification process remains, and the worktree is clean.
