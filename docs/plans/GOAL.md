# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. The bounded completion repair is committed; A9 awaits exact-head CI before replacement evidence starts.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten product crates from reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Clean revision `a963a7b134efdf0d70c3dd811e3243037abf0d0d` matched `origin/main` and passed exact-head GitHub Actions run `30836851178`.
- At `a963a7b1`, PR correctness passed 700 of 700 selected parents, broad full and soak each passed 933 of 933, all 11 exact full prerequisites passed and replayed, the live 62-case result-format corpus and implemented oracle matrix passed, and worker reproducibility produced 46 canonical receipts.
- All 138 controlled full and soak reports passed the unchanged `1.25x` Stim parity gate and produced 38 replayed rollups. Paired medians ranged from `0.000248x` through `1.142711x`; the worst confidence upper bound was `1.220137x`; the hottest accepted reading was 92300 millidegrees Celsius under the checked 100000-millidegree ceiling.
- DEM parse and print passed the seeded `1.15x` Stab self-regression checks at all 18 identities. The 17 compact groups correctly reported `unseeded`; no self-regression pass is claimed for them.
- Both immutable accepted-maximum DEM memory receipts were published. Swap is restored as `/swap.img`, size `137438949376` bytes, zero used bytes, priority `-2`, and no qualification process remains.
- The first `a9-release` completion attempt failed before publication under the ordinary soft `RLIMIT_NOFILE=1024`. Completion retained repeated descriptor-safe correctness trees for 138 reports until it exhausted file descriptors and misreported `EMFILE` as artifact mutation. The reported Pauli receipt still matched its checked SHA-256, inode, size, and timestamps.
- A fresh-path diagnostic with soft limit 8192 created and replayed the complete manifest, proving that the 38 rollups, 138 reports, parity, memory, and regression identities are otherwise coherent. It is diagnostic only and is not release authorization.
- Commit `08d0dfe46398f5447d74120fe79cfb9e3569bf5f` retains one correctness binding per exact prerequisite artifact and reports descriptor exhaustion truthfully. The patched operator also created and replayed the historical 38-rollup manifest in a detached clean worktree with soft `RLIMIT_NOFILE=1024`, directly reproducing the repaired resource boundary. Every `a963a7b1` artifact and this dirty-binary diagnostic remain immutable historical evidence.
- The post-repair milestone audit and full code review found no remaining P0 through P3 issue. Formatting, warnings-denied workspace Clippy, all workspace tests, architecture and documentation checks, performance qualification checking and deterministic regeneration, generated-status checking, benchmark smoke, and staged pre-commit passed.
- There is no current A9 completion checkpoint, final package preflight, crates.io publication, `v0.2.0` tag, draft, or public GitHub release.

## Next Actions

1. Push the focused repair and documentation commits and require exact-head `Rust` and `Qualification Contracts` CI.
2. From that clean verified revision, rerun and replay PR, full, and soak correctness, all 11 exact prerequisites, the live result-format corpus, the implemented oracle matrix, and worker reproducibility.
3. Produce new-path controlled AArch64 full and soak evidence for all 19 release groups, both accepted-maximum DEM memory receipts, 38 rollups, and one 138-report `a9-release` completion under the ordinary 1024 soft descriptor limit. Replay it once.
4. Preserve every failed, noisy, host-rejected, or resource-rejected outcome. Do not reuse an artifact path, relax a threshold, add a waiver, or rerun a stable product failure.
5. Restore swap after every timing session and verify that no qualification process remains.
6. Re-run milestone audit and full code review. Any confirmed executable, test, fixture, workflow, inventory, policy, or normative-contract fix invalidates affected evidence and must precede replacement runs.
7. If evidence remains valid, create the authenticated completion checkpoint and exactly one seven-path status descendant, push it, and require exact-revision CI.
8. Create and inspect one final immutable schema-version-4 package preflight, publish the reviewed crates, create and push the annotated tag, create and verify the private GitHub draft, then publish the release manually.

## Release Gates

- Keep the `1.25x` Stim parity threshold, `1.15x` Stab self-regression threshold, workload equivalence, memory limits, and controlled-host policy unchanged.
- Formal completion and replay must pass with soft `RLIMIT_NOFILE=1024`; raising the limit is diagnostic, not the product fix.
- `release::publish-reviewed` and `release::create-draft` must pass `qualification-status --check --require-release-completion` before reading a credential or making an irreversible request.
- Keep ops and test-support crates unpublished. Deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and a public execution IR remain outside `0.2.0`.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially A9
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Correctness contract](comprehensive-correctness-qualification-plan.md)
- [Performance contract](comprehensive-stim-performance-qualification-plan.md)
- [Release procedure](../RELEASING.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Done

A9 is complete only when source-current evidence and audits pass, all ten exact reviewed archives are visible with matching crates.io checksums, the annotated tag and verified GitHub assets bind the release commit, the draft is manually published, swap is restored, no qualification process remains, and the worktree is clean.
