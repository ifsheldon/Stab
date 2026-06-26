---
name: milestone-audit
description: Use when auditing whether a Stab milestone is properly implemented, whether its tests and benchmarks prove the promised parity, or whether implementation revealed loopholes, ambiguous acceptance criteria, or under-specified milestone scope.
---

# Milestone Audit

This skill defines the acceptance bar for Stab milestones.
Use it after a milestone implementation, before marking a milestone complete, when reviewing a milestone-focused PR, or when implementation work reveals that the milestone plan may be incomplete.

## Audit Stance

Act like a release engineer and compatibility reviewer.
This is not a general code review and not a style pass.
The main question is whether the milestone contract is actually satisfied and whether the contract itself needs revision.

Findings must be evidence-backed:

- Lead with findings, ordered by severity.
- Include exact file paths and tight line references when a finding points at code, docs, tests, or benchmark artifacts.
- Distinguish implementation defects from milestone specification gaps.
- State whether each issue blocks milestone completion or should become follow-up work.
- Suggest the smallest concrete remediation, including plan edits when the milestone text is the problem.
- Avoid style-only feedback unless it hides a compatibility, maintainability, test-quality, or operational risk.

## Inputs To Gather

Start from the workspace root and gather the milestone evidence before judging it:

- The milestone section in `docs/plans/rust-stim-drop-in-rewrite.md`, including Objective, Tasks, Linked tests and benchmarks, and Done criteria.
- The matching test hierarchy in `docs/plans/stim-test-porting-plan.md`.
- Any compatibility matrix, oracle fixture manifest, benchmark report, profiler note, or release gate artifact introduced by the milestone.
- The implementation diff or commits claimed to satisfy the milestone.
- Relevant `just` recipes, ops binaries, CI checks, docs, and command outputs.
- Current repository instructions in `AGENTS.md`.

Do not treat missing future-plan work as a milestone failure unless the milestone explicitly claims it.

## Audit Workflow

### 1. Identify The Milestone Contract

- Extract the milestone identifier, objective, task list, linked tests and benchmarks, and done criteria.
- List the expected crates, modules, CLI commands, file formats, docs, reports, recipes, and artifacts.
- Note explicit deferrals and future-plan exclusions.
- Translate vague phrases such as "basic support", "compatible enough", "fast", or "covered" into concrete questions that evidence must answer.

### 2. Build An Evidence Inventory

- Map implementation files to each task.
- Map tests, oracle cases, statistical checks, snapshot fixtures, and benchmark reports to each linked test or benchmark requirement.
- Record commands that were run and whether they are reproducible through `just`, Cargo, or an ops binary.
- Record checks that were not run, could not run, or depend on missing tools.
- Check whether docs and plans were updated when the implementation changed scope, public behavior, CLI behavior, file formats, operational workflows, or acceptance gates.

### 3. Audit Completion

For every task and done criterion, assign one status:

- `Satisfied`: implemented and backed by direct evidence.
- `Partially satisfied`: meaningful progress exists, but the evidence or behavior is incomplete.
- `Missing`: required behavior, test, benchmark, doc, or artifact is absent.
- `Not applicable`: the plan explicitly excludes the item or the implementation no longer touches that surface.
- `Blocked`: completion depends on an unavailable external tool, missing decision, or upstream artifact.

Require evidence for every `Satisfied` item.
Evidence can be code references, test names, fixture manifests, benchmark reports, docs, or reproducible commands.

### 4. Audit Loopholes And Under-Specified Scope

Look for milestone text that let incomplete work appear complete:

- Undefined compatibility target, comparator, fixture source, command surface, output format, or failure mode.
- Missing edge cases for parsing, printing, repeat blocks, detector error models, result formats, probabilistic behavior, or malformed inputs.
- Tests that prove only construction or smoke behavior when the milestone promises Stim parity.
- Benchmark requirements without workloads, sample sizes, hardware notes, threshold definitions, or report locations.
- Acceptance criteria that rely on "manual inspection" without saying what must be inspected.
- Public behavior changes that lack documentation or compatibility rationale.
- Operational requirements that bypass the repository rules for `just` recipes and Rust ops binaries.
- Typed-boundary or hostile-input requirements that are implied by the feature but missing from the milestone.

When a loophole is confirmed, recommend a concrete plan amendment.
Do not mark the implementation defective when the code followed the written milestone but the written milestone was too weak.
Instead, report "complete against current text, but milestone spec needs follow-up" when that is the accurate state.

### 5. Review Risk Lanes

Cover the lanes that the milestone touches:

- Stim compatibility: `.stim`, `.dem`, result formats, CLI stdout and stderr, exit status, flags, defaults, and probabilistic semantic equivalence.
- Tests and oracles: upstream Stim v1.16.0 comparison, fixture coverage, property checks, statistical checks, regression tests, and non-triviality of tests.
- Benchmarks and performance: reproducible commands, report artifacts, hardware notes, median and variance handling, memory measurements, and performance gate thresholds.
- CLI and file boundaries: typed parsing, hostile paths, unsafe archive entries, unbounded input, output roots, byte order, line endings, and error messages.
- Architecture and maintainability: clear ownership of invariants, crate boundaries, parser/printer placement, simulator separation, bit-kernel isolation, and avoidance of large dumping-ground files.
- Operational workflow: no shell scripts, thin `just` recipes, complex logic in Rust ops binaries, and documented developer workflows.

## Output Format

Use this structure for a milestone audit report:

```text
Findings

- [P1] Short title
  Evidence: path/to/file:line, command, report, or fixture.
  Impact: why this blocks or weakens the milestone.
  Fix: smallest concrete remediation.

Milestone Status

Status: Complete | Incomplete | Complete With Spec Follow-ups | Blocked
Rationale: one short paragraph.

Completion Matrix

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Mx task or done criterion | Satisfied | path/to/file:line or command | brief note |

Spec Loopholes

- Current text: quote or paraphrase the weak requirement.
  Revealed by: implementation behavior or missing evidence.
  Amendment: concrete replacement or added criterion.

Verification

- Commands run.
- Commands not run and why.

Recommended Follow-ups

- Required before marking complete.
- Follow-up issue or future-plan item.
```

If there are no findings, say so clearly and still include residual risks, skipped checks, and any spec follow-ups.

## Completion Rules

- Do not accept a milestone as complete solely because code exists.
- Do not accept test parity when tests do not exercise the promised Stim behavior.
- Do not accept statistical equivalence without a defined statistical check, sample size, tolerance, and deterministic reporting strategy.
- Do not accept performance claims without reproducible benchmark commands and committed or documented reports.
- Do not require exact C++ Stim random streams unless the plan is deliberately changed to require them.
- Do not expand the milestone to include Python bindings, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM, Quirk, or GPU work unless the milestone explicitly includes them.
- Do not recommend GPU work as a milestone fix unless CPU portable-SIMD profiling shows a batch-parallel bottleneck and transfer overhead has been considered.
- Do not treat "needs more tests" as a useful finding without naming the exact behavior, edge case, fixture, or compatibility surface that needs coverage.
