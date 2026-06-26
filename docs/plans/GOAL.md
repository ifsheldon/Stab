# Milestone Implementation Goal

## Purpose

The goal of this plan is to make every Stab milestone mean the same thing: test parity is planned first, the feature is implemented to the milestone contract, done criteria are checked directly, and review findings are fixed before the milestone is considered complete.

## Scope

This goal applies to milestones M0 through M12 in `docs/plans/rust-stim-drop-in-rewrite.md`.
Future Plan items are out of scope until they are promoted into explicit milestones with objective, task, test, benchmark, and done-criteria sections.

## Milestone Completion Loop

A milestone is not complete until each gate below is completed in order.

### Gate 1: Port Or Create Tests

Before implementing milestone behavior, identify the tests needed to prove the milestone contract.
Use `docs/plans/stim-test-porting-plan.md` as the source for upstream Stim tests to port from `vendor/stim`.
When no direct upstream test exists, create focused Stab tests that prove the same public behavior, compatibility contract, statistical property, file-format rule, CLI behavior, or operational workflow.
Tests should be meaningful and should fail before the feature exists whenever practical.
Do not add tests that only restate constants, assert freshly constructed fields, or check generic library mechanics.
For probabilistic behavior, define the semantic or statistical property, sample size, tolerance, and deterministic reporting strategy before treating the test as parity evidence.
For performance milestones or performance-sensitive features, add or update benchmark workloads before making performance claims.

### Gate 2: Implement The Feature

Implement only the feature scope specified by the milestone objective and tasks unless the plan is deliberately updated.
Keep the implementation aligned with the workspace architecture, typed-boundary rules, portable-SIMD policy, operational-command policy, and Stim v1.16.0 compatibility target.
Update docs in the same change set when implementation changes public behavior, CLI behavior, file formats, benchmark gates, operational workflows, or developer workflows.
If implementation reveals an ambiguity in the milestone text, do not silently choose a broad interpretation.
Either revise the plan before continuing, or log the ambiguity in `docs/plans/milestone-spec-gaps.md` if it can safely be handled as follow-up specification work.

### Gate 3: Check Done Criteria

Check every task and done criterion in the milestone section directly.
For each requirement, record whether it is satisfied, partially satisfied, missing, not applicable, or blocked.
Evidence must point to code, tests, oracle fixtures, benchmark reports, docs, reproducible commands, or accepted plan text.
Do not mark a requirement satisfied because adjacent work exists.
If a linked test or benchmark cannot run, record the blocker and do not count it as passing evidence.

### Gate 4: Run Milestone Audit

Run the `.agents/skills/milestone-audit` workflow for the milestone.
Fix every implementation, test, benchmark, documentation, compatibility, workflow, and verification issue found by the audit.
Under-specification findings are handled differently: log them in `docs/plans/milestone-spec-gaps.md` with the milestone id, the weak current text, the implementation evidence that revealed the gap, and the proposed amendment.
If an under-specification finding makes it impossible to decide whether the milestone is complete, the milestone remains blocked or incomplete until the plan is clarified.

### Gate 5: Run Full Code Review

Run the `.agents/skills/full-code-review` workflow after the milestone audit is clean or has only logged under-specification follow-ups.
Fix every correctness, compatibility, security, performance, architecture, test-quality, and documentation issue found by the review.
If a finding is intentionally deferred, record the rationale and the target follow-up location.
Re-run the relevant tests, benchmarks, audits, or review slices after fixes that could affect previous evidence.

## Completion Evidence

Every completed milestone should have enough evidence for a future agent to reconstruct why it was accepted.
At minimum, the evidence should include the milestone id, tests or benchmarks added or ported, implementation areas changed, commands run, done-criteria status, milestone-audit outcome, full-code-review outcome, and any unresolved under-specification log entries.
This evidence can live in a PR description, issue, milestone report, or another durable project-tracking artifact.
Under-specification findings must always be recorded in `docs/plans/milestone-spec-gaps.md`.

## Acceptance Rule

A milestone is complete only when tests or benchmarks exist for the required behavior, the feature is implemented, done criteria are satisfied, milestone-audit issues are fixed or logged as under-specification follow-ups, full-code-review issues are fixed or explicitly deferred, and the verification evidence is durable.
