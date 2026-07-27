# Goal: Close A2 Before Splitting Crates

## Objective

Finish and audit milestone A2 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), commit its implementation and evidence contracts, and produce source-current diagnostics from that clean revision.

Do not start A3 or claim that crates were split until this contract is complete. Physical extraction of `stab-bits` and `stab-records` belongs to A3.

## Sources Of Truth

- Active architecture plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md)
- Append-only implementation record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Architecture contracts: [../architecture/README.md](../architecture/README.md)
- Generated qualification state: [../qualification-status.md](../qualification-status.md)
- Correctness and performance contracts: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md) and [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Planning lessons and genuine specification gaps: [lessons-learned.md](lessons-learned.md) and [milestone-spec-gaps.md](milestone-spec-gaps.md)

Stop and repair the owning source when code, tests, generated inventories, or these contracts disagree.

## Current State

- A0 and A1 are complete.
- A2 is active and incomplete.
- The dirty worktree contains a closure candidate for stable parser diagnostics, exact byte spans, the finite opaque-tag transform matrix, fingerprints, capabilities, `inspect`, `plan sample`, and seven operation-owned resource policies.
- Review-driven repairs cover parser source order, transform and analyzer tag preservation, live pinned non-UTF-8 metadata evidence, bounded rejected-line preparation, linear metadata classification, finite detection traversal and compiled-plan defaults, programmatic detection-depth admission before recursion, platform-capacity admission, zero-width materialization, iterative deep folded-DEM ownership and transforms, separately bounded streaming DEM replay work, analyzer and `m2d` admission before output activation, and removal of duplicate detection compilation.
- The per-dimension resource matrix now has an executable real-default or justified reduced-boundary selector for every policy dimension. Practical defaults are executed directly, including the 4,096-detector hyperedge boundary; traversal and retained-state arithmetic have direct overflow selectors.
- Correctness inventory: `ccbeb26a1f4d10fedf68ef0aa66634c6b2b6607af76184598282501419c74a1d`.
- Performance inventory: `0d1fb8a08702dbb57b55e734e4735b3ce39f41388846d7b9ed715031feb88f54`.
- An earlier dirty-worktree checkpoint passed formatting, Clippy, workspace tests, architecture checks, and generated-inventory checks, but review repairs have changed the source since that checkpoint and the full sequence must be rerun.
- Source-current clean-revision diagnostics and final audit closure remain outstanding.
- A3 has not started, and `stab-bits` and `stab-records` have not been physically extracted.

## Remaining Sequence

1. Execute every selector in the per-dimension resource matrix and finish `milestone-audit` and `full-code-review`; fix every confirmed implementation, evidence, or documentation finding.
2. Regenerate correctness, performance, and status artifacts after the final source edit.
3. Run format, workspace Clippy, workspace tests, rustdoc, architecture checks, implemented oracle checks, result-format oracle checks, benchmark smoke, and pre-commit.
4. Commit A2 in focused implementation, CLI, qualification, benchmark-contract, and documentation commits.
5. From that clean unchanged revision, run the exact allocation-invariant, circuit-parser, and four Stab-only diagnostic commands in the active architecture plan, using unique artifact paths.
6. Require `local_modifications=false`, unchanged `1.25x` gates where a Stim-relative gate exists, exact report identities, and explicit diagnostic-only classification for the four Stab-only groups.
7. Record the clean checkpoint and report digests in the progress report, rerun the two audits, and fix any source-current finding before marking A2 complete.
8. Rewrite this goal for A3 only after A2 has passed.

## Guardrails

- Human CLI behavior remains the default; JSON is additive.
- The Stim circuit and DEM dialects stay closed.
- `PlanFingerprint`, backend selection, sessions, and execution batching remain A4 work.
- A2 diagnostics remain Stab-only and report-only; independent medians do not imply incremental cost or Stim parity.
- No caller policy may bypass semantic, representation, recursive-safety, or platform invariants.
- Replay input is caller-owned storage, replay traversal is operation-owned work, and the historical active-footprint boundary remains enforced.
- Dirty or unverified-host timing is diagnostic and non-promotable.

## Required Checks

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
just architecture::check
just oracle::run --implemented-only
just oracle::result-formats --check
just qualification::correctness-check
just qualification::correctness-regenerate --check
just bench::qualification-check
just bench::qualification-regenerate --check
just qualification::status --check
just bench::smoke
just maintenance::pre-commit
```
