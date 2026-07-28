# Goal: Build The Sampling Plan And Session Boundary

## Objective

Finish milestone A4 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): compile sampling requests into immutable shareable plans, execute them through reusable mutable sessions, deliver typed batches to sinks, and migrate `stab sample` without changing established Stim-compatible behavior.

## Current State

- A0 through A3 are complete.
- `stab-bits` and `stab-records` are physical Stable Rust 1.97.1 crates.
- Clean A3 evidence is recorded in [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md).
- `CompiledSampler` remains the compatibility center and owns compilation, mutable execution state, and materialized output in one abstraction.
- `CompilationRequestFingerprint` is backend-neutral; A4 must add `PlanFingerprint` only after selecting a real backend and executable contract.
- Typed measurement batches and sinks are ready for engine use.

## Sources Of Truth

- Active milestone: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), A4
- Component boundaries: [../architecture/component-contracts.md](../architecture/component-contracts.md)
- Dependency graph: [../architecture/README.md](../architecture/README.md)
- API migration inventory: [../architecture/0.2-api-migration-inventory.md](../architecture/0.2-api-migration-inventory.md)
- Progress record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Specification gaps: [milestone-spec-gaps.md](milestone-spec-gaps.md)

Stop and repair the owning source when code, tests, generated inventories, benchmark contracts, or these documents disagree.

## Execution Sequence

1. Inventory every `CompiledSampler`, sampling helper, CLI, oracle, and benchmark call site; freeze old-versus-new behavior selectors before implementation.
2. Add failing semantic tests for compiler diagnostics, immutable-plan sharing, isolated sessions, same-session chunking, zero shots, cancellation, progress, sink failure, finalization failure, and session poisoning.
3. Introduce a compiler, private executable IR, immutable `SamplingPlan`, mutable non-`Sync` `SamplingSession`, random policy, run summary, and backend-bearing `PlanFingerprint`.
4. Reuse RNG, frames, reference samples, scratch, and at-most-64-shot batches after warmup; keep direct-Z, small-frame, and general-frame variants private.
5. Route execution only through typed `MeasurementSink`; keep materialized and byte-returning APIs as thin migration adapters.
6. Migrate `stab sample`, oracle paths, and benchmarks, preserving exact deterministic behavior where promised and statistical Stim parity elsewhere.
7. Regenerate correctness ownership, performance dispositions, capability descriptors, API migration records, and generated status in the same change set.
8. Measure compilation, session construction, raw execution, batch delivery, encoding, repeated-session execution, backend selection, and CLI end-to-end behavior with phase-separated workloads.
9. Run milestone-audit and full-code-review, fix every confirmed finding, and close A4 only from clean committed evidence.

## Nonnegotiable Contracts

- Execution imports no text codec, filesystem, CLI, or ops API.
- Plans are immutable, cloneable, `Send + Sync`, and do not serialize private IR.
- Sessions own mutation and reuse but are not promised `Sync`.
- Backend selection is static per plan; no dynamic dispatch enters the hot loop.
- Sink and finalization errors preserve the first error, exact committed progress, immediate stop, and poisoned-session semantics.
- Pre-execution validation failures do not poison a reusable session.
- Existing random-stream promises do not expand; pinned Stim statistical and semantic equivalence remains the compatibility target.
- Comparable rows retain the `1.25x` Stim parity gate and `15%` Stab self-regression gate.

## Done Criteria

- Every public sampling path delegates to the compiler, plan, session, and sink architecture.
- Old-versus-new seeded tests cover every private execution variant and chunking boundary.
- Plan sharing, session isolation, cancellation, error composition, poisoning, and bounded post-warmup allocation have direct tests.
- `stab sample` preserves flags, bytes, diagnostics, path safety, exit status, and resource boundaries.
- Qualification inventories regenerate exactly and no implemented A4 behavior has only planned ownership.
- Clean source-current benchmarks show no unexplained material regression.
- Milestone-audit, full-code-review, workspace verification, and pre-commit have no open A4 finding.

## Required Checks

Run targeted tests during implementation. Before each requested focused commit, run the touched-area checks; before A4 closure, run formatting, warnings-denied workspace Clippy and rustdoc, all workspace tests, architecture enforcement, implemented and result-format oracles, correctness and performance inventory checks, generated-status checking, benchmark smoke, and staged pre-commit validation.
