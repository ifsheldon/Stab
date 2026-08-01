# Goal: A8 Circuit Pass And Backend Extension Seams

Status: Active.

## Objective

Finish Milestone A8 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) by proving one meaningful built-in circuit transform and one external Stable transform can use a small typed pass contract, while backend discovery and explicit selection remain consistent with executable engine capabilities.

## Current State

- A0 through A7 are complete. A7 evidence is bound to measured source revision `38160da59e6a55b1e4efd753d2aee8b8eb18f2b0`; later A8 work must not replay or promote that evidence under a descendant.
- `stab-analysis` already owns the built-in circuit-without-noise transform and is the intended owner of the earned pass seam.
- Engine capability discovery already reports the executable compiler families and scalar sampling backend; `plan sample` reports selected backend state but does not yet accept explicit backend selection.
- A8 does not add dynamic Rust plugins, a GPU placeholder, a public execution IR, or external-process decoder transport.
- A9 owns controlled release evidence, reviewed self-regression baseline seeding, and the Stab 0.2.0 release.

## Sources Of Truth

- [Architecture plan](agent-native-modular-qec-architecture-plan.md), especially Milestone A8
- [Architecture progress report](agent-native-modular-qec-progress-report.md)
- [Component contracts](../architecture/component-contracts.md)
- [Decoder extension ADR](../architecture/adr-0006-decoder-extension-boundaries.md)
- [Correctness qualification contract](comprehensive-correctness-qualification-plan.md)
- [Performance qualification contract](comprehensive-stim-performance-qualification-plan.md)
- [Specification-gap log](milestone-spec-gaps.md)

Stop and repair the owning source when the plan, component graph, Stable boundary, pass API, backend resolver, CLI plan output, tests, inventories, or benchmark contracts disagree.

## Execution Sequence

1. Freeze the smallest pass contract earned by the existing without-noise transform and one external noise-insertion transform. Record typed options, context, limits, report, and diagnostics before implementation.
2. Adapt the built-in transform without changing its canonical output, preservation behavior, resource policy, or public compatibility path.
3. Add an unpublished Stable external-pass fixture crate that depends only on permitted public component APIs and inserts deterministic noise without changing the gate table or execution IR.
4. Validate pass outputs as Stim-compatible circuits and prove determinism, tag, repeat, coordinate, and target preservation, invalid-output rejection, unsupported-extension rejection, and resource admission.
5. Expose backend availability and explicit selection through the existing resolver, capabilities, and `plan sample`; prove auto, scalar, and unavailable selections report one consistent result.
6. Document external-process decoder protocol requirements without implementing transport or promising an ABI.
7. Add exact qualification ownership and only the earned diagnostics: built-in adaptation continuity and one external-pass throughput row. Reuse existing backend compilation diagnostics unless a measured selection-overhead risk justifies a new row.
8. Regenerate contracts, run focused Stable and Nightly checks, benchmark smoke and diagnostics, then run milestone-audit and full-code-review and fix every confirmed finding before closure.

## Nonnegotiable Contracts

- Passes operate on public typed circuit models and return validated circuits; they do not mutate the Stim gate registry or expose private execution internals.
- Per-pass options remain typed. Common context, limits, report, and diagnostics contain only behavior proven common by both implementations.
- External pass code compiles on Stable Rust 1.97.1 and cannot depend on `stab-core`, CLI, ops, private modules, Nightly, or portable SIMD.
- Backend requests use the existing typed resolver. Explicit unavailable backends fail clearly, and capabilities, plan summaries, and execution cannot disagree.
- Benchmarks measure complete public operations with source-owned semantic witnesses. No synthetic Stim comparator or placeholder backend microbenchmark is added.
- Historical evidence and failed artifact paths remain immutable.

## Done

A8 is complete only when the built-in and external passes share the earned typed seam, all pass outputs and backend selections have direct source-current behavioral evidence, the external Stable consumer compiles and runs, affected benchmark contracts and diagnostics pass, both final audits have no unresolved implementation finding, exact-revision CI is green, and the worktree is clean.
