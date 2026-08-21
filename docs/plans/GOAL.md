# Goal: Qualify And Release Stab 0.2.0

Status: Active. The August 2026 [post-review remediation plan](post-review-remediation-plan.md) closed Pass 1 (Gate 0, Batches A through C) and Pass 2 (Batch D), and its Batch E produced fresh full-tier correctness evidence and restored this document in the same change set; its backlog items remain deferred behind their promotion triggers. Milestones A0 through A8 remain complete, and the A9 sequence below is the active execution path.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten product crates from reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Revision `d424175c6620549086fb4ee358077632bdab97d9` passed exact-revision CI and produced a complete 138-report AArch64 chain. All reports passed Stim parity, with 122 median speedups and 16 median slowdowns, but the chain is historical and review-rejected under the repaired contracts.
- The rejected completion replay reopened validated report paths, did not recompute final Git cleanliness, and was followed by confirmed release argument, revision-binding, toolchain-provenance, and late remote-tag defects. None of its artifacts authorizes release.
- Focused repairs now retain replay and correctness inputs through checked-status publication, authenticate the exact soft `RLIMIT_NOFILE=1024` boundary, bind verified-host schema-version-3 DEM memory receipts from the formal sealed private worker into schema-version-4 completion, require the current 11 shared correctness artifacts, and preserve typed Git failures through publication. Release repairs freeze the complete workflow execution context, validate toolchains across targets, require protected ruleset `20419793`, capture the exact dispatch run, verify the private draft immediately before publication, verify the public release immediately afterward, quote typed `just` arguments, reject yanked registry recovery, and build each local operator in a fresh owner-only target.
- The earlier final reviews found no P0 or P1 issue, but the August 2026 full code review superseded that conclusion with eleven confirmed P1 findings; the [post-review remediation plan](post-review-remediation-plan.md) has since closed every Pass 1 and Pass 2 workstream with independent WS1, WS2, and WS3 milestone audits, retired the forward analyzer so the vendor-diffed sparse reverse tracker is the only sensitivity-propagation engine (all sixty-three consolidation-matrix entries byte-match pinned Stim in both fold modes, including the fused-`fmadd` probability-merge contraction of the frozen baseline), consolidated duplicate invariant owners across the workspace, and restored every reopened checklist row with named fresh evidence.
- Exact revision `a00705469ac3017060a48c192fedca915cdb36bb` passed GitHub Actions run `32377831292`, source-current PR correctness (729 of 729), broad full and soak correctness (962 of 962 each), all eleven exact full prerequisites, the live result-format and implemented oracle suites, and 46 worker reproducibility receipts.
- All seventeen non-DEM release groups passed full and soak Stim parity and produced replayed rollups. DEM parse passed all eighteen family, scale, and tier reports with worst confidence upper bound `1.187242x`, but folded-repeats-large failed Stab self-regression at `1.159361x` upper-bound deterioration against the unchanged `1.15x` limit. The stable report was not rerun, DEM print did not start, and every `a0070546` artifact is historical after the parser repair.
- The replacement parser stores up to two nested-body items before allocating exact persistent storage. Focused diagnostics reduce folded-large parse time from a 59.120 millisecond median to 37.335 milliseconds and requested allocation bytes from 8,142,488 to 5,029,528 without increasing allocation count. These dirty-tree diagnostics guide the fix but are not formal evidence.
- The controlled host ceiling is 100000 millidegrees Celsius. Firmware throttling and critical trips remain authoritative, and every other host-policy check remains unchanged.
- Swap configuration is `/swap.img`, size `137438949376` bytes, priority `-2`. Used bytes may change during ordinary host activity; formal timing must disable swap immediately before measurement and restore this exact configuration afterward.
- There is no source-current performance chain, completion checkpoint, final package preflight, crates.io publication, `v0.2.0` tag, draft, or public GitHub release, and the WS4 scratch-repository release rehearsal (remediation success criteria 2 and 3) remains pending before release day.

## Next Actions

1. Push the focused nested-body parser repair and synchronized status revision, then require exact-revision `Rust` and `Qualification Contracts` CI before producing replacement evidence.
2. Regenerate and replay PR, full, and soak correctness, all 11 exact prerequisites, the live result-format corpus, the implemented oracle matrix, and 46 worker reproducibility receipts at unique replacement paths.
3. Produce fresh controlled AArch64 full and soak evidence for all 19 release groups, both verified-host accepted-maximum DEM memory receipts, 38 rollups, and one 138-report schema-version-4 `a9-release` completion; run creation and replay through `prlimit --nofile=1024:1024 --` and use the checked 100000-millidegree Celsius ceiling.
4. Preserve every failed, noisy, host-rejected, or resource-rejected outcome. Never reuse an artifact path, relax a threshold, add a waiver, or rerun a stable product failure.
5. Re-run both audits. If no source fix is required, create the authenticated completion checkpoint and exactly one seven-path status descendant, push it, and require exact-revision CI.
6. Create and inspect one final immutable schema-version-4 package preflight, publish the reviewed crates, create and push the protected annotated tag, capture the release workflow run returned by exact-tag dispatch, verify the private draft immediately before manual publication, and verify the public release immediately afterward.

## Release Gates

- Keep the `1.25x` Stim parity threshold, `1.15x` Stab self-regression threshold, workload equivalence, memory limits, and controlled-host policy unchanged.
- Formal completion and replay must pass with soft `RLIMIT_NOFILE=1024`; raising the limit is diagnostic only.
- `release::publish-reviewed` and `release::create-draft` must pass `qualification-status --check --require-release-completion` before reading a credential or making an irreversible request.
- The exact active no-bypass tag ruleset, annotated remote tag, six retained assets, private draft state, and published release state must pass their source-owned checks at the handoff points documented in [RELEASING.md](../RELEASING.md); a latest-run query is never an acceptable workflow identity.
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
