# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. A9 is pre-evidence while the authenticated completion-closure and release-boundary repairs await focused commits, push, exact-revision CI, and one clean audit pass.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten product crates from reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Revision `d424175c6620549086fb4ee358077632bdab97d9` passed exact-revision CI and produced a complete 138-report AArch64 chain. All reports passed Stim parity, with 122 median speedups and 16 median slowdowns, but the chain is historical and review-rejected under the repaired contracts.
- The rejected completion replay reopened validated report paths, did not recompute final Git cleanliness, and was followed by confirmed release argument, revision-binding, toolchain-provenance, and late remote-tag defects. None of its artifacts authorizes release.
- Focused repairs now retain replay and correctness inputs through checked-status publication, authenticate the exact soft `RLIMIT_NOFILE=1024` boundary, bind verified-host schema-version-2 DEM memory receipts into schema-version-4 completion, require the current 11 shared correctness artifacts, and reject mixed-time final Git state. Release repairs validate toolchains across targets, recheck the remote tag after upload, bind workflow dispatch to the exact tag commit, quote typed `just` arguments, reject yanked registry recovery, and build each local operator in a fresh owner-only target.
- The controlled host ceiling is 100000 millidegrees Celsius. Firmware throttling and critical trips remain authoritative, and every other host-policy check remains unchanged.
- Swap configuration is restored as `/swap.img`, size `137438949376` bytes, priority `-2`. Used bytes may change during ordinary host activity; formal timing must disable swap immediately before measurement and restore this exact configuration afterward.
- There is no source-current correctness chain, performance chain, completion checkpoint, final package preflight, crates.io publication, `v0.2.0` tag, draft, or public GitHub release.

## Next Actions

1. Commit the authenticated completion code/tests and synchronized contracts as focused changes.
2. Run `milestone-audit` and `full-code-review`; fix every confirmed implementation, test, workflow, benchmark, or documentation finding before evidence.
3. Run the complete non-timing verification from a clean committed revision, push it, and require exact-revision `Rust` and `Qualification Contracts` CI.
4. Regenerate and replay PR, full, and soak correctness, all 11 exact prerequisites, the live result-format corpus, the implemented oracle matrix, and 46 worker reproducibility receipts at unique paths.
5. Produce fresh controlled AArch64 full and soak evidence for all 19 release groups, both verified-host accepted-maximum DEM memory receipts, 38 rollups, and one 138-report schema-version-4 `a9-release` completion; run creation and replay through `prlimit --nofile=1024:1024 --`.
6. Preserve every failed, noisy, host-rejected, or resource-rejected outcome. Never reuse an artifact path, relax a threshold, add a waiver, or rerun a stable product failure.
7. Re-run both audits. If no source fix is required, create the authenticated completion checkpoint and exactly one seven-path status descendant, push it, and require exact-revision CI.
8. Create and inspect one final immutable schema-version-4 package preflight, publish the reviewed crates, create and push the annotated tag, dispatch the release workflow from that exact tag ref, verify the private draft, and publish it manually.

## Release Gates

- Keep the `1.25x` Stim parity threshold, `1.15x` Stab self-regression threshold, workload equivalence, memory limits, and controlled-host policy unchanged.
- Formal completion and replay must pass with soft `RLIMIT_NOFILE=1024`; raising the limit is diagnostic only.
- `release::publish-reviewed` and `release::create-draft` must pass `qualification-status --check --require-release-completion` before reading a credential or making an irreversible request.
- Run irreversible local release commands in an isolated user session. Repository controls prevent accidental shared-path reuse and shell interpolation, but they do not claim to protect environment credentials from a malicious same-UID process with host-level process access.
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
