# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete; A9 remains pre-evidence.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): freeze the coordinated 0.2 product, produce source-current correctness and controlled performance evidence, publish the ten product crates from exact reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The active release matrix contains 19 promotable groups and 21 unique exact correctness parents. Formal A9 PR, full, soak, controlled timing, rollup, and completion evidence has not started.
- Historical package preflights remain diagnostic only. The latest useful probe is schema version 3 at `9b9150d8030991f88589cb5ecd417a54f80ad403`; later release repairs require a new unique schema-version-4 preflight.
- Commits through `9f035aefe765cb566e89f8596dd805eda4ede827` repair the reviewed publication boundary: isolated Cargo, bounded archives and subprocesses, exact canonical crates.io metadata, direct upload of the retained reviewed `.crate` bytes, checksum polling, machine-checked A9 authorization, descriptor-safe cleanup, pinned workflow actions, retained GitHub asset bytes, remote annotated-tag verification, private draft creation, remote size and SHA-256 verification, and publication-token-free release-operator builds in both workflow and local just paths.
- Explicit crates.io and GitHub publication-token variables are absent from Cargo, build scripts, inherited workflow and job environments, unrelated commands, arguments, reports, and logs. The required variable is bound only while executing the current already-built authenticated operator after local verification and A9 authorization. Reviewed pinned actions and their declared GitHub job permissions remain a separate trusted boundary.
- The latest formal completion is historical DEM-only evidence. There is no current A9 completion checkpoint, `v0.2.0` tag, crates.io publication, draft, or public GitHub release.
- `origin/main` still points to `c4299a23383043ee74daba80621d25072cdac5b9`; the local A9 repair series has no exact-revision GitHub CI. Older successful CI is not evidence for the local source.
- Swap is restored as `/swap.img`, size `137438949376` bytes, priority `-2`. Formal timing must preserve and restore that exact configuration.

## Next Actions

1. Synchronize the release and A9 documents, run `milestone-audit` and `full-code-review`, and fix every confirmed source or contract finding.
2. Run the complete non-timing local verification suite and one new diagnostic schema-version-4 `release::check` under a unique path from the final clean commit. Preserve but never promote or reuse failed and historical paths.
3. Push the exact final pre-evidence commit and require its own GitHub CI before formal correctness, timing, or publication work.
4. Run and replay PR, full, and soak correctness, the live result-format corpus, the implemented oracle matrix, worker reproducibility, and controlled AArch64 full and soak evidence for all 19 release groups.
5. Enforce `1.25x` Stim parity, seeded `1.15x` Stab self-regression, memory and scaling policy, and honest `unseeded` outcomes. Produce 38 rollups and the 138-report `a9-release` schema-version-3 completion, then replay it once.
6. Restore swap after every timing session, verify no qualification process remains, and preserve all failed or noisy artifacts.
7. Re-run both audits. Any confirmed executable, test, fixture, workflow, inventory, policy, or normative-contract fix invalidates affected evidence and must precede replacement runs.
8. If evidence remains valid, create the authenticated completion checkpoint and exactly one seven-path status descendant, push it, and require exact-revision CI.
9. From that clean descendant, create and inspect one final immutable schema-version-4 package preflight. Run `release::publish-reviewed`, create and push the annotated tag, dispatch `release::create-draft`, inspect the verified private draft, and publish it manually.

## Release Gates

- Do not relax the `1.25x` parity threshold, `1.15x` self-regression threshold, workload equivalence, memory limits, or controlled-host policy.
- Do not publish from a dirty tree, a stale preflight, a source outside the one-descendant completion contract, or a revision without exact-head CI.
- `release::publish-reviewed` and `release::create-draft` must both pass `qualification-status --check --require-release-completion` before reading a credential or making an irreversible request.
- Crates.io publication is irreversible and non-atomic. Upload only the canonical metadata and exact retained archive from the reviewed preflight, in source-owned order, and stop on any mismatch.
- GitHub creation is draft-only. Require the existing remote annotated tag to resolve to the reviewed commit; upload only retained assets; reject missing, extra, duplicate, wrong-size, or wrong-digest remote assets; never replace an existing release or asset.
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
