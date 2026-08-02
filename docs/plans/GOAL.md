# Goal: Qualify And Release Stab 0.2.0

Status: Active. Milestones A0 through A8 are complete; A9 is the remaining architecture-plan milestone.

## Objective

Finish Milestone A9 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md): freeze the coordinated 0.2 product, produce source-current correctness and controlled performance evidence, publish every product crate together, publish the `v0.2.0` GitHub release, and close the architecture migration without weakening compatibility or performance policy.

## Current State

- A8 measured source `c797ebc908ce1b81675e479031c39f71740058ae` passed exact-revision GitHub Actions through status descendant `c4299a23383043ee74daba80621d25072cdac5b9`, run `30732340918`.
- Ten product packages are already versioned `0.2.0` in publication order: `stab-kernels-simd`, `stab-model`, `stab-bits`, `stab-records`, `stab-algebra`, `stab-analysis`, `stab-decoder`, `stab-engine`, `stab-core`, and `stab-cli`; internal publishable dependencies require exact `=0.2.0` versions.
- The first A9 preflight reports are review-rejected because `cargo publish --dry-run` left stale shared `target/package` archives. The repaired `release::` surface creates fresh isolated archives, validates embedded commits, preserves immutable reviewed copies, records Cargo and rustc identities, rebuilds byte-identically before each explicit crates.io upload, and verifies the registry checksum before continuing.
- Clean schema-version-3 validation probes at `3cfef095c1c3d2cb765b394198a9ba6318c7c651` and `12de9967d7c61c3f76477bff54068994d483c724` produced source-current archives, but the post-audit repairs make every such path historical diagnostic evidence. The final publication source requires a new exact-commit preflight after the single status descendant passes CI.
- Tagged binaries are built inside the release operation, version and architecture checked, and bound to source and toolchain manifests. GitHub Actions aggregates both AArch64 targets and creates a draft only after complete asset verification; it never publishes a partial release or replaces assets.
- `docs/MIGRATING-0.2.md`, the component graph, facade tiers, migration inventory, README, feature checklist, qualification inventories, and generated dashboard exist.
- The active performance matrix contains 19 promotable groups and remains below the 40-group release cap. Eight product diagnostics plus one infrastructure group remain below the 60-group diagnostic cap.
- The 19 release groups reference 21 unique exact correctness parents. Formal source-current PR, full, and soak evidence has not started.
- The latest formal completion is historical DEM-only evidence. There is no current A9 completion checkpoint, `v0.2.0` tag, crates.io publication, or GitHub release.
- Completion schema 3 supports a source-derived `a9-release` scope covering all current promotable groups and records exact correctness evidence per group. Checkpoint schema 2 references a byte-identical checked copy of the replayed manifest; the dashboard hashes and validates that copy, requires a clean checked state, and permits at most one exact seven-path status descendant. Historical completion schemas 1 and 2 are readable but are not reported as replayed.
- The second pre-evidence review's implementation blockers are repaired locally: release interruption terminates the complete child process group, Cargo execution uses a private source-owned environment, credentials reach only the reviewed upload, archive expansion is bounded, asset execution and cleanup stay descriptor-bound, executable identity is exact, and every remote workflow action is pinned to a full commit SHA and checked by architecture policy.
- No Cargo registry credential is currently configured on this host. Publication waits for all pre-publication evidence and audits, then requires the user to provide a crates.io token through `CARGO_REGISTRY_TOKEN`; the token must never enter logs, files, arguments, or generated artifacts.
- The repaired source is not on GitHub yet: after a fresh fetch, `origin/main` still points to `c4299a23383043ee74daba80621d25072cdac5b9`. Formal correctness, timing, and release preflight remain stopped until the final pre-evidence source is committed, pushed, and passes exact-revision CI.

## Execution Sequence

1. Finish the pre-evidence repair, synchronize README, checklist, migration guide, plans, benchmark contracts, generated status, and release instructions, then run milestone-audit and full-code-review. Fix every confirmed finding before evidence.
2. Commit the final pre-evidence source, regenerate correctness and performance contracts without drift, push it, and require exact-revision CI before formal work.
3. From that clean commit, run one new diagnostic `release::check`, inspect every immutable archive, and preserve the report identity. This catches package-contract drift before expensive evidence but is not the final publication authority. Never promote or reuse a prior path.
4. Keep the 19 release groups and 21 exact correctness parents frozen. Run and replay PR, full, and soak correctness, the live result-format corpus, and the implemented oracle matrix.
5. Reproduce both workers, then produce controlled AArch64 full and soak evidence for all 19 groups and scales. Replay reports, enforce `1.25x` Stim parity and seeded `1.15x` Stab self-regression, report unseeded identities honestly, run memory/scaling checks, publish 38 rollups and the 138-report `a9-release` completion manifest, and replay it once.
6. Run the legacy primary timing and memory suites only as diagnostic continuity. Shared-host timing, unverified-host timing, and product diagnostics cannot replace controlled release evidence.
7. Restore the exact prior swap configuration after every timing session, verify no qualification process remains, and preserve every failed or historical artifact path.
8. Re-run milestone-audit and full-code-review against completed evidence. Any confirmed executable, test, fixture, workflow, inventory, policy, or normative-document fix invalidates affected evidence and must be committed before replacement runs.
9. If the evidence and reviews pass unchanged, run `just qualification::completion-checkpoint --input <a9-release-completion>` from the clean measured commit. Generate the dashboard, then create exactly one status descendant limited to the checked completion manifest, its checkpoint, generated dashboard, progress report, architecture-plan status, this GOAL, and specification-gap log. The manifest remains labeled with its measured parent commit.
10. Push that single descendant and require exact-revision CI. From the same clean descendant, create a new final immutable `release::check` report; inspect its exact archives, VCS and toolchain identities, Cargo isolation, and publication order before the first upload.
11. Publish the reviewed crates, create and push annotated tag `v0.2.0` at the status descendant, dispatch the draft-only release workflow, inspect and publish the complete draft, and verify every crates.io checksum, GitHub asset, source identity, and action identity. Record post-publication facts in registry metadata, the annotated tag, and release assets without creating a second source descendant.

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
- Do not publish from a dirty tree, a revision outside the exact status-descendant contract, or a source whose immutable package archives and byte-identical rebuilds were not preflighted.
- Crates.io publication is irreversible and non-atomic. Complete all local and remote preflight before the first upload, publish only in the source-owned topological order, and stop on any identity or availability mismatch.
- Keep ops and test-support crates unpublished. Keep deferred Stim products, Python, JS/WASM, GPU, dynamic plugins, runtime gate registration, external decoder transport, and a public execution IR outside 0.2.0.

## Done

The architecture plan is complete only when every A9 evidence and publication requirement is independently verified, `v0.2.0` is available from the intended registries and GitHub release, final audits are clean, swap is restored, no qualification process remains, and the worktree is clean.
