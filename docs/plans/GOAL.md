# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete; A9 is the remaining architecture-plan milestone.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): freeze the coordinated 0.2 product, produce source-current correctness and controlled performance evidence, publish every product crate together, publish the `v0.2.0` GitHub release, and close the architecture migration without weakening compatibility or performance policy.

## Current State

- A8 measured source `c797ebc908ce1b81675e479031c39f71740058ae` passed exact-revision GitHub Actions through status descendant `c4299a23383043ee74daba80621d25072cdac5b9`, run `30732340918`.
- Ten product packages are already versioned `0.2.0`: `stab-kernels-simd`, `stab-bits`, `stab-model`, `stab-records`, `stab-algebra`, `stab-analysis`, `stab-decoder`, `stab-engine`, `stab-core`, and `stab-cli`.
- All ten packages assemble successfully with `cargo package --no-verify`; internal publishable dependencies require exact `=0.2.0` versions.
- `docs/MIGRATING-0.2.md`, the component graph, facade tiers, migration inventory, README, feature checklist, qualification inventories, and generated dashboard exist.
- The active performance matrix contains 19 promotable groups and remains below the 40-group release cap. Eight product diagnostics plus one infrastructure group remain below the 60-group diagnostic cap.
- The 19 release groups reference 21 unique exact correctness parents. Formal source-current PR, full, and soak evidence has not started.
- The latest formal completion is historical DEM-only evidence. There is no current A9 completion checkpoint, `v0.2.0` tag, crates.io publication, or GitHub release.
- No Cargo registry credential is currently configured on this host. Publication waits for all pre-publication evidence and audits, then requires the user to provide a crates.io token through `cargo login` or `CARGO_REGISTRY_TOKEN`; the token must never enter logs, files, arguments, or generated artifacts.

## Execution Sequence

1. Freeze release metadata and operations. Give every package reviewable crates.io metadata, add a thin `release::` just surface backed by a Rust ops binary, replace release-workflow packaging logic with that binary, define the exact dependency-ordered publication procedure, and document partial-publication recovery.
2. Decide the finite A9 evidence scope before timing. Keep the 19 release groups unless a demonstrated architecture risk justifies a source-owned addition; do not promote diagnostics merely to increase coverage. Add one revision-scoped completion manifest only if the existing completion contract cannot represent the complete release matrix.
3. Run milestone-audit and full-code-review over release metadata, package boundaries, CI/release workflow, correctness ownership, benchmark comparability, resource limits, and documentation. Fix every confirmed finding before evidence.
4. Commit the final pre-evidence source, regenerate correctness and performance inventories and generated status, and require a clean unchanged worktree for every formal artifact.
5. Run the exact 21 release-group correctness prerequisites at PR, full, and soak tiers and replay each report. Run the live result-format corpus and implemented oracle matrix.
6. Reproduce both performance workers, then produce controlled AArch64 full and soak evidence for every scale of all 19 release groups. Replay reports, enforce the unchanged `1.25x` Stim parity gate, evaluate seeded self-regression at `1.15x`, report unseeded identities honestly, run required memory/scaling checks, and publish architecture-scoped rollups and the A9 completion checkpoint.
7. Run the legacy primary timing and memory suites only as diagnostic continuity. Shared-host timing, unverified-host timing, and product diagnostics cannot replace controlled release evidence.
8. Restore the exact prior swap configuration after every controlled timing session, verify no qualification process remains, and preserve every failed or historical artifact path.
9. Re-run milestone-audit and full-code-review against the completed evidence. Synchronize README, checklist, migration guide, plans, generated status, reports, and release notes.
10. From the final clean reviewed commit, publish the ten crates in dependency order, create and push signed or annotated tag `v0.2.0`, publish the GitHub release, let the release workflow attach binaries and checksums, and verify crates.io packages, release assets, and exact source identity.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially Milestone A9
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Correctness qualification contract](comprehensive-correctness-qualification-plan.md)
- [Performance qualification contract](comprehensive-stim-performance-qualification-plan.md)
- [Benchmark instructions](../../benchmarks/AGENTS.md)
- [0.2 migration guide](../MIGRATING-0.2.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Nonnegotiable Gates

- Do not relax the `1.25x` Stim parity threshold, the `1.15x` self-regression threshold, memory limits, workload equivalence, or host policy to obtain a release.
- Do not publish from a dirty tree, a revision different from the reviewed evidence source, or a source whose package archives were not preflighted.
- Crates.io publication is irreversible and non-atomic. Complete all local and remote preflight before the first upload, publish only in the source-owned topological order, and stop on any identity or availability mismatch.
- Keep ops and test-support crates unpublished. Keep deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and a public execution IR outside 0.2.0.

## Done

The architecture plan is complete only when every A9 evidence and publication requirement is independently verified, `v0.2.0` is available from the intended registries and GitHub release, final audits are clean, swap is restored, no qualification process remains, and the worktree is clean.
