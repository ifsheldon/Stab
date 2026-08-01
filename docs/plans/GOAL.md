# Goal: A7 Decoder Interoperability

Status: Active.

## Objective

Finish Milestone A7 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) by proving that an unpublished external decoder can compile an exact bounded DEM, consume truth-hidden typed detector batches, write caller-owned observable predictions, and participate in a real sample-to-detect-to-decode experiment through public Stable component APIs only.

## Current State

- A0 through A6 are complete. A6 is bound to measured source revision `adae364500744c33f98f7777901ff50a28cbfdf6`; later A7 work does not rewrite or promote that evidence.
- The physical component workspace, typed record batches, immutable engine plans, reusable sessions, bounded sinks, model fingerprints, and exact DEM syntax are available.
- `stab-decoder` and the reference decoder do not yet exist in the active tree.
- The parked A7 stashes are rejected prototypes. Reuse sound ideas manually; do not apply either stash wholesale.
- A9 owns formal controlled-host release evidence. A7 produces source-current correctness, executable benchmark contracts, and diagnostic self-regression measurements only.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially A7.0 through A7.6
- [Decoder boundary ADR](../architecture/adr-0006-decoder-extension-boundaries.md)
- [Component contracts](../architecture/component-contracts.md)
- [Correctness qualification contract](comprehensive-correctness-qualification-plan.md)
- [Performance qualification contract](comprehensive-stim-performance-qualification-plan.md)
- [Append-only progress report](agent-native-modular-qec-progress-report.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owning source when the plan, Cargo metadata, architecture checks, APIs, tests, inventories, benchmark contracts, or generated status disagree.

## Execution Sequence

1. Freeze the static decoder contract and rationales before source edits. Keep compilation implementation-specific and reject dynamic Rust plugins or per-shot dynamic dispatch.
2. Add correction-typed mutable prediction-prefix views and a bounded model-owned DEM error-mechanism visitor, with focused semantic and resource tests first.
3. Add Stable `stab-decoder`, canonical facade reexports, external Stable-consumer behavior, architecture enforcement, API docs, and the portable-SIMD Clippy CI lane.
4. Add unpublished `stab-reference-decoder` with the exact A7 limits, log-domain dynamic program, impossible-syndrome behavior, reusable session, and independent exhaustive oracle.
5. Prove distance-3 and distance-5 generated repetition models and one real public-only sampling, conversion, decoding, and logical-error experiment with seeded and partitioned execution.
6. Add exact correctness ownership and no more than three executable Stab-only benchmark groups for compilation, reused decode, and the full pipeline. Do not create a Stim ratio.
7. Regenerate correctness, performance, and status artifacts; run focused Stable and Nightly checks, source-current correctness tiers, benchmark smoke, and diagnostic measurements at unique paths.
8. Run milestone-audit and full-code-review, fix every confirmed finding, rerun affected evidence from the resulting clean commit, and synchronize closure documentation.

## Nonnegotiable Contracts

- Decoder inputs cannot expose true observable outcomes.
- Detector width, correction width, and shot count are validated before output mutation.
- `DecoderSession` uses generic static dispatch and a non-forgeable validated batch; no `dyn DecoderSession` enters the hot path.
- Cancellation is checked at record boundaries, commits only a completed prefix, and promises no wall-clock deadline for one record.
- The model, not each decoder, owns repeat, shift, separator, and absolute-target DEM traversal semantics.
- Exact ML admits at most 20 detectors, one observable, 256 mechanisms, 65,536 represented instruction visits, `2^21` joint states, 16 MiB temporary workspace, and `2^28` transitions.
- Reused exact-ML decoding allocates no memory and retains one byte per detector syndrome.
- The external decoder and experiment depend only on public Stable component APIs; they do not import `stab-core`, CLI, ops, private modules, or Nightly features.
- Decoder benchmarks are Stab self-regression only and are unseeded until A9 controlled full and soak evidence; the `1.25x` Stim gate is neither applied nor weakened.
- Historical evidence and failed artifact paths remain immutable.

## Done

A7 is complete only when every A7.0 through A7.6 acceptance claim has direct source-current evidence, the external experiment runs through public batches, the curated correctness and benchmark contracts regenerate cleanly, both final audits have no unresolved implementation finding, and the worktree is clean.
