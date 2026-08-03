# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete; A9 evidence must restart after the sixth DEM parser optimization.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): freeze the coordinated 0.2 product, produce source-current correctness and controlled performance evidence, publish the ten product crates from exact reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The active release matrix contains 19 promotable groups and 21 unique exact correctness parents.
- Historical package preflights remain diagnostic only. The latest useful probe is schema version 4 at clean revision `544e39653ffea370fc3a872b3d3f7e61bb5aa4cd`, stored at `target/releases/v0.2.0-544e3965-preflight` with report SHA-256 `3605104a5d3d0f757379035f1910e5859b9ce303bf4423b3ded33a1cee4936c0`.
- Commits through `212331732c7ebd0bbf55534c1d1fbd2935a1e84c` repair the reviewed publication boundary: isolated Cargo, bounded archives and subprocesses, exact canonical crates.io metadata, direct upload of the retained reviewed `.crate` bytes, checksum polling, machine-checked A9 authorization, descriptor-safe cleanup, pinned workflow actions, retained GitHub asset bytes, remote annotated-tag verification, private draft creation, remote size and SHA-256 verification, and publication-token-free release-operator builds in both workflow and local just paths.
- Explicit crates.io and GitHub publication-token variables are absent from Cargo, build scripts, inherited workflow and job environments, unrelated commands, arguments, reports, and logs. The required variable is bound only while executing the current already-built authenticated operator after local verification and A9 authorization. Reviewed pinned actions and their declared GitHub job permissions remain a separate trusted boundary.
- The pre-evidence milestone audit and full-code-review are complete. They found and repaired credential exposure through workflow and local `cargo run` dispatch; no further confirmed implementation defect, evidence loophole, or milestone under-specification remains before the diagnostic package preflight.
- Complete local non-timing verification passes at `212331732c7ebd0bbf55534c1d1fbd2935a1e84c`: Stable component tests, portable-SIMD tests and Clippy, formatting, workspace Clippy and tests, architecture and consumer checks, API and documentation checks, the live 62-case result-format corpus, the implemented oracle matrix, correctness and performance checks and deterministic regeneration, generated status, benchmark smoke, and pre-commit.
- Exact-revision GitHub run `30766263266` passed both `Rust` and `Qualification Contracts` for fifth-optimization commit `5417fd661a67b5bc181795dde7631218070cf494`.
- PR correctness passed 700 of 700 selected parents, broad full and soak each passed 933 of 933, all 11 exact full prerequisites passed and replayed, the live corpus and implemented oracle matrix passed, and worker reproducibility produced 46 canonical receipts at `5417fd66`.
- The controlled thermal ceiling is now 100000 millidegrees Celsius for both checked profiles. Every exposed thermal zone on the current AArch64 host reports a 104800 millidegree Celsius firmware critical trip. The 100 C value is an evidence-admissibility ceiling, not a hardware-safety claim; thermal throttling and every other host-policy check remain disqualifying.
- The first DEM parse report at `5417fd66` failed stably at median `1.307677x` with upper confidence bound `1.353705x`; it was replayed, preserved, and not rerun. The earlier `92b629fa` failure at median `1.524360x` also remains historical.
- The sixth optimization adds a checked canonical valid-instruction path and byte-oriented tagged-line scanning without changing the rich fallback grammar or diagnostics. Dirty-tree paired guidance is `1.051769x` for flat-errors, `1.054973x` for coordinate-sparse, and `0.969537x` for the failed small-scale shape.
- This source change makes every `5417fd66` correctness, worker, and performance artifact historical for current A9 completion. No old report may be combined with replacement evidence.
- The latest formal completion is historical DEM-only evidence. There is no current A9 completion checkpoint, `v0.2.0` tag, crates.io publication, draft, or public GitHub release.
- Local and remote `main` match `5417fd661a67b5bc181795dde7631218070cf494` before the sixth optimization. The optimization requires regenerated contracts, a focused commit, push, and exact-head CI.
- Swap is restored as `/swap.img`, size `137438949376` bytes, priority `-2`. Formal timing must preserve and restore that exact configuration.

## Next Actions

1. Regenerate correctness, performance, and generated-status contracts for the sixth DEM parser optimization; run targeted and standard checks; commit and push the focused fix.
2. Re-fetch `origin/main` and require both `Rust` and `Qualification Contracts` to pass for the exact repair revision.
3. From that clean verified revision, rerun and replay PR, full, and soak correctness, every exact prerequisite, the live result-format corpus, the implemented oracle matrix, and worker reproducibility.
4. Produce controlled AArch64 full and soak evidence for all 19 release groups from that one revision, using new artifact paths only.
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
