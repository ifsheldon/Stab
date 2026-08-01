# Goal: A8 Circuit Pass And Backend Extension Seams

Status: Active.

## Objective

Finish Milestone A8 of the [agent-native modular QEC architecture plan](agent-native-modular-qec-architecture-plan.md) by proving one meaningful built-in circuit transform and one external Stable transform can use a small typed pass contract, while backend discovery and explicit selection remain consistent with executable engine capabilities.

## Current State

- A0 through A7 are complete. A7 evidence is bound to measured source revision `38160da59e6a55b1e4efd753d2aee8b8eb18f2b0`; later A8 work must not replay or promote that evidence under a descendant.
- The source-current dirty worktree contains the typed `stab-analysis` pass executor, the adapted built-in without-noise pass, and a Stable external noise-pass proof crate; completed audit repairs add projected logical-payload admission before proportional lowering allocation and typed input/projection/output rejection stages, while the legacy compatibility function retains its previous direct resource policy.
- `stab plan sample` accepts typed auto, scalar, and portable-SIMD backend requests through the existing resolver; unavailable portable SIMD fails explicitly and does not fall back.
- Pre-audit dirty development probes exist for the external pass and built-in continuity row, but the projection-contract repair makes them stale as well as non-promotable; fresh diagnostics wait for committed source.
- The regenerated dirty-source identities are correctness `afec1b7090cc1254d6414ec4e10333e3d43976bbb5cc680822797ef231f4c676` and performance `5d35927f8518a6df5de141b674af8d38858b16338437f1e033897b0419090f20`; they become eligible for evidence only after the exact source is committed and all checks pass.
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
4. Admit input before dispatch, admit conservative projected output before proportional allocation, validate the actual Stim-compatible result, and prove determinism, preservation, projection-underestimate rejection, typed diagnostics, and closed-model unknown-gate rejection as separate contracts.
5. Expose backend availability and explicit selection through the existing resolver, capabilities, and `plan sample`; prove auto, scalar, and unavailable selections report one consistent result.
6. Document external-process decoder protocol requirements without implementing transport or promising an ABI.
7. Add exact qualification ownership and only the earned diagnostics: built-in adaptation continuity and one external-pass throughput row. Reuse existing backend compilation diagnostics unless a measured selection-overhead risk justifies a new row.
8. Regenerate contracts, run focused Stable and Nightly checks, benchmark smoke and diagnostics, then run milestone-audit and full-code-review and fix every confirmed finding before closure.

## Nonnegotiable Contracts

- Passes operate on public typed circuit models and return validated circuits; they do not mutate the Stim gate registry or expose private execution internals.
- Every implementation declares a checked conservative folded-output projection without allocating in proportion to it. The executor admits that projection before lowering, admits the actual output, exposes the typed rejection stage, and rejects underestimation before release. Projected payload bytes exclude allocator metadata and spare capacity and make no exact resident-memory claim.
- Per-pass options remain typed. Common context, limits, report, and diagnostics contain only behavior proven common by both implementations.
- External pass code compiles on Stable Rust 1.97.1 and cannot depend on `stab-core`, CLI, ops, private modules, Nightly, or portable SIMD.
- Backend requests use the existing typed resolver. Explicit unavailable backends fail clearly, and capabilities, plan summaries, and execution cannot disagree.
- Benchmarks measure complete public operations with source-owned semantic witnesses. Existing compile and execution diagnostics remain sufficient for a single executable backend, so no synthetic Stim comparator or placeholder backend microbenchmark is added.
- Historical evidence and failed artifact paths remain immutable.

## Done

A8 is complete only when the built-in and external passes share the earned typed seam, all pass outputs and backend selections have direct source-current behavioral evidence, the external Stable consumer compiles and runs, affected benchmark contracts and diagnostics pass, both final audits have no unresolved implementation finding, exact-revision CI is green, and the worktree is clean.
