# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. A9 remains open, and the fixed WS4 scratch-repository rehearsal is the next gate before replacement formal evidence.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): rehearse the reversible release path, produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten reviewed product crates, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Clean pushed revision `189fe3fe0c8be10cfa5f579e711a000cacf823fb` passed exact-revision CI and complete source-current correctness: PR 729 of 729, broad full and soak 962 of 962 each, all 11 exact prerequisites, live result-format and implemented oracle suites, worker reproducibility, and both DEM adapter probes.
- Formal timing on `189fe3fe` accepted circuit print, circuit parse, gate lookup, SIMD bits, all three SIMD not-zero groups, SIMD word, and sparse-XOR small/full. Sparse-XOR medium/full was host-rejected after unrelated host activity lowered available memory below the checked four-GiB minimum. Every attempted path is retained or retired as recorded, and swap was restored after each attempt.
- The partial `189fe3fe` evidence becomes historical when the release-rehearsal repair is committed. It must not be mixed with replacement evidence.
- A pre-evidence audit found that the old WS4 criterion was not executable: the only operator was fixed to production and required A9 completion, while “every command” included irreversible crates.io and public-release steps. The repair adds a separate non-production binary and frozen workflow fixed to public scratch repository `ifsheldon/Stab-release-rehearsal` (ID `1342241032`), active no-bypass ruleset `21169813`, and commit-derived annotated tags. The rehearsal binary has no registry-publication or public-release command; the production operator remains fixed and unchanged in destination.
- The controlled thermal ceiling is exactly `100000` millidegrees Celsius for both profiles. Firmware throttling and critical trips remain authoritative, and every other host-policy check remains unchanged.
- Swap configuration is `/swap.img`, size `137438949376` bytes, priority `-2`. Formal timing disables swap immediately before each report and restores this exact configuration afterward, including failure paths.
- There is no source-current complete performance chain, completion checkpoint, final package preflight, crates.io publication, production tag, draft, or public GitHub release.

## Next Actions

1. Finish the rehearsal operator, workflow, contract tests, and synchronized documentation. Run milestone-audit, full-code-review, broad verification, and staged pre-commit; create focused commits, push `main`, and require exact-revision `Rust` and `Qualification Contracts` CI.
2. Mirror that exact commit to the fixed scratch repository without rewriting history, create and push `v0.2.0-rehearsal-<full-commit>` as an annotated tag, dispatch only `release-rehearsal.yml` from the tag, and require exactly one successful workflow run for the exact SHA.
3. Download the two workflow artifacts into a unique path, verify the exact six assets and live private draft through the rehearsal-only verifier, and record the source revision, tag, run, repository/ruleset identities, and six digests. Never publish the rehearsal draft.
4. If rehearsal reveals a source defect, fix it and repeat actions 1 through 3 before formal evidence. A documentation-only rehearsal record may follow the exercised source revision, but no product, workflow, release-tool, qualification-contract, fixture, or policy change may ride that record; any such change invalidates the rehearsal and restarts it.
5. From the final clean revision, regenerate PR, full, soak, all 11 exact prerequisites, live oracle suites, 46 worker receipts, and both DEM adapter probes at unique paths.
6. Produce all 138 controlled AArch64 reports, two accepted-maximum DEM memory receipts, 38 rollups, and one schema-version-4 `a9-release` completion. Creation and replay run through `prlimit --nofile=1024:1024 --`; every failed, noisy, host-rejected, or resource-rejected path remains visible and is never reused.
7. Run both audits again. If no source fix is required, publish the authenticated completion checkpoint and exactly one permitted status descendant, push it, and require exact-revision CI.
8. Follow [RELEASING.md](../RELEASING.md): produce one immutable final package preflight, publish the reviewed crates, create and push protected annotated `v0.2.0`, capture the exact workflow run, verify the private draft immediately before manual publication, and verify the public release immediately afterward.

## Gates

- Keep Stim parity at `1.25x`, Stab self-regression at `1.15x`, all workload-equivalence checks, memory limits, and the controlled-host policy unchanged.
- Formal completion and replay must pass with soft `RLIMIT_NOFILE=1024`; a raised limit is diagnostic only.
- The production mutators must pass `qualification-status --check --require-release-completion` before reading credentials. The rehearsal mutator instead passes architecture and ordinary checked-status validation and is mechanically unable to publish crates or a public release.
- Run irreversible local release commands in an isolated user session. Repository controls prevent accidental credential propagation and path reuse, not inspection by a malicious same-UID host process.
- Keep ops and test-support crates unpublished. Deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and a public execution IR remain outside `0.2.0`.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially A9
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Correctness contract](comprehensive-correctness-qualification-plan.md)
- [Performance contract](comprehensive-stim-performance-qualification-plan.md)
- [Release procedure](../RELEASING.md)
- [Specification-gap log](milestone-spec-gaps.md)

## Done

A9 is complete only when the fixed scratch rehearsal, source-current evidence, and audits pass; all ten exact reviewed archives are visible with matching crates.io checksums; the protected annotated production tag and verified GitHub assets bind the release commit; the draft is manually published and reverified; swap is restored; no qualification process remains; and the worktree is clean.
