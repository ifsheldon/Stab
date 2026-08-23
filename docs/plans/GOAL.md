# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. The post-audit M10 benchmark repair and its replacement release rehearsal passed; this permitted rehearsal record and exact-head CI are the final gates before formal A9 evidence restarts.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): bind one final clean source, exercise the reversible release path, produce source-current correctness and controlled-performance evidence, authenticate the one permitted status descendant, publish all ten reviewed crates, and publish protected release `v0.2.0` without weakening compatibility, resource, or performance policy.

## Current State

- Replacement rehearsal source `4e71ccff0fb275e25f5ae546855003075d0a6608` passed run `32584757407`; private scratch draft `374979714` and its six digest-matched assets must never be published.
- Measured revision `515e4a0416ac2c83cd7b5d17eaf17ac6e3684697` passed exact-head CI, PR 729/729, broad full and soak 962/962, eleven exact prerequisites, both live oracle suites, 46 worker receipts, both DEM probes, 138 controlled reports, 38 rollups, two accepted-maximum memory receipts, and schema-version-4 completion under soft `RLIMIT_NOFILE=1024`.
- The `515e4a04` chain is historical after benchmark-source commits `e64f3a94` and `946a5d0c`; no completion checkpoint was published.
- `946a5d0c` corrects the M10 work mismatch by accumulating public numeric outputs like pinned Stim instead of black-boxing a full `Result` per call. Clean focused beta evidence at `target/benchmarks/a9-946a5d0c-r2-m10-error-decomp-clean-beta` passes all four pairs with worst ratio exactly `1.25x` and `local_modifications=false`.
- The legacy memory diagnostic preserves 14 old-limit failures and seven missing baselines. It is diagnostic continuity, not promotable A9 memory evidence; do not rewrite baselines or add closure waivers.
- The active thermal ceiling is exactly `100000` millidegrees Celsius. Historical A9 maximum was 85500. Swap is restored as `/swap.img` with priority `-2`, and no qualification process is running.
- Exact clean rehearsal source `4882f55bacd0c403cf6d5f94d6e8335b034c6f57` passed both jobs in GitHub Actions run `32623055871`. Protected annotated tag `v0.2.0-rehearsal-4882f55bacd0c403cf6d5f94d6e8335b034c6f57` produced exactly one workflow run, `32623915146`; both native AArch64 builds, private draft `375137275`, all six local and remote asset digests, and the live read-only verifier passed. The draft must never be published.
- There is no source-current formal evidence, authenticated checkpoint, final package preflight, crates.io publication, production tag, production draft, or public release after the benchmark repair.

## Next Actions

1. Commit and push only this permitted three-document rehearsal record, then require both exact-head CI jobs.
2. From that final rehearsal-record revision, regenerate PR, full, soak, all eleven exact prerequisites, both live oracle suites, 46 worker receipts, and both complete DEM adapter probes at unique paths.
3. Produce all 138 controlled AArch64 reports, two accepted-maximum DEM memory receipts, 38 rollups, and one schema-version-4 `a9-release` completion. Disable swap immediately before each formal timing command and restore the exact prior configuration through failure-safe traps.
4. Rerun legacy primary timing and memory diagnostics. Preserve failures; investigate comparable timing regressions, but do not invent ratios for non-comparable rows or rewrite memory baselines merely to close A9.
5. Run milestone-audit and full-code-review against the completed evidence. Fix confirmed implementation or contract defects; log genuine under-specification. Any source fix requires fresh rehearsal and evidence paths.
6. If review passes, replay and publish the completion checkpoint under soft `RLIMIT_NOFILE=1024`, create exactly one seven-path status descendant, push it, and require exact-head CI.
7. Follow [RELEASING.md](../RELEASING.md): create one immutable package preflight, publish the reviewed ten-crate set, create and push protected annotated `v0.2.0`, capture exactly one production workflow run, verify the six-asset private draft immediately before human publication, and verify the public release immediately afterward.

## Gates

- Keep Stim parity at `1.25x`, Stab self-regression at `1.15x`, workload equivalence, memory limits, and controlled-host policy unchanged.
- Formal completion creation, offline replay, and checkpoint replay must each pass with soft `RLIMIT_NOFILE=1024`.
- Never reuse a failed, noisy, controller-rejected, host-rejected, or incorrectly dispatched artifact path. Preserve the complete attempt ledger.
- Treat round trips, dirty reports, shared-host timing, legacy diagnostic passes, and operator observations as supporting evidence only.
- Production mutators must pass `qualification-status --check --require-release-completion` before reading credentials. Run irreversible local release commands only from the documented isolated user session.
- Keep ops and test-support crates unpublished. Deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and public execution IR remain outside `0.2.0`.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially A9
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Correctness contract](comprehensive-correctness-qualification-plan.md)
- [Performance contract](comprehensive-stim-performance-qualification-plan.md)
- [Release procedure](../RELEASING.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Done

A9 is complete only when the replacement rehearsal, source-current correctness and performance chain, final audits, authenticated status descendant, all ten exact reviewed crate uploads, protected production tag, six verified release assets, and public GitHub release pass; swap is restored; no qualification process remains; and the worktree is clean.
