# Goal: Build The Agent-Native Modular QEC Toolkit

## Objective

Implement [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md) completely and release the resulting architecture as Stab `0.2.0`.

Preserve implemented Stim v1.16.0 CLI and file-format compatibility while intentionally redesigning the pre-1.0 Rust API around typed models, explicit compilation, immutable plans, reusable sessions, typed batches, composable sinks, stable component crates, and public decoder and transform seams.

## Sources Of Truth

- Active execution plan: [agent-native-modular-qec-architecture-plan.md](agent-native-modular-qec-architecture-plan.md)
- Architecture contract: [../architecture/README.md](../architecture/README.md)
- Feature state: [../stab-feature-checklist.md](../stab-feature-checklist.md)
- Generated qualification state: [../qualification-status.md](../qualification-status.md)
- Qualification contracts: [correctness](comprehensive-correctness-qualification-plan.md) and [performance](comprehensive-stim-performance-qualification-plan.md)
- Lessons: [lessons-learned.md](lessons-learned.md)

Stop when these sources disagree; fix the owning source and regenerate derived state.

## Current State

- The accepted compatibility evidence revision is `68d107a42f655254f31628f0cbedc55479f6c0f3`.
- A0 and A1 are complete; A2 is active at committed checkpoint `6aad05b8`.
- Current correctness inventory: `b8ee2e2daa6a35e52d54713505c44ba08a1cd35a21a39ca77be60321bd55ea1c`.
- Current performance inventory: `95bfb5065c302569870ccc8fcd666268a315b6a4fb311a154be8df6c72466584`.
- Formal evidence for these inventories has not started; see [the A1 closure and reviewer feedback](agent-native-modular-qec-progress-report.md).

## Active Milestone

Milestone A2 is active.

Implement it in independently reviewable slices:

1. Add the result-format diagnostic nucleus: `ByteSpan`, `DiagnosticSeverity`, and a domain `FormatError` with stable codes.
2. Route materialized and streaming result readers through byte-aware diagnostics without changing accepted grammar or human CLI behavior.
3. Add schema-version-1 CLI JSON rendering as an additive `--error-format` mode.
4. Add operation-owned resource policies and estimates while preserving every current default limit and first rejection.
5. Add `ModelFingerprint` and backend-neutral `CompilationRequestFingerprint`.
6. Generate capabilities from current descriptors and add `capabilities`, `inspect`, and `plan sample`.

Do not create a backend-bearing `PlanFingerprint` in A2. A4 owns it because selected backend and executable-contract identity do not exist until compilation.

## Execution Loop

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
- Treat dirty benchmark comparisons as diagnostics only and record their exact baseline, source revision, run count, and local-modification state.

## Immediate Verification

```text
cargo test -p stab-core result_format --quiet
cargo test -p stab-cli error_format --quiet
just qualification::correctness-regenerate --check
just architecture::check
```
