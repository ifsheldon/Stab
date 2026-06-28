# Lessons Learned From The First Core Rewrite Pass

## Purpose

This document records planning lessons from the M0 through M12 implementation pass.
It focuses on what went wrong in the planning and acceptance process, so future milestone plans can avoid repeating the same issues.
It intentionally does not list remaining product, parity, performance, or future-roadmap fixes.

## High-Level Lesson

The main failure mode was not a lack of effort or tests.
The main failure mode was accepting milestone text that was too broad, too file-oriented, or too vague before implementation began.
When a milestone did not name exact subcases, comparators, resource limits, benchmark evidence, and deferrals, implementation could appear complete until milestone audit or full code review exposed the missing contract.

Future planning should make ambiguity expensive before coding starts, not after a feature is mostly implemented.

## Lessons

### 1. Do Not Treat Upstream Files As Acceptance Criteria

Several early milestones linked whole Stim test files even though those files mixed pure data-model behavior, CLI behavior, simulator behavior, benchmark behavior, Python binding behavior, and future ecosystem surfaces.
That made it too easy for a row marked implemented to be misread as full parity for the whole upstream file.

Future plans should split each linked upstream file into owned subcases, deferred subcases, and explicit extraction criteria before implementation starts.
If a test file is too broad to split immediately, the milestone should keep it manifest-only and say exactly what must be extracted later.

### 2. Define Scope Before Accepting Partial Parity

Several milestones were correct only after the plan clarified that they owned a scoped subset, not full Stim parity.
Examples included M4 decomposition utilities, M5 memory utilities, M6 util-top algebra helpers, M8 simulator-linked sampling behavior, M10 ErrorMatcher provenance, and M11 DEM sampler behavior.

Future plans should state the positive scope and the negative scope together.
A milestone should say "this is included" and "this is explicitly not included yet" in the same section, especially when the upstream feature is known to span multiple subsystems.

### 3. Make Comparator Ownership Executable

The oracle plan originally distinguished exact, structural, and statistical comparators, but some comparator implementations and evidence requirements became clear only when implementation milestones tried to mark rows implemented.

Future plans should assign each comparator to the first milestone that must run it, and the done criteria should require that comparator to be executable before any matching row is marked implemented.
For statistical comparators, the plan should name the sample count, fixed seed policy, bucket definitions, tolerance, and false-positive budget before code changes rely on the result.

### 4. Separate CLI Parity From Core Library Parity

Some early plan text blurred internal parser or model evidence with public CLI compatibility, especially around `convert`, smoke sampling, result-format conversion, and generated fixtures.
That caused confusion over whether a library feature, a Stab-only CLI helper, or a pinned-Stim-compatible command had actually been implemented.

Future milestones should name the public command shape, accepted flags, input paths, output paths, stdout behavior, stderr class, exit status, and unsupported-flag behavior whenever CLI parity is in scope.
If a CLI path exists only as an oracle smoke shim or Stab-specific helper, the milestone should say that it is not public Stim compatibility evidence.

### 5. Treat Resource Boundaries As First-Class Acceptance Criteria

Resource-boundary issues were found late in several milestones, including materialized detection conversion, analyzer repeat expansion, DEM sampler buffering, oversized CLI inputs, and `stab sample` output materialization.
Correctness tests and benchmark gates did not automatically prove hostile-input behavior.

Future milestones that expose public parsing, file input, conversion, sampling, replay, or generated output should include resource-boundary tasks up front.
The plan should require either streaming behavior or a documented cap, plus rejection tests for oversized inputs, excessive repeat expansion, excessive record widths, excessive shot counts, and unsafe path or scratch behavior where relevant.

### 6. Require Benchmark Comparability Classes Early

Benchmark evidence was repeatedly ambiguous because rows mixed direct internal perf matches, public CLI baselines, Stab-only contract timings, report-only rows, and proxy workloads.
Without machine-readable comparability classes, a report could look complete while some rows had no faithful Stab-vs-Stim ratio.

Future benchmark plans should classify every row before implementation begins.
Each row should say whether it is `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, or `contract-only`, and the milestone should say which classes may satisfy performance gates.

### 7. Do Not Let Non-Strict Benchmark Runs Become Completion Evidence

Several benchmark checks were initially too permissive because non-strict compare commands could succeed despite missing baseline rows, pending runners, placeholder baselines, stale milestone filters, or contract-only rows.

Future milestones should use strict benchmark modes for completion whenever the milestone claims Stab-vs-Stim comparison.
If report-only benchmark evidence is intentionally accepted, the milestone should say so and should avoid implying a performance ratio.

### 8. Record Fresh Baselines And Commit Metadata For Performance Claims

Benchmark reports were sometimes cited from stale local paths or from a baseline that no longer matched the selected rows.
Later M12 work fixed this by recording machine metadata, Stim commit metadata, Stab commit metadata, local-modification state, warmup state, measurement-run count, variance, ratios, and row status.

Future performance milestones should require named baseline and compare artifact paths, and reports should record the exact Stab commit and whether local modifications were present.
Completion text should identify the authoritative final reports instead of mixing exploratory probe reports with release-gate evidence.

### 9. Handle Tiny And Noisy Benchmarks Deliberately

Some sub-microsecond benchmark rows were unstable enough that single-run evidence could move across a gate.
The fix was to add warmup runs, repeated recorded runs, median aggregation, paired submeasurement ratios, and threshold exclusions for rows that were not stable enough for a 1.25x guard.

Future plans should define warmup, repeated-run, median or worst-run policy, and absolute-duration concerns before using tiny measurements as gates.
Rows that pass a beta gate but are not stable enough for a tighter regression threshold should be explicitly left out of the threshold file with a source-owned note.

### 10. Pair Submeasurements When A Row Contains Multiple Operations

Some benchmark rows hid slow subcases behind a passing row median.
The later paired-ratio gate fixed this by comparing matching submeasurements and using the worst paired ratio where appropriate.

Future benchmark plans should say whether a row is a single workload or a bundle of submeasurements.
For bundled direct-match rows, the gate should compare paired submeasurements and use the stricter result instead of relying only on the row median.

### 11. Make Waivers Source-Owned And Checked

M12 had rows that could not prove a faithful pinned-Stim timing ratio because Stim v1.16.0 did not expose an equivalent public CLI or perf workload.
Those rows became acceptable only after the plan required source-owned waiver files with reasons, follow-up text, and validator checks against stale or misapplied waivers.

Future plans should not allow informal waiver prose in progress reports to satisfy gates.
Any waiver that affects completion should live in a source-owned machine-checked file and should fail when the row becomes comparable, disappears from the selection, or exceeds a gate it claims to waive.

### 12. Require Audit And Full Review Evidence In The Milestone Report

Several completion reports initially omitted durable milestone-audit or full-code-review closure evidence.
That made it harder to know whether a milestone was truly accepted or merely had implementation evidence.

Future milestone reports should always include the GOAL checklist: tests first, implementation work, done-criteria matrix, milestone-audit outcome, full-code-review outcome, and exact verification commands.
If an audit reveals under-specification, the report should link to the corresponding `milestone-spec-gaps.md` entry and say whether the gap is resolved or intentionally deferred.

### 13. Avoid "Complete" Until Documentation Matches Behavior

Several issues were documentation problems caused by stale wording, stale status labels, stale benchmark paths, or report text that overstated what a command proved.
Even when the code was correct, stale evidence could make the milestone look stronger or weaker than it really was.

Future completion work should include a final documentation consistency pass.
The pass should check milestone status, report paths, commit ids, row counts, deferred-scope language, and whether each cited command actually proves the claim next to it.

### 14. Keep Large Feature Files From Becoming Evidence Dumps

Some implementation files and reports approached the project large-file threshold as fixes accumulated.
Large files make review slower and encourage unrelated behavior to accumulate in one place.

Future planning should include decomposition checkpoints for feature-heavy milestones.
If a source or report file approaches the watch-list threshold, the milestone should split helpers, tests, report sections, or operational evidence before the next wave of changes lands.

## Planning Checklist For Future Milestones

- Name the exact public surface, internal surface, file format, CLI command, or benchmark row the milestone owns.
- Split upstream test sources into owned subcases, deferred subcases, and semantic-mining-only sources.
- Define exact, structural, statistical, and benchmark comparators before implementation starts.
- State unsupported behavior as explicit rejection, explicit manifest-only follow-up, or explicit future-plan work.
- Add resource-boundary acceptance criteria for every public input, output, replay, parser, converter, sampler, and generated artifact path.
- Classify every benchmark row by comparability class before claiming performance evidence.
- Require strict compare mode for Stab-vs-Stim benchmark claims, and label report-only evidence honestly.
- Require warmup, repeated measurements, variance, commit metadata, and local-modification metadata for performance gates.
- Require paired submeasurement gates for bundled direct-match rows.
- Keep waivers source-owned, machine-checked, and tied to follow-up text.
- Record audit and full-code-review closure in the milestone report.
- Run a final documentation consistency pass before marking the milestone complete.
