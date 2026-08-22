# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete. The reversible A9 release rehearsal passed; its one permitted documentation-only record and exact-head CI are the gate before replacement formal evidence.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): produce source-current correctness and controlled performance evidence, authenticate one release completion, publish the ten reviewed product crates, publish `v0.2.0`, and close the architecture migration without weakening compatibility, resource, or performance policy.

## Current State

- Ten product packages are versioned `0.2.0` with exact sibling requirements and a source-owned publication order.
- The release matrix contains 19 promotable groups, 138 full and soak reports, 38 rollups, and 21 unique exact correctness parents across 11 prerequisite sets.
- Every correctness and timing artifact from revisions before the final rehearsal record is immutable historical evidence and cannot authorize A9 completion.
- Rehearsal source `a57910ee00f53cb59253b91df31176cc9ec371b6` passed exact-head CI run `32550073066`. Protected annotated tag `v0.2.0-rehearsal-a57910ee00f53cb59253b91df31176cc9ec371b6` produced the single successful scratch workflow run `32551080518`; both native AArch64 builds, the private draft, all six asset digests, and the live read-only verifier passed.
- Scratch repository `ifsheldon/Stab-release-rehearsal` has numeric ID `1342241032`; active ruleset `21169813` protects the rehearsal tags without bypass. Private draft `374797341` must never be published. Every failed protected tag and run is retained in the architecture progress report and is never reused.
- This documentation change is the one permitted descendant of the exercised rehearsal source. No product, workflow, release-tool, qualification-contract, fixture, or policy change accompanies it.
- The controlled thermal ceiling is exactly `100000` millidegrees Celsius for both profiles. Firmware throttling and critical trips remain authoritative, and every other host-policy check remains unchanged.
- Swap configuration is `/swap.img`, size `137438949376` bytes, priority `-2`. Formal timing disables swap immediately before each report and restores this exact configuration afterward, including failure paths; used swap is mutable host state rather than part of the configuration identity.
- There is no source-current correctness chain, complete performance chain, completion checkpoint, final package preflight, crates.io publication, production tag, draft, or public GitHub release.

## Next Actions

1. Commit and push only this permitted rehearsal record, then require exact-head `Rust` and `Qualification Contracts` CI.
2. From that final clean revision, regenerate PR, full, soak, all 11 exact prerequisites, live oracle suites, 46 worker receipts, and both DEM adapter probes at unique paths.
3. Produce all 138 controlled AArch64 reports, two accepted-maximum DEM memory receipts, 38 rollups, and one schema-version-4 `a9-release` completion. Creation and replay run through `prlimit --nofile=1024:1024 --`; every failed, noisy, host-rejected, or resource-rejected path remains visible and is never reused.
4. Run milestone-audit and full-code-review. Fix confirmed source defects before promotion; any source change restarts the affected evidence from a new clean revision.
5. If no source fix is required, publish the authenticated completion checkpoint and exactly one permitted status descendant, push it, and require exact-head CI.
6. Follow [RELEASING.md](../RELEASING.md): produce one immutable final package preflight, publish the reviewed crates, create and push protected annotated `v0.2.0`, capture the exact workflow run, verify the private draft immediately before manual publication, and verify the public release immediately afterward.

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
