# Goal: Build The Sampling Plan And Session Boundary

## Objective

Finish milestone A4 of [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md): compile sampling requests into immutable shareable plans, execute them through reusable mutable sessions, deliver typed batches to sinks, and migrate `stab sample` without changing established Stim-compatible behavior.

## Current State

- A0 through A3 are complete.
- `stab-bits` and `stab-records` are physical Stable Rust 1.97.1 crates.
- Clean A3 evidence is recorded in [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md).
- A4 code is implemented but remains open until focused commits, audits, and clean evidence complete.
- `SamplingCompiler`, immutable `SamplingPlan`, mutable `SamplingSession`, typed batches, sink composition, and backend-bearing `PlanFingerprint` are the canonical sampling path.
- `CompiledSampler` remains a source-compatibility adapter, and `stab sample` uses the public plan/session/sink path.
- Scalar is the sole registered backend; explicit portable SIMD requests fail before lowering until A6 provides a real implementation.

## Sources Of Truth

- Active milestone: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md), A4
- Component boundaries: [../architecture/component-contracts.md](../architecture/component-contracts.md)
- Dependency graph: [../architecture/README.md](../architecture/README.md)
- API migration inventory: [../architecture/0.2-api-migration-inventory.md](../architecture/0.2-api-migration-inventory.md)
- Progress record: [agent-native-modular-qec-progress-report.md](agent-native-modular-qec-progress-report.md)
- Specification gaps: [milestone-spec-gaps.md](milestone-spec-gaps.md)

Stop and repair the owning source when code, tests, generated inventories, benchmark contracts, or these documents disagree.

## Execution Sequence

1. Finish source-current correctness and performance inventory regeneration and generated status.
2. Run focused A4 tests, complete workspace verification, architecture enforcement, implemented oracles, and benchmark smoke.
3. Run milestone-audit and full-code-review; fix every confirmed implementation, test, benchmark, and documentation finding.
4. Commit source, CLI, benchmark, qualification, and documentation changes in focused commits.
5. From the clean source revision, rerun phase diagnostics and pinned comparable sampling rows using unique artifact paths and controlled affinity.
6. Record clean evidence and close A4 only when the unchanged `1.25x` process-symmetric Stim parity gate passes and every new A4 measurement is explicitly recorded as an unseeded candidate for later `15%` Stab self-regression.

## Nonnegotiable Contracts

- Execution imports no text codec, filesystem, CLI, or ops API.
- Plans are immutable, cloneable, `Send + Sync`, and do not serialize private IR.
- Sessions own mutation and reuse but are not promised `Sync`.
- Backend selection is static per plan; A4 registers only the real scalar backend, rejects explicit portable SIMD as unavailable, and leaves genuine SIMD registration to A6.
- Ordinary sampling has no placeholder `SamplingLimits`; fixed representability checks remain compiler semantics until a real caller-selectable compilation budget exists.
- Sink and finalization errors preserve the first error, exact committed progress, immediate stop, and poisoned-session semantics.
- Each nonempty run finalizes one supplied sink lifecycle; chunked codec output uses fresh sinks and composes finalized streams.
- Cooperative cancellation is checked between completed batches; it does not promise a wall-clock deadline inside one expensive shot.
- Session construction rejects a conservative estimate above 256 MiB before allocating reusable frame, span, record, reference, or bit-plane storage.
- Pre-execution validation failures do not poison a reusable session.
- Existing random-stream promises do not expand; pinned Stim statistical and semantic equivalence remains the compatibility target.
- Comparable process rows retain the `1.25x` Stim parity gate. New A4 phase and process measurement identities have no semantically identical pre-A4 baseline, so the clean A4 report seeds candidates and the `15%` Stab self-regression gate begins with later identity-matched revisions.

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
