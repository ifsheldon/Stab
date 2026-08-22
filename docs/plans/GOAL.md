# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. The first reversible release rehearsal passed, but A9 formal evidence is blocked until the DEM parser regression fix and its replacement rehearsal pass.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): repair the folded-repeat DEM parse regression, produce one source-current correctness and controlled-performance chain, authenticate release completion, publish the ten reviewed crates, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Historical rehearsal source `a57910ee00f53cb59253b91df31176cc9ec371b6`, its protected tag, native AArch64 assets, private scratch draft, and live verification passed. Scratch draft `374797341` must never be published. The product-source fix requires a new source-derived rehearsal before replacement evidence.
- Clean revision `e74458691eeec46807cfdbf17e0ee940cbd02095` passed exact-head CI, PR/full/soak correctness, all eleven exact prerequisites, both live oracle suites, 46 worker receipts, both DEM probes, all 102 non-DEM reports, 34 non-DEM rollups, and all 18 DEM parse parity reports.
- The `e7445869` chain is historical because seeded self-regression rejected `folded-repeats-small/parse`: median deterioration `1.220460x`, upper-bound deterioration `1.254744x`, limit `1.15x`. It was not rerun, waived, or hidden. DEM print did not start.
- The source-current candidate replaces the nested `ArrayVec`-then-copy builder with one retained `Vec` that reserves two slots on first use. Empty bodies remain allocation-free; one- and two-item bodies avoid completion copying.
- Focused model tests and warnings-denied Clippy pass. Seven alternating exact-shape pairs per scale preserve output identity and show candidate-to-committed medians of `0.843705x`, `0.820535x`, and `0.860373x` across folded small, medium, and large. These diagnostics are not formal evidence.
- The active thermal ceiling is exactly `100000` millidegrees Celsius. Firmware throttling and critical trips remain authoritative. Swap configuration is `/swap.img`, size `137438949376` bytes, priority `-2`, and no qualification process is running.
- There is no source-current A9 evidence chain, authenticated completion, final package preflight, crates.io publication, production tag, draft, or public release.

## Next Actions

1. Bind the parser fix, regression coverage, profiler note, progress checkpoint, and regenerated qualification identities without changing either performance threshold.
2. Run milestone-audit, full-code-review, broad local verification, focused commits, push, and exact-head `Rust` and `Qualification Contracts` CI.
3. From that exact clean revision, create a new protected source-derived rehearsal tag, dispatch the exact-SHA scratch workflow, verify all six assets and the new private draft, and record the immutable identities in the one permitted documentation-only descendant. Push that record and require exact-head CI.
4. From the final rehearsal-record revision, regenerate PR, full, soak, all eleven exact prerequisites, both live oracle suites, 46 worker receipts, and both DEM adapter probes at unique paths.
5. Produce all 138 controlled AArch64 reports, two accepted-maximum DEM memory receipts, 38 rollups, and one schema-version-4 `a9-release` completion. Disable swap immediately before each formal report and restore the exact prior configuration afterward, including failures.
6. Run milestone-audit and full-code-review against the completed evidence. Any confirmed source or contract defect invalidates affected evidence and requires fresh paths from a replacement clean revision.
7. If no source fix is required, publish the authenticated completion checkpoint and its one permitted status descendant, push it, and require exact-head CI.
8. Follow [RELEASING.md](../RELEASING.md): produce one immutable package preflight, publish the reviewed crates, create and push protected annotated `v0.2.0`, verify the private draft immediately before manual publication, and verify the public release immediately afterward.

## Gates

- Keep Stim parity at `1.25x`, Stab self-regression at `1.15x`, workload equivalence, memory limits, and controlled-host policy unchanged.
- Formal completion and replay must pass with soft `RLIMIT_NOFILE=1024`; a raised limit is diagnostic only.
- Never reuse a failed, noisy, controller-rejected, or host-rejected artifact path. Preserve historical evidence under its exact source contract.
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

A9 is complete only when the fixed rehearsal, source-current evidence, final audits, all ten exact reviewed archives, protected production tag, verified release assets, and public GitHub release pass; swap is restored; no qualification process remains; and the worktree is clean.
