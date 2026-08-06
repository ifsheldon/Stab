# Goal: Qualify And Release Stab 0.2.0

Status: Reopened for remediation. Milestones A0 through A8 remain complete, but A9 is frozen: the August 2026 full code review recorded eleven confirmed P1 findings, and [post-review-remediation-plan.md](post-review-remediation-plan.md) is the active execution contract until its Pass 1 closes. The A9 pre-evidence state described below resumes only after that plan restores this document to an active release state.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten product crates from reviewed bytes, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Revision `d424175c6620549086fb4ee358077632bdab97d9` passed exact-revision CI and produced a complete 138-report AArch64 chain. All reports passed Stim parity, with 122 median speedups and 16 median slowdowns, but the chain is historical and review-rejected under the repaired contracts.
- The rejected completion replay reopened validated report paths, did not recompute final Git cleanliness, and was followed by confirmed release argument, revision-binding, toolchain-provenance, and late remote-tag defects. None of its artifacts authorizes release.
- Focused repairs now retain replay and correctness inputs through checked-status publication, authenticate the exact soft `RLIMIT_NOFILE=1024` boundary, bind verified-host schema-version-3 DEM memory receipts from the formal sealed private worker into schema-version-4 completion, require the current 11 shared correctness artifacts, and preserve typed Git failures through publication. Release repairs freeze the complete workflow execution context, validate toolchains across targets, require protected ruleset `20419793`, capture the exact dispatch run, verify the private draft immediately before publication, verify the public release immediately afterward, quote typed `just` arguments, reject yanked registry recovery, and build each local operator in a fresh owner-only target.
- The earlier final reviews found no P0 or P1 issue, but the August 2026 full code review superseded that conclusion with eleven confirmed P1 findings recorded in [post-review-remediation-plan.md](post-review-remediation-plan.md); the affected checklist rows are reopened and their qualification claims withdrawn. Commit `47f68446` closes the remaining P2 retained-asset gap by rehashing every retained descriptor, checking its reviewed length and digest, and rechecking the exact six-entry set. Commit `5a17c86c` uses validated protocol digest types for current formal memory receipts without changing their schema-version-3 wire format and removes one derive-only error-conversion test.
- The first broad workspace run exposed one test-harness-only Linux `ETXTBSY` failure while executing a generated AArch64 fixture. Commit `9904e03f` explicitly syncs and closes the writer and permits only four 10-millisecond retries for that one transient OS error. The complete local non-timing verification then passed from the clean `9904e03f` source tree.
- The controlled host ceiling is 100000 millidegrees Celsius. Firmware throttling and critical trips remain authoritative, and every other host-policy check remains unchanged.
- Swap configuration is restored as `/swap.img`, size `137438949376` bytes, priority `-2`. Used bytes may change during ordinary host activity; formal timing must disable swap immediately before measurement and restore this exact configuration afterward.
- There is no source-current correctness chain, performance chain, completion checkpoint, final package preflight, crates.io publication, `v0.2.0` tag, draft, or public GitHub release.

## Remediation Freeze

Until Pass 1 of [post-review-remediation-plan.md](post-review-remediation-plan.md) closes and that plan restores this document to an active release state, the following actions are prohibited:

- Producing new A9 correctness or performance evidence, rollups, memory receipts, or completion manifests.
- Creating a completion checkpoint or package preflight.
- Publishing any crate to crates.io.
- Creating or pushing the `v0.2.0` tag.
- Creating a GitHub draft or published release.

Diagnostic and report-only runs remain permitted; nothing produced during the freeze is promotable release evidence.

## Next Actions

The numbered actions below are the frozen A9 sequence; they resume only after the remediation freeze lifts.

1. Commit this final documentation synchronization, push the resulting clean revision, and require exact-revision `Rust` and `Qualification Contracts` CI.
2. Regenerate and replay PR, full, and soak correctness, all 11 exact prerequisites, the live result-format corpus, the implemented oracle matrix, and 46 worker reproducibility receipts at unique paths.
3. Produce fresh controlled AArch64 full and soak evidence for all 19 release groups, both verified-host accepted-maximum DEM memory receipts, 38 rollups, and one 138-report schema-version-4 `a9-release` completion; run creation and replay through `prlimit --nofile=1024:1024 --`.
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
