# Goal: Build The Agent-Native Modular QEC Toolkit

## Objective

Implement [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) completely and release the resulting architecture as Stab `0.2.0`.

Preserve implemented Stim v1.16.0 CLI and file-format compatibility while intentionally redesigning the pre-1.0 Rust API around typed models, explicit compilation, immutable plans, reusable sessions, typed batches, composable sinks, stable component crates, and public decoder and transform seams.

## Sources Of Truth

- Active execution plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md)
- Architecture contract: [../architecture/README.md](../architecture/README.md)
- Feature state: [../stab-feature-checklist.md](../stab-feature-checklist.md)
- Generated qualification state: [../qualification-status.md](../qualification-status.md)
- Correctness contract: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md)
- Performance contract: [comprehensive-stim-performance-qualification-plan.md](comprehensive-stim-performance-qualification-plan.md)
- Lessons: [lessons-learned.md](lessons-learned.md)

Stop when these sources disagree.

Fix the owning source and regenerate derived state instead of choosing the easiest interpretation.

## Current State

- The pre-refactor repository checkpoint is `cfaa1098fe7d37512b71bd2f5974196bbcdb14b9`.
- The accepted compatibility evidence revision is `68d107a42f655254f31628f0cbedc55479f6c0f3`.
- The previous qualification-economy program is complete and historical.
- Milestone A0 is active.

## Execution Loop

For each milestone:

1. Read the complete milestone, linked contracts, and affected source before editing.
2. Add or port meaningful tests that fail for the missing contract.
3. Implement the smallest complete architectural layer.
4. Run focused correctness, allocation, and benchmark checks for the changed path.
5. Update public, generated, operational, and migration documentation in the same change set.
6. Run milestone-audit and fix implementation findings; log only genuine under-specification.
7. Run full-code-review on the changed ownership boundary and fix confirmed findings.
8. Commit the milestone in focused Conventional Commit changes.

Do not defer a milestone defect merely because a later crate extraction could hide it.

## Non-Negotiable Rules

- Keep the Stim circuit and DEM dialects closed.
- Keep the executable IR private.
- Product crates never depend on ops.
- Execution never depends on codecs, paths, or filesystem handles.
- Default resource limits preserve current accepted and rejected boundaries.
- Human CLI output remains the default; JSON diagnostics are additive.
- Keep Stim parity at median and confidence upper bound no greater than `1.25x`.
- Treat missing self-regression identities as unseeded, never passing.
- Do not introduce dynamic Rust plugins, runtime gate registration, a placeholder GPU backend, or serialized compiled plans.
- Preserve every historical evidence artifact and source identity.

## Completion

The goal completes only when A0 through A9 satisfy their tests, benchmarks, acceptance criteria, audits, synchronized documentation, controlled AArch64 evidence, and `0.2.0` release requirements.
